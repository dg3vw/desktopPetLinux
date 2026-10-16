use super::anim::{def, pick_next, AnimId, Rng, SpriteColor, TILE_H, TILE_W};
use super::geometry::GroundSegment;
use super::surfaces::{Surface, WindowId};
use super::world::World;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Facing {
    Left,
    Right,
}

/// Wie weit vor dem Ende einer Fensterkante (Sprite-Mitte) das Schaf
/// stehen bleibt und überlegt, was es tut.
const EDGE_MARGIN: f32 = 10.0;
/// Wahrscheinlichkeit (%) an einer äußeren Bildschirmkante aus dem Bild zu
/// laufen und später von oben wieder hereinzufallen.
const LEAVE_PERCENT: u32 = 20;
/// Wahrscheinlichkeit (%) an einer äußeren Bildschirmkante die Wand
/// hochzuklettern (zusätzlich zu LEAVE_PERCENT).
const CLIMB_PERCENT: u32 = 30;
/// Globaler Faktor auf alle Lauf- und Klettergeschwindigkeiten
/// (`dx_per_tick` der Animationen, Wand). Physik (Fallen, Sprünge vom
/// Fenster, Meteor) bleibt davon unberührt.
const SPEED_SCALE: f32 = 0.5;
/// Klettergeschwindigkeit an der Wand (Pixel pro 16-ms-Tick, vor SPEED_SCALE).
const CLIMB_PX_PER_TICK: f32 = 1.5;
/// Absprung beim jump_down: horizontal / nach oben (Pixel pro Sekunde).
const JUMP_VX: f32 = 150.0;
const JUMP_VY: f32 = 300.0;
/// Sackt das Trägerfenster um mehr als so viele Pixel ab, verliert das
/// Schaf den Halt (kleinere Sprünge werden einfach nachgeführt).
const DETACH_PX: f32 = 3.0;
/// Scheitelhöhe des Bocksprungs (Pixel).
const JUMP_HEIGHT: f32 = 45.0;
/// Wie hoch das Schaf beim Aussteigen in der Wanne steht (Original offsety).
const BATH_STAND_PX: f32 = 20.0;
/// Fluggeschwindigkeit des Bade-Meteors je Achse (Pixel pro Sekunde).
const BATH_SPEED: f32 = 260.0;
/// So lange bleibt die Wanne nach dem Aussteigen noch stehen (ms).
const TUB_LINGER_MS: u32 = 1500;
/// Ausblendzeit des schwarzen Schafs (ms).
const FADE_MS: u32 = 500;
/// Anteile (%) der Auftritte nach dem Rauslaufen: Bade-Meteor, Besuch vom
/// schwarzen Schaf; der Rest fällt wie gehabt von oben herein.
const ENTRY_BATH_PERCENT: u32 = 15;
const ENTRY_VISIT_PERCENT: u32 = 15;
/// Abstand der Sprite-Mitten, ab dem ein laufendes Schaf auf ein anderes
/// vor sich reagiert (Bocksprung oder Umdrehen).
const MEET_DISTANCE: f32 = 55.0;
/// Wahrscheinlichkeit (%), über ein Schaf im Weg zu springen statt
/// umzudrehen – im Gehen bzw. im Sprint.
const LEAP_PERCENT_WALK: u32 = 40;
const LEAP_PERCENT_RUN: u32 = 75;
/// Höhenunterschied (Pixel), bis zu dem zwei Schafe als auf derselben
/// Fläche stehend gelten.
const SAME_LEVEL_PX: f32 = 4.0;

/// Wie ein Schaf (neu) ins Bild kommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// Fällt von oben herein (Original: spawn id 2).
    Sky,
    /// Saust als brennender Meteor in eine Badewanne (Original: spawn id 3).
    Bath,
    /// Läuft von der Seite herein und trifft ein schwarzes Schaf
    /// (Original: spawn id 4).
    Visit,
}

impl Entry {
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "sky" => Self::Sky,
            "bath" => Self::Bath,
            "visit" => Self::Visit,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PropKind {
    Tub,
    BlackSheep,
}

/// Zweites Sprite, das zu einem Schaf gehört (Original: `child`) – die
/// Badewanne oder das schwarze Schaf. Liegt auf demselben Monitor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prop {
    pub x: f32,
    pub y: f32,
    pub frame: u32,
    /// Sprite horizontal spiegeln.
    pub mirrored: bool,
    /// Deckkraft 0..1.
    pub alpha: f32,
    kind: PropKind,
}

pub struct Sheep {
    /// Position in Overlay-lokalen Koordinaten des aktuellen Monitors
    /// (Anker: linke obere Ecke des Sprites).
    pub x: f32,
    pub y: f32,
    pub facing: Facing,
    /// Index in die von der Platform gelieferte Segment-/Overlay-Liste.
    /// Wechselt beim Übergang über eine angrenzende Monitorkante.
    pub monitor_idx: usize,
    /// Farbvariante, mit der dieses Schaf gezeichnet wird.
    pub color: SpriteColor,
    /// True während der Benutzer das Sprite zieht: zappelt, aber bewegt
    /// sich nicht von selbst und wechselt den Zustand nicht.
    pub dragged: bool,
    /// True während des freien Falls (Loslassen, Sprung, Absturz vom Fenster).
    falling: bool,
    /// Geschwindigkeit für die Fall-Physik (Pixel pro Sekunde).
    vx: f32,
    vy: f32,
    /// Fenster unter den Füßen; `None` = Boden (Bildschirmunterkante).
    support: Option<WindowId>,
    /// Sprite-Versatz unter die Kante, solange das Schaf an ihr hängt.
    hang_offset: f32,
    /// Landeanimation des laufenden Falls; `None` = zufällig FallHard/Boing
    /// mit vorherigem Nachfedern.
    land_anim: Option<AnimId>,
    /// Läuft gerade aus dem Bild, um danach von oben neu hereinzufallen.
    leaving: bool,
    /// Auftritt, der beim ersten Tick mit bekannten Overlay-Maßen startet.
    pending_entry: Option<Entry>,
    /// Badewanne bzw. schwarzes Schaf.
    prop: Option<Prop>,
    /// Restlebensdauer des Props (ms); `None` = bleibt vorerst.
    prop_ttl_ms: Option<u32>,
    /// Treffpunkt beim Besuch des schwarzen Schafs (x, Overlay-lokal).
    meet_x: f32,
    anim: AnimId,
    frame_idx: usize,
    frame_elapsed_ms: u32,
    repeats_done: u32,
    rng: Rng,
}

impl Sheep {
    /// Erzeugt ein Schaf, das über Position `x` auf Monitor `monitor_idx`
    /// von oben ins Bild fällt. Expliziter RNG-Seed, damit sich mehrere
    /// Instanzen zuverlässig unterscheiden.
    pub fn new_at(monitor_idx: usize, x: f32, seed: u64) -> Self {
        let mut s = Self {
            x,
            y: 0.0,
            facing: Facing::Right,
            monitor_idx,
            color: SpriteColor::Default,
            dragged: false,
            falling: false,
            vx: 0.0,
            vy: 0.0,
            support: None,
            hang_offset: 0.0,
            land_anim: None,
            leaving: false,
            pending_entry: None,
            prop: None,
            prop_ttl_ms: None,
            meet_x: 0.0,
            anim: AnimId::Walk,
            frame_idx: 0,
            frame_elapsed_ms: 0,
            repeats_done: 0,
            rng: Rng::from_seed(seed),
        };
        s.drop_in(monitor_idx, x);
        s
    }

    /// Wählt eine zufällige Farbvariante (Modus „jedes Schaf anders“).
    pub fn randomize_color(&mut self) {
        let n = SpriteColor::ALL.len() as u32;
        self.color = SpriteColor::ALL[self.rng.range(n) as usize];
    }

