//! Animations-Tabelle (aus Pets/esheep64/animations.xml abgeleitet, vereinfacht).

pub const TILES_X: u32 = 16;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Alle Varianten in Menü-Reihenfolge.
    pub const ALL: [Self; 8] = [
        Self::Default,
        Self::Blue,
        Self::Green,
        Self::Orange,
        Self::Pink,
        Self::Purple,
        Self::Red,
        Self::Yellow,
    ];

    /// Name wie bei `--color` (Umkehrung von `from_name`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Orange => "orange",
            Self::Pink => "pink",
            Self::Purple => "purple",
            Self::Red => "red",
            Self::Yellow => "yellow",
        }
    }

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
    /// Frei komponiert: Fenster wurde unter dem Schaf weggezogen – es hängt
    /// kurz verdutzt in der Luft (Glubschauge), schaut runter, dann Fall.
    Surprised,
    /// Panik-Fall nach Surprised: Frontansicht mit hochgerissenen Beinen
    /// (Frames aus "fall fast", id=6).
    FallPanic,
    /// vertical_walk_up (id=37), Anlauf-Teil: an der Wand aufrichten.
    ClimbStart,
    /// vertical_walk_up (id=37), Loop-Teil: die Bildschirmwand hochklettern.
    ClimbUp,
    /// top_walk (id=38): oben ankommen und kopfüber an die Decke drehen.
    TopWalkStart,
    /// top_walk2 (id=39): kopfüber an der Bildschirmoberkante laufen.
    TopWalk,
    /// top_walk3 (id=40): an der gegenüberliegenden Wand umgreifen.
    TopWalkEnd,
    /// vertical_walk_down (id=41): die Wand wieder herunterklettern.
    ClimbDown,
    /// vertical_walk_over (id=42): unten wieder auf die Füße kommen.
    ClimbOver,
    /// jump (id=25): Bocksprung aus dem Sprint heraus.
    Jump,
    /// sleep3a (id=19) Vorlauf: zum Betrachter drehen, Augen fallen zu.
    Sleep3Yawn,
    /// sleep3a (id=19) Loop-Teil: von vorn sitzend schlafen.
    Sleep3,
    /// sleep3b (id=20): aufwachen, zurückdrehen.
    Sleep3Wake,
    /// fall soft (id=9): weiche Landung auf dem Hintern.
    FallSoft,
    /// batha (id=21): als Meteor schräg ins Bild sausen.
    BathFly,
    /// bathb (id=22): brennend/verkohlt weiterfliegen bis in die Wanne.
    BathBurn,
    /// bathc (id=47): in der Badewanne entspannen.
    BathSoak,
    /// bathd (id=48): aus der Wanne steigen.
    BathOut,
    /// blacksheepa (id=28): zum Treffpunkt rennen.
    VisitRun,
    /// blacksheepb (id=29): die letzten Meter gehen.
    VisitWalk,
    /// blacksheepc (id=30): vor dem schwarzen Schaf erröten.
    VisitBlush,
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
        (62, AnimId::Walk),
        (7, AnimId::YawnSleep),
        (5, AnimId::Sleep2Yawn),
        (5, AnimId::Sleep3Yawn),
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
    next: &[(60, AnimId::Run), (25, AnimId::RunEnd), (15, AnimId::Jump)],
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

/// Surprised (frei komponiert): Glubschauge (50 → 51), Blick nach unten (78),
/// danach FallPanic. Insgesamt gut eine halbe Sekunde „in der Luft stehen“.
pub const SURPRISED: AnimDef = AnimDef {
    frames: &[50, 51, 51, 51, 51, 78, 78],
    interval_ms: 90,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::FallPanic)],
    flip_on_end: false,
};

/// FallPanic: Flug-Frames; Ende bestimmt die Landung, nicht `next`.
pub const FALL_PANIC: AnimDef = AnimDef {
    frames: &[46, 46, 47, 47],
    interval_ms: 80,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::FallPanic)],
    flip_on_end: false,
};

