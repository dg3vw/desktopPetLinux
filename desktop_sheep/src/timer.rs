//! Eieruhr: Zustand eines Countdowns, unabhängig von GTK und testbar.

use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct Timer {
    run: Option<Run>,
}

#[derive(Debug, Clone, Copy)]
struct Run {
    deadline: Instant,
    total: Duration,
}

impl Timer {
    /// Startet (bzw. ersetzt) den Countdown über `total`.
    pub fn start(&mut self, total: Duration, now: Instant) {
        self.run = Some(Run {
            deadline: now + total,
            total,
        });
    }

    pub fn cancel(&mut self) {
        self.run = None;
    }

    pub fn is_running(&self) -> bool {
        self.run.is_some()
    }

    /// Ablaufzeitpunkt eines laufenden Countdowns.
    pub fn deadline(&self) -> Option<Instant> {
        self.run.map(|r| r.deadline)
    }

    /// Restzeit eines laufenden Countdowns.
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        self.run.map(|r| r.deadline.saturating_duration_since(now))
    }

    /// Liefert genau einmal die Gesamtdauer, wenn der Countdown abgelaufen
    /// ist; danach läuft kein Timer mehr.
    pub fn poll(&mut self, now: Instant) -> Option<Duration> {
        let run = self.run?;
        if now < run.deadline {
            return None;
        }
        self.run = None;
        Some(run.total)
    }
}

/// Restzeit als „m:ss“, aufgerundet auf volle Sekunden – die Anzeige
/// springt also erst beim Ablauf auf 0:00.
pub fn fmt_remaining(d: Duration) -> String {
    let secs = d.as_millis().div_ceil(1000) as u64;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Dauer für Menü und Blase: „10 min“, unter einer Minute „30 s“.
pub fn fmt_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{secs} s")
    } else {
        format!("{} min", secs.div_ceil(60))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    #[test]
    fn counts_down_and_fires_once() {
        let t0 = Instant::now();
        let mut t = Timer::default();
        assert!(!t.is_running());
        t.start(5 * MIN, t0);
        assert!(t.is_running());
        assert_eq!(t.remaining(t0 + MIN), Some(4 * MIN));
        assert_eq!(t.poll(t0 + MIN), None);
        assert_eq!(t.poll(t0 + 5 * MIN), Some(5 * MIN));
        assert!(!t.is_running());
        assert_eq!(t.poll(t0 + 6 * MIN), None);
        assert_eq!(t.remaining(t0 + 6 * MIN), None);
    }

    #[test]
    fn cancel_stops_without_firing() {
        let t0 = Instant::now();
        let mut t = Timer::default();
        t.start(MIN, t0);
        t.cancel();
        assert_eq!(t.poll(t0 + 2 * MIN), None);
    }

    #[test]
    fn restart_replaces_running_timer() {
        let t0 = Instant::now();
        let mut t = Timer::default();
        t.start(MIN, t0);
        t.start(10 * MIN, t0);
        assert_eq!(t.poll(t0 + 2 * MIN), None);
        assert_eq!(t.poll(t0 + 10 * MIN), Some(10 * MIN));
    }

    #[test]
    fn formatting() {
        assert_eq!(fmt_remaining(Duration::from_secs(252)), "4:12");
        assert_eq!(fmt_remaining(Duration::from_millis(251_200)), "4:12");
        assert_eq!(fmt_remaining(Duration::ZERO), "0:00");
        assert_eq!(fmt_remaining(60 * MIN), "60:00");
        assert_eq!(fmt_duration(10 * MIN), "10 min");
        assert_eq!(fmt_duration(Duration::from_secs(30)), "30 s");
    }
}