    /// Statt von oben hereinzufallen, den Auftritt `entry` spielen, sobald
    /// die Overlay-Maße bekannt sind (Start/Debug).
    pub fn set_entry(&mut self, entry: Entry) {
        self.pending_entry = Some(entry);
    }

    /// Badewanne bzw. schwarzes Schaf, falls gerade sichtbar – wird vor
    /// dem Schaf gezeichnet.
    pub fn prop(&self) -> Option<&Prop> {
        self.prop.as_ref()
    }

    /// Aktuell zu zeichnender Frame-Index im Sprite-Sheet.
    pub fn current_frame(&self) -> u32 {
        def(self.anim).frames[self.frame_idx]
    }

    /// True, solange das Schaf in irgendeinem Schlaf-Loop steckt
    /// (klassisches Liegen oder Sitzen/Dösen).
    #[cfg(test)]
    pub fn is_sleeping(&self) -> bool {
        matches!(self.anim, AnimId::Sleep | AnimId::Sleep2 | AnimId::Sleep3)
    }

    /// Fenster, auf dem das Schaf gerade steht (`None` = Boden oder in der Luft).
    #[cfg(test)]
    pub fn support_window(&self) -> Option<WindowId> {
        self.support
    }

    /// Aktuelle Sprite-Bounding-Box in Overlay-lokalen Koordinaten.
    pub fn hitbox(&self) -> (i32, i32, i32, i32) {
        (
            self.x as i32,
            self.y as i32,
            TILE_W as i32,
            TILE_H as i32,
        )
    }

    /// Trefft der Punkt (mx, my) das Sprite?
    pub fn contains(&self, mx: f32, my: f32) -> bool {
        mx >= self.x
            && mx < self.x + TILE_W as f32
            && my >= self.y
            && my < self.y + TILE_H as f32
    }

    /// Bricht den Schlaf-Loop ab und wechselt sofort zur WakeUp-Animation.
    /// Kein Effekt, wenn das Schaf gerade nicht im Sleep-Zustand ist.
    pub fn wake_from_sleep(&mut self) {
        let wake = match self.anim {
            AnimId::Sleep => Some(AnimId::WakeUp),
            AnimId::Sleep2 => Some(AnimId::Sleep2Wake),
            AnimId::Sleep3 => Some(AnimId::Sleep3Wake),
            _ => None,
        };
        if let Some(next) = wake {
            self.set_anim(next);
        }
    }

    /// User hat das Sheep gepackt: Zappel-Animation starten, Bewegung
    /// einfrieren, ggf. Fall abbrechen.
    pub fn start_drag(&mut self) {
        self.dragged = true;
        self.falling = false;
        self.vx = 0.0;
        self.vy = 0.0;
        self.support = None;
        self.hang_offset = 0.0;
        self.leaving = false;
        self.pending_entry = None;
        self.prop = None;
        self.prop_ttl_ms = None;
        self.set_anim(AnimId::Drag);
    }

    /// User hat losgelassen: fällt mit Gravitation, bounct, landet – auf dem
    /// Boden oder auf einer Fensterkante darunter.
    pub fn end_drag(&mut self) {
        self.dragged = false;
        self.set_anim(AnimId::Fall);
    }

    /// Setzt die Position (samt Ziel-Monitor). Nur während Drag sinnvoll.
    pub fn set_pos(&mut self, monitor_idx: usize, x: f32, y: f32) {
        self.monitor_idx = monitor_idx;
        self.x = x;
        self.y = y;
    }

    /// Führt einen Tick der Simulation aus.
    ///
    /// * `segments` – ein GroundSegment pro Monitor (sortiert nach x_start,
    ///   passend zu den Overlay-Fenstern im Backend)
    /// * `world` – Live-Maße der Overlays und begehbare Fensterkanten
    ///
    /// Auf dem Boden: beim Erreichen einer Segmentkante wird geprüft, ob das
    /// Nachbarsegment nahtlos anschließt (X-Kanten in Kontakt, gleiche
    /// Bodenhöhe). Wenn ja, wechselt das Schaf den Monitor, sonst dreht es um
    /// oder läuft aus dem Bild. Auf einem Fenster: verschwindet es unter dem
    /// Schaf, fällt es (nach verdutztem Blick); an der Kante entscheidet es
    /// zwischen Umdrehen, Runterspringen und Abrutschen.
    pub fn tick(&mut self, dt_ms: u32, segments: &[GroundSegment], world: &World) {
        if let Some(entry) = self.pending_entry {
            if world.dims(self.monitor_idx).1 > 0 {
                self.pending_entry = None;
                self.start_entry(entry, world);
            }
        }
        self.tick_inner(dt_ms, segments, world);
        self.update_prop(dt_ms);
    }

    fn tick_inner(&mut self, dt_ms: u32, segments: &[GroundSegment], world: &World) {
        // Während Drag: nur Zappel-Frames animieren, sonst gar nichts.
        if self.dragged {
            self.advance_frames_loop(dt_ms);
            return;
        }

        // Während Fall: Gravitation, Position, Landung – in Teilschritten,
        // damit Bounce/Landung auch bei ruckelnden Ticks stabil bleiben.
        if self.falling {
            let mut rest = dt_ms;
            while rest > 0 && self.falling {
                let step = rest.min(16);
                self.tick_falling(step, world);
                rest -= step;
            }
            if self.falling {
                self.advance_frames_loop(dt_ms);
                return;
            }
            // Sonst gerade gelandet – Rest des Ticks (Lande-Sequenz) läuft
            // durch die reguläre Zustandsmaschine unten weiter.
        }

        // 1) Frame + Zustands-Übergang
        self.step_frames(dt_ms);
        if self.falling {
            // Eintritt in eine Fall-Animation (Sprung/Abrutschen).
            return;
        }
        match self.anim {
            AnimId::Surprised => {
                // Hängt verdutzt in der Luft – weder Bewegung noch Boden-Snap.
                return;
            }
            AnimId::ClimbStart | AnimId::ClimbUp | AnimId::ClimbDown => {
                self.tick_wall(dt_ms, world);
                return;
            }
            AnimId::TopWalkStart | AnimId::TopWalk | AnimId::TopWalkEnd => {
                self.tick_ceiling(dt_ms, segments, world);
                return;
            }
            AnimId::BathFly | AnimId::BathBurn => {
                self.tick_bath_flight(dt_ms, world);
                return;
            }
            AnimId::VisitRun | AnimId::VisitWalk => {
                self.tick_visit(dt_ms, world);
                return;
            }
            _ => {}
        }

        // 2) Halt auf dem Fenster prüfen. Das Fenster nimmt das Schaf nicht
        //    mit: Zieht man es weg oder lässt es absacken, steht das Schaf
        //    plötzlich in der Luft, schaut verdutzt und fällt dann. Nur wenn
        //    das Fenster nach oben geschoben wird, wird es mit angehoben.
        let mut seg = None;
        if let Some(window) = self.support {
            let feet = self.y + TILE_H as f32 - self.hang_offset + self.lift();
            match self.find_support(window, world) {
                Some(s) if s.y as f32 <= feet + DETACH_PX => seg = Some(s),
                _ => {
                    self.lose_support();
                    return;
                }
            }
        }

        // 3) Bewegung (nur wenn dx_per_tick > 0: Walk/Run)
        let cur = def(self.anim);
        if cur.dx_per_tick > 0 {
            let dx = self.step_px(dt_ms) * self.forward();
            self.x += dx;
            match seg {
                Some(s) => self.check_window_edge(&s),
                None => {
                    if self.check_floor_edge(segments, world) {
                        // Aus dem Bild gelaufen und neu aufgetreten – der
                        // Auftritt hat Position und Höhe selbst gesetzt.
                        return;
                    }
                }
            }
        }

        // 4) Höhe: Fensterkante bzw. Unterkante der Zeichenfläche des
        //    aktuellen Monitors, Fallback auf Segment-y falls Overlay-Maße
        //    noch unbekannt.
        if let Some(s) = seg {
            self.y = s.y as f32 - TILE_H as f32 + self.hang_offset;
        } else {
            let (_, ah) = world.dims(self.monitor_idx);
            let bottom = if ah > 0 {
                ah
            } else {
                segments.get(self.monitor_idx).map(|s| s.y).unwrap_or(0)
            };
            self.y = (bottom as u32).saturating_sub(TILE_H) as f32;
        }
        self.y -= self.lift();
    }

