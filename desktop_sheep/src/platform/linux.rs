use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Instant;

use cairo::ImageSurface;
use gdk4::prelude::*;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{Application, ApplicationWindow};

use crate::core::anim::{frame_to_tile, SpriteColor, TILE_H, TILE_W};
use crate::core::geometry::Rect;
use crate::core::sheep::{Facing, Sheep};
use crate::core::surfaces::{walkable_surfaces, Surface};
use crate::core::world::World;
use crate::desktop_files::{DesktopFile, ICON_PNG};
use crate::platform::x11_windows;
use crate::platform::Platform;
use crate::settings::{ColorChoice, Settings};

/// Takt des Fenster-Pollers. Abgefragt wird nur jeden
/// `WINDOW_POLL_IDLE_EVERY`-ten Takt (pro Abfrage ein paar X11-Roundtrips je
/// Fenster), außer die letzte Abfrage hat eine Änderung gefunden – dann
/// sofort wieder, damit ein Schaf einem gezogenen Fenster flüssig folgt.
const WINDOW_POLL_MS: u64 = 50;
const WINDOW_POLL_IDLE_EVERY: u32 = 5;

struct OverlayHandle {
    area: gtk4::glib::WeakRef<SpriteCanvas>,
    window: gtk4::glib::WeakRef<ApplicationWindow>,
}

/// Kachel des Sprite-Sheets: Farbe, Frame, gespiegelt.
type TileKey = (SpriteColor, u32, bool);

/// Klickbares Rechteck eines Sprites (x, y, Breite, Höhe).
type InputRect = (i32, i32, i32, i32);

/// Ein auf einem Overlay platziertes Sprite (ganzzahlige Position).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Placed {
    key: TileKey,
    x: i32,
    y: i32,
    alpha: f32,
}

/// Zugeschnittene (ggf. gespiegelte) 40×40-Kacheln als Texturen. Einmal
/// erzeugt, lädt GTK sie nur einmal auf die GPU und verwendet sie danach weiter.
struct TileCache {
    sheets: Vec<Option<ImageSurface>>,
    textures: HashMap<TileKey, gdk4::Texture>,
}

impl TileCache {
    fn new() -> Self {
        Self {
            sheets: SpriteColor::ALL.iter().map(|&c| load_sprite(c)).collect(),
            textures: HashMap::new(),
        }
    }

    fn get(&mut self, key: TileKey) -> Option<gdk4::Texture> {
        if let Some(t) = self.textures.get(&key) {
            return Some(t.clone());
        }
        let idx = SpriteColor::ALL.iter().position(|&c| c == key.0)?;
        let tex = tile_texture(self.sheets[idx].as_ref()?, key.1, key.2)?;
        self.textures.insert(key, tex.clone());
        Some(tex)
    }
}

/// Schneidet eine Kachel aus dem Sprite-Sheet und macht daraus eine Textur.
fn tile_texture(sheet: &ImageSurface, frame: u32, mirrored: bool) -> Option<gdk4::Texture> {
    let mut tile =
        ImageSurface::create(cairo::Format::ARgb32, TILE_W as i32, TILE_H as i32).ok()?;
    {
        let cr = cairo::Context::new(&tile).ok()?;
        draw_tile(&cr, sheet, frame, mirrored);
    }
    let stride = tile.stride() as usize;
    let data = tile.data().ok()?.to_vec();
    // Cairo-ARGB32 liegt im Speicher als BGRA (little endian), vormultipliziert.
    Some(
        gdk4::MemoryTexture::new(
            TILE_W as i32,
            TILE_H as i32,
            gdk4::MemoryFormat::B8g8r8a8Premultiplied,
            &glib::Bytes::from_owned(data),
            stride,
        )
        .upcast(),
    )
}

mod canvas {
    use super::*;

    /// Zeichenfläche eines Overlays: ein Textur-Knoten pro Sprite statt
    /// eines bildschirmgroßen Cairo-Bildes. GTK vergleicht die Render-Bäume
    /// zweier Frames und zeichnet nur die Stellen neu, an denen sich ein
    /// Sprite geändert hat – entsprechend wenig muss der Compositor neu
    /// zusammensetzen.
    #[derive(Default)]
    pub struct SpriteCanvas {
        pub(super) sprites: RefCell<Vec<(Placed, gdk4::Texture)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SpriteCanvas {
        const NAME: &'static str = "DesktopSheepCanvas";
        type Type = super::SpriteCanvas;
        type ParentType = gtk4::Widget;
    }

    impl ObjectImpl for SpriteCanvas {}