/// ClimbStart (id=37, Frames vor `repeatfrom`): an der Wand aufrichten.
/// Ungespiegelt hängen die Füße an der linken Wand.
pub const CLIMB_START: AnimDef = AnimDef {
    frames: &[31, 30],
    interval_ms: 150,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::ClimbUp)],
    flip_on_end: false,
};

/// ClimbUp (id=37, Loop-Teil): hochklettern. Endet nicht über `next`,
/// sondern sobald die Oberkante erreicht ist (siehe Sheep).
pub const CLIMB_UP: AnimDef = AnimDef {
    frames: &[15, 16],
    interval_ms: 150,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::ClimbUp)],
    flip_on_end: false,
};

/// TopWalkStart (id=38): um die obere Ecke herum, danach in die
/// Gegenrichtung (weg von der Wand).
pub const TOP_WALK_START: AnimDef = AnimDef {
    frames: &[16, 17, 28],
    interval_ms: 200,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::TopWalk)],
    flip_on_end: true,
};

/// TopWalk (id=39): kopfüber laufen. Im Original endet das nur an der
/// Wand; hier lässt das Schaf gelegentlich los und fällt – so landet es
/// ohne Hilfe auf Fenstern.
pub const TOP_WALK: AnimDef = AnimDef {
    frames: &[98, 97],
    interval_ms: 150,
    dx_per_tick: 2,
    repeat: 6,
    next: &[(80, AnimId::TopWalk), (20, AnimId::FallPanic)],
    flip_on_end: false,
};

/// TopWalkEnd (id=40): an der Gegenwand umgreifen, Blickrichtung kippt.
pub const TOP_WALK_END: AnimDef = AnimDef {
    frames: &[97, 97],
    interval_ms: 400,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::ClimbDown)],
    flip_on_end: true,
};

/// ClimbDown (id=41): kopfabwärts klettern. Ungespiegelt hängen die Füße
/// an der rechten Wand. Endet am Boden (siehe Sheep).
pub const CLIMB_DOWN: AnimDef = AnimDef {
    frames: &[19, 20],
    interval_ms: 150,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::ClimbDown)],
    flip_on_end: false,
};

/// ClimbOver (id=42): unten ankommen, aufrichten, weiterlaufen.
pub const CLIMB_OVER: AnimDef = AnimDef {
    frames: &[24, 6, 6, 6],
    interval_ms: 150,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: false,
};

/// Jump (id=25): Bocksprung. Die Flugkurve (Höhe über dem Boden) rechnet
/// Sheep aus dem Fortschritt der Sequenz, danach geht's im Sprint weiter.
pub const JUMP: AnimDef = AnimDef {
    frames: &[76, 30, 30, 30, 30, 23, 23, 23, 23, 23, 24, 24, 24, 24, 77],
    interval_ms: 70,
    dx_per_tick: 4,
    repeat: 1,
    next: &[(100, AnimId::Run)],
    flip_on_end: false,
};

/// Sleep3Yawn (id=19, Frames vor `repeatfrom`): zum Betrachter drehen,
/// hinsetzen, Augen fallen langsam zu.
pub const SLEEP3_YAWN: AnimDef = AnimDef {
    frames: &[3, 9, 10, 34, 35, 34, 35, 36],
    interval_ms: 250,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Sleep3)],
    flip_on_end: false,
};

/// Sleep3 (id=19, Loop-Teil): mit geschlossenen Augen sitzen. Wie SLEEP
/// endet es regulär über Mausbewegung; `repeat` ist nur der Notaus.
pub const SLEEP3: AnimDef = AnimDef {
    frames: &[36],
    interval_ms: 700,
    dx_per_tick: 0,
    repeat: 300,
    next: &[(100, AnimId::Sleep3Wake)],
    flip_on_end: false,
};

/// Sleep3Wake (id=20): blinzeln, aufwachen, zurück in die Laufrichtung.
pub const SLEEP3_WAKE: AnimDef = AnimDef {
    frames: &[35, 36, 36, 35, 34, 34, 34, 10, 9],
    interval_ms: 200,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: false,
};

