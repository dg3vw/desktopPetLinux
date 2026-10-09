//! Begehbare Fensteroberkanten (Plan Abschnitt 6.1).
//!
//! Das Backend liefert die sichtbaren Top-Level-Fenster in Stapelreihenfolge
//! (unten → oben) in Root-Koordinaten. Daraus wird pro Overlay/Monitor eine
//! Liste horizontaler Kanten in Overlay-lokalen Koordinaten berechnet, auf
//! denen ein Schaf stehen kann – nur dort, wo kein höher gestapeltes Fenster
//! die Kante überdeckt.

use super::anim::{TILE_H, TILE_W};
use super::geometry::Rect;

pub type WindowId = u64;

/// Sichtbares Rechteck eines fremden Fensters (inkl. WM-Rahmen, ohne
/// CSD-Schatten) in Root-Koordinaten.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowRect {
    pub id: WindowId,
    pub rect: Rect,
}

/// Ein begehbares Stück Fensteroberkante in Overlay-lokalen Koordinaten.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    pub window: WindowId,
    /// Begehbarer Bereich [x_start, x_end).
    pub x_start: i32,
    pub x_end: i32,
    /// Höhe der Kante – hier stehen die Füße des Schafs.
    pub y: i32,
    /// Linke Fensterkante (ungeclippt). Ändert sich nur, wenn das Fenster
    /// verschoben wird – daran erkennt das Schaf horizontale Mitnahme.
    pub win_x: i32,
}

/// Abstand zur Overlay-Unterkante, unterhalb dessen eine Fensterkante nicht
/// mehr als eigene Fläche zählt (der Boden übernimmt).
const FLOOR_MARGIN: i32 = 4;

/// Berechnet die begehbaren Fensterkanten pro Overlay.
///
/// * `windows` – Stapelreihenfolge unten → oben (wie `_NET_CLIENT_LIST_STACKING`)
/// * `overlays` – Root-Rechteck jeder Overlay-Zeichenfläche, in derselben
///   Reihenfolge wie die Monitore im Sheep (`monitor_idx`)
///
/// Ergebnis: `out[i]` gehört zu `overlays[i]`, sortiert nach `x_start`.
pub fn walkable_surfaces(windows: &[WindowRect], overlays: &[Rect]) -> Vec<Vec<Surface>> {
    let mut out: Vec<Vec<Surface>> = vec![Vec::new(); overlays.len()];

    for (i, w) in windows.iter().enumerate() {
        let y = w.rect.y;
        let mut spans = vec![(w.rect.x, w.rect.right())];

        // Höher gestapelte Fenster, die die Kantenzeile überdecken, abziehen.
        for upper in &windows[i + 1..] {
            if upper.rect.y <= y && upper.rect.bottom() > y {
                spans = subtract(&spans, (upper.rect.x, upper.rect.right()));
                if spans.is_empty() {
                    break;
                }
            }
        }

        for (oi, o) in overlays.iter().enumerate() {
            let local_y = y - o.y;
            if local_y < TILE_H as i32 || local_y >= o.height - FLOOR_MARGIN {
                continue;
            }
            for &(a, b) in &spans {
                let a = a.max(o.x);
                let b = b.min(o.right());
                if b - a < TILE_W as i32 {
                    continue;
                }
                out[oi].push(Surface {
                    window: w.id,
                    x_start: a - o.x,
                    x_end: b - o.x,
                    y: local_y,
                    win_x: w.rect.x - o.x,
                });
            }
        }
    }

    for list in &mut out {
        list.sort_by_key(|s| (s.x_start, s.y));
    }
    out
}

