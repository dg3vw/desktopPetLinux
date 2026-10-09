//! Animations-Tabelle (aus Pets/esheep64/animations.xml abgeleitet, vereinfacht).

pub const TILES_X: u32 = 16;
#[allow(dead_code)]
pub const TILES_Y: u32 = 11;
pub const TILE_W: u32 = 40;
pub const TILE_H: u32 = 40;

pub fn frame_to_tile(index: u32) -> (u32, u32) {
    (index % TILES_X, index / TILES_X)
}

/// Farbvariante des Sprite-Sheets. `Default` = das originale esheep64
/// (`assets/sheep.png`); die übrigen sind aus den Pets-Ordnern des
/// Upstream-Repos extrahiert und liegen als `sheep_<farbe>.png` in `assets/`.
/// Alle Varianten teilen die Frame-Nummerierung der ersten 11 Zeilen mit dem
/// Original, weshalb dieselbe Animations-Tabelle funktioniert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpriteColor {
    Default,
    Blue,
    Green,
    Orange,
    Pink,
    Purple,
    Red,
    Yellow,
}

impl SpriteColor {
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "default" | "original" | "white" => Self::Default,
            "blue" => Self::Blue,
            "green" => Self::Green,
            "orange" => Self::Orange,
            "pink" => Self::Pink,
            "purple" => Self::Purple,
            "red" => Self::Red,
            "yellow" => Self::Yellow,
            _ => return None,
        })
    }

    pub fn png_bytes(self) -> &'static [u8] {
        match self {
            Self::Default => include_bytes!("../../assets/sheep.png"),
            Self::Blue => include_bytes!("../../assets/sheep_blue.png"),
            Self::Green => include_bytes!("../../assets/sheep_green.png"),
            Self::Orange => include_bytes!("../../assets/sheep_orange.png"),
            Self::Pink => include_bytes!("../../assets/sheep_pink.png"),
            Self::Purple => include_bytes!("../../assets/sheep_purple.png"),
            Self::Red => include_bytes!("../../assets/sheep_red.png"),
            Self::Yellow => include_bytes!("../../assets/sheep_yellow.png"),
        }
    }
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
    /// Alternative Schlafvariante (id=17/18): sitzen/dösen statt liegen.
    Sleep2Yawn,
    Sleep2,
    Sleep2Wake,
    Eat,
    /// Anlauf-Frames vor dem Sprinten (id=35).
    RunBegin,
    /// Sprinten (id=7): schnellere Frames + höheres dx.
    Run,
    /// Abbrems-Frames nach dem Sprinten (id=36).
    RunEnd,
    /// Zappeln während der Benutzer das Sprite hält (id=4).
    Drag,
    /// Frei fallen nach dem Loslassen (id=5).
    Fall,
    /// Kurzer "Aua"-Aufschlag am Boden (id=10) → geht in Walk über.
    FallHard,
    /// Ausgedehnter, weicherer Landungs-Bounce (id=8) – Alternative zu FallHard.
    Boing,
    /// Frei komponiert: Schaf knallt gegen die Wand statt sanft umzudrehen.
    /// Nutzt die "Aua"-Sprites aus Zeile 4 des Sheets (Frames 48/49). Ersetzt
    /// bei Bedarf den Rotate1A/B-Ablauf an einer Segmentkante.
    WallRam,
    /// look_down (id=43): an der Fensterkante runterschauen und überlegen.
    LookDown,
    /// jump_down (id=44): Absprung vorbereiten.
    JumpDown,
    /// jump_down2 (id=45): in der Luft – Physik übernimmt bis zur Landung.
    JumpDownAir,
    /// jump_down3 (id=46): Landung nach dem Sprung.
    JumpDownLand,
    /// fall_wina (id=51): über die Kante rutschen, umdrehen.
    FallWinA,
    /// fall_winb (id=52): an der Kante hängen und strampeln.
    FallWinB,
    /// fall_winc (id=53): loslassen und fallen.
    FallWinC,
    /// fall_wind (id=54): benommen aufrappeln.
    FallWinD,
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
    next: &[
        (67, AnimId::Walk),
        (7, AnimId::YawnSleep),
        (5, AnimId::Sleep2Yawn),
        (6, AnimId::Eat),
        (10, AnimId::RunBegin),
        (5, AnimId::Rotate1A),
    ],
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

/// Sleep2 – Sitz-Doze-Variante. Teil 1 (id=17 Vorlauf): setzen und Kopf senken.
pub const SLEEP2_YAWN: AnimDef = AnimDef {
    frames: &[3, 6, 7, 8],
    interval_ms: 300,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Sleep2)],
    flip_on_end: false,
};

/// Sleep2-Loop (id=17 Loop-Teil): kleines Nick-Muster in Sitzposition.
/// Wie SLEEP dient `repeat` nur als Notaus; regulär endet der Zustand über
/// Cursor-Aktivität (siehe Sheep::wake_from_sleep).
pub const SLEEP2: AnimDef = AnimDef {
    frames: &[8, 7, 8, 8],
    interval_ms: 700,
    dx_per_tick: 0,
    repeat: 200,
    next: &[(100, AnimId::Sleep2Wake)],
    flip_on_end: false,
};

/// Sleep2b (id=18): Aufstehen aus dem Sitz, dann Walk.
pub const SLEEP2_WAKE: AnimDef = AnimDef {
    frames: &[8, 7, 6],
    interval_ms: 300,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: false,
};

/// RunBegin (id=35): Anlauf-Ramp – Übergang von Walk zum Sprint.
pub const RUN_BEGIN: AnimDef = AnimDef {
    frames: &[2, 3, 2, 5, 4, 5, 4, 5],
    interval_ms: 150,
    dx_per_tick: 3,
    repeat: 1,
    next: &[(100, AnimId::Run)],
    flip_on_end: false,
};