    /// Höhe des Sprites über seiner Standfläche: Sprungbogen beim Jump,
    /// Aufstehen in der Wanne.
    fn lift(&self) -> f32 {
        match self.anim {
            AnimId::Jump => {
                let cur = def(self.anim);
                let total = (cur.frames.len() as u32 * cur.interval_ms) as f32;
                let t = self.frame_idx as u32 * cur.interval_ms + self.frame_elapsed_ms;
                let p = (t as f32 / total).clamp(0.0, 1.0);
                4.0 * JUMP_HEIGHT * p * (1.0 - p)
            }
            AnimId::BathOut => BATH_STAND_PX,
            _ => 0.0,
        }
    }

    /// Startet einen Auftritt auf einem zufälligen Monitor. Bad und Besuch
    /// fallen auf `Sky` zurück, wenn der Monitor zu klein dafür ist.
    fn start_entry(&mut self, entry: Entry, world: &World) {
        let n = world.area_dims.len().max(1) as u32;
        let idx = self.rng.range(n) as usize;
        let (aw, ah) = world.dims(idx);
        let floor = (ah - TILE_H as i32).max(0) as f32;
        let from_right = self.rng.range(2) == 0;

        self.monitor_idx = idx;
        self.falling = false;
        self.vx = 0.0;
        self.vy = 0.0;
        self.support = None;
        self.hang_offset = 0.0;
        self.leaving = false;
        self.prop = None;
        self.prop_ttl_ms = None;
        self.facing = if from_right { Facing::Left } else { Facing::Right };

        match entry {
            Entry::Bath if (aw - TILE_W as i32).min(ah - TILE_H as i32) >= 240 => {
                // Wanne am Boden; der Meteor startet knapp außerhalb des
                // Bildrands und fliegt im 45°-Winkel genau hinein.
                let d_max = (aw - TILE_W as i32).min(ah - TILE_H as i32) as u32;
                let d = (200 + self.rng.range(d_max - 200 + 1)) as f32;
                let x0 = if from_right { aw as f32 } else { -(TILE_W as f32) };
                let tub_x = x0 + self.forward() * d;
                self.x = x0;
                self.y = floor - d;
                self.prop = Some(Prop {
                    x: tub_x,
                    y: floor,
                    frame: 146,
                    mirrored: false,
                    alpha: 1.0,
                    kind: PropKind::Tub,
                });
                self.set_anim(AnimId::BathFly);
            }
            Entry::Visit if aw >= 4 * TILE_W as i32 => {
                // Treffpunkt um die Bildschirmmitte, das schwarze Schaf
                // kommt spiegelbildlich von der anderen Seite.
                let jitter = self.rng.range((aw / 3).max(1) as u32) as f32 - aw as f32 / 6.0;
                self.meet_x = aw as f32 / 2.0 + jitter;
                self.x = if from_right { aw as f32 } else { -(TILE_W as f32) };
                self.y = floor;
                self.prop = Some(Prop {
                    x: 0.0,
                    y: floor,
                    frame: 155,
                    mirrored: !from_right,
                    alpha: 1.0,
                    kind: PropKind::BlackSheep,
                });
                self.set_anim(AnimId::VisitRun);
            }
            _ => {
                let x = self.rng.range((aw - TILE_W as i32).max(1) as u32) as f32;
                self.drop_in(idx, x);
            }
        }
    }

    /// Meteor-Flug: diagonal nach unten, bis die Wanne erreicht ist.
    fn tick_bath_flight(&mut self, dt_ms: u32, world: &World) {
        let (_, ah) = world.dims(self.monitor_idx);
        let floor = (ah - TILE_H as i32).max(0) as f32;
        let step = BATH_SPEED * dt_ms as f32 / 1000.0;
        self.x += self.forward() * step;
        self.y += step;
        if self.y >= floor {
            self.y = floor;
            if let Some(tub) = self.prop {
                self.x = tub.x;
            }
            self.set_anim(AnimId::BathSoak);
        }
    }

    /// Besuch: auf den Treffpunkt zulaufen, die letzten Meter gehen,
    /// dann erröten. Das schwarze Schaf spiegelt die Bewegung (update_prop).
    fn tick_visit(&mut self, dt_ms: u32, world: &World) {
        let (_, ah) = world.dims(self.monitor_idx);
        self.y = (ah - TILE_H as i32).max(0) as f32;
        let target = if self.facing == Facing::Left {
            self.meet_x
        } else {
            self.meet_x - TILE_W as f32
        };
        let dx = self.step_px(dt_ms);
        self.x += self.forward() * dx;
        let remaining = (target - self.x) * self.forward();
        if remaining <= 0.0 {
            self.x = target;
            self.set_anim(AnimId::VisitBlush);
        } else if self.anim == AnimId::VisitRun && remaining <= 2.5 * TILE_W as f32 {
            self.set_anim(AnimId::VisitWalk);
        }
    }

    /// Prop an den Zustand des Schafs anpassen und ggf. ablaufen lassen.
    fn update_prop(&mut self, dt_ms: u32) {
        if let Some(ttl) = self.prop_ttl_ms {
            let left = ttl.saturating_sub(dt_ms);
            if left == 0 {
                self.prop = None;
                self.prop_ttl_ms = None;
                return;
            }
            self.prop_ttl_ms = Some(left);
        }
        let anim = self.anim;
        let idx = self.frame_idx;
        let ttl = self.prop_ttl_ms;
        let white_x = self.x;
        let meet = self.meet_x;
        let Some(p) = self.prop.as_mut() else { return };
        match p.kind {
            PropKind::Tub => {
                p.frame = if anim == AnimId::BathSoak { 147 + (idx % 2) as u32 } else { 146 };
            }
            PropKind::BlackSheep => {
                p.frame = match anim {
                    AnimId::VisitRun => [155, 154, 154][idx % 3],
                    AnimId::VisitWalk => [156, 157][idx % 2],
                    _ => 157,
                };
                // Spiegelbildlich zum weißen Schaf, bis es nach dem Erröten
                // weiterläuft – dann bleibt das schwarze stehen und verblasst.
                if matches!(anim, AnimId::VisitRun | AnimId::VisitWalk | AnimId::VisitBlush) {
                    p.x = 2.0 * meet - TILE_W as f32 - white_x;
                }
                p.alpha = ttl.map_or(1.0, |t| (t as f32 / FADE_MS as f32).min(1.0));
            }
        }
    }

    /// Laufstrecke der aktuellen Animation in `dt_ms` (ohne Richtung).
    fn step_px(&self, dt_ms: u32) -> f32 {
        def(self.anim).dx_per_tick as f32 * SPEED_SCALE * (dt_ms as f32 / 16.0)
    }

    fn forward(&self) -> f32 {
        if self.facing == Facing::Right { 1.0 } else { -1.0 }
    }

    /// Wechselt in eine neue Animation und löst deren Eintritts-Effekte aus.
    fn set_anim(&mut self, next: AnimId) {
        self.anim = next;
        self.frame_idx = 0;
        self.frame_elapsed_ms = 0;
        self.repeats_done = 0;
        self.on_enter(next);
    }