    impl WidgetImpl for SpriteCanvas {
        fn snapshot(&self, snapshot: &gtk4::Snapshot) {
            // Transparente Grundfläche: Ohne einen einzigen Knoten rendert
            // GTK das Fenster gar nicht mehr, und der letzte Frame (z. B.
            // eines Schafs, das gerade auf den Nachbarmonitor gewechselt
            // ist) bliebe stehen. Da sie sich nie ändert, erzeugt sie
            // beim Frame-Vergleich keinen Schaden.
            let w = self.obj().width() as f32;
            let h = self.obj().height() as f32;
            snapshot.append_color(
                &gdk4::RGBA::TRANSPARENT,
                &gtk4::graphene::Rect::new(0.0, 0.0, w, h),
            );
            for (p, tex) in self.sprites.borrow().iter() {
                let bounds =
                    gtk4::graphene::Rect::new(p.x as f32, p.y as f32, TILE_W as f32, TILE_H as f32);
                let faded = p.alpha < 1.0;
                if faded {
                    snapshot.push_opacity(p.alpha as f64);
                }
                snapshot.append_texture(tex, &bounds);
                if faded {
                    snapshot.pop();
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct SpriteCanvas(ObjectSubclass<canvas::SpriteCanvas>) @extends gtk4::Widget;
}

impl SpriteCanvas {
    fn new() -> Self {
        glib::Object::new()
    }

    /// Übernimmt die Sprites dieses Overlays; neu gezeichnet wird nur, wenn
    /// sich gegenüber dem letzten Frame etwas geändert hat.
    fn set_sprites(&self, wanted: &[Placed], tiles: &mut TileCache) {
        let imp = self.imp();
        if imp
            .sprites
            .borrow()
            .iter()
            .map(|(p, _)| p)
            .eq(wanted.iter())
        {
            return;
        }
        *imp.sprites.borrow_mut() = wanted
            .iter()
            .filter_map(|&p| Some((p, tiles.get(p.key)?)))
            .collect();
        self.queue_draw();
    }
}

/// Zustand einer laufenden Drag-Geste.
struct DragState {
    sheep_idx: usize,
    /// Klickposition innerhalb des Sprites (Sprite-lokal, in Pixel).
    click_offset: (f32, f32),
    /// Ursprung der Drag-Geste in Root-Koordinaten – nötig, damit wir bei
    /// Bewegung über Monitor-Grenzen hinweg absolute Positionen berechnen
    /// und den passenden Ziel-Monitor bestimmen können.
    start_root: (i32, i32),
}

/// Im Tray-Menü geänderte Einstellung, die der GTK-Thread live übernimmt.
#[derive(Debug, Clone, Copy)]
enum TrayCmd {
    SheepCount(usize),
    Color(ColorChoice),
    WalkOnWindows(bool),
    Quit,
}

pub struct LinuxPlatform {
    settings: Settings,
}

impl LinuxPlatform {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }
}

impl Platform for LinuxPlatform {
    fn monitor_geometry(&self) -> Vec<Rect> {
        // Braucht laufenden GDK-Display; wird nach gtk::init aufgerufen.
        gtk4::init().ok();
        let display = match gdk4::Display::default() {
            Some(d) => d,
            None => return vec![],
        };
        let monitors = display.monitors();
        let mut out = Vec::new();
        for i in 0..monitors.n_items() {
            if let Some(obj) = monitors.item(i) {
                if let Ok(m) = obj.downcast::<gdk4::Monitor>() {
                    let g = m.geometry();
                    out.push(Rect {
                        x: g.x(),
                        y: g.y(),
                        width: g.width(),
                        height: g.height(),
                    });
                }
            }
        }
        // Wichtig: gleiche Sortierung wie ground_segments (nach x aufsteigend),
        // damit monitor_idx im Sheep und Overlay-Index im Backend übereinstimmen.
        out.sort_by_key(|r| r.x);
        out
    }

    fn run<SpawnFn, TickFn>(
        self,
        sheeps: Vec<Sheep>,
        spawn: SpawnFn,
        tick_ms: u32,
        tick: TickFn,
    ) -> Result<(), String>
    where
        SpawnFn: FnMut(usize) -> Sheep + 'static,
        TickFn: FnMut(&mut [Sheep], u32, &World) + 'static,
    {
        let app = Application::builder()
            .application_id("ch.local.desktop_sheep")
            .build();

        let sheeps = Rc::new(RefCell::new(sheeps));
        let spawn = Rc::new(RefCell::new(spawn));
        let tick = Rc::new(RefCell::new(tick));
        let settings = self.settings;

        let built = Cell::new(false);
        app.connect_activate(move |app| {
            // Ein weiterer Start (z. B. aus dem Startmenü) landet über die
            // Application-ID per D-Bus als erneutes activate bei dieser
            // Instanz – Overlays und Tray gibt es dann schon.
            if built.replace(true) {
                return;
            }
            // Tray-Icon in eigenem Thread; Menü-Aktionen kommen über den Kanal
            // und werden im GTK-Loop (build_ui) angewendet. Erst hier gestartet,
            // damit ein zweiter Prozess kein eigenes Icon anlegt.
            let tray_rx = start_tray_service(settings);
            build_ui(
                app,
                sheeps.clone(),
                spawn.clone(),
                tick.clone(),
                tick_ms,
                settings,
                tray_rx,
            );
        });

        // SIGINT (Ctrl+C im Terminal) sauber abfangen und Application beenden.
        // SIGINT = 2 auf Linux (POSIX).
        let app_for_sig = app.clone();
        glib::unix_signal_add_local(2, move || {
            app_for_sig.quit();
            glib::ControlFlow::Break
        });

        // Wir starten die App mit leerem Arg-Vektor, damit GTK unsere CLI-Args
        // (die es nicht gibt) nicht interpretiert.
        let empty: Vec<&str> = vec![];
        let code = app.run_with_args(&empty);
        if code == glib::ExitCode::SUCCESS {
            Ok(())
        } else {
            Err(format!("gtk exit code {:?}", code))
        }
    }
}

fn build_ui<SpawnFn, TickFn>(
    app: &Application,
    sheeps: Rc<RefCell<Vec<Sheep>>>,
    spawn: Rc<RefCell<SpawnFn>>,
    tick: Rc<RefCell<TickFn>>,
    tick_ms: u32,
    settings: Settings,
    tray_rx: mpsc::Receiver<TrayCmd>,
) where
    SpawnFn: FnMut(usize) -> Sheep + 'static,
    TickFn: FnMut(&mut [Sheep], u32, &World) + 'static,
{
    let display = gdk4::Display::default().expect("kein GDK-Display");

    let mut mons: Vec<gdk4::Monitor> = Vec::new();
    let list = display.monitors();
    for i in 0..list.n_items() {
        if let Some(obj) = list.item(i) {
            if let Ok(m) = obj.downcast::<gdk4::Monitor>() {
                mons.push(m);
            }
        }
    }
    mons.sort_by_key(|m| m.geometry().x());
    if mons.is_empty() {
        return;
    }

    // Jedes Schaf wird mit seiner eigenen Farbe gezeichnet; die Kacheln
    // aller Farbvarianten entstehen bei Bedarf im TileCache.
    let mut tiles = TileCache::new();
    for s in sheeps.borrow_mut().iter_mut() {
        settings.color.apply_to(s);
    }

    let css = gtk4::CssProvider::new();
    css.load_from_string("window, .background { background-color: rgba(0,0,0,0); }");
    gtk4::style_context_add_provider_for_display(
        &display,
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let overlays: Rc<RefCell<Vec<OverlayHandle>>> =
        Rc::new(RefCell::new(Vec::with_capacity(mons.len())));

    // Begehbare Fensterkanten pro Overlay; aktualisiert vom Fenster-Poller.
    let surfaces: Rc<RefCell<Vec<Vec<Surface>>>> =
        Rc::new(RefCell::new(vec![Vec::new(); mons.len()]));

    // Monitor-Rechtecke einmal einsammeln – für Cross-Monitor-Drag brauchen
    // wir sie im Drag-Handler, um den Ziel-Monitor aus Root-Koordinaten
    // ableiten zu können.
    let mon_rects: Rc<Vec<(i32, i32, i32, i32)>> = Rc::new(
        mons.iter()
            .map(|m| {
                let g = m.geometry();
                (g.x(), g.y(), g.width(), g.height())
            })
            .collect(),
    );

    // Aktuell gezogenes Sheep – geteilt zwischen allen Overlays. Ein Sheep
    // gleichzeitig ist genug; multi-touch würden wir hier erweitern.
    let drag_state: Rc<RefCell<Option<DragState>>> = Rc::new(RefCell::new(None));

    for (idx, monitor) in mons.iter().enumerate() {
        let title = format!("Desktop Sheep Overlay {}", idx);
        let geo = monitor.geometry();
        let window = ApplicationWindow::builder()
            .application(app)
            .title(&title)
            .decorated(false)
            .resizable(false)
            .default_width(geo.width())
            .default_height(geo.height())
            .build();
        window.add_css_class("background");

        let area = SpriteCanvas::new();
        area.set_hexpand(true);
        area.set_vexpand(true);
        let my_idx = idx;

        // Drag-Geste anhängen. Root-Koordinaten damit das Sheep über die
        // Monitor-Grenze gezogen werden kann (Pointer-Grab bleibt bei diesem
        // Overlay, aber wir rechnen intern in globalen Koords).
        let drag = gtk4::GestureDrag::new();
        let start_mon_x = geo.x();
        let start_mon_y = geo.y();
        {
            let sheeps_g = sheeps.clone();
            let ds = drag_state.clone();
            drag.connect_drag_begin(move |_, x, y| {
                let mut all = sheeps_g.borrow_mut();
                for (i, s) in all.iter_mut().enumerate() {
                    if s.monitor_idx == my_idx && s.contains(x as f32, y as f32) {
                        let click_offset = (x as f32 - s.x, y as f32 - s.y);
                        s.start_drag();
                        *ds.borrow_mut() = Some(DragState {
                            sheep_idx: i,
                            click_offset,
                            start_root: (start_mon_x + x as i32, start_mon_y + y as i32),
                        });
                        break;
                    }
                }
            });
        }
        {
            let sheeps_g = sheeps.clone();
            let ds = drag_state.clone();
            let mon_rects_c = mon_rects.clone();
            drag.connect_drag_update(move |_, dx, dy| {
                let state_snapshot = {
                    let st = ds.borrow();
                    match st.as_ref() {
                        Some(s) => (s.sheep_idx, s.click_offset, s.start_root),
                        None => return,
                    }
                };
                let (sheep_idx, click_offset, start_root) = state_snapshot;

                // Cursor absolut im Root, Sprite-Ecke = Cursor - Klick-Offset.
                let cursor_root_x = start_root.0 as f32 + dx as f32;
                let cursor_root_y = start_root.1 as f32 + dy as f32;
                let sprite_root_x = cursor_root_x - click_offset.0;
                let sprite_root_y = cursor_root_y - click_offset.1;

                // Ziel-Monitor: derjenige, der den Sprite-Mittelpunkt enthält.
                let cx = sprite_root_x + TILE_W as f32 / 2.0;
                let cy = sprite_root_y + TILE_H as f32 / 2.0;
                let target = mon_rects_c
                    .iter()
                    .enumerate()
                    .find(|(_, (mx, my, mw, mh))| {
                        (cx as i32) >= *mx
                            && (cx as i32) < *mx + *mw
                            && (cy as i32) >= *my
                            && (cy as i32) < *my + *mh
                    });
                let Some((target_idx, (tmx, tmy, tmw, tmh))) = target else {
                    return;
                };

                let max_x = (tmw - TILE_W as i32).max(0) as f32;
                let max_y = (tmh - TILE_H as i32).max(0) as f32;
                let local_x = (sprite_root_x - *tmx as f32).clamp(0.0, max_x);
                let local_y = (sprite_root_y - *tmy as f32).clamp(0.0, max_y);

                let mut all = sheeps_g.borrow_mut();
                if let Some(s) = all.get_mut(sheep_idx) {
                    s.set_pos(target_idx, local_x, local_y);
                }
            });
        }
        {
            let sheeps_g = sheeps.clone();
            let ds = drag_state.clone();
            drag.connect_drag_end(move |_, _dx, _dy| {
                let mut st = ds.borrow_mut();
                if let Some(state) = st.take() {
                    if let Some(s) = sheeps_g.borrow_mut().get_mut(state.sheep_idx) {
                        s.end_drag();
                    }
                }
            });
        }
        area.add_controller(drag);

        window.set_child(Some(&area));
        overlays.borrow_mut().push(OverlayHandle {
            area: area.downgrade(),
            window: window.downgrade(),
        });

        window.present();

        // Overlay per Xlib exakt auf Monitor-Geometrie legen; kein Fullscreen
        // (sonst blendet Cinnamon Panels aus). WM-States (above/sticky/skip)
        // ebenfalls direkt via X11 – wmctrl-Subprozess ist zu langsam,
        // Cinnamon hat sonst schon Task-Einträge angelegt.
        let window_weak = window.downgrade();
        let geo_x = geo.x();
        let geo_y = geo.y();
        let geo_w = geo.width();
        let geo_h = geo.height();
        glib::idle_add_local_once(move || {
            if let Some(win) = window_weak.upgrade() {
                // Initial klickdurchlässig; die tatsächliche Input-Region
                // (Rechtecke um jedes Sprite) setzt der Tick-Loop pro Frame.
                if let Some(surface) = win.native().and_then(|n| n.surface()) {
                    let empty = cairo::Region::create();
                    surface.set_input_region(&empty);
                }
                place_overlay_via_x11(&win, geo_x, geo_y, geo_w, geo_h);
                apply_overlay_wm_state(&win);
            }
        });
        // Bei Fokusverlust WM-State erneut anwenden – Muffin verliert
        // _NET_WM_STATE_ABOVE gerne beim Fensterwechsel.
        let window_focus = window.downgrade();
        window.connect_is_active_notify(move |w| {
            if !w.is_active() {
                if let Some(win) = window_focus.upgrade() {
                    apply_overlay_wm_state(&win);
                }
            }
        });
    }

    // Tick-Loop
    let sheeps_tick = sheeps.clone();
    let tick_cb = tick.clone();
    let overlays_tick = overlays.clone();
    let surfaces_tick = surfaces.clone();
    let last = Rc::new(RefCell::new(Instant::now()));
    // Zuletzt gesetzte Input-Region je Overlay – nur bei Änderung neu setzen.
    let mut last_input: Vec<Option<Vec<InputRect>>> = vec![None; mons.len()];
    glib::timeout_add_local(
        std::time::Duration::from_millis(tick_ms as u64),
        move || {
            let now = Instant::now();
            let dt = now.duration_since(*last.borrow()).as_millis() as u32;
            *last.borrow_mut() = now;

            let dims: Vec<(i32, i32)> = overlays_tick
                .borrow()
                .iter()
                .map(|o| {
                    o.area
                        .upgrade()
                        .map(|a| (a.width(), a.height()))
                        .unwrap_or((0, 0))
                })
                .collect();

            {
                let mut s = sheeps_tick.borrow_mut();
                let surf = surfaces_tick.borrow();
                let world = World {
                    area_dims: &dims,
                    surfaces: &surf,
                };
                (tick_cb.borrow_mut())(&mut s, dt.max(1), &world);
            }

            // Pro Overlay Sprites und Input-Region aktualisieren – beides nur,
            // wenn sich etwas geändert hat. Nur der Sprite-Bereich ist klick-/
            // dragbar, der Rest bleibt durchlässig.
            {
                let all = sheeps_tick.borrow();
                for (idx, oh) in overlays_tick.borrow().iter().enumerate() {
                    let here = || all.iter().filter(|s| s.monitor_idx == idx);
                    if let Some(a) = oh.area.upgrade() {
                        let mut wanted = Vec::new();
                        for s in here() {
                            if let Some(p) = s.prop() {
                                wanted.push(Placed {
                                    key: (s.color, p.frame, p.mirrored),
                                    x: p.x.round() as i32,
                                    y: p.y.round() as i32,
                                    alpha: p.alpha,
                                });
                            }
                            wanted.push(Placed {
                                key: (s.color, s.current_frame(), s.facing == Facing::Right),
                                x: s.x.round() as i32,
                                y: s.y.round() as i32,
                                alpha: 1.0,
                            });
                        }
                        a.set_sprites(&wanted, &mut tiles);
                    }

                    let input: Vec<_> = here().map(|s| s.hitbox()).collect();
                    if last_input[idx].as_ref() == Some(&input) {
                        continue;
                    }
                    let Some(win) = oh.window.upgrade() else {
                        continue;
                    };
                    let Some(surface) = win.native().and_then(|n| n.surface()) else {
                        continue;
                    };
                    let region = cairo::Region::create();
                    for &(bx, by, bw, bh) in &input {
                        region
                            .union_rectangle(&cairo::RectangleInt::new(bx, by, bw, bh))
                            .ok();
                    }
                    surface.set_input_region(&region);
                    last_input[idx] = Some(input);
                }
            }
            glib::ControlFlow::Continue
        },
    );

    // Fensteroberkanten als Laufflächen; im Tray-Menü abschaltbar.
    let walk_on_windows = Rc::new(Cell::new(settings.walk_on_windows));
    start_window_poller(overlays.clone(), surfaces.clone(), walk_on_windows.clone());

    // Menü-Aktionen aus dem Tray-Thread anwenden.
    let app_quit = app.downgrade();
    let sheeps_menu = sheeps.clone();
    let drag_menu = drag_state.clone();
    let mut color = settings.color;
    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        while let Ok(cmd) = tray_rx.try_recv() {
            match cmd {
                TrayCmd::Quit => {
                    if let Some(app) = app_quit.upgrade() {
                        app.quit();
                    }
                    return glib::ControlFlow::Break;
                }
                TrayCmd::SheepCount(n) => {
                    let mut all = sheeps_menu.borrow_mut();
                    all.truncate(n);
                    while all.len() < n {
                        let i = all.len();
                        let mut s = (spawn.borrow_mut())(i);
                        color.apply_to(&mut s);
                        all.push(s);
                    }
                    // Wurde gerade das gezogene Schaf entfernt, die Geste verwerfen.
                    let mut ds = drag_menu.borrow_mut();
                    if ds.as_ref().is_some_and(|d| d.sheep_idx >= n) {
                        *ds = None;
                    }
                }
                TrayCmd::Color(c) => {
                    // Erneut „Zufällig“ wählen würfelt die Farben neu.
                    color = c;
                    for s in sheeps_menu.borrow_mut().iter_mut() {
                        c.apply_to(s);
                    }
                }
                TrayCmd::WalkOnWindows(on) => walk_on_windows.set(on),
            }
        }
        glib::ControlFlow::Continue
    });

    // Cursor-Poller: bei Mausbewegung alle schlafenden Sheeps wecken.
    let poller = PointerPoller::new();
    let sheeps_wake = sheeps.clone();
    let poller_cell = Rc::new(RefCell::new(poller));
    glib::timeout_add_local(std::time::Duration::from_millis(300), move || {
        if let Some(p) = poller_cell.borrow_mut().as_mut() {
            if p.moved_since_last() {
                for s in sheeps_wake.borrow_mut().iter_mut() {
                    s.wake_from_sleep();
                }
            }
        }
        glib::ControlFlow::Continue
    });
}

/// Liest periodisch die Fensterliste und berechnet daraus die begehbaren
/// Kanten pro Overlay. Läuft auf GDKs eigener X-Verbindung innerhalb eines
/// Error-Traps – verschwindet ein Fenster mitten in der Abfrage, wird das
/// `BadWindow` geschluckt statt den Prozess zu beenden.
///
/// Mit `DESKTOP_SHEEP_DEBUG=1` werden Fenster und Kanten bei jeder Änderung
/// auf stderr ausgegeben (zum Abgleich mit `wmctrl -lG` / `xprop`).
///
/// Ist `enabled` aus, bleiben die Kanten leer – Schafe auf Fenstern fallen
/// dann herunter wie bei einem weggezogenen Fenster.
fn start_window_poller(
    overlays: Rc<RefCell<Vec<OverlayHandle>>>,
    surfaces: Rc<RefCell<Vec<Vec<Surface>>>>,
    enabled: Rc<Cell<bool>>,
) {
    use gdk4_x11::prelude::*;
    let Some(display) = gdk4::Display::default() else {
        return;
    };
    let Ok(x11display) = display.downcast::<gdk4_x11::X11Display>() else {
        eprintln!("Kein X11-Display – Fenster-Laufen deaktiviert.");
        return;
    };
    let debug = std::env::var_os("DESKTOP_SHEEP_DEBUG").is_some();
    let atoms = unsafe { x11_windows::Atoms::new(x11display.xdisplay()) };
    let mut last_debug = String::new();
    let mut ticks: u32 = 0;
    let mut changing = true;

    glib::timeout_add_local(
        std::time::Duration::from_millis(WINDOW_POLL_MS),
        move || {
            ticks = ticks.wrapping_add(1);
            if !enabled.get() {
                if surfaces.borrow().iter().any(|l| !l.is_empty()) {
                    surfaces.borrow_mut().iter_mut().for_each(Vec::clear);
                }
                // Nach dem Wiedereinschalten sofort abfragen.
                changing = true;
                return glib::ControlFlow::Continue;
            }
            if !changing && ticks % WINDOW_POLL_IDLE_EVERY != 0 {
                return glib::ControlFlow::Continue;
            }
            let xdisp = unsafe { x11display.xdisplay() };
            x11display.error_trap_push();
            let windows = unsafe { x11_windows::list_windows(xdisp, &atoms) };
            // Root-Rechteck jeder Overlay-Zeichenfläche: tatsächliche Fenster-
            // position (der WM kann verschieben) + Live-Größe der Zeichenfläche.
            let overlay_rects: Vec<Rect> = overlays
                .borrow()
                .iter()
                .map(|o| {
                    let (Some(win), Some(area)) = (o.window.upgrade(), o.area.upgrade()) else {
                        return Rect {
                            x: 0,
                            y: 0,
                            width: 0,
                            height: 0,
                        };
                    };
                    let origin = win
                        .native()
                        .and_then(|n| n.surface())
                        .and_then(|s| s.downcast::<gdk4_x11::X11Surface>().ok())
                        .and_then(|s| unsafe { x11_windows::root_origin(xdisp, s.xid()) })
                        .unwrap_or((0, 0));
                    Rect {
                        x: origin.0,
                        y: origin.1,
                        width: area.width(),
                        height: area.height(),
                    }
                })
                .collect();
            let names: Vec<String> = if debug {
                windows
                    .iter()
                    .map(|w| unsafe { x11_windows::window_name(xdisp, w.id as _) })
                    .collect()
            } else {
                Vec::new()
            };
            x11display.error_trap_pop_ignored();

            let new_surfaces = walkable_surfaces(&windows, &overlay_rects);
            if debug {
                let mut out = String::new();
                for (o, r) in overlay_rects.iter().enumerate() {
                    out += &format!("overlay {o}: {r:?}\n");
                }
                for (w, name) in windows.iter().zip(&names) {
                    out += &format!("  win 0x{:08x} {:?} {name:?}\n", w.id, w.rect);
                }
                for (o, list) in new_surfaces.iter().enumerate() {
                    for e in list {
                        out += &format!(
                            "  kante overlay {o}: 0x{:08x} x={}..{} y={}\n",
                            e.window, e.x_start, e.x_end, e.y
                        );
                    }
                }
                if out != last_debug {
                    eprint!("--- Fenster/Kanten ---\n{out}");
                    last_debug = out;
                }
            }
            changing = *surfaces.borrow() != new_surfaces;
            if changing {
                *surfaces.borrow_mut() = new_surfaces;
            }
            glib::ControlFlow::Continue
        },
    );
}

/// Zeichnet eine Kachel des Sprite-Sheets an (0, 0). Die Kacheln schauen
/// nach links; `mirrored` spiegelt horizontal.
fn draw_tile(cr: &cairo::Context, sheet: &ImageSurface, frame: u32, mirrored: bool) {
    let (col, row) = frame_to_tile(frame);
    let sx = (col * TILE_W) as f64;
    let sy = (row * TILE_H) as f64;
    if mirrored {
        cr.translate(TILE_W as f64, 0.0);
        cr.scale(-1.0, 1.0);
    }
    cr.set_source_surface(sheet, -sx, -sy).ok();
    cr.rectangle(0.0, 0.0, TILE_W as f64, TILE_H as f64);
    cr.clip();
    cr.paint().ok();
}

struct PointerPoller {
    display: *mut x11::xlib::Display,
    root: x11::xlib::Window,
    last: Option<(i32, i32)>,
}

impl PointerPoller {
    fn new() -> Option<Self> {
        unsafe {
            let d = x11::xlib::XOpenDisplay(std::ptr::null());
            if d.is_null() {
                return None;
            }
            let root = x11::xlib::XDefaultRootWindow(d);
            Some(Self {
                display: d,
                root,
                last: None,
            })
        }
    }

