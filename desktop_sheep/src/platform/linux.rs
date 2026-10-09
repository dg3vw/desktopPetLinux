use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use cairo::ImageSurface;
use gdk4::prelude::*;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea};

use crate::core::anim::{TILE_H, TILE_W, frame_to_tile};
use crate::core::geometry::Rect;
use crate::core::sheep::{Facing, Sheep};
use crate::core::surfaces::{walkable_surfaces, Surface};
use crate::core::world::World;
use crate::platform::x11_windows;
use crate::platform::Platform;

/// Takt des Fenster-Pollers. Abgefragt wird nur jeden
/// `WINDOW_POLL_IDLE_EVERY`-ten Takt (pro Abfrage ein paar X11-Roundtrips je
/// Fenster), außer die letzte Abfrage hat eine Änderung gefunden – dann
/// sofort wieder, damit ein Schaf einem gezogenen Fenster flüssig folgt.
const WINDOW_POLL_MS: u64 = 50;
const WINDOW_POLL_IDLE_EVERY: u32 = 5;

struct OverlayHandle {
    area: gtk4::glib::WeakRef<DrawingArea>,
    window: gtk4::glib::WeakRef<ApplicationWindow>,
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

pub struct LinuxPlatform {
    sprite_bytes: &'static [u8],
    /// Fensteroberkanten als Laufflächen anbieten (Plan Abschnitt 6).
    walk_on_windows: bool,
}

impl LinuxPlatform {
    pub fn new(color: crate::core::anim::SpriteColor, walk_on_windows: bool) -> Self {
        Self { sprite_bytes: color.png_bytes(), walk_on_windows }
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

    fn run<TickFn>(self, sheeps: Vec<Sheep>, tick_ms: u32, tick: TickFn) -> Result<(), String>
    where
        TickFn: FnMut(&mut [Sheep], u32, &World) + 'static,
    {
        let app = Application::builder()
            .application_id("ch.local.desktop_sheep")
            .build();

        let sheeps = Rc::new(RefCell::new(sheeps));
        let tick = Rc::new(RefCell::new(tick));
        let sprite_bytes = self.sprite_bytes;
        let walk_on_windows = self.walk_on_windows;

        app.connect_activate(move |app| {
            build_ui(app, sheeps.clone(), tick.clone(), tick_ms, sprite_bytes, walk_on_windows);
        });

        // SIGINT (Ctrl+C im Terminal) sauber abfangen und Application beenden.
        // SIGINT = 2 auf Linux (POSIX).
        let app_for_sig = app.clone();
        glib::unix_signal_add_local(2, move || {
            app_for_sig.quit();
            glib::ControlFlow::Break
        });

        // Tray-Icon in eigenem Thread. Beim Klick auf "Beenden" wird ein
        // AtomicBool gesetzt; der GTK-Loop pollt es und beendet dann sauber.
        let quit_flag = start_tray_service();
        let app_for_tray = app.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
            if quit_flag.load(Ordering::Relaxed) {
                app_for_tray.quit();
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
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

fn build_ui<TickFn>(
    app: &Application,
    sheeps: Rc<RefCell<Vec<Sheep>>>,
    tick: Rc<RefCell<TickFn>>,
    tick_ms: u32,
    sprite_bytes: &'static [u8],
    walk_on_windows: bool,
) where
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

    let sprite = load_sprite(sprite_bytes);

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

        let area = DrawingArea::new();
        area.set_hexpand(true);
        area.set_vexpand(true);

        // Draw-Func: rendert alle Sheeps, die auf DIESEM Monitor stehen.
        let sheeps_draw = sheeps.clone();
        let sprite_draw = sprite.clone();
        let my_idx = idx;
        area.set_draw_func(move |_, cr, _w, _h| {
            let all = sheeps_draw.borrow();
            for s in all.iter().filter(|s| s.monitor_idx == my_idx) {
                draw_sheep(cr, s, sprite_draw.as_ref());
            }
        });

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
                let target = mon_rects_c.iter().enumerate().find(|(_, (mx, my, mw, mh))| {
                    (cx as i32) >= *mx
                        && (cx as i32) < *mx + *mw
                        && (cy as i32) >= *my
                        && (cy as i32) < *my + *mh
                });
                let Some((target_idx, (tmx, tmy, tmw, tmh))) = target else { return };

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
                let _ = w;
            }
        });
    }

