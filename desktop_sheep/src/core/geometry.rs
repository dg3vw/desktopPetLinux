#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn right(&self) -> i32 {
        self.x + self.width
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GroundSegment {
    pub x_start: i32,
    #[allow(dead_code)] // wird bei Multi-Monitor-Segmentgrenzen relevant
    pub x_end: i32,
    pub y: i32,
}

pub fn ground_segments(monitors: &[Rect]) -> Vec<GroundSegment> {
    let mut segments: Vec<GroundSegment> = monitors
        .iter()
        .map(|m| GroundSegment {
            x_start: m.x,
            x_end: m.right(),
            y: m.bottom(),
        })
        .collect();
    segments.sort_by_key(|s| s.x_start);
    segments
}

#[allow(dead_code)] // Hilfsfunktion für spätere Multi-Overlay-Varianten
pub fn virtual_bounds(monitors: &[Rect]) -> Rect {
    if monitors.is_empty() {
        return Rect { x: 0, y: 0, width: 0, height: 0 };
    }
    let min_x = monitors.iter().map(|m| m.x).min().unwrap();
    let min_y = monitors.iter().map(|m| m.y).min().unwrap();
    let max_x = monitors.iter().map(|m| m.right()).max().unwrap();
    let max_y = monitors.iter().map(|m| m.bottom()).max().unwrap();
    Rect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}