    fn query(&self) -> Option<(i32, i32)> {
        unsafe {
            let mut root_ret: x11::xlib::Window = 0;
            let mut child_ret: x11::xlib::Window = 0;
            let mut rx: std::os::raw::c_int = 0;
            let mut ry: std::os::raw::c_int = 0;
            let mut wx: std::os::raw::c_int = 0;
            let mut wy: std::os::raw::c_int = 0;
            let mut mask: std::os::raw::c_uint = 0;
            let ok = x11::xlib::XQueryPointer(
                self.display,
                self.root,
                &mut root_ret,
                &mut child_ret,
                &mut rx,
                &mut ry,
                &mut wx,
                &mut wy,
                &mut mask,
            );
            if ok == 0 {
                None
            } else {
                Some((rx, ry))
            }
        }
    }

    /// True, wenn sich die Cursorposition seit dem letzten Aufruf verändert
    /// hat. Beim allerersten Aufruf wird nur der Referenzwert gesetzt und
    /// false zurückgegeben (sonst würde jedes Sheep beim Start sofort wach).
    fn moved_since_last(&mut self) -> bool {
        let Some(pos) = self.query() else {
            return false;
        };
        let moved = match self.last {
            Some(prev) => prev != pos,
            None => false,
        };
        self.last = Some(pos);
        moved
    }
}

impl Drop for PointerPoller {
    fn drop(&mut self) {
        unsafe {
            if !self.display.is_null() {
                x11::xlib::XCloseDisplay(self.display);
            }
        }
    }
}

/// Setzt _NET_WM_STATE-Flags (above, sticky, skip_taskbar, skip_pager) direkt
/// per XSendEvent an das Root-Fenster. Läuft synchron und wirkt sofort,
/// bevor Cinnamon Task-/Pager-Einträge anlegt.
fn apply_overlay_wm_state(window: &ApplicationWindow) {
    use gdk4_x11::prelude::*;
    let Some(surface) = window.native().and_then(|n| n.surface()) else {
        return;
    };
    let Ok(x11surf) = surface.downcast::<gdk4_x11::X11Surface>() else {
        return;
    };
    let xid = x11surf.xid();
    let Some(display) = gdk4::Display::default() else {
        return;
    };
    let Ok(x11display) = display.downcast::<gdk4_x11::X11Display>() else {
        return;
    };
    unsafe {
        let xdisp = x11display.xdisplay();
        for state in [
            "_NET_WM_STATE_ABOVE",
            "_NET_WM_STATE_STICKY",
            "_NET_WM_STATE_SKIP_TASKBAR",
            "_NET_WM_STATE_SKIP_PAGER",
        ] {
            send_wm_state(xdisp, xid, /*add=*/ true, state);
        }
        x11::xlib::XFlush(xdisp);
    }
}

/// Sendet eine _NET_WM_STATE-Client-Message (add/remove eines Zustands) an
/// das Root-Fenster – EWMH-konform, so wie wmctrl es intern macht.
unsafe fn send_wm_state(
    xdisp: *mut x11::xlib::Display,
    xid: x11::xlib::Window,
    add: bool,
    state_name: &str,
) {
    let state_c = std::ffi::CString::new(state_name).unwrap();
    let msg_c = std::ffi::CString::new("_NET_WM_STATE").unwrap();
    let state_atom = x11::xlib::XInternAtom(xdisp, state_c.as_ptr(), 0);
    let msg_atom = x11::xlib::XInternAtom(xdisp, msg_c.as_ptr(), 0);
    let root = x11::xlib::XDefaultRootWindow(xdisp);

    let mut ev: x11::xlib::XEvent = std::mem::zeroed();
    ev.client_message.type_ = x11::xlib::ClientMessage;
    ev.client_message.serial = 0;
    ev.client_message.send_event = 1;
    ev.client_message.display = xdisp;
    ev.client_message.window = xid;
    ev.client_message.message_type = msg_atom;
    ev.client_message.format = 32;
    ev.client_message.data.set_long(0, if add { 1 } else { 0 });
    ev.client_message.data.set_long(1, state_atom as i64);
    ev.client_message.data.set_long(2, 0);
    ev.client_message.data.set_long(3, 1); // source: application
    ev.client_message.data.set_long(4, 0);

    x11::xlib::XSendEvent(
        xdisp,
        root,
        0,
        x11::xlib::SubstructureNotifyMask | x11::xlib::SubstructureRedirectMask,
        &mut ev,
    );
}

/// Verschiebt und dimensioniert das Overlay-Fenster exakt auf das gegebene
/// Rechteck (Wurzelkoordinaten) via Xlib. Wmctrl scheidet aus, weil Muffin
/// die Zielkoordinaten monitor-relativ interpretiert und Multi-Monitor-Setups
/// dadurch komplett verrutschen.
fn place_overlay_via_x11(window: &ApplicationWindow, x: i32, y: i32, w: i32, h: i32) {
    use gdk4_x11::prelude::*;
    let Some(surface) = window.native().and_then(|n| n.surface()) else {
        return;
    };
    let Ok(x11surf) = surface.downcast::<gdk4_x11::X11Surface>() else {
        return;
    };
    let xid = x11surf.xid();
    let Some(display) = gdk4::Display::default() else {
        return;
    };
    let Ok(x11display) = display.downcast::<gdk4_x11::X11Display>() else {
        return;
    };
    unsafe {
        let xdisp = x11display.xdisplay();
        x11::xlib::XMoveResizeWindow(xdisp, xid, x, y, w as u32, h as u32);
        x11::xlib::XFlush(xdisp);
    }
}

/// Größte Anzahl, die das Menü direkt anbietet; per `--sheep` gesetzte
/// größere Werte erscheinen zusätzlich.
const MENU_MAX_SHEEP: usize = 10;

/// Menü-Bezeichnung einer Farbwahl.
fn color_label(c: ColorChoice) -> &'static str {
    match c {
        ColorChoice::Single(SpriteColor::Default) => "Weiß (Original)",
        ColorChoice::Single(SpriteColor::Blue) => "Blau",
        ColorChoice::Single(SpriteColor::Green) => "Grün",
        ColorChoice::Single(SpriteColor::Orange) => "Orange",
        ColorChoice::Single(SpriteColor::Pink) => "Rosa",
        ColorChoice::Single(SpriteColor::Purple) => "Lila",
        ColorChoice::Single(SpriteColor::Red) => "Rot",
        ColorChoice::Single(SpriteColor::Yellow) => "Gelb",
        ColorChoice::Random => "Zufällig (jedes Schaf anders)",
    }
}

/// Läuft im ksni-Thread und hält die Einstellungen, die das Menü anzeigt.
struct DesktopSheepTray {
    settings: Settings,
    tx: mpsc::Sender<TrayCmd>,
    /// Schaf-Icon in mehreren Größen (ARGB), einmal beim Start erzeugt.
    icons: Vec<ksni::Icon>,
}

impl DesktopSheepTray {
    /// Übernimmt eine geänderte Einstellung, speichert sie und reicht sie an
    /// den GTK-Thread weiter.
    fn apply(&mut self, cmd: TrayCmd) {
        let before = self.settings;
        match cmd {
            TrayCmd::SheepCount(n) => self.settings.n_sheep = n,
            TrayCmd::Color(c) => self.settings.color = c,
            TrayCmd::WalkOnWindows(on) => self.settings.walk_on_windows = on,
            TrayCmd::Quit => {}
        }
        if self.settings != before {
            if let Err(e) = self.settings.save() {
                eprintln!("Einstellungen konnten nicht gespeichert werden: {e}");
            }
        }
        let _ = self.tx.send(cmd);
    }
}

impl ksni::Tray for DesktopSheepTray {
    // Als Pixeldaten statt Theme-Name: braucht kein installiertes Icon und
    // funktioniert so auch direkt aus dem AppImage.
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icons.clone()
    }
    fn title(&self) -> String {
        "Desktop Sheep".into()
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Desktop Sheep".into(),
            description: "Overlay-Schafe laufen an der Bildschirmunterkante".into(),
            icon_name: String::new(),
            icon_pixmap: self.icons.clone(),
        }
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let n = self.settings.n_sheep;
        let mut counts: Vec<usize> = (1..=MENU_MAX_SHEEP).collect();
        if n > MENU_MAX_SHEEP {
            counts.push(n);
        }
        let count_options = counts
            .iter()
            .map(|c| RadioItem {
                label: c.to_string(),
                ..Default::default()
            })
            .collect();
        // Alle Einzelfarben, dann „Zufällig“.
        let choices: Vec<ColorChoice> = SpriteColor::ALL
            .iter()
            .map(|&c| ColorChoice::Single(c))
            .chain([ColorChoice::Random])
            .collect();
        let color_options = choices
            .iter()
            .map(|&c| RadioItem {
                label: color_label(c).into(),
                ..Default::default()
            })
            .collect();

