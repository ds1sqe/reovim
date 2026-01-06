//! Theme system for semantic color definitions
//!
//! The theme system is organized into logical sub-structs for better maintainability:
//! - `base`: Default and cursor line styles
//! - `gutter`: Line numbers, sign column
//! - `selection`: Visual selection styles
//! - `statusline`: Status line with mode-specific styles
//! - `popup`: Completion popup styles
//! - `telescope`: Fuzzy finder styles
//! - `whichkey`: Which-key panel styles
//! - `fold`: Code folding styles
//! - `indent`: Indentation guide styles
//! - `scrollbar`: Scrollbar with diagnostic marks
//! - `search`: Search highlight styles
//! - `tab`: Tab line styles
//! - `window`: Window separator styles
//! - `diagnostic`: Enhanced diagnostic styles with underline colors
//! - `cursor`: Cursor effects (glow, pulse)
//! - `semantic`: Language-specific semantic token styles
//! - `leap`: Leap navigation label styles
//! - `animation`: Animation configuration

use {crate::highlight::Style, reovim_sys::style::Color};

// ============================================================================
// Theme Name Enum
// ============================================================================

/// Available theme names
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    Dark,
    Light,
    TokyoNightOrange,
}

impl ThemeName {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "dark" | "onedark" | "one-dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "tokyonight" | "tokyo-night" | "tokyonight-orange" | "tokyo-night-orange" => {
                Some(Self::TokyoNightOrange)
            }
            _ => None,
        }
    }
}

// ============================================================================
// Sub-Struct Definitions
// ============================================================================

#[derive(Debug, Clone)]
pub struct BaseStyles {
    pub default: Style,
    pub cursor_line: Style,
    pub command_line: Style,
}

#[derive(Debug, Clone)]
pub struct GutterStyles {
    pub line_number: Style,
    pub current_line_number: Style,
    pub inactive_line_number: Style,
    /// Background style for sign column
    pub sign_column_bg: Style,
}

#[derive(Debug, Clone)]
pub struct SelectionStyles {
    pub visual: Style,
    pub block: Style,
}

#[derive(Debug, Clone)]
pub struct StatusLineStyles {
    pub background: Style,
    /// Style for interactor/component name (e.g., "Editor", "Explorer", "Telescope")
    pub interactor: Style,
    pub mode: StatusLineModeStyles,
    pub filename: Style,
    pub modified: Style,
    pub position: Style,
    pub filetype: Style,
    pub separator: StatusLineSeparator,
}

#[derive(Debug, Clone)]
pub struct StatusLineModeStyles {
    pub normal: Style,
    pub insert: Style,
    pub visual: Style,
    pub command: Style,
    pub replace: Style,
    pub operator_pending: Style,
}

/// Separator configuration for status line sections
#[derive(Debug, Clone)]
pub struct StatusLineSeparator {
    /// Left separator character (e.g., "", "")
    pub left: &'static str,
    /// Right separator character (e.g., "", "")
    pub right: &'static str,
    /// Separator between sections (e.g., "|", "│")
    pub section: &'static str,
    /// Style for separator characters
    pub style: Style,
}

#[derive(Debug, Clone)]
pub struct PopupStyles {
    pub normal: Style,
    pub selected: Style,
    pub border: Style,
    /// Foreground color for matched characters in fuzzy completion
    pub match_fg: Style,
}

#[derive(Debug, Clone)]
pub struct WhichKeyStyles {
    pub background: Style,
    pub key: Style,
    pub description: Style,
    pub prefix: Style,
    pub border: Style,
}

#[derive(Debug, Clone)]
pub struct FoldStyles {
    pub marker: Style,
    pub folded_line: Style,
}

#[derive(Debug, Clone)]
pub struct IndentStyles {
    pub guide: Style,
    pub active: Style,
}

#[derive(Debug, Clone)]
pub struct ScrollbarStyles {
    pub track: Style,
    pub thumb: Style,
    pub search_mark: Style,
    pub error_mark: Style,
    pub warn_mark: Style,
    pub info_mark: Style,
    pub hint_mark: Style,
}

