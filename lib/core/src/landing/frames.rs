//! ASCII art frames for landing page lion animations
//!
//! Three sizes with practical ASCII-only art:
//! - Large (~28 cols x 14 rows): Roar animation (6 frames)
//! - Medium (~30 cols x 9 rows): Sleep animation (4 frames)
//! - Small (~16 cols x 7 rows): Breathing animation (4 frames)
//!
//! Design principles:
//! - ASCII-only (no Unicode blocks or special chars)
//! - Eyelids close realistically: O (open) -> - (halfway) -> _ (closed)
//! - Mane gradient: % # * ~ - . '

// =============================================================================
// LARGE LION - ROAR ANIMATION (6 frames)
// All lines padded to 28 chars for consistent centering
// =============================================================================

/// Large lion frame 0: Resting (Calm)
pub const LARGE_FRAME_0: &[&str] = &[
    r"        %%%###%%%       ",
    r"     %##*~-...-~*##%    ",
    r"   %#*'           '*#%  ",
    r"  %#'   .--___--.   '#% ",
    r" %#    /  O   O  \    #%",
    r" #*   |     Y     |   *#",
    r" #*   |  '-----'  |   *#",
    r" %#    \         /    #%",
    r"  %#*   `-------'   *#% ",
    r"    %##*~-._..-~*##%    ",
    r"       %%#####%%        ",
    r"                        ",
    r"       [ REOVIM ]       ",
];

/// Large lion frame 1: Alert (ears perk up)
pub const LARGE_FRAME_1: &[&str] = &[
    r"       ^%%###%%^        ",
    r"     %##*~-...-~*##%    ",
    r"   %#*'           '*#%  ",
    r"  %#'   .--___--.   '#% ",
    r" %#    /  O   O  \    #%",
    r" #*   |     Y     |   *#",
    r" #*   |  '-----'  |   *#",
    r" %#    \         /    #%",
    r"  %#*   `-------'   *#% ",
    r"    %##*~-._..-~*##%    ",
    r"       %%#####%%        ",
    r"                        ",
    r"       [ REOVIM ]       ",
];

/// Large lion frame 2: Mouth opens (first sound wave)
pub const LARGE_FRAME_2: &[&str] = &[
    r"       ^%%###%%^        ",
    r"     %##*~-...-~*##%    ",
    r"   %#*'           '*#%  ",
    r"  %#'   .--___--.   '#% ",
    r" %#    /  @   @  \    #%",
    r" #*   |     Y     |   *#",
    r" #*   |   /===\   |   *#",
    r" %#    \ |     | /    #%",
    r"  %#*   `-------'   *#% ",
    r"    %##*~-._..-~*##%    ",
    r"       %%#####%%      ) ",
    r"                        ",
    r"       [ REOVIM ]       ",
];

/// Large lion frame 3: ROAR PEAK!
pub const LARGE_FRAME_3: &[&str] = &[
    r"       ^%%###%%^     )))",
    r"     %##*~-...-~*##% )))",
    r"   %#*'           '*#%  ",
    r"  %#'   .--___--.   '#% ",
    r" %#    /  @   @  \    #%",
    r" #*   |     Y     |   *#",
    r" #*   |  /=====\  |   *#",
    r" %#    \|       |/    #%",
    r"  %#*   `-------'   *#% ",
    r"    %##*~-._..-~*##%    ",
    r"       %%#####%%    ))) ",
    r"                        ",
    r"       [ REOVIM ]       ",
];

/// Large lion frame 4: Sustain (waves spreading)
pub const LARGE_FRAME_4: &[&str] = &[
    r"       ^%%###%%^      ))",
    r"     %##*~-...-~*##%  ))",
    r"   %#*'           '*#%  ",
    r"  %#'   .--___--.   '#% ",
    r" %#    /  @   @  \    #%",
    r" #*   |     Y     |   *#",
    r" #*   |   /===\   |   *#",
    r" %#    \ |     | /    #%",
    r"  %#*   `-------'   *#% ",
    r"    %##*~-._..-~*##%    ",
    r"       %%#####%%     )) ",
    r"                        ",
    r"       [ REOVIM ]       ",
];

/// Large lion frame 5: Recovery (back to calm)
pub const LARGE_FRAME_5: &[&str] = &[
    r"        %%###%%         ",
    r"     %##*~-...-~*##%    ",
    r"   %#*'           '*#%  ",
    r"  %#'   .--___--.   '#% ",
    r" %#    /  O   O  \    #%",
    r" #*   |     Y     |   *#",
    r" #*   |  '-----'  |   *#",
    r" %#    \         /    #%",
    r"  %#*   `-------'   *#% ",
    r"    %##*~-._..-~*##%    ",
    r"       %%#####%%        ",
    r"                        ",
    r"       [ REOVIM ]       ",
];

/// All large lion frames for roar animation
pub static LARGE_ROAR_FRAMES: &[&[&str]] = &[
    LARGE_FRAME_0,
    LARGE_FRAME_1,
    LARGE_FRAME_2,
    LARGE_FRAME_3,
    LARGE_FRAME_4,
    LARGE_FRAME_5,
];

// =============================================================================
// MEDIUM LION - SLEEP ANIMATION (4 frames)
// Eye progression: O (open) -> - (halfway) -> _ (closed)
// All lines padded to 25 chars for consistent centering
// =============================================================================