/// Run (id=7): Sprint, schneller Frame-Wechsel + größeres dx.
pub const RUN: AnimDef = AnimDef {
    frames: &[5, 4, 4],
    interval_ms: 100,
    dx_per_tick: 5,
    repeat: 6,
    next: &[(70, AnimId::Run), (30, AnimId::RunEnd)],
    flip_on_end: false,
};

/// RunEnd (id=36): Abbrems-Ramp – Übergang zurück zu Walk.
pub const RUN_END: AnimDef = AnimDef {
    frames: &[5, 4, 5, 4, 5, 3, 2, 3, 2, 3],
    interval_ms: 150,
    dx_per_tick: 3,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: false,
};

/// Boing (id=8): elastische Landung – Alternative zu FallHard. Wird von der
/// Fall-Physik gelegentlich statt FallHard gewählt, damit Landungen visuell
/// abwechslungsreicher wirken.
pub const BOING: AnimDef = AnimDef {
    frames: &[62, 62, 63, 64, 65, 66, 67, 68, 69, 70, 6],
    interval_ms: 100,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: false,
};

/// WallRam (frei komponiert, kein Pendant in animations.xml):
/// Schaf knallt frontal gegen den Bildschirmrand. Sprites aus Zeile 4 des
/// Sheets: 48 = "Aua"-Frame mit X-Augen, 49 = seitlich weggekippt. Endet mit
/// neutralem Standframe (6) und Facing-Flip, sodass der Übergang in Walk
/// bereits in die neue Richtung zeigt.
pub const WALL_RAM: AnimDef = AnimDef {
    frames: &[48, 48, 49, 49, 49, 48, 48, 6],
    interval_ms: 180,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: true,
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

/// LookDown (id=43): an der Fensterkante stehen und runterschauen. Danach
/// umdrehen, runterspringen oder über die Kante abrutschen.
pub const LOOK_DOWN: AnimDef = AnimDef {
    frames: &[6, 78, 78, 78, 79, 80, 79, 78, 78, 78, 78, 78],
    interval_ms: 120,
    dx_per_tick: 0,
    repeat: 1,
    next: &[
        (40, AnimId::Rotate1A),
        (30, AnimId::JumpDown),
        (30, AnimId::FallWinA),
    ],
    flip_on_end: false,
};

/// JumpDown (id=44): Anlauf zum Sprung. Die eigentliche Flugbahn rechnet
/// die Fall-Physik ab JumpDownAir.
pub const JUMP_DOWN: AnimDef = AnimDef {
    frames: &[31, 30, 30, 4, 4, 4, 21],
    interval_ms: 150,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::JumpDownAir)],
    flip_on_end: false,
};

/// JumpDownAir (id=45): Flug-Frame; Ende bestimmt die Landung, nicht `next`.
pub const JUMP_DOWN_AIR: AnimDef = AnimDef {
    frames: &[25],
    interval_ms: 50,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::JumpDownAir)],
    flip_on_end: false,
};

/// JumpDownLand (id=46): abfedern und weiterlaufen.
pub const JUMP_DOWN_LAND: AnimDef = AnimDef {
    frames: &[24, 23, 23, 23, 23, 23, 31, 3, 3, 3, 3],
    interval_ms: 100,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: false,
};

/// FallWinA (id=51): Schaf rutscht mit den Vorderbeinen über die Kante und
/// dreht sich dabei zum Fenster um (Versatz setzt Sheep beim Eintritt).
pub const FALL_WIN_A: AnimDef = AnimDef {
    frames: &[77],
    interval_ms: 100,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::FallWinB)],
    flip_on_end: true,
};

/// FallWinB (id=52): an der Kante hängen und strampeln.
pub const FALL_WIN_B: AnimDef = AnimDef {
    frames: &[40, 41],
    interval_ms: 100,
    dx_per_tick: 0,
    repeat: 14,
    next: &[(100, AnimId::FallWinC)],
    flip_on_end: false,
};

/// FallWinC (id=53): Fall-Frame; Ende bestimmt die Landung, nicht `next`.
pub const FALL_WIN_C: AnimDef = AnimDef {
    frames: &[29],
    interval_ms: 50,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::FallWinC)],
    flip_on_end: false,
};

/// FallWinD (id=54): nach dem Aufprall benommen aufrappeln.
pub const FALL_WIN_D: AnimDef = AnimDef {
    frames: &[45, 45, 18, 76, 6],
    interval_ms: 400,
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
        AnimId::Sleep2Yawn => &SLEEP2_YAWN,
        AnimId::Sleep2 => &SLEEP2,
        AnimId::Sleep2Wake => &SLEEP2_WAKE,
        AnimId::Eat => &EAT,
        AnimId::RunBegin => &RUN_BEGIN,
        AnimId::Run => &RUN,
        AnimId::RunEnd => &RUN_END,
        AnimId::Drag => &DRAG,
        AnimId::Fall => &FALL,
        AnimId::FallHard => &FALL_HARD,
        AnimId::Boing => &BOING,
        AnimId::WallRam => &WALL_RAM,
        AnimId::LookDown => &LOOK_DOWN,
        AnimId::JumpDown => &JUMP_DOWN,
        AnimId::JumpDownAir => &JUMP_DOWN_AIR,
        AnimId::JumpDownLand => &JUMP_DOWN_LAND,
        AnimId::FallWinA => &FALL_WIN_A,
        AnimId::FallWinB => &FALL_WIN_B,
        AnimId::FallWinC => &FALL_WIN_C,
        AnimId::FallWinD => &FALL_WIN_D,
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