    /// Seiteneffekte beim Betreten einer Animation – alles, was über reines
    /// Frame-Abspielen hinausgeht (Versatz, Physik-Start).
    fn on_enter(&mut self, anim: AnimId) {
        match anim {
            AnimId::JumpDownAir => {
                let fwd = self.forward();
                self.start_fall(Some(AnimId::JumpDownLand));
                self.vx = fwd * JUMP_VX;
                self.vy = -JUMP_VY;
            }
            AnimId::FallWinA => {
                // Vorderbeine über die Kante, Sprite rutscht etwas tiefer
                // (Original: x -imageW*0.7, y +imageH*0.45).
                self.x += self.forward() * TILE_W as f32 * 0.7;
                self.hang_offset = TILE_H as f32 * 0.45;
            }
            AnimId::FallWinC => self.start_fall(Some(AnimId::FallWinD)),
            AnimId::Fall | AnimId::FallPanic => self.start_fall(None),
            AnimId::BathOut => {
                let out = def(AnimId::BathOut);
                self.prop_ttl_ms = Some(out.frames.len() as u32 * out.interval_ms + TUB_LINGER_MS);
            }
            AnimId::VisitBlush => {
                let blush = def(AnimId::VisitBlush);
                self.prop_ttl_ms = Some(blush.frames.len() as u32 * blush.interval_ms + FADE_MS);
            }
            _ => {}
        }
    }

    /// Frame weiterschalten; am Sequenzende gewichteten Nachfolger wählen.
    fn step_frames(&mut self, dt_ms: u32) {
        let cur = def(self.anim);
        self.frame_elapsed_ms += dt_ms;
        while self.frame_elapsed_ms >= cur.interval_ms {
            self.frame_elapsed_ms -= cur.interval_ms;
            self.frame_idx += 1;
            if self.frame_idx < cur.frames.len() {
                continue;
            }
            self.frame_idx = 0;
            self.repeats_done += 1;
            if self.repeats_done < cur.repeat {
                continue;
            }
            self.repeats_done = 0;
            // Beim Rauslaufen keine Zustandswechsel – sonst bleibt das Schaf
            // halb im Bildrand stehen.
            let nxt = if self.leaving {
                self.anim
            } else {
                pick_next(cur.next, &mut self.rng)
            };
            // Facing beim Ende der aktuellen Animation umkehren
            // (z. B. Rotate1A -> Rotate1B mit gedrehtem Sprite).
            if cur.flip_on_end {
                self.facing = match self.facing {
                    Facing::Left => Facing::Right,
                    Facing::Right => Facing::Left,
                };
            }
            if nxt != self.anim {
                self.set_anim(nxt);
                break;
            }
        }
    }

    /// Sucht die Kante des Trägerfensters unter der Sprite-Mitte. Hängt das
    /// Schaf gerade an der Kante, reicht es, wenn die Kante in Reichweite ist.
    fn find_support(&self, window: WindowId, world: &World) -> Option<Surface> {
        let cx = self.x + TILE_W as f32 / 2.0;
        let tol = if self.hang_offset > 0.0 { TILE_W as f32 } else { 0.0 };
        world
            .surfaces(self.monitor_idx)
            .iter()
            .find(|s| {
                s.window == window
                    && cx >= s.x_start as f32 - tol
                    && cx <= s.x_end as f32 + tol
            })
            .copied()
    }

    /// Fenster ist unter dem Schaf weg (weggezogen, abgesackt, geschlossen,
    /// minimiert, verdeckt). Wer an der Kante hängt, lässt sofort los; sonst
    /// erst der verdutzte Blick, dann der Fall.
    fn lose_support(&mut self) {
        self.support = None;
        if self.hang_offset > 0.0 {
            self.set_anim(AnimId::FallWinC);
        } else {
            self.set_anim(AnimId::Surprised);
        }
    }

    /// Am Ende einer Fensterkante: stehen bleiben und reagieren.
    fn check_window_edge(&mut self, seg: &Surface) {
        let half = TILE_W as f32 / 2.0;
        let cx = self.x + half;
        let drop = match self.facing {
            Facing::Right if cx >= seg.x_end as f32 - EDGE_MARGIN => {
                self.x = seg.x_end as f32 - EDGE_MARGIN - half;
                seg.right_is_drop()
            }
            Facing::Left if cx <= seg.x_start as f32 + EDGE_MARGIN => {
                self.x = seg.x_start as f32 + EDGE_MARGIN - half;
                seg.left_is_drop()
            }
            _ => return,
        };
        // Endet die Kante, weil ein höheres Fenster sie verdeckt oder der
        // Monitor endet, steht dort eine Wand – umdrehen bzw. dagegenrennen
        // wie am Bildschirmrand, nicht runterspringen.
        if !drop {
            self.begin_turn();
            return;
        }
        let next = match self.anim {
            AnimId::Run | AnimId::RunBegin | AnimId::RunEnd => {
                if self.rng.range(2) == 0 { AnimId::JumpDown } else { AnimId::Rotate1A }
            }
            _ => {
                if self.rng.range(100) < 60 { AnimId::LookDown } else { AnimId::Rotate1A }
            }
        };
        self.set_anim(next);
    }

    /// Am Boden: Monitorwechsel, Umdrehen oder aus dem Bild laufen.
    /// Rückgabe: true, wenn das Schaf ganz draußen war und neu aufgetreten ist.
    fn check_floor_edge(&mut self, segments: &[GroundSegment], world: &World) -> bool {
        let (aw, _) = world.dims(self.monitor_idx);
        if self.leaving {
            if self.x <= -(TILE_W as f32) || self.x >= aw as f32 {
                self.respawn(world);
                return true;
            }
            return false;
        }
        let min_x = 0.0f32;
        let max_x = (aw - TILE_W as i32).max(0) as f32;
        let dir = if self.x <= min_x {
            Facing::Left
        } else if self.x >= max_x {
            Facing::Right
        } else {
            return false;
        };
        if self.try_cross(segments, world, dir) {
            return false;
        }
        let r = self.rng.range(100);
        if r < LEAVE_PERCENT {
            self.leaving = true;
            return false;
        }
        self.x = if dir == Facing::Left { min_x } else { max_x };
        if r < LEAVE_PERCENT + CLIMB_PERCENT {
            self.set_anim(AnimId::ClimbStart);
        } else {
            self.begin_turn();
        }
        false
    }

    /// An der Bildschirmwand: hoch- bzw. runterklettern. Hochgeklettert wird
    /// an der Wand in Blickrichtung, runter (nach dem Flip in TopWalkEnd) an
    /// der Wand im Rücken – passend zur Ausrichtung der Sprites.
    fn tick_wall(&mut self, dt_ms: u32, world: &World) {
        let (aw, ah) = world.dims(self.monitor_idx);
        let up = self.anim != AnimId::ClimbDown;
        let wall_left = (self.facing == Facing::Left) == up;
        self.x = if wall_left { 0.0 } else { (aw - TILE_W as i32).max(0) as f32 };
        let floor = (ah - TILE_H as i32).max(0) as f32;
        let dy = CLIMB_PX_PER_TICK * SPEED_SCALE * (dt_ms as f32 / 16.0);
        match self.anim {
            AnimId::ClimbStart => self.y = floor,
            AnimId::ClimbUp => {
                self.y -= dy;
                if self.y <= 0.0 {
                    self.y = 0.0;
                    self.set_anim(AnimId::TopWalkStart);
                }
            }
            _ => {
                self.y += dy;
                if self.y >= floor {
                    self.y = floor;
                    self.set_anim(AnimId::ClimbOver);
                }
            }
        }
    }

