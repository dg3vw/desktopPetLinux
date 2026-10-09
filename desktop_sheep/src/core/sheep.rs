use super::anim::{def, pick_next, AnimId, Rng, TILE_H, TILE_W};
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
/// Absprung beim jump_down: horizontal / nach oben (Pixel pro Sekunde).
const JUMP_VX: f32 = 150.0;
const JUMP_VY: f32 = 300.0;
/// Sackt das Trägerfenster um mehr als so viele Pixel ab, verliert das
/// Schaf den Halt (kleinere Sprünge werden einfach nachgeführt).
const DETACH_PX: f32 = 3.0;

pub struct Sheep {
    /// Position in Overlay-lokalen Koordinaten des aktuellen Monitors
    /// (Anker: linke obere Ecke des Sprites).
    pub x: f32,
    pub y: f32,
    pub facing: Facing,
    /// Index in die von der Platform gelieferte Segment-/Overlay-Liste.
    /// Wechselt beim Übergang über eine angrenzende Monitorkante.
    pub monitor_idx: usize,
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
            dragged: false,
            falling: false,
            vx: 0.0,
            vy: 0.0,
            support: None,
            hang_offset: 0.0,
            land_anim: None,
            leaving: false,
            anim: AnimId::Walk,
            frame_idx: 0,
            frame_elapsed_ms: 0,
            repeats_done: 0,
            rng: Rng::from_seed(seed),
        };
        s.drop_in(monitor_idx, x);
        s
    }

    /// Aktuell zu zeichnender Frame-Index im Sprite-Sheet.
    pub fn current_frame(&self) -> u32 {
        def(self.anim).frames[self.frame_idx]
    }

    /// True, solange das Schaf in irgendeinem Schlaf-Loop steckt
    /// (klassisches Liegen oder Sitzen/Dösen).
    #[allow(dead_code)] // API für spätere UI/Debug-Anzeige
    pub fn is_sleeping(&self) -> bool {
        matches!(self.anim, AnimId::Sleep | AnimId::Sleep2)
    }

    /// Fenster, auf dem das Schaf gerade steht (`None` = Boden oder in der Luft).
    #[allow(dead_code)] // für Tests und Debug-Ausgabe
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
    /// oder läuft aus dem Bild. Auf einem Fenster: das Schaf wandert mit dem
    /// Fenster mit, fällt, wenn es verschwindet, und entscheidet an dessen
    /// Kante zwischen Umdrehen, Runterspringen und Abrutschen.
    pub fn tick(&mut self, dt_ms: u32, segments: &[GroundSegment], world: &World) {
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
        if self.anim == AnimId::Surprised {
            // Hängt verdutzt in der Luft – weder Bewegung noch Boden-Snap.
            return;
        }

        // 2) Halt auf dem Fenster prüfen. Das Fenster nimmt das Schaf nicht
        //    mit: Zieht man es weg oder lässt es absacken, steht das Schaf
        //    plötzlich in der Luft, schaut verdutzt und fällt dann. Nur wenn
        //    das Fenster nach oben geschoben wird, wird es mit angehoben.
        let mut seg = None;
        if let Some(window) = self.support {
            let feet = self.y + TILE_H as f32 - self.hang_offset;
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
            let dx = cur.dx_per_tick as f32 * self.forward() * (dt_ms as f32 / 16.0);
            self.x += dx;
            match seg {
                Some(s) => self.check_window_edge(&s),
                None => {
                    self.check_floor_edge(segments, world);
                    if self.falling {
                        // Aus dem Bild gelaufen und oben neu eingestiegen.
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
    fn check_floor_edge(&mut self, segments: &[GroundSegment], world: &World) {
        let (aw, _) = world.dims(self.monitor_idx);
        if self.leaving {
            if self.x <= -(TILE_W as f32) || self.x >= aw as f32 {
                self.respawn_from_sky(world);
            }
            return;
        }
        let min_x = 0.0f32;
        let max_x = (aw - TILE_W as i32).max(0) as f32;
        let dir = if self.x <= min_x {
            Facing::Left
        } else if self.x >= max_x {
            Facing::Right
        } else {
            return;
        };
        if self.try_cross(segments, world, dir) {
            return;
        }
        if self.rng.range(100) < LEAVE_PERCENT {
            self.leaving = true;
            return;
        }
        self.x = if dir == Facing::Left { min_x } else { max_x };
        self.begin_turn();
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

    /// Neu von oben ins Bild fallen – zufälliger Monitor, zufällige Stelle.
    fn respawn_from_sky(&mut self, world: &World) {
        let n = world.area_dims.len().max(1) as u32;
        let idx = self.rng.range(n) as usize;
        let (aw, _) = world.dims(idx);
        let x = self.rng.range((aw - TILE_W as i32).max(1) as u32) as f32;
        self.facing = if self.rng.range(2) == 0 { Facing::Left } else { Facing::Right };
        self.drop_in(idx, x);
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
            if crossed && under && hit.map_or(true, |(hy, _)| sy < hy) {
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
        // FallHard oder das elastischere Boing, damit Landungen
        // abwechslungsreich aussehen.
        self.falling = false;
        self.vx = 0.0;
        self.vy = 0.0;
        self.support = seg.map(|s| s.window);
        let next = match self.land_anim.take() {
            Some(a) => a,
            None if self.rng.range(2) == 0 => AnimId::FallHard,
            None => AnimId::Boing,
        };
        self.set_anim(next);
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