/// Zieht das halboffene Intervall `cut` von allen `spans` ab.
fn subtract(spans: &[(i32, i32)], cut: (i32, i32)) -> Vec<(i32, i32)> {
    let (c0, c1) = cut;
    let mut out = Vec::with_capacity(spans.len() + 1);
    for &(a, b) in spans {
        if c1 <= a || c0 >= b {
            out.push((a, b));
            continue;
        }
        if a < c0 {
            out.push((a, c0));
        }
        if c1 < b {
            out.push((c1, b));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(id: WindowId, x: i32, y: i32, w: i32, h: i32) -> WindowRect {
        WindowRect { id, rect: Rect { x, y, width: w, height: h } }
    }

    const MON: Rect = Rect { x: 0, y: 0, width: 1920, height: 1080 };

    #[test]
    fn single_window_gives_full_edge() {
        let s = walkable_surfaces(&[win(1, 100, 300, 800, 400)], &[MON]);
        assert_eq!(
            s[0],
            vec![Surface { window: 1, x_start: 100, x_end: 900, y: 300, win_x: 100 }]
        );
    }

    #[test]
    fn upper_window_splits_edge() {
        // Fenster 2 liegt darüber und überdeckt die Kante von 1 in der Mitte.
        let s = walkable_surfaces(
            &[win(1, 100, 300, 800, 400), win(2, 400, 200, 200, 300)],
            &[MON],
        );
        let edges1: Vec<_> = s[0].iter().filter(|e| e.window == 1).map(|e| (e.x_start, e.x_end)).collect();
        assert_eq!(edges1, vec![(100, 400), (600, 900)]);
        // Die Kante von Fenster 2 selbst bleibt komplett.
        assert!(s[0].iter().any(|e| e.window == 2 && e.x_start == 400 && e.x_end == 600));
    }

    #[test]
    fn lower_window_does_not_occlude() {
        // Fenster 2 ist unten im Stapel – überdeckt die Kante von 1 nicht.
        let s = walkable_surfaces(
            &[win(2, 400, 200, 200, 300), win(1, 100, 300, 800, 400)],
            &[MON],
        );
        let edges1: Vec<_> = s[0].iter().filter(|e| e.window == 1).collect();
        assert_eq!(edges1.len(), 1);
        assert_eq!((edges1[0].x_start, edges1[0].x_end), (100, 900));
        // 1 liegt zwar darüber, beginnt aber erst bei y=300 – die Kante von 2
        // (y=200) bleibt frei.
        assert!(s[0].iter().any(|e| e.window == 2 && e.x_start == 400 && e.x_end == 600));
    }

    #[test]
    fn window_entirely_above_edge_does_not_occlude() {
        // Fenster 2 endet oberhalb der Kante von 1 (bottom == y) → kein Schnitt.
        let s = walkable_surfaces(
            &[win(1, 100, 300, 800, 400), win(2, 400, 100, 200, 200)],
            &[MON],
        );
        assert!(s[0].iter().any(|e| e.window == 1 && e.x_start == 100 && e.x_end == 900));
    }

    #[test]
    fn fully_covered_edge_disappears() {
        let s = walkable_surfaces(
            &[win(1, 100, 300, 800, 400), win(2, 0, 0, 1920, 1080)],
            &[MON],
        );
        assert!(s[0].iter().all(|e| e.window != 1));
    }

    #[test]
    fn maximized_and_too_narrow_are_dropped() {
        let s = walkable_surfaces(
            &[win(1, 0, 10, 1920, 1000), win(2, 500, 500, 20, 100)],
            &[MON],
        );
        assert!(s[0].is_empty());
    }

    #[test]
    fn edge_at_floor_is_dropped() {
        let s = walkable_surfaces(&[win(1, 100, 1078, 400, 2)], &[MON]);
        assert!(s[0].is_empty());
    }

    #[test]
    fn edge_spanning_two_monitors_is_split_and_localized() {
        let right = Rect { x: 1920, y: 0, width: 1280, height: 1024 };
        let s = walkable_surfaces(&[win(7, 1500, 400, 1000, 300)], &[MON, right]);
        assert_eq!(
            s[0],
            vec![Surface { window: 7, x_start: 1500, x_end: 1920, y: 400, win_x: 1500 }]
        );
        assert_eq!(
            s[1],
            vec![Surface { window: 7, x_start: 0, x_end: 580, y: 400, win_x: -420 }]
        );
    }

    #[test]
    fn subtract_edges() {
        assert_eq!(subtract(&[(0, 10)], (3, 5)), vec![(0, 3), (5, 10)]);
        assert_eq!(subtract(&[(0, 10)], (-5, 3)), vec![(3, 10)]);
        assert_eq!(subtract(&[(0, 10)], (8, 20)), vec![(0, 8)]);
        assert_eq!(subtract(&[(0, 10)], (10, 20)), vec![(0, 10)]);
        assert!(subtract(&[(0, 10)], (0, 10)).is_empty());
    }
}