    /// Kopfüber an der Bildschirmoberkante. Am Monitorübergang geht es wie
    /// am Boden auf den Nachbarmonitor, an der Außenwand wieder runter.
    fn tick_ceiling(&mut self, dt_ms: u32, segments: &[GroundSegment], world: &World) {
        self.y = 0.0;
        if self.anim != AnimId::TopWalk {
            return; // Um die Ecke greifen – auf der Stelle.
        }
        let (aw, _) = world.dims(self.monitor_idx);
        let dx = self.step_px(dt_ms) * self.forward();
        self.x += dx;
        let max_x = (aw - TILE_W as i32).max(0) as f32;
        let dir = if self.x <= 0.0 {
            Facing::Left
        } else if self.x >= max_x {
            Facing::Right
        } else {
            return;
        };
        if self.try_cross(segments, world, dir) {
            return;
        }
        self.x = self.x.clamp(0.0, max_x);
        self.set_anim(AnimId::TopWalkEnd);
    }

    /// Reaktion beim Aufprall an der Segment-/Bildschirmkante.
    ///
    /// Wählt zwischen sanftem Umdrehen (Rotate1A → Rotate1B → Walk) und dem
    /// dramatischen WallRam (Aufprall + Aua-Sprites, Facing-Flip am Ende).
    /// Beim Sprint wird immer geramt, im normalen Walk nur gelegentlich –
    /// sonst wirkt jede Kante zu heftig.
    fn begin_turn(&mut self) {
        let ram = match self.anim {
            AnimId::Run | AnimId::RunBegin => true,
            AnimId::RunEnd => self.rng.range(2) == 0,
            AnimId::Walk => self.rng.range(4) == 0,
            _ => false,
        };
        self.set_anim(if ram { AnimId::WallRam } else { AnimId::Rotate1A });
    }

    /// Nach dem Rauslaufen neu auftreten: meist von oben, gelegentlich als
    /// Bade-Meteor oder mit Besuch vom schwarzen Schaf.
    fn respawn(&mut self, world: &World) {
        let r = self.rng.range(100);
        let entry = if r < ENTRY_BATH_PERCENT {
            Entry::Bath
        } else if r < ENTRY_BATH_PERCENT + ENTRY_VISIT_PERCENT {
            Entry::Visit
        } else {
            Entry::Sky
        };
        self.start_entry(entry, world);
    }

    /// Setzt das Schaf knapp oberhalb des Overlays und lässt es fallen.
    fn drop_in(&mut self, monitor_idx: usize, x: f32) {
        self.monitor_idx = monitor_idx;
        self.x = x;
        self.y = -(TILE_H as f32);
        self.leaving = false;
        self.set_anim(AnimId::Fall);
    }

    /// Fall-Physik einschalten. Die laufende Animation bleibt (Flug-Frame).
    fn start_fall(&mut self, land_anim: Option<AnimId>) {
        self.falling = true;
        self.support = None;
        self.hang_offset = 0.0;
        self.vx = 0.0;
        self.vy = 0.0;
        self.land_anim = land_anim;
    }

    /// Advance frames der aktuellen Animation ohne jegliche Zustandsübergänge.
    /// Für Drag/Fall, wo der Wechsel extern (User bzw. Physik) getrieben wird.
    fn advance_frames_loop(&mut self, dt_ms: u32) {
        let cur = def(self.anim);
        self.frame_elapsed_ms += dt_ms;
        while self.frame_elapsed_ms >= cur.interval_ms {
            self.frame_elapsed_ms -= cur.interval_ms;
            self.frame_idx = (self.frame_idx + 1) % cur.frames.len();
        }
    }

    /// Physik während des Falls: Gravitation → Position → Landung.
    ///
    /// Gelandet wird auf der höchsten Fensterkante, die die Füße in diesem
    /// Tick von oben überquert haben und die unter der Sprite-Mitte liegt –
    /// sonst auf dem Boden. Eine reine Nähe-Prüfung reicht nicht: bei hoher
    /// Fallgeschwindigkeit legt das Schaf 30+ px pro Tick zurück.
    fn tick_falling(&mut self, dt_ms: u32, world: &World) {
        const GRAVITY: f32 = 2400.0; // Pixel pro Sekunde²
        const BOUNCE_DAMP: f32 = 0.42;
        const BOUNCE_STOP: f32 = 80.0; // Pixel pro Sekunde – kleiner ⇒ landen

        let (aw, ah) = world.dims(self.monitor_idx);
        if ah <= 0 {
            // Overlay noch nicht realisiert – Boden unbekannt, warten.
            return;
        }

        let dt_s = dt_ms as f32 / 1000.0;
        let prev_bottom = self.y + TILE_H as f32;
        self.vy += GRAVITY * dt_s;
        self.y += self.vy * dt_s;
        self.x += self.vx * dt_s;
        let max_x = (aw - TILE_W as i32).max(0) as f32;
        if self.x < 0.0 || self.x > max_x {
            self.x = self.x.clamp(0.0, max_x);
            self.vx = 0.0;
        }
        if self.vy <= 0.0 {
            return; // steigt noch
        }

        let new_bottom = self.y + TILE_H as f32;
        let cx = self.x + TILE_W as f32 / 2.0;
        let mut hit: Option<(f32, Option<Surface>)> = None;
        for s in world.surfaces(self.monitor_idx) {
            let sy = s.y as f32;
            let crossed = prev_bottom <= sy + 0.5 && new_bottom >= sy;
            let under = cx >= s.x_start as f32 && cx <= s.x_end as f32;
            if crossed && under && hit.is_none_or(|(hy, _)| sy < hy) {
                hit = Some((sy, Some(*s)));
            }
        }
        if hit.is_none() && new_bottom >= ah as f32 {
            hit = Some((ah as f32, None));
        }
        let Some((ground_y, seg)) = hit else { return };

        self.y = ground_y - TILE_H as f32;
        if self.land_anim.is_none() && self.vy > BOUNCE_STOP {
            // bouncen – Richtung umkehren, Energie verlieren
            self.vy = -self.vy * BOUNCE_DAMP;
            return;
        }

        // Endgültig gelandet. Ohne vorgegebene Landeanimation zufällig
        // FallHard, das elastischere Boing oder FallSoft, damit Landungen
        // abwechslungsreich aussehen.
        self.falling = false;
        self.vx = 0.0;
        self.vy = 0.0;
        self.support = seg.map(|s| s.window);
        let next = match self.land_anim.take() {
            Some(a) => a,
            None => [AnimId::FallHard, AnimId::Boing, AnimId::FallSoft][self.rng.range(3) as usize],
        };
        self.set_anim(next);
    }

    /// Läuft das Schaf gerade normal auf Boden oder Fensterkante – nur dann
    /// weicht es anderen Schafen aus.
    fn walking_on_level(&self) -> bool {
        !self.falling
            && !self.dragged
            && !self.leaving
            && self.pending_entry.is_none()
            && self.hang_offset == 0.0
            && matches!(self.anim, AnimId::Walk | AnimId::RunBegin | AnimId::Run | AnimId::RunEnd)
    }

    /// Wie weit `other` in Laufrichtung vor diesem Schaf auf derselben
    /// Fläche steht (Abstand der Sprite-Mitten); `None`, wenn nicht.
    fn ahead_on_level(&self, other: &Sheep) -> Option<f32> {
        if other.monitor_idx != self.monitor_idx
            || other.falling
            || other.dragged
            || other.support != self.support
            || (other.y - self.y).abs() > SAME_LEVEL_PX
        {
            return None;
        }
        let ahead = (other.x - self.x) * self.forward();
        (ahead > 0.0).then_some(ahead)
    }