/// Medium lion frame 0: Awake (eyes open)
pub const MEDIUM_FRAME_0: &[&str] = &[
    r"                         ",
    r"  .~*##*~.   .~*##*~.    ",
    r".*#       ###       #*.  ",
    r"#    O         O     #   ",
    r"*        ___         *   ",
    r"#      '-----'       #   ",
    r" *#.               .#*   ",
    r"   '~*#.  ___  .#*~'     ",
    r"        REOVIM           ",
];

/// Medium lion frame 1: Drowsy (eyelids halfway)
pub const MEDIUM_FRAME_1: &[&str] = &[
    r"                         ",
    r"  .~*##*~.   .~*##*~.    ",
    r".*#       ###       #*.  ",
    r"#    -         -     #   ",
    r"*        ___         *   ",
    r"#      '-----'       #   ",
    r" *#.               .#*   ",
    r"   '~*#.  ___  .#*~'     ",
    r"        REOVIM           ",
];

/// Medium lion frame 2: Sleeping (eyelids closed) + z
pub const MEDIUM_FRAME_2: &[&str] = &[
    r"                      z  ",
    r"  .~*##*~.   .~*##*~. z  ",
    r".*#       ###       #*.  ",
    r"#    _         _     #   ",
    r"*        ___         *   ",
    r"#      '-----'       #   ",
    r" *#.               .#*   ",
    r"   '~*#.  ___  .#*~'     ",
    r"        REOVIM           ",
];

/// Medium lion frame 3: Deep sleep + zzZ
pub const MEDIUM_FRAME_3: &[&str] = &[
    r"                    zzZ  ",
    r"  .~*##*~.   .~*##*~. z  ",
    r".*#       ###       #*.  ",
    r"#    _         _     #   ",
    r"*        ___         *   ",
    r"#      '-----'       #   ",
    r" *#.               .#*   ",
    r"   '~*#.  ___  .#*~'     ",
    r"        REOVIM           ",
];

/// All medium lion frames for sleep animation
pub static MEDIUM_SLEEP_FRAMES: &[&[&str]] = &[
    MEDIUM_FRAME_0,
    MEDIUM_FRAME_1,
    MEDIUM_FRAME_2,
    MEDIUM_FRAME_3,
];

// =============================================================================
// SMALL LION - BREATHING ANIMATION (4 frames)
// Mane expands/contracts subtly
// =============================================================================

/// Small lion frame 0: Rest (baseline)
pub const SMALL_FRAME_0: &[&str] = &[
    r"  /\   /\   ",
    r" '  \_/  '  ",
    r" | O   O |  ",
    r" |   w   |  ",
    r"  \_____/   ",
    r"   REOVIM   ",
];

/// Small lion frame 1: Inhale (mane expands)
pub const SMALL_FRAME_1: &[&str] = &[
    r"  /\     /\ ",
    r" '   \_/   '",
    r" |  O   O  |",
    r" |    w    |",
    r"  \_______/ ",
    r"    REOVIM  ",
];

/// Small lion frame 2: Peak (hold, relaxed eyes)
pub const SMALL_FRAME_2: &[&str] = &[
    r"  /\     /\ ",
    r" '   \_/   '",
    r" |  -   -  |",
    r" |    w    |",
    r"  \_______/ ",
    r"    REOVIM  ",
];

/// Small lion frame 3: Exhale (back to baseline)
pub const SMALL_FRAME_3: &[&str] = &[
    r"  /\   /\   ",
    r" '  \_/  '  ",
    r" | O   O |  ",
    r" |   w   |  ",
    r"  \_____/   ",
    r"   REOVIM   ",
];

/// All small lion frames for breathing animation
pub static SMALL_BREATHING_FRAMES: &[&[&str]] = &[
    SMALL_FRAME_0,
    SMALL_FRAME_1,
    SMALL_FRAME_2,
    SMALL_FRAME_3,
];

// =============================================================================
// SIZE SELECTION
// =============================================================================

/// Logo size categories based on terminal dimensions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoSize {
    /// Large lion with roar animation
    Large,
    /// Medium lion with sleep animation
    Medium,
    /// Small lion with breathing animation
    Small,
}

impl LogoSize {
    /// Select appropriate logo size based on terminal dimensions
    #[must_use]
    pub const fn from_dimensions(width: u16, height: u16) -> Self {
        // Large: need spacious terminal (50+ cols, 24+ rows)
        if width >= 50 && height >= 24 {
            return Self::Large;
        }
        // Medium: normal terminal (35+ cols, 16+ rows)
        if width >= 35 && height >= 16 {
            return Self::Medium;
        }
        // Small: fallback for split panes and tiny windows
        Self::Small
    }

    /// Get the frames for this logo size
    #[must_use]
    pub const fn frames(self) -> &'static [&'static [&'static str]] {
        match self {
            Self::Large => LARGE_ROAR_FRAMES,
            Self::Medium => MEDIUM_SLEEP_FRAMES,
            Self::Small => SMALL_BREATHING_FRAMES,
        }
    }

    /// Get recommended frame duration in milliseconds
    #[must_use]
    pub const fn frame_duration_ms(self) -> u32 {
        match self {
            Self::Large => 250,  // Roar: faster, dramatic
            Self::Medium => 500, // Sleep: slower, peaceful
            Self::Small => 400,  // Breathing: moderate
        }
    }

    /// Get pause duration after animation cycle (in milliseconds)
    #[must_use]
    pub const fn pause_duration_ms(self) -> u32 {
        match self {
            Self::Large => 3000,  // Long pause before next roar
            Self::Medium => 1000, // Brief pause in ping-pong
            Self::Small => 0,     // Continuous breathing
        }
    }
}