/// FallSoft (id=9, ohne den Flug-Frame 133): auf den Hintern plumpsen
/// und wieder umdrehen.
pub const FALL_SOFT: AnimDef = AnimDef {
    frames: &[49, 13, 12, 6],
    interval_ms: 250,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Walk)],
    flip_on_end: false,
};

/// BathFly (id=21/22): Schaf saust schräg herein und fängt Feuer.
/// Die Flugbahn steuert Sheep, nicht `dx_per_tick`.
pub const BATH_FLY: AnimDef = AnimDef {
    frames: &[134, 134, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143],
    interval_ms: 90,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::BathBurn)],
    flip_on_end: false,
};

/// BathBurn (id=22, Ende): verkohlt weiterfliegen bis zur Wanne.
pub const BATH_BURN: AnimDef = AnimDef {
    frames: &[144, 145],
    interval_ms: 90,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::BathBurn)],
    flip_on_end: false,
};

/// BathSoak (id=47): nur der Kopf schaut aus dem Wasser, Augen fallen zu.
pub const BATH_SOAK: AnimDef = AnimDef {
    frames: &[169, 169, 169, 170, 171, 171, 171, 170, 169, 169],
    interval_ms: 200,
    dx_per_tick: 0,
    repeat: 3,
    next: &[(100, AnimId::BathOut)],
    flip_on_end: false,
};

/// BathOut (id=48): in der Wanne aufstehen (20 px höher, siehe Sheep)
/// und herausdrehen.
pub const BATH_OUT: AnimDef = AnimDef {
    frames: &[119, 81, 81, 82, 82, 10],
    interval_ms: 180,
    dx_per_tick: 0,
    repeat: 1,
    next: &[(100, AnimId::Rotate1B)],
    flip_on_end: false,
};

/// VisitRun (id=28): zum Treffpunkt sprinten. Ziel steuert Sheep.
pub const VISIT_RUN: AnimDef = AnimDef {
    frames: &[5, 4, 4],
    interval_ms: 100,
    dx_per_tick: 5,
    repeat: 1,
    next: &[(100, AnimId::VisitRun)],
    flip_on_end: false,
};

/// VisitWalk (id=29): die letzten Meter gehen.
pub const VISIT_WALK: AnimDef = AnimDef {
    frames: &[2, 3],
    interval_ms: 200,
    dx_per_tick: 2,
    repeat: 1,
    next: &[(100, AnimId::VisitWalk)],
    flip_on_end: false,
};

/// VisitBlush (id=30): anschauen, erröten (127–130), wieder fassen.
pub const VISIT_BLUSH: AnimDef = AnimDef {
    frames: &[
        8, 3, 3, 3, 127, 128, 129, 130, 130, 130, 130, 129, 128, 127, 3, 3, 3,
    ],
    interval_ms: 250,
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
        AnimId::Surprised => &SURPRISED,
        AnimId::FallPanic => &FALL_PANIC,
        AnimId::ClimbStart => &CLIMB_START,
        AnimId::ClimbUp => &CLIMB_UP,
        AnimId::TopWalkStart => &TOP_WALK_START,
        AnimId::TopWalk => &TOP_WALK,
        AnimId::TopWalkEnd => &TOP_WALK_END,
        AnimId::ClimbDown => &CLIMB_DOWN,
        AnimId::ClimbOver => &CLIMB_OVER,
        AnimId::Jump => &JUMP,
        AnimId::Sleep3Yawn => &SLEEP3_YAWN,
        AnimId::Sleep3 => &SLEEP3,
        AnimId::Sleep3Wake => &SLEEP3_WAKE,
        AnimId::FallSoft => &FALL_SOFT,
        AnimId::BathFly => &BATH_FLY,
        AnimId::BathBurn => &BATH_BURN,
        AnimId::BathSoak => &BATH_SOAK,
        AnimId::BathOut => &BATH_OUT,
        AnimId::VisitRun => &VISIT_RUN,
        AnimId::VisitWalk => &VISIT_WALK,
        AnimId::VisitBlush => &VISIT_BLUSH,
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