    // Tick-Loop
    let sheeps_tick = sheeps.clone();
    let tick_cb = tick.clone();
    let overlays_tick = overlays.clone();
    let surfaces_tick = surfaces.clone();
    let last = Rc::new(RefCell::new(Instant::now()));
    glib::timeout_add_local(std::time::Duration::from_millis(tick_ms as u64), move || {
        let now = Instant::now();
        let dt = now.duration_since(*last.borrow()).as_millis() as u32;
        *last.borrow_mut() = now;

        let dims: Vec<(i32, i32)> = overlays_tick
            .borrow()
            .iter()
            .map(|o| o.area.upgrade().map(|a| (a.width(), a.height())).unwrap_or((0, 0)))
            .collect();

        {
            let mut s = sheeps_tick.borrow_mut();
            let surf = surfaces_tick.borrow();
            let world = World { area_dims: &dims, surfaces: &surf };
            (tick_cb.borrow_mut())(&mut *s, dt.max(1), &world);
        }

        // Input-Region pro Overlay aus Sprite-Bboxes bauen und setzen –
        // nur der Sprite-Bereich ist klick-/dragbar, Rest bleibt durchlässig.
        {
            let all = sheeps_tick.borrow();
            for (idx, oh) in overlays_tick.borrow().iter().enumerate() {
                let Some(win) = oh.window.upgrade() else { continue };
                let Some(surface) = win.native().and_then(|n| n.surface()) else { continue };
                let region = cairo::Region::create();
                for s in all.iter().filter(|s| s.monitor_idx == idx) {
                    let (bx, by, bw, bh) = s.hitbox();
                    let rect = cairo::RectangleInt::new(bx, by, bw, bh);
                    region.union_rectangle(&rect).ok();
                }
                surface.set_input_region(&region);
                if let Some(a) = oh.area.upgrade() {
                    a.queue_draw();
                }
            }
        }
        glib::ControlFlow::Continue
    });

    if walk_on_windows {
        start_window_poller(overlays.clone(), surfaces.clone());
    }

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
fn start_window_poller(
    overlays: Rc<RefCell<Vec<OverlayHandle>>>,
    surfaces: Rc<RefCell<Vec<Vec<Surface>>>>,
) {
    use gdk4_x11::prelude::*;
    let Some(display) = gdk4::Display::default() else { return };
    let Ok(x11display) = display.downcast::<gdk4_x11::X11Display>() else {
        eprintln!("Kein X11-Display – Fenster-Laufen deaktiviert.");
        return;
    };
    let debug = std::env::var_os("DESKTOP_SHEEP_DEBUG").is_some();
    let atoms = unsafe { x11_windows::Atoms::new(x11display.xdisplay()) };
    let mut last_debug = String::new();
    let mut ticks: u32 = 0;
    let mut changing = true;

    glib::timeout_add_local(std::time::Duration::from_millis(WINDOW_POLL_MS), move || {
        ticks = ticks.wrapping_add(1);
        if !changing && ticks % WINDOW_POLL_IDLE_EVERY != 0 {
            return glib::ControlFlow::Continue;
        }
        let xdisp = unsafe { x11display.xdisplay() };
        x11display.error_trap_push();
        let windows = unsafe { x11_windows::list_windows(xdisp, &atoms) };
        // Root-Rechteck jeder Overlay-Zeichenfläche: tatsächliche Fenster-
        // position (der WM kann verschieben) + Live-Größe der DrawingArea.
        let overlay_rects: Vec<Rect> = overlays
            .borrow()
            .iter()
            .map(|o| {
                let (Some(win), Some(area)) = (o.window.upgrade(), o.area.upgrade()) else {
                    return Rect { x: 0, y: 0, width: 0, height: 0 };
                };
                let origin = win
                    .native()
                    .and_then(|n| n.surface())
                    .and_then(|s| s.downcast::<gdk4_x11::X11Surface>().ok())
                    .and_then(|s| unsafe { x11_windows::root_origin(xdisp, s.xid()) })
                    .unwrap_or((0, 0));
                Rect { x: origin.0, y: origin.1, width: area.width(), height: area.height() }
            })
            .collect();
        let names: Vec<String> = if debug {
            windows.iter().map(|w| unsafe { x11_windows::window_name(xdisp, w.id as _) }).collect()
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
    });
}

fn draw_sheep(cr: &cairo::Context, s: &Sheep, sprite: &Option<ImageSurface>) {
    let f = s.current_frame();
    let (col, row) = frame_to_tile(f);
    let sx = (col * TILE_W) as f64;
    let sy = (row * TILE_H) as f64;
    let dst_x = s.x as f64;
    let dst_y = s.y as f64;

    cr.save().ok();
    if s.facing == Facing::Right {
        cr.translate(dst_x + TILE_W as f64, dst_y);
        cr.scale(-1.0, 1.0);
    } else {
        cr.translate(dst_x, dst_y);
    }
    if let Some(img) = sprite {
        cr.set_source_surface(img, -sx, -sy).ok();
        cr.rectangle(0.0, 0.0, TILE_W as f64, TILE_H as f64);
        cr.fill().ok();
    }
    cr.restore().ok();
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
            Some(Self { display: d, root, last: None })
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
            if ok == 0 { None } else { Some((rx, ry)) }
        }
    }