#[derive(Debug, Clone)]
pub struct SearchStyles {
    pub match_highlight: Style,
    pub current_match: Style,
    pub inc_search: Style,
}

#[derive(Debug, Clone)]
pub struct TabStyles {
    pub active: Style,
    pub inactive: Style,
    pub fill: Style,
}

#[derive(Debug, Clone)]
pub struct WindowStyles {
    pub separator: Style,
}

#[derive(Debug, Clone)]
pub struct BracketStyles {
    /// Rainbow bracket colors (6 colors cycling by depth)
    pub rainbow: [Style; 6],
    /// Matched pair highlight (when cursor is on bracket)
    pub matched: Style,
    /// Unmatched bracket (error highlighting)
    pub unmatched: Style,
}

/// Enhanced diagnostic styles with colored underlines
#[derive(Debug, Clone)]
pub struct DiagnosticStyles {
    /// Hint diagnostic (lowest severity) - curly underline, cyan
    pub hint: Style,
    /// Info diagnostic - curly underline, blue
    pub info: Style,
    /// Warning diagnostic - curly underline, yellow
    pub warn: Style,
    /// Error diagnostic (highest severity) - curly underline, red
    pub error: Style,
    /// Deprecated code - strikethrough + dim
    pub deprecated: Style,
    /// Unnecessary/unused code - dimmed
    pub unnecessary: Style,
}

/// Cursor-related styles and effect configurations
#[derive(Debug, Clone)]
pub struct CursorStyles {
    /// Cursor line background
    pub line: Style,
    /// Cursor column background (if enabled)
    pub column: Style,
    /// Cursor glow effect configuration
    pub glow: Option<EffectConfig>,
    /// Cursor pulse effect configuration
    pub pulse: Option<EffectConfig>,
}

/// Configuration for visual effects
#[derive(Debug, Clone)]
pub struct EffectConfig {
    /// Base style for the effect
    pub style: Style,
    /// Effect radius in cells (for glow)
    pub radius: u8,
    /// Animation speed in ms per frame (0 = no animation)
    pub animation_ms: u16,
}

/// Semantic token styles for language-specific highlighting
#[derive(Debug, Clone)]
pub struct SemanticStyles {
    /// Lifetime annotations ('a, 'static)
    pub lifetime: Style,
    /// Trait bounds (: Trait, where T: Bound)
    pub trait_bound: Style,
    /// async keyword
    pub async_keyword: Style,
    /// await keyword
    pub await_keyword: Style,
    /// unsafe keyword and blocks
    pub unsafe_keyword: Style,
    /// Macro invocations
    pub macro_invocation: Style,
    /// Attribute annotations (#[...])
    pub attribute: Style,
    /// Mutable variables (typically underlined)
    pub mutable: Style,
    /// Constant values
    pub constant: Style,
    /// Static variables
    pub static_var: Style,
}

/// Leap navigation label styles
#[derive(Debug, Clone)]
pub struct LeapStyles {
    /// Primary label (first character)
    pub label_primary: Style,
    /// Secondary label (second character)
    pub label_secondary: Style,
    /// Dimmed background text during leap
    pub dimmed: Style,
}

/// Animation configuration
#[derive(Debug, Clone)]
pub struct AnimationConfig {
    /// Enable animations globally
    pub enabled: bool,
    /// Animation frame rate (fps, default 30)
    pub frame_rate: u8,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            frame_rate: 30,
        }
    }
}

// ============================================================================
// Main Theme Struct
// ============================================================================

