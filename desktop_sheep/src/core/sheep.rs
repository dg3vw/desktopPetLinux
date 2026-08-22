use std::time::{SystemTime, UNIX_EPOCH};

use super::anim::{def, pick_next, AnimId, Rng, TILE_H, TILE_W};
use super::geometry::GroundSegment;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Facing {
    Left,
    Right,
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
    /// True während der Benutzer das Sprite zieht: zappelt, aber bewegt
    /// sich nicht von selbst und wechselt den Zustand nicht.
    pub dragged: bool,
    /// True während des freien Falls nach dem Loslassen (bis inkl. bounce).
    falling: bool,
    /// Vertikale Geschwindigkeit für die Fall-Physik (Pixel pro Sekunde).
    vy: f32,
    anim: AnimId,
    frame_idx: usize,
    frame_elapsed_ms: u32,
    repeats_done: u32,
    rng: Rng,
}

impl Sheep {
    /// Erzeugt ein Schaf an einer bestimmten Startposition mit explizitem
    /// RNG-Seed – so lassen sich mehrere Instanzen zuverlässig
    /// unterschiedlich seeden.
    pub fn new_at(
        segments: &[GroundSegment],
        monitor_idx: usize,
        x: f32,
        seed: u64,
    ) -> Self {
        let seg = segments
            .get(monitor_idx)
            .copied()
            .unwrap_or(segments[0]);
        Self {
            x,
            y: (seg.y as u32).saturating_sub(TILE_H) as f32,
            facing: Facing::Right,
            monitor_idx,
            dragged: false,
            falling: false,
            vy: 0.0,
            anim: AnimId::Walk,
            frame_idx: 0,
            frame_elapsed_ms: 0,
            repeats_done: 0,
            rng: Rng::from_seed(seed),
        }
    }