    /// True, wenn sich die Cursorposition seit dem letzten Aufruf verändert
    /// hat. Beim allerersten Aufruf wird nur der Referenzwert gesetzt und
    /// false zurückgegeben (sonst würde jedes Sheep beim Start sofort wach).
    fn moved_since_last(&mut self) -> bool {
        let Some(pos) = self.query() else { return false };
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
    let Some(surface) = window.native().and_then(|n| n.surface()) else { return };
    let Ok(x11surf) = surface.downcast::<gdk4_x11::X11Surface>() else { return };
    let xid = x11surf.xid();
    let Some(display) = gdk4::Display::default() else { return };
    let Ok(x11display) = display.downcast::<gdk4_x11::X11Display>() else { return };
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
    let Some(surface) = window.native().and_then(|n| n.surface()) else { return };
    let Ok(x11surf) = surface.downcast::<gdk4_x11::X11Surface>() else { return };
    let xid = x11surf.xid();
    let Some(display) = gdk4::Display::default() else { return };
    let Ok(x11display) = display.downcast::<gdk4_x11::X11Display>() else { return };
    unsafe {
        let xdisp = x11display.xdisplay();
        x11::xlib::XMoveResizeWindow(xdisp, xid, x, y, w as u32, h as u32);
        x11::xlib::XFlush(xdisp);
    }
}

struct DesktopSheepTray {
    quit_flag: Arc<AtomicBool>,
}

impl ksni::Tray for DesktopSheepTray {
    fn icon_name(&self) -> String {
        // Standardsymbol aus dem Icon-Theme – kein Custom-Icon nötig.
        "face-smile".into()
    }
    fn title(&self) -> String {
        "Desktop Sheep".into()
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Desktop Sheep".into(),
            description: "Overlay-Schafe laufen an der Bildschirmunterkante".into(),
            icon_name: "face-smile".into(),
            icon_pixmap: Vec::new(),
        }
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![StandardItem {
            label: "Beenden".into(),
            icon_name: "application-exit".into(),
            activate: Box::new(|tray: &mut Self| {
                tray.quit_flag.store(true, Ordering::Relaxed);
            }),
            ..Default::default()
        }
        .into()]
    }
}

/// Startet den ksni-TrayService in einem eigenen Thread und liefert ein
/// AtomicBool zurück, das vom Beenden-Menüpunkt gesetzt wird.
fn start_tray_service() -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let tray = DesktopSheepTray { quit_flag: flag.clone() };
    let service = ksni::TrayService::new(tray);
    service.spawn();
    flag
}

fn load_sprite(bytes: &'static [u8]) -> Rc<Option<ImageSurface>> {
    let mut reader = std::io::Cursor::new(bytes);
    match ImageSurface::create_from_png(&mut reader) {
        Ok(s) => Rc::new(Some(s)),
        Err(e) => {
            eprintln!("Sprite konnte nicht geladen werden: {e}");
            Rc::new(None)
        }
    }
}
