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
//! - `leap`: Jump navigation styles
//! - `fold`: Code folding styles
//! - `indent`: Indentation guide styles
//! - `scrollbar`: Scrollbar with diagnostic marks
//! - `search`: Search highlight styles
//! - `tab`: Tab line styles
//! - `window`: Window separator styles

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
    pub sign_column: Style,
}

#[derive(Debug, Clone)]
pub struct SelectionStyles {
    pub visual: Style,
    pub block: Style,
}

#[derive(Debug, Clone)]
pub struct StatusLineStyles {
    pub background: Style,
    pub mode: StatusLineModeStyles,
    pub filename: Style,
    pub modified: Style,
    pub position: Style,
    pub filetype: Style,
}

#[derive(Debug, Clone)]
pub struct StatusLineModeStyles {
    pub normal: Style,
    pub insert: Style,
    pub visual: Style,
    pub command: Style,
    pub explorer: Style,
}

#[derive(Debug, Clone)]
pub struct PopupStyles {
    pub normal: Style,
    pub selected: Style,
    pub border: Style,
}

#[derive(Debug, Clone)]
pub struct TelescopeStyles {
    pub border: Style,
    pub normal: Style,
    pub selected: Style,
    pub preview: Style,
    pub preview_highlight: Style,
    pub prompt: Style,
    pub input: Style,
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
pub struct LeapStyles {
    pub label: Style,
    pub match_highlight: Style,
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
    pub telescope: TelescopeStyles,
    pub whichkey: WhichKeyStyles,
    pub leap: LeapStyles,
    pub fold: FoldStyles,
    pub indent: IndentStyles,
    pub scrollbar: ScrollbarStyles,
    pub search: SearchStyles,
    pub tab: TabStyles,
    pub window: WindowStyles,
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
                default: Style::new().fg(fg).bg(bg_dark),
                cursor_line: Style::new().bg(bg_light),
                command_line: Style::new().fg(fg).bg(bg_dark),
            },
            gutter: GutterStyles {
                line_number: Style::new().fg(fg_dark).bg(bg_dark),
                current_line_number: Style::new().fg(yellow).bg(bg_dark).bold(),
                sign_column: Style::new().fg(fg_dark).bg(bg_dark),
            },
            selection: SelectionStyles {
                visual: Style::new().bg(bg_light),
                block: Style::new().bg(bg_light),
            },
            statusline: StatusLineStyles {
                background: Style::new().fg(fg).bg(bg_light),
                mode: StatusLineModeStyles {
                    normal: Style::new().fg(bg_dark).bg(green).bold(),
                    insert: Style::new().fg(bg_dark).bg(blue).bold(),
                    visual: Style::new().fg(bg_dark).bg(magenta).bold(),
                    command: Style::new().fg(bg_dark).bg(yellow).bold(),
                    explorer: Style::new().fg(bg_dark).bg(cyan).bold(),
                },
                filename: Style::new().fg(fg).bg(bg_light),
                modified: Style::new().fg(yellow).bg(bg_light),
                position: Style::new().fg(fg_dark).bg(bg_light),
                filetype: Style::new().fg(cyan).bg(bg_light),
            },
            popup: PopupStyles {
                normal: Style::new().fg(fg).bg(bg_medium),
                selected: Style::new().fg(bg_dark).bg(blue),
                border: Style::new().fg(fg_dark).bg(bg_medium),
            },
            telescope: TelescopeStyles {
                border: Style::new().fg(blue),
                normal: Style::new().fg(fg).bg(bg_medium),
                selected: Style::new().fg(bg_dark).bg(blue),
                preview: Style::new().fg(fg_dark).bg(bg_medium),
                preview_highlight: Style::new().fg(yellow).bg(bg_light),
                prompt: Style::new().fg(blue).bold(),
                input: Style::new().fg(fg),
            },
            whichkey: WhichKeyStyles {
                background: Style::new().bg(bg_medium),
                key: Style::new().fg(yellow).bg(bg_medium).bold(),
                description: Style::new().fg(fg).bg(bg_medium),
                prefix: Style::new().fg(cyan).bg(bg_medium).bold(),
                border: Style::new().fg(fg_dark).bg(bg_medium),
            },
            leap: LeapStyles {
                label: Style::new().fg(bg_dark).bg(yellow).bold(),
                match_highlight: Style::new().fg(magenta).bold(),
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
                default: Style::new().fg(fg_light).bg(bg_light),
                cursor_line: Style::new().bg(bg_highlight),
                command_line: Style::new().fg(fg_light).bg(bg_light),
            },
            gutter: GutterStyles {
                line_number: Style::new().fg(Color::Grey),
                current_line_number: Style::new().fg(Color::DarkBlue).bold(),
                sign_column: Style::new().fg(Color::Grey),
            },
            selection: SelectionStyles {
                visual: Style::new().bg(Color::AnsiValue(153)),
                block: Style::new().bg(Color::AnsiValue(153)),
            },
            statusline: StatusLineStyles {
                background: Style::new().fg(fg_light).bg(bg_medium),
                mode: StatusLineModeStyles {
                    normal: Style::new().fg(Color::White).bg(Color::DarkGreen).bold(),
                    insert: Style::new().fg(Color::White).bg(Color::DarkBlue).bold(),
                    visual: Style::new().fg(Color::White).bg(Color::DarkMagenta).bold(),
                    command: Style::new().fg(Color::Black).bg(Color::DarkYellow).bold(),
                    explorer: Style::new().fg(Color::White).bg(Color::DarkCyan).bold(),
                },
                filename: Style::new().fg(fg_light),
                modified: Style::new().fg(Color::DarkYellow),
                position: Style::new().fg(Color::Grey),
                filetype: Style::new().fg(Color::DarkCyan),
            },
            popup: PopupStyles {
                normal: Style::new().fg(fg_light).bg(Color::AnsiValue(252)),
                selected: Style::new().fg(Color::White).bg(Color::DarkBlue),
                border: Style::new().fg(Color::Grey).bg(Color::AnsiValue(252)),
            },
            telescope: TelescopeStyles {
                border: Style::new().fg(Color::DarkBlue),
                normal: Style::new().fg(fg_light).bg(Color::AnsiValue(254)),
                selected: Style::new().fg(Color::White).bg(Color::DarkBlue),
                preview: Style::new().fg(Color::DarkGrey).bg(Color::AnsiValue(255)),
                preview_highlight: Style::new().fg(Color::DarkBlue).bg(Color::AnsiValue(250)),
                prompt: Style::new().fg(Color::DarkBlue).bold(),
                input: Style::new().fg(fg_light),
            },
            whichkey: WhichKeyStyles {
                background: Style::new().bg(bg_medium),
                key: Style::new().fg(Color::DarkBlue).bg(bg_medium).bold(),
                description: Style::new().fg(fg_light).bg(bg_medium),
                prefix: Style::new().fg(Color::DarkCyan).bg(bg_medium).bold(),
                border: Style::new().fg(Color::Grey).bg(bg_medium),
            },
            leap: LeapStyles {
                label: Style::new().fg(Color::White).bg(Color::DarkYellow).bold(),
                match_highlight: Style::new().fg(Color::DarkMagenta).bold(),
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
        let purple = Color::Rgb {
            r: 187,
            g: 154,
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
                default: Style::new().fg(fg).bg(bg),
                cursor_line: Style::new().bg(bg_highlight),
                command_line: Style::new().fg(fg).bg(bg),
            },
            gutter: GutterStyles {
                line_number: Style::new().fg(fg_dark),
                current_line_number: Style::new().fg(orange).bold(),
                sign_column: Style::new().fg(fg_dark),
            },
            selection: SelectionStyles {
                visual: Style::new().bg(bg_highlight),
                block: Style::new().bg(bg_highlight),
            },
            statusline: StatusLineStyles {
                background: Style::new().fg(fg).bg(bg_dark),
                mode: StatusLineModeStyles {
                    normal: Style::new().fg(bg).bg(green).bold(),
                    insert: Style::new().fg(bg).bg(blue).bold(),
                    visual: Style::new().fg(bg).bg(magenta).bold(),
                    command: Style::new().fg(bg).bg(orange).bold(),
                    explorer: Style::new().fg(bg).bg(cyan).bold(),
                },
                filename: Style::new().fg(fg),
                modified: Style::new().fg(orange),
                position: Style::new().fg(fg_dark),
                filetype: Style::new().fg(cyan),
            },
            popup: PopupStyles {
                normal: Style::new().fg(fg).bg(bg_dark),
                selected: Style::new().fg(bg).bg(blue),
                border: Style::new().fg(fg_dark).bg(bg_dark),
            },
            telescope: TelescopeStyles {
                border: Style::new().fg(blue),
                normal: Style::new().fg(fg).bg(bg_dark),
                selected: Style::new().fg(bg).bg(blue),
                preview: Style::new().fg(comment).bg(bg_dark),
                preview_highlight: Style::new().fg(orange).bg(bg_highlight),
                prompt: Style::new().fg(orange).bold(),
                input: Style::new().fg(fg),
            },
            whichkey: WhichKeyStyles {
                background: Style::new().bg(bg_dark),
                key: Style::new().fg(orange).bg(bg_dark).bold(),
                description: Style::new().fg(fg).bg(bg_dark),
                prefix: Style::new().fg(cyan).bg(bg_dark).bold(),
                border: Style::new().fg(fg_dark).bg(bg_dark),
            },
            leap: LeapStyles {
                label: Style::new().fg(bg).bg(orange).bold(),
                match_highlight: Style::new().fg(purple).bold(),
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
        assert!(theme.base.default.bg.is_some());
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
        assert!(theme.statusline.mode.explorer.bg.is_some());
    }

    #[test]
    fn test_theme_from_name() {
        // All themes should have fg and bg set for proper diff-based rendering
        assert!(Theme::from_name(ThemeName::Dark).base.default.fg.is_some());
        assert!(Theme::from_name(ThemeName::Dark).base.default.bg.is_some());
        assert!(Theme::from_name(ThemeName::Light).base.default.fg.is_some());
        assert!(Theme::from_name(ThemeName::Light).base.default.bg.is_some());
    }

    #[test]
    fn test_theme_name_parse() {
        assert_eq!(ThemeName::parse("dark"), Some(ThemeName::Dark));
        assert_eq!(ThemeName::parse("tokyonight"), Some(ThemeName::TokyoNightOrange));
        assert_eq!(ThemeName::parse("invalid"), None);
    }
}