    /// Strecke, die der Bocksprung zurücklegt.
    fn jump_distance() -> f32 {
        let jump = def(AnimId::Jump);
        let ticks = (jump.frames.len() as u32 * jump.interval_ms) as f32 / 16.0;
        jump.dx_per_tick as f32 * SPEED_SCALE * ticks
    }

    /// Ist hinter dem Hindernis genug Platz, um nach dem Bocksprung noch auf
    /// derselben Fläche zu landen?
    fn room_to_leap(&self, world: &World) -> bool {
        let half = TILE_W as f32 / 2.0;
        let land = self.x + half + self.forward() * Self::jump_distance();
        match self.support {
            Some(window) => self.find_support(window, world).is_some_and(|s| {
                land >= s.x_start as f32 + EDGE_MARGIN && land <= s.x_end as f32 - EDGE_MARGIN
            }),
            None => {
                let (aw, _) = world.dims(self.monitor_idx);
                land >= half && land <= aw as f32 - half
            }
        }
    }

    /// Ein Schaf steht im Weg: drüberspringen oder umdrehen. Springt das
    /// andere gerade selbst, wird umgedreht – sonst treffen sie sich in der Luft.
    fn meet_sheep(&mut self, other_jumping: bool, world: &World) {
        let leap_percent = match self.anim {
            AnimId::Run | AnimId::RunBegin => LEAP_PERCENT_RUN,
            _ => LEAP_PERCENT_WALK,
        };
        let leap = !other_jumping
            && self.rng.range(100) < leap_percent
            && self.room_to_leap(world);
        self.set_anim(if leap { AnimId::Jump } else { AnimId::Rotate1A });
    }

    /// Versucht, in Bewegungsrichtung auf den Nachbarmonitor zu wechseln.
    /// Voraussetzung: das Nachbarsegment schließt in X nahtlos an und hat
    /// (annähernd) dieselbe Bodenhöhe. Rückgabe: true wenn gewechselt.
    fn try_cross(&mut self, segments: &[GroundSegment], world: &World, dir: Facing) -> bool {
        const KANTE_TOLERANZ: i32 = 8;
        let cur = match segments.get(self.monitor_idx) {
            Some(s) => *s,
            None => return false,
        };
        match dir {
            Facing::Right => {
                let next_idx = self.monitor_idx + 1;
                let Some(next) = segments.get(next_idx) else { return false };
                if (next.x_start - cur.x_end).abs() > KANTE_TOLERANZ {
                    return false;
                }
                if (next.y - cur.y).abs() > KANTE_TOLERANZ {
                    return false;
                }
                self.monitor_idx = next_idx;
                self.x = 0.0;
                self.facing = Facing::Right;
                true
            }
            Facing::Left => {
                if self.monitor_idx == 0 {
                    return false;
                }
                let prev_idx = self.monitor_idx - 1;
                let prev = segments[prev_idx];
                if (cur.x_start - prev.x_end).abs() > KANTE_TOLERANZ {
                    return false;
                }
                if (prev.y - cur.y).abs() > KANTE_TOLERANZ {
                    return false;
                }
                let (prev_w, _) = world.dims(prev_idx);
                self.monitor_idx = prev_idx;
                self.x = (prev_w - TILE_W as i32).max(0) as f32;
                self.facing = Facing::Left;
                true
            }
        }
    }
}