    /// Bequemer Default: erste Position auf dem ersten Monitor, Seed aus
    /// Systemzeit. Für Einzel-Schaf-Läufe.
    #[allow(dead_code)]
    pub fn new(segments: &[GroundSegment]) -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xDEAD_BEEF_C0FFEE);
        Self::new_at(segments, 0, 20.0, seed)
    }

    /// Aktuell zu zeichnender Frame-Index im Sprite-Sheet.
    pub fn current_frame(&self) -> u32 {
        def(self.anim).frames[self.frame_idx]
    }

    /// True, solange das Schaf im Schlaf-Loop steckt.
    #[allow(dead_code)] // API für spätere UI/Debug-Anzeige
    pub fn is_sleeping(&self) -> bool {
        self.anim == AnimId::Sleep
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
        if self.anim == AnimId::Sleep {
            self.anim = AnimId::WakeUp;
            self.frame_idx = 0;
            self.frame_elapsed_ms = 0;
            self.repeats_done = 0;
        }
    }

    /// User hat das Sheep gepackt: Zappel-Animation starten, Bewegung
    /// einfrieren, ggf. Fall abbrechen.
    pub fn start_drag(&mut self) {
        self.dragged = true;
        self.falling = false;
        self.vy = 0.0;
        self.anim = AnimId::Drag;
        self.frame_idx = 0;
        self.frame_elapsed_ms = 0;
        self.repeats_done = 0;
    }

    /// User hat losgelassen: fällt mit Gravitation, bounct einmal, landet.
    pub fn end_drag(&mut self) {
        self.dragged = false;
        self.falling = true;
        self.vy = 0.0;
        self.anim = AnimId::Fall;
        self.frame_idx = 0;
        self.frame_elapsed_ms = 0;
        self.repeats_done = 0;
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
    /// * `area_dims` – Live-Maße jeder DrawingArea in derselben Reihenfolge
    ///
    /// Beim Erreichen einer Segmentkante wird geprüft, ob das Nachbar­segment
    /// nahtlos anschließt (X-Kanten in Kontakt, gleiche Bodenhöhe). Wenn ja,
    /// wechselt das Schaf den Monitor, sonst dreht es um.
    pub fn tick(
        &mut self,
        dt_ms: u32,
        segments: &[GroundSegment],
        area_dims: &[(i32, i32)],
    ) {
        // Während Drag: nur Zappel-Frames animieren, sonst gar nichts.
        if self.dragged {
            self.advance_frames_loop(dt_ms);
            return;
        }

        // Während Fall: Gravitation, Position, Bounce – dann ggf. FallHard.
        if self.falling {
            self.tick_falling(dt_ms, area_dims);
            if self.falling {
                self.advance_frames_loop(dt_ms);
                return;
            }
            // Sonst gerade gelandet – Rest des Ticks (FallHard-Sequenz) läuft
            // durch die reguläre Zustandsmaschine unten weiter.
        }

        // 1) Frame + Zustands-Übergang
        let cur = def(self.anim);
        self.frame_elapsed_ms += dt_ms;
        while self.frame_elapsed_ms >= cur.interval_ms {
            self.frame_elapsed_ms -= cur.interval_ms;
            self.frame_idx += 1;
            if self.frame_idx >= cur.frames.len() {
                self.frame_idx = 0;
                self.repeats_done += 1;
                if self.repeats_done >= cur.repeat {
                    self.repeats_done = 0;
                    let nxt = pick_next(cur.next, &mut self.rng);
                    // Facing beim Ende der aktuellen Animation umkehren
                    // (z. B. Rotate1A -> Rotate1B mit gedrehtem Sprite).
                    if cur.flip_on_end {
                        self.facing = match self.facing {
                            Facing::Left => Facing::Right,
                            Facing::Right => Facing::Left,
                        };
                    }
                    if nxt != self.anim {
                        self.anim = nxt;
                        self.frame_idx = 0;
                        self.frame_elapsed_ms = 0;
                        break;
                    }
                }
            }
        }

        // Aktuelle Overlay-Maße für den Monitor, auf dem wir stehen.
        let (aw, ah) = area_dims
            .get(self.monitor_idx)
            .copied()
            .unwrap_or((0, 0));

        // 2) Bewegung (nur wenn dx_per_tick > 0, also aktuell nur Walk)
        let cur = def(self.anim);
        if cur.dx_per_tick > 0 {
            let sign = if self.facing == Facing::Right { 1.0 } else { -1.0 };
            let dx = cur.dx_per_tick as f32 * sign * (dt_ms as f32 / 16.0);
            self.x += dx;

            let min_x = 0.0f32;
            let max_x = (aw - TILE_W as i32).max(0) as f32;
            if self.x <= min_x {
                if !self.try_cross(segments, area_dims, Facing::Left) {
                    self.x = min_x;
                    self.begin_turn();
                }
            } else if self.x >= max_x {
                if !self.try_cross(segments, area_dims, Facing::Right) {
                    self.x = max_x;
                    self.begin_turn();
                }
            }
        }

        // 3) Boden = Unterkante der Zeichenfläche des aktuellen Monitors,
        //    Fallback auf Segment-y falls Overlay-Maße noch unbekannt.
        let bottom = if ah > 0 {
            ah
        } else {
            segments
                .get(self.monitor_idx)
                .map(|s| s.y)
                .unwrap_or(0)
        };
        self.y = (bottom as u32).saturating_sub(TILE_H) as f32;
    }

    /// Startet die sanfte Umdreh-Sequenz (Rotate1A -> Rotate1B -> Walk).
    /// Das eigentliche Facing-Flippen erledigt der Zustandsübergang am Ende
    /// von Rotate1A (dank flip_on_end=true).
    fn begin_turn(&mut self) {
        self.anim = AnimId::Rotate1A;
        self.frame_idx = 0;
        self.frame_elapsed_ms = 0;
        self.repeats_done = 0;
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

    /// Physik während des Falls: Gravitation → Position → Bounce/Landung.
    /// Setzt self.falling=false und wechselt auf FallHard, sobald gelandet.
    fn tick_falling(&mut self, dt_ms: u32, area_dims: &[(i32, i32)]) {
        const GRAVITY: f32 = 2400.0; // Pixel pro Sekunde²
        const BOUNCE_DAMP: f32 = 0.42;
        const BOUNCE_STOP: f32 = 80.0; // Pixel pro Sekunde – kleiner ⇒ landen

        let (_aw, ah) = area_dims.get(self.monitor_idx).copied().unwrap_or((0, 0));
        let floor = (ah - TILE_H as i32).max(0) as f32;

        let dt_s = dt_ms as f32 / 1000.0;
        self.vy += GRAVITY * dt_s;
        self.y += self.vy * dt_s;

        if self.y >= floor {
            self.y = floor;
            if self.vy > BOUNCE_STOP {
                // bouncen – Richtung umkehren, Energie verlieren
                self.vy = -self.vy * BOUNCE_DAMP;
            } else {
                // Endgültig gelandet – FallHard startet
                self.vy = 0.0;
                self.falling = false;
                self.anim = AnimId::FallHard;
                self.frame_idx = 0;
                self.frame_elapsed_ms = 0;
                self.repeats_done = 0;
            }
        }
    }

    /// Versucht, in Bewegungsrichtung auf den Nachbarmonitor zu wechseln.
    /// Voraussetzung: das Nachbarsegment schließt in X nahtlos an und hat
    /// (annähernd) dieselbe Bodenhöhe. Rückgabe: true wenn gewechselt.
    fn try_cross(
        &mut self,
        segments: &[GroundSegment],
        area_dims: &[(i32, i32)],
        dir: Facing,
    ) -> bool {
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
                let (prev_w, _) = area_dims.get(prev_idx).copied().unwrap_or((0, 0));
                self.monitor_idx = prev_idx;
                self.x = (prev_w - TILE_W as i32).max(0) as f32;
                self.facing = Facing::Left;
                true
            }
        }
    }
}
