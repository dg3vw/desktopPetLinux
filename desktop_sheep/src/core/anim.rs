//! Animations-Tabelle (aus Pets/esheep64/animations.xml abgeleitet, vereinfacht).

pub const TILES_X: u32 = 16;
#[allow(dead_code)]
pub const TILES_Y: u32 = 11;
pub const TILE_W: u32 = 40;
pub const TILE_H: u32 = 40;

pub fn frame_to_tile(index: u32) -> (u32, u32) {
    (index % TILES_X, index / TILES_X)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimId {
    Walk,
    /// Sanftes Umdrehen, Teil 1 (id=2): Kopf/Körper dreht sich.
    /// `flip_on_end` schaltet am Ende die Facing-Richtung um.
    Rotate1A,
    /// Sanftes Umdrehen, Teil 2 (id=3): Rückweg in die neue Richtung,
    /// danach zurück zu Walk.
    Rotate1B,
    YawnSleep,
    Sleep,
    WakeUp,
    Eat,
    /// Zappeln während der Benutzer das Sprite hält (id=4).
    Drag,
    /// Frei fallen nach dem Loslassen (id=5).
    Fall,
    /// Kurzer "Aua"-Aufschlag am Boden (id=10) → geht in Walk über.
    FallHard,
}

pub struct AnimDef {
    pub frames: &'static [u32],
    pub interval_ms: u32,
    /// Bewegung pro Tick (>0 nur im Walk-Zustand). Richtung ergibt sich aus Facing.
    pub dx_per_tick: i32,
    /// Wie oft die Sequenz komplett läuft, bevor ein neuer Zustand gewählt wird.
    pub repeat: u32,
    /// Gewichtete Nachfolge­zustände (Summe der Gewichte = 100).
    pub next: &'static [(u32, AnimId)],
    /// Kehrt beim Zustandswechsel die Facing-Richtung um. Für Rotate1A: die
    /// visuelle Drehung endet mit gespiegeltem Sprite in der neuen Richtung.
    pub flip_on_end: bool,
}

/// Walk (animations.xml id=1): 2 Frames links-gerichtet, 200 ms, dx=-2.
/// Übergänge: mehrheitlich weiterlaufen, gelegentlich schlafen oder fressen.
pub const WALK: AnimDef = AnimDef {
    frames: &[2, 3],
    interval_ms: 200,
    dx_per_tick: 2,
    repeat: 6,
    next: &[(85, AnimId::Walk), (10, AnimId::YawnSleep), (5, AnimId::Eat)],
    flip_on_end: false,
};

/// Rotate1A (id=2): Drehung Teil 1 – Frames 3 → 9 → 10, dann Facing flippen.
pub const ROTATE1A: AnimDef = AnimDef {
    frames: &[3, 9, 10],
    interval_ms: 180,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Rotate1B)],
    flip_on_end: true,
};

/// Rotate1B (id=3): Drehung Teil 2 – Frames 10 → 9 → 3, dann in Walk.
pub const ROTATE1B: AnimDef = AnimDef {
    frames: &[10, 9, 3],
    interval_ms: 180,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: false,
};

/// Sleep1a (id=15) ohne Loop-Teil: gähnen, hinlegen, augen zu. Danach → Sleep.
pub const YAWN_SLEEP: AnimDef = AnimDef {
    frames: &[3, 107, 108, 107, 108, 107, 31, 32, 33],
    interval_ms: 350,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Sleep)],
    flip_on_end: false,
};

/// Schlaf-Loop: ruhig atmen (Frames 0/1). Läuft im Regelfall so lange, bis
/// die Maus bewegt wird (siehe Sheep::wake_from_sleep im Backend). Die
/// `repeat`-Zahl dient nur als Notaus, damit das Schaf auch ohne
/// Cursor-Aktivität irgendwann von selbst aufwacht (~3,5 Min).
pub const SLEEP: AnimDef = AnimDef {
    frames: &[0, 1],
    interval_ms: 700,
    dx_per_tick: 0,
    repeat: 300,
    next: &[(100, AnimId::WakeUp)],
    flip_on_end: false,
};

/// Sleep1b (id=16): aufwachen, hochstehen.
pub const WAKE_UP: AnimDef = AnimDef {
    frames: &[0, 80, 79, 78, 77, 37, 38, 39, 38, 37, 6],
    interval_ms: 200,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: false,
};

/// Eat (id=26): grasen.
pub const EAT: AnimDef = AnimDef {
    frames: &[6, 6, 58, 59, 59, 60, 61, 60, 61, 6],
    interval_ms: 300,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: false,
};

/// Drag (id=4): Zappeln, während der User das Schaf hält. Loop, keine
/// Zustands­übergänge – wird nur beendet, indem der User loslässt.
pub const DRAG: AnimDef = AnimDef {
    frames: &[42, 43, 43, 42, 44, 44],
    interval_ms: 100,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Drag)],
    flip_on_end: false,
};

/// Fall (id=5): Einzel-Frame während des freien Falls.
pub const FALL: AnimDef = AnimDef {
    frames: &[133],
    interval_ms: 80,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: false,
};

/// FallHard (id=10): kurzer Landungs-Bumper, dann zurück zu Walk.
pub const FALL_HARD: AnimDef = AnimDef {
    frames: &[48, 48, 48, 48, 47],
    interval_ms: 120,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: false,
};

pub fn def(id: AnimId) -> &'static AnimDef {
    match id {
        AnimId::Walk => &WALK,
        AnimId::Rotate1A => &ROTATE1A,
        AnimId::Rotate1B => &ROTATE1B,
        AnimId::YawnSleep => &YAWN_SLEEP,
        AnimId::Sleep => &SLEEP,
        AnimId::WakeUp => &WAKE_UP,
        AnimId::Eat => &EAT,
        AnimId::Drag => &DRAG,
        AnimId::Fall => &FALL,
        AnimId::FallHard => &FALL_HARD,
    }
}

/// Einfacher xorshift64* – deterministisch aus Seed, kein Crate nötig.
pub struct Rng(pub u64);

impl Rng {
    pub fn from_seed(seed: u64) -> Self {
        Self(seed.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    pub fn range(&mut self, hi: u32) -> u32 {
        if hi == 0 {
            0
        } else {
            (self.next_u64() % hi as u64) as u32
        }
    }
}

/// Wählt aus einer gewichteten Tabelle einen Nachfolger.
pub fn pick_next(table: &[(u32, AnimId)], rng: &mut Rng) -> AnimId {
    let total: u32 = table.iter().map(|(w, _)| *w).sum();
    if total == 0 {
        return table[0].1;
    }
    let r = rng.range(total);
    let mut acc = 0u32;
    for (w, id) in table {
        acc += *w;
        if r < acc {
            return *id;
        }
    }
    table[table.len() - 1].1
}
