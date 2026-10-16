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