        vec![
            SubMenu {
                label: format!("Anzahl Schafe ({n})"),
                submenu: vec![RadioGroup {
                    selected: counts.iter().position(|&c| c == n).unwrap_or(0),
                    select: Box::new(move |tray: &mut Self, i| {
                        tray.apply(TrayCmd::SheepCount(counts[i]));
                    }),
                    options: count_options,
                }
                .into()],
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: "Farbe".into(),
                submenu: vec![RadioGroup {
                    selected: choices
                        .iter()
                        .position(|&c| c == self.settings.color)
                        .unwrap_or(0),
                    select: Box::new(move |tray: &mut Self, i| {
                        tray.apply(TrayCmd::Color(choices[i]));
                    }),
                    options: color_options,
                }
                .into()],
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Auf Fenstern laufen".into(),
                checked: self.settings.walk_on_windows,
                activate: Box::new(|tray: &mut Self| {
                    let on = !tray.settings.walk_on_windows;
                    tray.apply(TrayCmd::WalkOnWindows(on));
                }),
                ..Default::default()
            }
            .into(),
            desktop_file_item("Beim Anmelden starten", DesktopFile::Autostart),
            desktop_file_item("Im Startmenü anzeigen", DesktopFile::StartMenu),
            MenuItem::Separator,
            StandardItem {
                label: "Beenden".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| tray.apply(TrayCmd::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Menüpunkt, der einen .desktop-Eintrag an- bzw. abschaltet.
fn desktop_file_item(label: &str, file: DesktopFile) -> ksni::MenuItem<DesktopSheepTray> {
    ksni::menu::CheckmarkItem {
        label: label.into(),
        checked: file.is_installed(),
        activate: Box::new(move |_: &mut DesktopSheepTray| {
            if let Err(e) = file.toggle() {
                let path = file.path().unwrap_or_default();
                eprintln!("{} konnte nicht geändert werden: {e}", path.display());
            }
        }),
        ..Default::default()
    }
    .into()
}

/// Startet den ksni-TrayService in einem eigenen Thread. Menü-Aktionen
/// kommen über den zurückgegebenen Kanal beim GTK-Loop an.
fn start_tray_service(settings: Settings) -> mpsc::Receiver<TrayCmd> {
    let (tx, rx) = mpsc::channel();
    let tray = DesktopSheepTray {
        settings,
        tx,
        icons: tray_icons(),
    };
    let service = ksni::TrayService::new(tray);
    service.spawn();
    rx
}

/// Größen, in denen das Tray-Icon angeboten wird; das Panel wählt die
/// passende, statt selbst (unscharf) zu skalieren.
const TRAY_ICON_SIZES: [i32; 5] = [16, 22, 24, 32, 48];

/// Das Programm-Icon als ARGB32-Pixmaps (Netzwerk-Byte-Reihenfolge, nicht
/// vormultipliziert), wie es das StatusNotifierItem-Protokoll verlangt.
fn tray_icons() -> Vec<ksni::Icon> {
    let Ok(src) = ImageSurface::create_from_png(&mut std::io::Cursor::new(ICON_PNG)) else {
        eprintln!("Tray-Icon konnte nicht geladen werden.");
        return Vec::new();
    };
    TRAY_ICON_SIZES
        .iter()
        .filter_map(|&size| {
            let mut img = ImageSurface::create(cairo::Format::ARgb32, size, size).ok()?;
            {
                let cr = cairo::Context::new(&img).ok()?;
                let scale = size as f64 / src.width() as f64;
                cr.scale(scale, scale);
                cr.set_source_surface(&src, 0.0, 0.0).ok()?;
                cr.source().set_filter(cairo::Filter::Good);
                cr.paint().ok()?;
            }
            let stride = img.stride() as usize;
            let data = img.data().ok()?;
            let mut argb = Vec::with_capacity((size * size * 4) as usize);
            for row in data.chunks(stride).take(size as usize) {
                for px in row[..size as usize * 4].chunks_exact(4) {
                    // Cairo: u32 in nativer Byte-Reihenfolge, vormultipliziert.
                    let [b, g, r, a] =
                        u32::from_ne_bytes([px[0], px[1], px[2], px[3]]).to_le_bytes();
                    let unmul = |c: u8| {
                        if a == 0 {
                            0
                        } else {
                            (c as u32 * 255 / a as u32) as u8
                        }
                    };
                    argb.extend_from_slice(&[a, unmul(r), unmul(g), unmul(b)]);
                }
            }
            Some(ksni::Icon {
                width: size,
                height: size,
                data: argb,
            })
        })
        .collect()
}

fn load_sprite(color: SpriteColor) -> Option<ImageSurface> {
    let mut reader = std::io::Cursor::new(color.png_bytes());
    match ImageSurface::create_from_png(&mut reader) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("Sprite konnte nicht geladen werden: {e}");
            None
        }
    }
}