/// Lässt Schafe nicht durcheinander hindurchlaufen: Kommt ein laufendes
/// Schaf einem anderen auf derselben Fläche zu nahe, springt es darüber
/// oder dreht um. Nach jedem Tick aller Schafe aufrufen.
pub fn avoid_collisions(sheeps: &mut [Sheep], world: &World) {
    for i in 0..sheeps.len() {
        if !sheeps[i].walking_on_level() {
            continue;
        }
        let me = &sheeps[i];
        let nearest = sheeps
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != i)
            .filter_map(|(_, o)| Some((me.ahead_on_level(o)?, o.anim == AnimId::Jump)))
            .filter(|&(ahead, _)| ahead <= MEET_DISTANCE)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, other_jumping)) = nearest {
            sheeps[i].meet_sheep(other_jumping, world);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIMS: [(i32, i32); 1] = [(1920, 1080)];
    const SEGS: [GroundSegment; 1] = [GroundSegment { x_start: 0, x_end: 1920, y: 1080 }];

    fn surface(window: WindowId, x_start: i32, x_end: i32, y: i32) -> Surface {
        Surface { window, x_start, x_end, y, win_x: x_start, win_right: x_end }
    }

    /// Tickt, bis das Schaf nicht mehr fällt (max. 10 s Simulationszeit).
    /// Ein laufendes Surprised wird erst abgewartet.
    fn tick_until_landed(s: &mut Sheep, world: &World, dt: u32) {
        while s.anim == AnimId::Surprised {
            s.tick(dt, &SEGS, world);
        }
        for _ in 0..(10_000 / dt) {
            s.tick(dt, &SEGS, world);
            if !s.falling {
                return;
            }
        }
        panic!("Schaf ist nicht gelandet");
    }

    #[test]
    fn lands_on_window_below() {
        let surfaces = vec![vec![surface(1, 100, 900, 300)]];
        let world = World { area_dims: &DIMS, surfaces: &surfaces };
        let mut s = Sheep::new_at(0, 400.0, 42);
        tick_until_landed(&mut s, &world, 16);
        assert_eq!(s.support_window(), Some(1));
        assert_eq!(s.y + TILE_H as f32, 300.0);
    }

    #[test]
    fn fast_fall_does_not_tunnel_through_edge() {
        // 100-ms-Ticks: das Schaf legt am Ende >200 px pro Tick zurück.
        let surfaces = vec![vec![surface(1, 100, 900, 900)]];
        let world = World { area_dims: &DIMS, surfaces: &surfaces };
        let mut s = Sheep::new_at(0, 400.0, 7);
        tick_until_landed(&mut s, &world, 100);
        assert_eq!(s.support_window(), Some(1));
    }

    #[test]
    fn misses_window_when_center_is_outside() {
        let surfaces = vec![vec![surface(1, 100, 300, 500)]];
        let world = World { area_dims: &DIMS, surfaces: &surfaces };
        let mut s = Sheep::new_at(0, 600.0, 1);
        tick_until_landed(&mut s, &world, 16);
        assert_eq!(s.support_window(), None);
        assert_eq!(s.y + TILE_H as f32, 1080.0);
    }

    /// Landet das Schaf auf Fenster 1 bei x=400 und liefert es zurück.
    fn landed_on_window() -> Sheep {
        let surfaces = vec![vec![surface(1, 100, 900, 300)]];
        let world = World { area_dims: &DIMS, surfaces: &surfaces };
        let mut s = Sheep::new_at(0, 400.0, 3);
        tick_until_landed(&mut s, &world, 16);
        assert_eq!(s.support_window(), Some(1));
        s
    }

    #[test]
    fn window_slides_underneath_without_carrying_sheep() {
        let mut s = landed_on_window();
        let x0 = s.x;
        // Fenster um 50 px nach rechts, Schaf steht noch drauf. Landeanimationen
        // haben dx=0 – das Schaf bleibt also exakt stehen.
        let moved = vec![vec![surface(1, 150, 950, 300)]];
        s.tick(16, &SEGS, &World { area_dims: &DIMS, surfaces: &moved });
        assert_eq!(s.x, x0);
        assert_eq!(s.support_window(), Some(1));
    }

    #[test]
    fn window_pushed_up_lifts_sheep() {
        let mut s = landed_on_window();
        let up = vec![vec![surface(1, 100, 900, 250)]];
        s.tick(16, &SEGS, &World { area_dims: &DIMS, surfaces: &up });
        assert_eq!(s.y + TILE_H as f32, 250.0);
        assert_eq!(s.support_window(), Some(1));
    }

    #[test]
    fn window_pulled_away_surprise_then_fall_to_floor() {
        let mut s = landed_on_window();
        let y0 = s.y;
        // Fenster komplett nach rechts weggezogen.
        let away = vec![vec![surface(1, 700, 1500, 300)]];
        let world = World { area_dims: &DIMS, surfaces: &away };
        s.tick(16, &SEGS, &world);
        assert_eq!(s.anim, AnimId::Surprised);
        assert_eq!(s.support_window(), None);
        assert!(!s.falling);
        // Hängt erst eine Weile verdutzt in der Luft …
        for _ in 0..10 {
            s.tick(16, &SEGS, &world);
        }
        assert_eq!(s.y, y0);
        assert!(!s.falling);
        // … und fällt dann auf den Boden.
        tick_until_landed(&mut s, &world, 16);
        assert_eq!(s.support_window(), None);
        assert_eq!(s.y + TILE_H as f32, 1080.0);
    }

    #[test]
    fn window_dropped_down_sheep_falls_after_it() {
        let mut s = landed_on_window();
        let down = vec![vec![surface(1, 100, 900, 500)]];
        let world = World { area_dims: &DIMS, surfaces: &down };
        s.tick(16, &SEGS, &world);
        assert_eq!(s.anim, AnimId::Surprised);
        tick_until_landed(&mut s, &world, 16);
        assert_eq!(s.support_window(), Some(1));
        assert_eq!(s.y + TILE_H as f32, 500.0);
    }

    #[test]
    fn window_vanishes_sheep_falls() {
        let mut s = landed_on_window();
        let empty: Vec<Vec<Surface>> = vec![vec![]];
        let world = World { area_dims: &DIMS, surfaces: &empty };
        s.tick(16, &SEGS, &world);
        assert_eq!(s.anim, AnimId::Surprised);
        tick_until_landed(&mut s, &world, 16);
        assert_eq!(s.y + TILE_H as f32, 1080.0);
    }

    /// Schaf auf dem Boden an der linken Wand, kurz vorm Hochklettern.
    fn at_left_wall_climbing(seed: u64) -> Sheep {
        let none: Vec<Vec<Surface>> = vec![vec![]];
        let world = World { area_dims: &DIMS, surfaces: &none };
        let mut s = Sheep::new_at(0, 0.0, seed);
        tick_until_landed(&mut s, &world, 16);
        s.facing = Facing::Left;
        s.set_anim(AnimId::ClimbStart);
        s
    }

    #[test]
    fn climbs_wall_crosses_ceiling_and_comes_down() {
        let none: Vec<Vec<Surface>> = vec![vec![]];
        let world = World { area_dims: &DIMS, surfaces: &none };
        // Seeds durchprobieren, bis eins die ganze Decke schafft (ohne
        // vorher loszulassen) – deterministisch, da fester RNG.
        for seed in 1..200 {
            let mut s = at_left_wall_climbing(seed);
            let mut seen_top = false;
            let mut dropped = false;
            for _ in 0..(120_000 / 16) {
                s.tick(16, &SEGS, &world);
                match s.anim {
                    AnimId::ClimbUp => assert_eq!(s.x, 0.0),
                    AnimId::TopWalk => {
                        seen_top = true;
                        assert_eq!(s.y, 0.0);
                        assert_eq!(s.facing, Facing::Right);
                    }
                    AnimId::ClimbDown => assert_eq!(s.x, (1920 - TILE_W) as f32),
                    AnimId::FallPanic => dropped = true,
                    _ => {}
                }
                if dropped || s.anim == AnimId::ClimbOver {
                    break;
                }
            }
            assert!(seen_top);
            if !dropped {
                assert_eq!(s.anim, AnimId::ClimbOver);
                assert_eq!(s.y + TILE_H as f32, 1080.0);
                return;
            }
        }
        panic!("kein Schaf hat die Decke komplett überquert");
    }

    #[test]
    fn drops_from_ceiling_onto_window() {
        // Fenster unterhalb fast der ganzen Decke: wer loslässt, landet drauf.
        let surfaces = vec![vec![surface(1, 50, 1870, 600)]];
        let world = World { area_dims: &DIMS, surfaces: &surfaces };
        for seed in 1..200 {
            let mut s = at_left_wall_climbing(seed);
            for _ in 0..(120_000 / 16) {
                s.tick(16, &SEGS, &world);
                if s.anim == AnimId::FallPanic {
                    tick_until_landed(&mut s, &world, 16);
                    assert_eq!(s.support_window(), Some(1));
                    assert_eq!(s.y + TILE_H as f32, 600.0);
                    return;
                }
                if s.anim == AnimId::ClimbOver {
                    break;
                }
            }
        }
        panic!("kein Schaf hat an der Decke losgelassen");
    }

    fn on_floor(seed: u64) -> Sheep {
        let none: Vec<Vec<Surface>> = vec![vec![]];
        let world = World { area_dims: &DIMS, surfaces: &none };
        let mut s = Sheep::new_at(0, 800.0, seed);
        tick_until_landed(&mut s, &world, 16);
        s
    }

    #[test]
    fn jump_arcs_and_lands_in_sprint() {
        let none: Vec<Vec<Surface>> = vec![vec![]];
        let world = World { area_dims: &DIMS, surfaces: &none };
        let mut s = on_floor(11);
        s.facing = Facing::Right;
        s.x = 200.0;
        s.set_anim(AnimId::Jump);
        let floor = 1080.0 - TILE_H as f32;
        let mut peak = 0.0f32;
        while s.anim == AnimId::Jump {
            s.tick(16, &SEGS, &world);
            peak = peak.max(floor - s.y);
        }
        assert!(peak > JUMP_HEIGHT * 0.9, "Scheitel {peak}");
        assert_eq!(s.anim, AnimId::Run);
        assert_eq!(s.y, floor);
        assert!(s.x > 300.0);
    }

    /// Zwei Schafe am Boden, die aufeinander zulaufen.
    fn facing_each_other(seed: u64, anim: AnimId) -> [Sheep; 2] {
        let mut a = on_floor(seed);
        let mut b = on_floor(seed.wrapping_mul(31).wrapping_add(7));
        (a.x, a.facing) = (600.0, Facing::Right);
        (b.x, b.facing) = (700.0, Facing::Left);
        a.set_anim(anim);
        b.set_anim(anim);
        [a, b]
    }

    #[test]
    fn meeting_sheep_leap_or_turn() {
        let none: Vec<Vec<Surface>> = vec![vec![]];
        let world = World { area_dims: &DIMS, surfaces: &none };
        let (mut leaps, mut turns) = (0, 0);
        for seed in 0..40 {
            let mut pair = facing_each_other(seed, AnimId::Run);
            let mut reacted = [None, None];
            for _ in 0..60 {
                for s in pair.iter_mut() {
                    s.tick(16, &SEGS, &world);
                }
                avoid_collisions(&mut pair, &world);
                for (r, s) in reacted.iter_mut().zip(&pair) {
                    if r.is_none() && matches!(s.anim, AnimId::Jump | AnimId::Rotate1A) {
                        *r = Some(s.anim);
                    }
                }
            }
            // Beide reagieren, aber nie springen beide gleichzeitig.
            assert!(reacted.iter().all(Option::is_some), "seed {seed}: {reacted:?}");
            assert_ne!(reacted, [Some(AnimId::Jump); 2], "seed {seed}");
            leaps += reacted.iter().filter(|r| **r == Some(AnimId::Jump)).count();
            turns += reacted.iter().filter(|r| **r == Some(AnimId::Rotate1A)).count();
        }
        assert!(leaps > 0 && turns > 0, "Sprünge {leaps}, Umdrehen {turns}");
    }

    #[test]
    fn sheep_never_walk_through_each_other() {
        let none: Vec<Vec<Surface>> = vec![vec![]];
        let world = World { area_dims: &DIMS, surfaces: &none };
        let floor = 1080.0 - TILE_H as f32;
        // Am Boden laufen und nicht gerade springen/auftreten.
        let grounded = |s: &Sheep| {
            s.y == floor
                && !s.falling
                && !matches!(s.anim, AnimId::Jump | AnimId::VisitRun | AnimId::VisitWalk)
        };
        for seed in 0..30 {
            for anim in [AnimId::Walk, AnimId::Run] {
                let mut pair = facing_each_other(seed, anim);
                let mut prev = pair.each_ref().map(|s| (s.x, grounded(s)));
                for step in 0..(60_000 / 16) {
                    for s in pair.iter_mut() {
                        s.tick(16, &SEGS, &world);
                    }
                    avoid_collisions(&mut pair, &world);
                    let now = pair.each_ref().map(|s| (s.x, grounded(s)));
                    let all_grounded = prev.iter().chain(&now).all(|&(_, g)| g);
                    let order_flipped = (prev[0].0 < prev[1].0) != (now[0].0 < now[1].0);
                    assert!(
                        !(all_grounded && order_flipped),
                        "seed {seed} {anim:?} Schritt {step}: durcheinander hindurch ({:?} / {:?}) {prev:?} {now:?} y={} {}",
                        pair[0].anim,
                        pair[1].anim,
                        pair[0].y,
                        pair[1].y
                    );
                    prev = now;
                }
            }
        }
    }

    #[test]
    fn no_leap_without_room_to_land() {
        let none: Vec<Vec<Surface>> = vec![vec![]];
        let world = World { area_dims: &DIMS, surfaces: &none };
        for seed in 0..40 {
            // Kurz vor der rechten Wand: hinter dem anderen Schaf ist kein Platz.
            let mut pair = facing_each_other(seed, AnimId::Run);
            pair[0].x = 1920.0 - TILE_W as f32 - 100.0;
            pair[1].x = pair[0].x + 50.0;
            pair[1].set_anim(AnimId::Eat);
            avoid_collisions(&mut pair, &world);
            assert_eq!(pair[0].anim, AnimId::Rotate1A, "seed {seed}");
        }
    }

    #[test]
    fn bath_meteor_after_walking_off_starts_in_the_air() {
        let none: Vec<Vec<Surface>> = vec![vec![]];
        let world = World { area_dims: &DIMS, surfaces: &none };
        let floor = 1080.0 - TILE_H as f32;
        let mut meteors = 0;
        for seed in 0..60 {
            let mut s = on_floor(seed);
            (s.x, s.facing, s.leaving) = (1919.0, Facing::Right, true);
            s.set_anim(AnimId::Walk);
            s.tick(16, &SEGS, &world);
            if s.anim == AnimId::BathFly {
                meteors += 1;
                assert!(s.y < floor - 100.0, "seed {seed}: Meteor startet bei y={}", s.y);
            }
        }
        assert!(meteors > 0);
    }

    #[test]
    fn sleep3_wakes_on_mouse() {
        let none: Vec<Vec<Surface>> = vec![vec![]];
        let world = World { area_dims: &DIMS, surfaces: &none };
        let mut s = on_floor(12);
        s.set_anim(AnimId::Sleep3Yawn);
        for _ in 0..200 {
            s.tick(16, &SEGS, &world);
        }
        assert!(s.is_sleeping());
        s.wake_from_sleep();
        assert_eq!(s.anim, AnimId::Sleep3Wake);
    }

    #[test]
    fn bath_meteor_lands_in_tub_and_tub_disappears() {
        let none: Vec<Vec<Surface>> = vec![vec![]];
        let world = World { area_dims: &DIMS, surfaces: &none };
        let mut s = Sheep::new_at(0, 800.0, 21);
        s.set_entry(Entry::Bath);
        s.tick(16, &SEGS, &world);
        assert_eq!(s.anim, AnimId::BathFly);
        let tub = *s.prop().expect("Wanne fehlt");
        assert_eq!(tub.y, 1080.0 - TILE_H as f32);
        let mut steps = 0;
        while s.anim != AnimId::BathSoak {
            s.tick(16, &SEGS, &world);
            steps += 1;
            assert!(steps < 2000, "Meteor kommt nicht an");
        }
        assert_eq!((s.x, s.y), (tub.x, tub.y));
        assert!(matches!(s.prop().unwrap().frame, 147 | 148));
        while s.prop().is_some() {
            s.tick(16, &SEGS, &world);
            steps += 1;
            assert!(steps < 5000, "Wanne verschwindet nicht");
        }
        assert!(matches!(s.anim, AnimId::Walk | AnimId::Rotate1B));
    }

    #[test]
    fn black_sheep_visit_meets_in_the_middle_and_fades() {
        let none: Vec<Vec<Surface>> = vec![vec![]];
        let world = World { area_dims: &DIMS, surfaces: &none };
        for seed in [31, 32] {
            let mut s = Sheep::new_at(0, 800.0, seed);
            s.set_entry(Entry::Visit);
            s.tick(16, &SEGS, &world);
            let mut steps = 0;
            while s.anim != AnimId::VisitBlush {
                s.tick(16, &SEGS, &world);
                steps += 1;
                assert!(steps < 2000, "Treffpunkt nicht erreicht");
            }
            let black = *s.prop().expect("schwarzes Schaf fehlt");
            // Direkt nebeneinander, einander zugewandt.
            assert_eq!((black.x - s.x).abs(), TILE_W as f32);
            assert_eq!(black.x < s.x, s.facing == Facing::Left);
            assert_eq!(black.mirrored, s.facing == Facing::Right);
            let mut faded = false;
            while let Some(p) = s.prop() {
                faded |= p.alpha < 1.0;
                s.tick(16, &SEGS, &world);
                steps += 1;
                assert!(steps < 5000, "schwarzes Schaf verschwindet nicht");
            }
            assert!(faded);
        }
    }

    #[test]
    fn never_jumps_off_at_walls() {
        // Kante an beiden Enden verdeckt (Fenster reicht weiter): das Schaf
        // darf hier nur umdrehen/dagegenrennen, nie runterspringen.
        let surfaces = vec![vec![Surface {
            window: 1,
            x_start: 300,
            x_end: 500,
            y: 400,
            win_x: 100,
            win_right: 900,
        }]];
        let world = World { area_dims: &DIMS, surfaces: &surfaces };
        let mut s = Sheep::new_at(0, 380.0, 5);
        tick_until_landed(&mut s, &world, 16);
        for _ in 0..(300_000 / 16) {
            s.tick(16, &SEGS, &world);
            assert_eq!(s.support_window(), Some(1));
            assert!(!matches!(
                s.anim,
                AnimId::LookDown | AnimId::JumpDown | AnimId::FallWinA
            ));
        }
    }

    #[test]
    fn walks_off_window_eventually_and_reaches_floor() {
        // Schmale Kante: das Schaf muss früher oder später springen oder
        // abrutschen und unten auf dem Boden ankommen.
        let surfaces = vec![vec![surface(1, 300, 500, 400)]];
        let world = World { area_dims: &DIMS, surfaces: &surfaces };
        let mut s = Sheep::new_at(0, 380.0, 99);
        tick_until_landed(&mut s, &world, 16);
        assert_eq!(s.support_window(), Some(1));
        let mut reached_floor = false;
        for _ in 0..(600_000 / 16) {
            s.tick(16, &SEGS, &world);
            if !s.falling && s.support_window().is_none() && s.y + TILE_H as f32 == 1080.0 {
                reached_floor = true;
                break;
            }
        }
        assert!(reached_floor);
    }
}
