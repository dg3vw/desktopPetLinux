//! Platzierung einer Sprechblase über (oder unter) einem Sprite.

/// Höhe der Blasenspitze zwischen Blase und Sprite (Pixel).
pub const TAIL: i32 = 7;
/// Eckenradius der Blase; die Spitze bleibt mindestens so weit vom Rand weg.
pub const RADIUS: i32 = 8;

/// Lage einer Blase in Overlay-lokalen Koordinaten.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BubblePlacement {
    /// Linke obere Ecke und Größe des Blasenkörpers (ohne Spitze).
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// x-Position der Spitze (Mitte), zeigt auf das Sprite.
    pub tail_x: i32,
    /// Blase sitzt unter dem Sprite (Spitze zeigt nach oben).
    pub below: bool,
}

impl BubblePlacement {
    /// Rechteck aus Körper und Spitze (x, y, Breite, Höhe) – für die
    /// Input-Region.
    pub fn outer_rect(&self) -> (i32, i32, i32, i32) {
        let y = if self.below { self.y - TAIL } else { self.y };
        (self.x, y, self.width, self.height + TAIL)
    }
}

/// Setzt eine Blase der Größe `size` mittig über das Sprite `sprite`
/// (x, y, Breite, Höhe) und hält sie innerhalb der Zeichenfläche `area`.
/// Ist über dem Sprite kein Platz (es läuft an der Decke), kommt sie darunter.
pub fn place(sprite: (i32, i32, i32, i32), size: (i32, i32), area: (i32, i32)) -> BubblePlacement {
    let (sx, sy, sw, sh) = sprite;
    let (width, height) = size;
    let (aw, _) = area;
    let center = sx + sw / 2;

    let x = (center - width / 2).clamp(0, (aw - width).max(0));
    let above = sy - TAIL - height;
    let below = above < 0;
    let y = if below { sy + sh + TAIL } else { above };
    let tail_lo = x + RADIUS + TAIL;
    let tail_hi = (x + width - RADIUS - TAIL).max(tail_lo);
    BubblePlacement {
        x,
        y,
        width,
        height,
        tail_x: center.clamp(tail_lo, tail_hi),
        below,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: (i32, i32) = (1920, 1080);

    #[test]
    fn centered_above_sprite() {
        let b = place((500, 1040, 40, 40), (100, 30), AREA);
        assert_eq!((b.x, b.y), (470, 1040 - TAIL - 30));
        assert_eq!(b.tail_x, 520);
        assert!(!b.below);
    }

    #[test]
    fn clamped_to_left_and_right_edge() {
        // Sprites ragen teilweise aus dem Bild (Rauslaufen): Körper und
        // Spitze bleiben drin, die Spitze hält Abstand zur Ecke.
        let left = place((-15, 1040, 40, 40), (100, 30), AREA);
        assert_eq!(left.x, 0);
        assert_eq!(left.tail_x, RADIUS + TAIL);

        let right = place((1895, 1040, 40, 40), (100, 30), AREA);
        assert_eq!(right.x, 1820);
        assert_eq!(right.tail_x, 1920 - RADIUS - TAIL);
    }

    #[test]
    fn below_sprite_at_the_ceiling() {
        let b = place((500, 0, 40, 40), (100, 30), AREA);
        assert!(b.below);
        assert_eq!(b.y, 40 + TAIL);
        assert_eq!(b.outer_rect(), (470, 40, 100, 30 + TAIL));
    }
}
