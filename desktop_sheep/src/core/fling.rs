//! Wurfgeschwindigkeit beim Loslassen eines gezogenen Schafs.

use std::collections::VecDeque;

/// Nur die Mausbewegung so kurz vor dem Loslassen zählt (ms).
const WINDOW_MS: f64 = 100.0;
/// Stand die Maus vor dem Loslassen länger still, gibt es keinen Schwung (ms).
const STILL_MS: f64 = 50.0;
/// Höchstgeschwindigkeit eines Wurfs (Pixel pro Sekunde).
const MAX_SPEED: f32 = 2000.0;

/// Merkt sich die letzten Mauspositionen einer Drag-Geste.
#[derive(Debug, Default)]
pub struct FlingTracker {
    /// (Zeit in ms, x, y) – ältester zuerst.
    samples: VecDeque<(f64, f32, f32)>,
}

impl FlingTracker {
    pub fn push(&mut self, t_ms: f64, x: f32, y: f32) {
        self.samples.push_back((t_ms, x, y));
        while self
            .samples
            .front()
            .is_some_and(|&(t, _, _)| t_ms - t > WINDOW_MS)
        {
            self.samples.pop_front();
        }
    }

    /// Geschwindigkeit (Pixel pro Sekunde) beim Loslassen zur Zeit `now_ms`,
    /// auf `MAX_SPEED` begrenzt.
    pub fn velocity(&self, now_ms: f64) -> (f32, f32) {
        let (Some(&(t0, x0, y0)), Some(&(t1, x1, y1))) =
            (self.samples.front(), self.samples.back())
        else {
            return (0.0, 0.0);
        };
        let dt = t1 - t0;
        if now_ms - t1 > STILL_MS || dt < 10.0 {
            return (0.0, 0.0);
        }
        let s = 1000.0 / dt as f32;
        let (vx, vy) = ((x1 - x0) * s, (y1 - y0) * s);
        let speed = vx.hypot(vy);
        if speed > MAX_SPEED {
            let k = MAX_SPEED / speed;
            (vx * k, vy * k)
        } else {
            (vx, vy)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_recent_motion_only() {
        let mut f = FlingTracker::default();
        // Erst langsam nach links, dann zügig nach rechts oben.
        f.push(0.0, 500.0, 500.0);
        f.push(200.0, 400.0, 500.0);
        for i in 1..=5 {
            f.push(
                200.0 + 16.0 * i as f64,
                400.0 + 8.0 * i as f32,
                500.0 - 4.0 * i as f32,
            );
        }
        let (vx, vy) = f.velocity(285.0);
        assert!((vx - 500.0).abs() < 1.0, "vx {vx}");
        assert!((vy + 250.0).abs() < 1.0, "vy {vy}");
    }

    #[test]
    fn no_fling_after_holding_still() {
        let mut f = FlingTracker::default();
        f.push(0.0, 0.0, 0.0);
        f.push(30.0, 60.0, 0.0);
        assert_eq!(f.velocity(200.0), (0.0, 0.0));
        assert_eq!(FlingTracker::default().velocity(0.0), (0.0, 0.0));
    }

    #[test]
    fn speed_is_capped() {
        let mut f = FlingTracker::default();
        f.push(0.0, 0.0, 0.0);
        f.push(20.0, 1000.0, 0.0);
        let (vx, vy) = f.velocity(20.0);
        assert_eq!((vx, vy), (MAX_SPEED, 0.0));
    }
}