#[derive(Debug, Clone)]
pub struct Theme {
    pub base: BaseStyles,
    pub gutter: GutterStyles,
    pub selection: SelectionStyles,
    pub statusline: StatusLineStyles,
    pub popup: PopupStyles,
    pub whichkey: WhichKeyStyles,
    pub fold: FoldStyles,
    pub indent: IndentStyles,
    pub scrollbar: ScrollbarStyles,
    pub search: SearchStyles,
    pub tab: TabStyles,
    pub window: WindowStyles,
    pub brackets: BracketStyles,
    // Extended style categories
    pub diagnostic: DiagnosticStyles,
    pub cursor: CursorStyles,
    pub semantic: SemanticStyles,
    pub leap: LeapStyles,
    pub animation: AnimationConfig,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    #[must_use]
    pub fn from_name(name: ThemeName) -> Self {
        match name {
            ThemeName::Dark => Self::dark(),
            ThemeName::Light => Self::light(),
            ThemeName::TokyoNightOrange => Self::tokyo_night_orange(),
        }
    }

    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn dark() -> Self {
        let bg_dark = Color::Rgb {
            r: 33,
            g: 37,
            b: 43,
        };
        let bg_medium = Color::Rgb {
            r: 40,
            g: 44,
            b: 52,
        };
        let bg_light = Color::Rgb {
            r: 50,
            g: 56,
            b: 66,
        };
        let fg = Color::Rgb {
            r: 171,
            g: 178,
            b: 191,
        };
        let fg_dark = Color::Rgb {
            r: 92,
            g: 99,
            b: 112,
        };
        let yellow = Color::Rgb {
            r: 229,
            g: 192,
            b: 123,
        };
        let blue = Color::Rgb {
            r: 97,
            g: 175,
            b: 239,
        };
        let green = Color::Rgb {
            r: 152,
            g: 195,
            b: 121,
        };
        let red = Color::Rgb {
            r: 224,
            g: 108,
            b: 117,
        };
        let cyan = Color::Rgb {
            r: 86,
            g: 182,
            b: 194,
        };
        let magenta = Color::Rgb {
            r: 198,
            g: 120,
            b: 221,
        };

        Self {
            base: BaseStyles {
                default: Style::new().fg(fg),
                cursor_line: Style::new().bg(bg_light),
                command_line: Style::new().fg(fg),
            },
            gutter: GutterStyles {
                line_number: Style::new().fg(fg_dark).bg(bg_dark),
                current_line_number: Style::new().fg(yellow).bg(bg_dark).bold(),
                inactive_line_number: Style::new().fg(fg_dark).bg(bg_dark),
                sign_column_bg: Style::new().bg(bg_dark),
            },
            selection: SelectionStyles {
                visual: Style::new().bg(bg_light),
                block: Style::new().bg(bg_light),
            },
            statusline: StatusLineStyles {
                background: Style::new().fg(fg).bg(bg_light),
                // Interactor section (e.g., "Editor") - distinct purple/violet color
                interactor: Style::new().fg(fg).bg(Color::AnsiValue(61)).bold(), // Purple/violet
                mode: StatusLineModeStyles {
                    normal: Style::new().fg(bg_dark).bg(green).bold(),
                    insert: Style::new().fg(bg_dark).bg(blue).bold(),
                    visual: Style::new().fg(bg_dark).bg(magenta).bold(),
                    command: Style::new().fg(bg_dark).bg(yellow).bold(),
                    replace: Style::new().fg(bg_dark).bg(red).bold(),
                    operator_pending: Style::new().fg(bg_dark).bg(cyan).bold(),
                },
                filename: Style::new().fg(fg).bg(bg_light),
                modified: Style::new().fg(yellow).bg(bg_light),
                position: Style::new().fg(fg_dark).bg(bg_light),
                filetype: Style::new().fg(cyan).bg(bg_light),
                separator: StatusLineSeparator {
                    left: "",
                    right: "",
                    section: " │ ",
                    style: Style::new().fg(fg_dark),
                },
            },
            popup: PopupStyles {
                normal: Style::new().fg(fg).bg(bg_medium),
                selected: Style::new().fg(bg_dark).bg(blue),
                border: Style::new().fg(fg_dark).bg(bg_medium),
                match_fg: Style::new().fg(yellow).bold(),
            },
            whichkey: WhichKeyStyles {
                background: Style::new().bg(bg_medium),
                key: Style::new().fg(yellow).bg(bg_medium).bold(),
                description: Style::new().fg(fg).bg(bg_medium),
                prefix: Style::new().fg(cyan).bg(bg_medium).bold(),
                border: Style::new().fg(fg_dark).bg(bg_medium),
            },
            fold: FoldStyles {
                marker: Style::new().fg(fg_dark).italic(),
                folded_line: Style::new().fg(fg_dark),
            },
            indent: IndentStyles {
                guide: Style::new().fg(fg_dark),
                active: Style::new().fg(fg),
            },
            scrollbar: ScrollbarStyles {
                track: Style::new().fg(bg_light),
                thumb: Style::new().fg(fg_dark).bg(bg_light),
                search_mark: Style::new().fg(yellow),
                error_mark: Style::new().fg(red),
                warn_mark: Style::new().fg(yellow),
                info_mark: Style::new().fg(blue),
                hint_mark: Style::new().fg(cyan),
            },
            search: SearchStyles {
                match_highlight: Style::new().fg(bg_dark).bg(yellow),
                current_match: Style::new().fg(bg_dark).bg(Color::Rgb {
                    r: 255,
                    g: 165,
                    b: 0,
                }),
                inc_search: Style::new().fg(bg_dark).bg(yellow),
            },
            tab: TabStyles {
                active: Style::new().fg(fg).bg(bg_medium).bold(),
                inactive: Style::new().fg(fg_dark).bg(bg_dark),
                fill: Style::new().bg(bg_dark),
            },
            window: WindowStyles {
                separator: Style::new().fg(fg_dark),
            },
            brackets: BracketStyles {
                rainbow: [
                    Style::new().fg(red),     // Depth 0: Red
                    Style::new().fg(yellow),  // Depth 1: Yellow
                    Style::new().fg(green),   // Depth 2: Green
                    Style::new().fg(cyan),    // Depth 3: Cyan
                    Style::new().fg(blue),    // Depth 4: Blue
                    Style::new().fg(magenta), // Depth 5: Magenta
                ],
                matched: Style::new().bold().underline(),
                unmatched: Style::new().fg(red).underline(),
            },
            // Extended style categories
            diagnostic: DiagnosticStyles {
                hint: Style::new()
                    .fg(cyan)
                    .curly_underline()
                    .underline_color(cyan),
                info: Style::new()
                    .fg(blue)
                    .curly_underline()
                    .underline_color(blue),
                warn: Style::new()
                    .fg(yellow)
                    .curly_underline()
                    .underline_color(yellow),
                error: Style::new().fg(red).curly_underline().underline_color(red),
                deprecated: Style::new().strikethrough().dim(),
                unnecessary: Style::new().dim(),
            },
            cursor: CursorStyles {
                line: Style::new().bg(bg_light),
                column: Style::new().bg(bg_light),
                glow: None, // No animation by default
                pulse: None,
            },
            semantic: SemanticStyles {
                lifetime: Style::new().fg(magenta).italic(),
                trait_bound: Style::new().fg(yellow).italic(),
                async_keyword: Style::new().fg(magenta).bold().italic(),
                await_keyword: Style::new().fg(magenta).bold().italic(),
                unsafe_keyword: Style::new().fg(red).bold(),
                macro_invocation: Style::new().fg(cyan).bold(),
                attribute: Style::new().fg(yellow),
                mutable: Style::new().underline(),
                constant: Style::new().fg(cyan).bold(),
                static_var: Style::new().fg(cyan),
            },
            leap: LeapStyles {
                label_primary: Style::new().fg(bg_dark).bg(magenta).bold(),
                label_secondary: Style::new().fg(bg_dark).bg(yellow).bold(),
                dimmed: Style::new().dim(),
            },
            animation: AnimationConfig::default(),
        }
    }

    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn light() -> Self {
        let fg_light = Color::Rgb {
            r: 56,
            g: 58,
            b: 66,
        };
        let bg_light = Color::Rgb {
            r: 250,
            g: 250,
            b: 250,
        };
        let bg_medium = Color::Rgb {
            r: 233,
            g: 233,
            b: 233,
        };
        let bg_highlight = Color::AnsiValue(254);

        Self {
            base: BaseStyles {
                default: Style::new().fg(fg_light),
                cursor_line: Style::new().bg(bg_highlight),
                command_line: Style::new().fg(fg_light),
            },
            gutter: GutterStyles {
                line_number: Style::new().fg(Color::Grey),
                current_line_number: Style::new().fg(Color::DarkBlue).bold(),
                inactive_line_number: Style::new().fg(Color::Grey),
                sign_column_bg: Style::new().bg(bg_medium),
            },
            selection: SelectionStyles {
                visual: Style::new().bg(Color::AnsiValue(153)),
                block: Style::new().bg(Color::AnsiValue(153)),
            },
            statusline: StatusLineStyles {
                background: Style::new().fg(fg_light).bg(bg_medium),
                // Interactor section - distinct purple color for light theme
                interactor: Style::new()
                    .fg(Color::White)
                    .bg(Color::AnsiValue(61))
                    .bold(),
                mode: StatusLineModeStyles {
                    normal: Style::new().fg(Color::White).bg(Color::DarkGreen).bold(),
                    insert: Style::new().fg(Color::White).bg(Color::DarkBlue).bold(),
                    visual: Style::new().fg(Color::White).bg(Color::DarkMagenta).bold(),
                    command: Style::new().fg(Color::Black).bg(Color::DarkYellow).bold(),
                    replace: Style::new().fg(Color::White).bg(Color::DarkRed).bold(),
                    operator_pending: Style::new().fg(Color::White).bg(Color::DarkCyan).bold(),
                },
                filename: Style::new().fg(fg_light),
                modified: Style::new().fg(Color::DarkYellow),
                position: Style::new().fg(Color::Grey),
                filetype: Style::new().fg(Color::DarkCyan),
                separator: StatusLineSeparator {
                    left: "",
                    right: "",
                    section: " │ ",
                    style: Style::new().fg(Color::Grey),
                },
            },
            popup: PopupStyles {
                normal: Style::new().fg(fg_light).bg(Color::AnsiValue(252)),
                selected: Style::new().fg(Color::White).bg(Color::DarkBlue),
                border: Style::new().fg(Color::Grey).bg(Color::AnsiValue(252)),
                match_fg: Style::new().fg(Color::DarkYellow).bold(),
            },
            whichkey: WhichKeyStyles {
                background: Style::new().bg(bg_medium),
                key: Style::new().fg(Color::DarkBlue).bg(bg_medium).bold(),
                description: Style::new().fg(fg_light).bg(bg_medium),
                prefix: Style::new().fg(Color::DarkCyan).bg(bg_medium).bold(),
                border: Style::new().fg(Color::Grey).bg(bg_medium),
            },
            fold: FoldStyles {
                marker: Style::new().fg(Color::Grey).italic(),
                folded_line: Style::new().fg(Color::Grey),
            },
            indent: IndentStyles {
                guide: Style::new().fg(Color::AnsiValue(250)),
                active: Style::new().fg(Color::Grey),
            },
            scrollbar: ScrollbarStyles {
                track: Style::new().fg(Color::AnsiValue(252)),
                thumb: Style::new().fg(Color::Grey).bg(Color::AnsiValue(250)),
                search_mark: Style::new().fg(Color::DarkYellow),
                error_mark: Style::new().fg(Color::DarkRed),
                warn_mark: Style::new().fg(Color::DarkYellow),
                info_mark: Style::new().fg(Color::DarkBlue),
                hint_mark: Style::new().fg(Color::DarkCyan),
            },
            search: SearchStyles {
                match_highlight: Style::new().fg(Color::Black).bg(Color::Yellow),
                current_match: Style::new().fg(Color::Black).bg(Color::Rgb {
                    r: 255,
                    g: 165,
                    b: 0,
                }),
                inc_search: Style::new().fg(Color::Black).bg(Color::Yellow),
            },
            tab: TabStyles {
                active: Style::new().fg(fg_light).bg(bg_light).bold(),
                inactive: Style::new().fg(Color::Grey).bg(bg_medium),
                fill: Style::new().bg(bg_medium),
            },
            window: WindowStyles {
                separator: Style::new().fg(Color::Grey),
            },
            brackets: BracketStyles {
                rainbow: [
                    Style::new().fg(Color::DarkRed),     // Depth 0: Red
                    Style::new().fg(Color::DarkYellow),  // Depth 1: Yellow
                    Style::new().fg(Color::DarkGreen),   // Depth 2: Green
                    Style::new().fg(Color::DarkCyan),    // Depth 3: Cyan
                    Style::new().fg(Color::DarkBlue),    // Depth 4: Blue
                    Style::new().fg(Color::DarkMagenta), // Depth 5: Magenta
                ],
                matched: Style::new().bold().underline(),
                unmatched: Style::new().fg(Color::DarkRed).underline(),
            },
            // Extended style categories
            diagnostic: DiagnosticStyles {
                hint: Style::new()
                    .fg(Color::DarkCyan)
                    .curly_underline()
                    .underline_color(Color::DarkCyan),
                info: Style::new()
                    .fg(Color::DarkBlue)
                    .curly_underline()
                    .underline_color(Color::DarkBlue),
                warn: Style::new()
                    .fg(Color::DarkYellow)
                    .curly_underline()
                    .underline_color(Color::DarkYellow),
                error: Style::new()
                    .fg(Color::DarkRed)
                    .curly_underline()
                    .underline_color(Color::DarkRed),
                deprecated: Style::new().strikethrough().dim(),
                unnecessary: Style::new().dim(),
            },
            cursor: CursorStyles {
                line: Style::new().bg(bg_highlight),
                column: Style::new().bg(bg_highlight),
                glow: None,
                pulse: None,
            },
            semantic: SemanticStyles {
                lifetime: Style::new().fg(Color::DarkMagenta).italic(),
                trait_bound: Style::new().fg(Color::DarkYellow).italic(),
                async_keyword: Style::new().fg(Color::DarkMagenta).bold().italic(),
                await_keyword: Style::new().fg(Color::DarkMagenta).bold().italic(),
                unsafe_keyword: Style::new().fg(Color::DarkRed).bold(),
                macro_invocation: Style::new().fg(Color::DarkCyan).bold(),
                attribute: Style::new().fg(Color::DarkYellow),
                mutable: Style::new().underline(),
                constant: Style::new().fg(Color::DarkCyan).bold(),
                static_var: Style::new().fg(Color::DarkCyan),
            },
            leap: LeapStyles {
                label_primary: Style::new().fg(Color::White).bg(Color::DarkMagenta).bold(),
                label_secondary: Style::new().fg(Color::Black).bg(Color::Yellow).bold(),
                dimmed: Style::new().dim(),
            },
            animation: AnimationConfig::default(),
        }
    }

    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn tokyo_night_orange() -> Self {
        let bg = Color::Rgb {
            r: 26,
            g: 27,
            b: 38,
        };
        let bg_dark = Color::Rgb {
            r: 22,
            g: 23,
            b: 34,
        };
        let bg_highlight = Color::Rgb {
            r: 41,
            g: 46,
            b: 66,
        };
        let fg = Color::Rgb {
            r: 192,
            g: 202,
            b: 245,
        };
        let fg_dark = Color::Rgb {
            r: 86,
            g: 95,
            b: 137,
        };
        // Much darker color for inactive windows (#292e42)
        let fg_inactive = Color::Rgb {
            r: 41,
            g: 46,
            b: 66,
        };
        let comment = Color::Rgb {
            r: 199,
            g: 199,
            b: 199,
        };
        let orange = Color::Rgb {
            r: 255,
            g: 158,
            b: 100,
        };
        let orange_bright = Color::Rgb {
            r: 255,
            g: 122,
            b: 0,
        };
        let blue = Color::Rgb {
            r: 122,
            g: 162,
            b: 247,
        };
        let green = Color::Rgb {
            r: 158,
            g: 206,
            b: 106,
        };
        let cyan = Color::Rgb {
            r: 125,
            g: 207,
            b: 255,
        };
        let red = Color::Rgb {
            r: 247,
            g: 118,
            b: 142,
        };
        let magenta = Color::Rgb {
            r: 255,
            g: 0,
            b: 127,
        };
        let yellow = Color::Rgb {
            r: 224,
            g: 175,
            b: 104,
        };

        Self {
            base: BaseStyles {
                default: Style::new().fg(fg),
                cursor_line: Style::new().bg(bg_highlight),
                command_line: Style::new().fg(fg),
            },
            gutter: GutterStyles {
                line_number: Style::new().fg(fg_dark),
                current_line_number: Style::new().fg(orange_bright).bold(),
                inactive_line_number: Style::new().fg(fg_inactive),
                sign_column_bg: Style::new().bg(bg_dark),
            },
            selection: SelectionStyles {
                visual: Style::new().bg(bg_highlight),
                block: Style::new().bg(bg_highlight),
            },
            statusline: StatusLineStyles {
                background: Style::new().fg(fg).bg(bg_dark),
                // Interactor section - purple/violet for tokyo night theme
                interactor: Style::new()
                    .fg(fg)
                    .bg(Color::Rgb {
                        r: 122,
                        g: 112,
                        b: 183,
                    })
                    .bold(), // Soft purple
                mode: StatusLineModeStyles {
                    normal: Style::new().fg(bg).bg(green).bold(),
                    insert: Style::new().fg(bg).bg(blue).bold(),
                    visual: Style::new().fg(bg).bg(magenta).bold(),
                    command: Style::new().fg(bg).bg(orange).bold(),
                    replace: Style::new().fg(bg).bg(red).bold(),
                    operator_pending: Style::new().fg(bg).bg(cyan).bold(),
                },
                filename: Style::new().fg(fg),
                modified: Style::new().fg(orange),
                position: Style::new().fg(fg_dark),
                filetype: Style::new().fg(cyan),
                separator: StatusLineSeparator {
                    left: "",
                    right: "",
                    section: " │ ",
                    style: Style::new().fg(fg_dark),
                },
            },
            popup: PopupStyles {
                normal: Style::new().fg(fg).bg(bg_dark),
                selected: Style::new().fg(bg).bg(blue),
                border: Style::new().fg(fg_dark).bg(bg_dark),
                match_fg: Style::new().fg(orange).bold(),
            },
            whichkey: WhichKeyStyles {
                background: Style::new().bg(bg_dark),
                key: Style::new().fg(orange).bg(bg_dark).bold(),
                description: Style::new().fg(fg).bg(bg_dark),
                prefix: Style::new().fg(cyan).bg(bg_dark).bold(),
                border: Style::new().fg(fg_dark).bg(bg_dark),
            },
            fold: FoldStyles {
                marker: Style::new().fg(fg_dark).italic(),
                folded_line: Style::new().fg(comment),
            },
            indent: IndentStyles {
                guide: Style::new().fg(fg_dark),
                active: Style::new().fg(fg),
            },
            scrollbar: ScrollbarStyles {
                track: Style::new().fg(bg_highlight),
                thumb: Style::new().fg(fg_dark).bg(bg_highlight),
                search_mark: Style::new().fg(orange_bright),
                error_mark: Style::new().fg(red),
                warn_mark: Style::new().fg(yellow),
                info_mark: Style::new().fg(blue),
                hint_mark: Style::new().fg(cyan),
            },
            search: SearchStyles {
                match_highlight: Style::new().fg(bg).bg(orange),
                current_match: Style::new().fg(bg).bg(orange_bright),
                inc_search: Style::new().fg(bg).bg(orange),
            },
            tab: TabStyles {
                active: Style::new().fg(fg).bg(bg).bold(),
                inactive: Style::new().fg(fg_dark).bg(bg_dark),
                fill: Style::new().bg(bg_dark),
            },
            window: WindowStyles {
                separator: Style::new().fg(fg_dark),
            },
            brackets: BracketStyles {
                rainbow: [
                    Style::new().fg(red),    // Depth 0: Red/Pink #f7768e
                    Style::new().fg(orange), // Depth 1: Orange #ff9e64
                    Style::new().fg(yellow), // Depth 2: Yellow #e0af68
                    Style::new().fg(green),  // Depth 3: Green #9ece6a
                    Style::new().fg(cyan),   // Depth 4: Cyan #7dcfff
                    Style::new().fg(Color::Rgb {
                        // Depth 5: Purple #bb9af7
                        r: 187,
                        g: 154,
                        b: 247,
                    }),
                ],
                matched: Style::new().bold().underline(),
                unmatched: Style::new().fg(red).underline(),
            },
            // Extended style categories
            diagnostic: DiagnosticStyles {
                hint: Style::new()
                    .fg(cyan)
                    .curly_underline()
                    .underline_color(cyan),
                info: Style::new()
                    .fg(blue)
                    .curly_underline()
                    .underline_color(blue),
                warn: Style::new()
                    .fg(yellow)
                    .curly_underline()
                    .underline_color(yellow),
                error: Style::new().fg(red).curly_underline().underline_color(red),
                deprecated: Style::new().strikethrough().dim(),
                unnecessary: Style::new().dim(),
            },
            cursor: CursorStyles {
                line: Style::new().bg(bg_highlight),
                column: Style::new().bg(bg_highlight),
                glow: None,
                pulse: None,
            },
            semantic: SemanticStyles {
                lifetime: Style::new().fg(magenta).italic(),
                trait_bound: Style::new().fg(yellow).italic(),
                async_keyword: Style::new().fg(magenta).bold().italic(),
                await_keyword: Style::new().fg(magenta).bold().italic(),
                unsafe_keyword: Style::new().fg(red).bold(),
                macro_invocation: Style::new().fg(cyan).bold(),
                attribute: Style::new().fg(yellow),
                mutable: Style::new().underline(),
                constant: Style::new().fg(cyan).bold(),
                static_var: Style::new().fg(cyan),
            },
            leap: LeapStyles {
                label_primary: Style::new().fg(bg).bg(magenta).bold(),
                label_secondary: Style::new().fg(bg).bg(orange).bold(),
                dimmed: Style::new().dim(),
            },
            animation: AnimationConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme() {
        let theme = Theme::default();
        assert!(theme.gutter.line_number.fg.is_some());
        assert!(
            theme
                .gutter
                .current_line_number
                .attributes
                .contains(crate::highlight::Attributes::BOLD)
        );
    }

    #[test]
    fn test_dark_theme() {
        let theme = Theme::dark();
        assert!(theme.selection.visual.bg.is_some());
    }

    #[test]
    fn test_light_theme() {
        let theme = Theme::light();
        assert!(theme.base.default.fg.is_some());
    }

    #[test]
    fn test_tokyo_night_orange_theme() {
        let theme = Theme::tokyo_night_orange();
        assert!(theme.gutter.current_line_number.fg.is_some());
        assert!(
            theme
                .gutter
                .current_line_number
                .attributes
                .contains(crate::highlight::Attributes::BOLD)
        );
    }

    #[test]
    fn test_status_line_modes() {
        let theme = Theme::dark();
        assert!(theme.statusline.mode.normal.bg.is_some());
        assert!(theme.statusline.mode.insert.bg.is_some());
        assert!(theme.statusline.mode.visual.bg.is_some());
        assert!(theme.statusline.mode.command.bg.is_some());
    }

    #[test]
    fn test_theme_from_name() {
        // All themes should have fg set
        assert!(Theme::from_name(ThemeName::Dark).base.default.fg.is_some());
        assert!(Theme::from_name(ThemeName::Light).base.default.fg.is_some());
    }

    #[test]
    fn test_theme_name_parse() {
        assert_eq!(ThemeName::parse("dark"), Some(ThemeName::Dark));
        assert_eq!(ThemeName::parse("tokyonight"), Some(ThemeName::TokyoNightOrange));
        assert_eq!(ThemeName::parse("invalid"), None);
    }
}
