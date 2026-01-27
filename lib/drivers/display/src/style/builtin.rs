//! Built-in theme color definitions.
//!
//! Each theme provides complete color definitions for all 34 built-in highlight groups.
//! Uses `std::sync::LazyLock` for efficient one-time initialization.
//!
//! # Architecture
//!
//! This is part of the **mechanism layer**. It provides the actual color data
//! that themes need, but doesn't decide WHEN to use these colors (that's policy).
//!
//! Module-specific groups (e.g., rainbow brackets) are registered by modules
//! via `StyleGroupRegistry` at runtime.
//!
//! # Available Themes
//!
//! - **Dark** (default): OneDark-inspired dark theme
//! - **Light**: High-contrast light theme
//! - **`TokyoNightOrange`**: Tokyo Night variant with orange accents

use std::{collections::HashMap, sync::LazyLock};

use reovim_arch::Color;

use {super::groups, crate::highlight::Style};

// =============================================================================
// Dark Theme (OneDark-inspired)
// =============================================================================

/// Dark theme palette (OneDark-inspired).
///
/// Base colors:
/// - Background: #282c34 (Dark grey-blue)
/// - Foreground: #abb2bf (Light grey)
static DARK_PALETTE: LazyLock<HashMap<&'static str, Style>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // -------------------------------------------------------------------------
    // Syntax colors (13 groups)
    // -------------------------------------------------------------------------
    m.insert(
        groups::KEYWORD,
        Style::new()
            .fg(Color::Rgb {
                r: 198,
                g: 120,
                b: 221,
            }) // #c678dd Purple
            .bold(),
    );
    m.insert(
        groups::FUNCTION,
        Style::new().fg(Color::Rgb {
            r: 97,
            g: 175,
            b: 239,
        }), // #61afef Blue
    );
    m.insert(
        groups::TYPE,
        Style::new().fg(Color::Rgb {
            r: 229,
            g: 192,
            b: 123,
        }), // #e5c07b Yellow
    );
    m.insert(
        groups::STRING,
        Style::new().fg(Color::Rgb {
            r: 152,
            g: 195,
            b: 121,
        }), // #98c379 Green
    );
    m.insert(
        groups::NUMBER,
        Style::new().fg(Color::Rgb {
            r: 209,
            g: 154,
            b: 102,
        }), // #d19a66 Orange
    );
    m.insert(
        groups::COMMENT,
        Style::new()
            .fg(Color::Rgb {
                r: 92,
                g: 99,
                b: 112,
            }) // #5c6370 Grey
            .italic(),
    );
    m.insert(
        groups::OPERATOR,
        Style::new().fg(Color::Rgb {
            r: 86,
            g: 182,
            b: 194,
        }), // #56b6c2 Cyan
    );
    m.insert(
        groups::PUNCTUATION,
        Style::new().fg(Color::Rgb {
            r: 171,
            g: 178,
            b: 191,
        }), // #abb2bf Foreground
    );
    m.insert(
        groups::VARIABLE,
        Style::new().fg(Color::Rgb {
            r: 224,
            g: 108,
            b: 117,
        }), // #e06c75 Red
    );
    m.insert(
        groups::CONSTANT,
        Style::new()
            .fg(Color::Rgb {
                r: 209,
                g: 154,
                b: 102,
            }) // #d19a66 Orange
            .bold(),
    );
    m.insert(
        groups::ATTRIBUTE,
        Style::new().fg(Color::Rgb {
            r: 229,
            g: 192,
            b: 123,
        }), // #e5c07b Yellow
    );
    m.insert(
        groups::PROPERTY,
        Style::new().fg(Color::Rgb {
            r: 224,
            g: 108,
            b: 117,
        }), // #e06c75 Red
    );
    m.insert(
        groups::TAG,
        Style::new().fg(Color::Rgb {
            r: 224,
            g: 108,
            b: 117,
        }), // #e06c75 Red
    );

    // -------------------------------------------------------------------------
    // UI colors (17 groups)
    // -------------------------------------------------------------------------
    m.insert(
        groups::BACKGROUND,
        Style::new().bg(Color::Rgb {
            r: 40,
            g: 44,
            b: 52,
        }), // #282c34
    );
    m.insert(
        groups::FOREGROUND,
        Style::new().fg(Color::Rgb {
            r: 171,
            g: 178,
            b: 191,
        }), // #abb2bf
    );
    m.insert(
        groups::CURSOR,
        Style::new()
            .bg(Color::Rgb {
                r: 97,
                g: 175,
                b: 239,
            }) // #61afef Blue
            .fg(Color::Rgb {
                r: 40,
                g: 44,
                b: 52,
            }), // #282c34
    );
    m.insert(
        groups::SELECTION,
        Style::new().bg(Color::Rgb {
            r: 62,
            g: 68,
            b: 81,
        }), // #3e4451
    );
    m.insert(
        groups::LINE_NUMBER,
        Style::new().fg(Color::Rgb {
            r: 76,
            g: 82,
            b: 99,
        }), // #4b5263
    );
    m.insert(
        groups::LINE_NUMBER_ACTIVE,
        Style::new().fg(Color::Rgb {
            r: 171,
            g: 178,
            b: 191,
        }), // #abb2bf
    );

    // Statusline
    m.insert(
        groups::STATUSLINE_BG,
        Style::new().bg(Color::Rgb {
            r: 33,
            g: 37,
            b: 43,
        }), // #21252b
    );
    m.insert(
        groups::STATUSLINE_FG,
        Style::new().fg(Color::Rgb {
            r: 171,
            g: 178,
            b: 191,
        }), // #abb2bf
    );

    // Mode indicators
    m.insert(
        groups::MODE_NORMAL,
        Style::new()
            .fg(Color::Rgb {
                r: 40,
                g: 44,
                b: 52,
            })
            .bg(Color::Rgb {
                r: 97,
                g: 175,
                b: 239,
            }) // Blue
            .bold(),
    );
    m.insert(
        groups::MODE_INSERT,
        Style::new()
            .fg(Color::Rgb {
                r: 40,
                g: 44,
                b: 52,
            })
            .bg(Color::Rgb {
                r: 152,
                g: 195,
                b: 121,
            }) // Green
            .bold(),
    );
    m.insert(
        groups::MODE_VISUAL,
        Style::new()
            .fg(Color::Rgb {
                r: 40,
                g: 44,
                b: 52,
            })
            .bg(Color::Rgb {
                r: 198,
                g: 120,
                b: 221,
            }) // Purple
            .bold(),
    );
    m.insert(
        groups::MODE_COMMAND,
        Style::new()
            .fg(Color::Rgb {
                r: 40,
                g: 44,
                b: 52,
            })
            .bg(Color::Rgb {
                r: 229,
                g: 192,
                b: 123,
            }) // Yellow
            .bold(),
    );

    // UI elements
    m.insert(
        groups::BORDER,
        Style::new().fg(Color::Rgb {
            r: 62,
            g: 68,
            b: 81,
        }), // #3e4451
    );
    m.insert(
        groups::POPUP_BG,
        Style::new().bg(Color::Rgb {
            r: 33,
            g: 37,
            b: 43,
        }), // #21252b
    );
    m.insert(
        groups::POPUP_FG,
        Style::new().fg(Color::Rgb {
            r: 171,
            g: 178,
            b: 191,
        }), // #abb2bf
    );
    m.insert(
        groups::MENU_SELECTED,
        Style::new().bg(Color::Rgb {
            r: 62,
            g: 68,
            b: 81,
        }), // #3e4451
    );
    m.insert(
        groups::SEARCH_MATCH,
        Style::new()
            .bg(Color::Rgb {
                r: 229,
                g: 192,
                b: 123,
            }) // Yellow
            .fg(Color::Rgb {
                r: 40,
                g: 44,
                b: 52,
            }), // Dark bg
    );

    // -------------------------------------------------------------------------
    // Diagnostics (4 groups)
    // -------------------------------------------------------------------------
    m.insert(
        groups::DIAGNOSTIC_ERROR,
        Style::new().fg(Color::Rgb {
            r: 224,
            g: 108,
            b: 117,
        }), // Red
    );
    m.insert(
        groups::DIAGNOSTIC_WARN,
        Style::new().fg(Color::Rgb {
            r: 229,
            g: 192,
            b: 123,
        }), // Yellow
    );
    m.insert(
        groups::DIAGNOSTIC_INFO,
        Style::new().fg(Color::Rgb {
            r: 97,
            g: 175,
            b: 239,
        }), // Blue
    );
    m.insert(
        groups::DIAGNOSTIC_HINT,
        Style::new().fg(Color::Rgb {
            r: 86,
            g: 182,
            b: 194,
        }), // Cyan
    );

    m
});

// =============================================================================
// Light Theme
// =============================================================================

/// Light theme palette.
///
/// Base colors:
/// - Background: #fafafa (Almost white)
/// - Foreground: #383a42 (Dark grey)
static LIGHT_PALETTE: LazyLock<HashMap<&'static str, Style>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // -------------------------------------------------------------------------
    // Syntax colors (13 groups)
    // -------------------------------------------------------------------------
    m.insert(
        groups::KEYWORD,
        Style::new()
            .fg(Color::Rgb {
                r: 166,
                g: 38,
                b: 164,
            }) // #a626a4 Magenta
            .bold(),
    );
    m.insert(
        groups::FUNCTION,
        Style::new().fg(Color::Rgb {
            r: 64,
            g: 120,
            b: 242,
        }), // #4078f2 Blue
    );
    m.insert(
        groups::TYPE,
        Style::new().fg(Color::Rgb {
            r: 193,
            g: 132,
            b: 1,
        }), // #c18401 Yellow
    );
    m.insert(
        groups::STRING,
        Style::new().fg(Color::Rgb {
            r: 80,
            g: 161,
            b: 79,
        }), // #50a14f Green
    );
    m.insert(
        groups::NUMBER,
        Style::new().fg(Color::Rgb {
            r: 152,
            g: 104,
            b: 1,
        }), // #986801 Orange
    );
    m.insert(
        groups::COMMENT,
        Style::new()
            .fg(Color::Rgb {
                r: 160,
                g: 161,
                b: 167,
            }) // #a0a1a7 Grey
            .italic(),
    );
    m.insert(
        groups::OPERATOR,
        Style::new().fg(Color::Rgb {
            r: 1,
            g: 132,
            b: 188,
        }), // #0184bc Cyan
    );
    m.insert(
        groups::PUNCTUATION,
        Style::new().fg(Color::Rgb {
            r: 56,
            g: 58,
            b: 66,
        }), // #383a42 Foreground
    );
    m.insert(
        groups::VARIABLE,
        Style::new().fg(Color::Rgb {
            r: 228,
            g: 86,
            b: 73,
        }), // #e45649 Red
    );
    m.insert(
        groups::CONSTANT,
        Style::new()
            .fg(Color::Rgb {
                r: 152,
                g: 104,
                b: 1,
            }) // #986801 Orange
            .bold(),
    );
    m.insert(
        groups::ATTRIBUTE,
        Style::new().fg(Color::Rgb {
            r: 193,
            g: 132,
            b: 1,
        }), // #c18401 Yellow
    );
    m.insert(
        groups::PROPERTY,
        Style::new().fg(Color::Rgb {
            r: 228,
            g: 86,
            b: 73,
        }), // #e45649 Red
    );
    m.insert(
        groups::TAG,
        Style::new().fg(Color::Rgb {
            r: 228,
            g: 86,
            b: 73,
        }), // #e45649 Red
    );

    // -------------------------------------------------------------------------
    // UI colors (17 groups)
    // -------------------------------------------------------------------------
    m.insert(
        groups::BACKGROUND,
        Style::new().bg(Color::Rgb {
            r: 250,
            g: 250,
            b: 250,
        }), // #fafafa
    );
    m.insert(
        groups::FOREGROUND,
        Style::new().fg(Color::Rgb {
            r: 56,
            g: 58,
            b: 66,
        }), // #383a42
    );
    m.insert(
        groups::CURSOR,
        Style::new()
            .bg(Color::Rgb {
                r: 64,
                g: 120,
                b: 242,
            }) // Blue
            .fg(Color::Rgb {
                r: 250,
                g: 250,
                b: 250,
            }), // White
    );
    m.insert(
        groups::SELECTION,
        Style::new().bg(Color::Rgb {
            r: 216,
            g: 222,
            b: 233,
        }), // Light blue-grey
    );
    m.insert(
        groups::LINE_NUMBER,
        Style::new().fg(Color::Rgb {
            r: 160,
            g: 161,
            b: 167,
        }), // Grey
    );
    m.insert(
        groups::LINE_NUMBER_ACTIVE,
        Style::new().fg(Color::Rgb {
            r: 56,
            g: 58,
            b: 66,
        }), // Dark
    );

    // Statusline
    m.insert(
        groups::STATUSLINE_BG,
        Style::new().bg(Color::Rgb {
            r: 233,
            g: 233,
            b: 236,
        }), // #e9e9ec
    );
    m.insert(
        groups::STATUSLINE_FG,
        Style::new().fg(Color::Rgb {
            r: 56,
            g: 58,
            b: 66,
        }), // #383a42
    );

    // Mode indicators
    m.insert(
        groups::MODE_NORMAL,
        Style::new()
            .fg(Color::Rgb {
                r: 250,
                g: 250,
                b: 250,
            })
            .bg(Color::Rgb {
                r: 64,
                g: 120,
                b: 242,
            }) // Blue
            .bold(),
    );
    m.insert(
        groups::MODE_INSERT,
        Style::new()
            .fg(Color::Rgb {
                r: 250,
                g: 250,
                b: 250,
            })
            .bg(Color::Rgb {
                r: 80,
                g: 161,
                b: 79,
            }) // Green
            .bold(),
    );
    m.insert(
        groups::MODE_VISUAL,
        Style::new()
            .fg(Color::Rgb {
                r: 250,
                g: 250,
                b: 250,
            })
            .bg(Color::Rgb {
                r: 166,
                g: 38,
                b: 164,
            }) // Magenta
            .bold(),
    );
    m.insert(
        groups::MODE_COMMAND,
        Style::new()
            .fg(Color::Rgb {
                r: 56,
                g: 58,
                b: 66,
            })
            .bg(Color::Rgb {
                r: 193,
                g: 132,
                b: 1,
            }) // Yellow
            .bold(),
    );

    // UI elements
    m.insert(
        groups::BORDER,
        Style::new().fg(Color::Rgb {
            r: 216,
            g: 222,
            b: 233,
        }), // Light border
    );
    m.insert(
        groups::POPUP_BG,
        Style::new().bg(Color::Rgb {
            r: 233,
            g: 233,
            b: 236,
        }), // #e9e9ec
    );
    m.insert(
        groups::POPUP_FG,
        Style::new().fg(Color::Rgb {
            r: 56,
            g: 58,
            b: 66,
        }), // #383a42
    );
    m.insert(
        groups::MENU_SELECTED,
        Style::new().bg(Color::Rgb {
            r: 216,
            g: 222,
            b: 233,
        }), // Light blue-grey
    );
    m.insert(
        groups::SEARCH_MATCH,
        Style::new()
            .bg(Color::Rgb {
                r: 193,
                g: 132,
                b: 1,
            }) // Yellow
            .fg(Color::Rgb {
                r: 250,
                g: 250,
                b: 250,
            }), // White
    );

    // -------------------------------------------------------------------------
    // Diagnostics (4 groups)
    // -------------------------------------------------------------------------
    m.insert(
        groups::DIAGNOSTIC_ERROR,
        Style::new().fg(Color::Rgb {
            r: 228,
            g: 86,
            b: 73,
        }), // Red
    );
    m.insert(
        groups::DIAGNOSTIC_WARN,
        Style::new().fg(Color::Rgb {
            r: 193,
            g: 132,
            b: 1,
        }), // Yellow
    );
    m.insert(
        groups::DIAGNOSTIC_INFO,
        Style::new().fg(Color::Rgb {
            r: 64,
            g: 120,
            b: 242,
        }), // Blue
    );
    m.insert(
        groups::DIAGNOSTIC_HINT,
        Style::new().fg(Color::Rgb {
            r: 1,
            g: 132,
            b: 188,
        }), // Cyan
    );

    m
});

// =============================================================================
// Tokyo Night Orange Theme
// =============================================================================

/// Tokyo Night Orange theme palette.
///
/// Based on Tokyo Night with orange accent.
/// Background: #1a1b26
static TOKYO_NIGHT_ORANGE_PALETTE: LazyLock<HashMap<&'static str, Style>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // -------------------------------------------------------------------------
    // Syntax colors (13 groups)
    // -------------------------------------------------------------------------
    m.insert(
        groups::KEYWORD,
        Style::new()
            .fg(Color::Rgb {
                r: 187,
                g: 154,
                b: 247,
            }) // #bb9af7 Purple
            .bold(),
    );
    m.insert(
        groups::FUNCTION,
        Style::new().fg(Color::Rgb {
            r: 122,
            g: 162,
            b: 247,
        }), // #7aa2f7 Blue
    );
    m.insert(
        groups::TYPE,
        Style::new().fg(Color::Rgb {
            r: 255,
            g: 158,
            b: 100,
        }), // #ff9e64 Orange
    );
    m.insert(
        groups::STRING,
        Style::new().fg(Color::Rgb {
            r: 158,
            g: 206,
            b: 106,
        }), // #9ece6a Green
    );
    m.insert(
        groups::NUMBER,
        Style::new().fg(Color::Rgb {
            r: 255,
            g: 158,
            b: 100,
        }), // #ff9e64 Orange
    );
    m.insert(
        groups::COMMENT,
        Style::new()
            .fg(Color::Rgb {
                r: 86,
                g: 95,
                b: 137,
            }) // #565f89 Grey
            .italic(),
    );
    m.insert(
        groups::OPERATOR,
        Style::new().fg(Color::Rgb {
            r: 137,
            g: 221,
            b: 255,
        }), // #89ddff Cyan
    );
    m.insert(
        groups::PUNCTUATION,
        Style::new().fg(Color::Rgb {
            r: 169,
            g: 177,
            b: 214,
        }), // #a9b1d6 Foreground
    );
    m.insert(
        groups::VARIABLE,
        Style::new().fg(Color::Rgb {
            r: 247,
            g: 118,
            b: 142,
        }), // #f7768e Red
    );
    m.insert(
        groups::CONSTANT,
        Style::new()
            .fg(Color::Rgb {
                r: 255,
                g: 158,
                b: 100,
            }) // #ff9e64 Orange
            .bold(),
    );
    m.insert(
        groups::ATTRIBUTE,
        Style::new().fg(Color::Rgb {
            r: 224,
            g: 175,
            b: 104,
        }), // #e0af68 Yellow
    );
    m.insert(
        groups::PROPERTY,
        Style::new().fg(Color::Rgb {
            r: 115,
            g: 218,
            b: 202,
        }), // #73daca Teal
    );
    m.insert(
        groups::TAG,
        Style::new().fg(Color::Rgb {
            r: 247,
            g: 118,
            b: 142,
        }), // #f7768e Red
    );

    // -------------------------------------------------------------------------
    // UI colors (17 groups)
    // -------------------------------------------------------------------------
    m.insert(
        groups::BACKGROUND,
        Style::new().bg(Color::Rgb {
            r: 26,
            g: 27,
            b: 38,
        }), // #1a1b26
    );
    m.insert(
        groups::FOREGROUND,
        Style::new().fg(Color::Rgb {
            r: 169,
            g: 177,
            b: 214,
        }), // #a9b1d6
    );
    m.insert(
        groups::CURSOR,
        Style::new()
            .bg(Color::Rgb {
                r: 255,
                g: 158,
                b: 100,
            }) // Orange
            .fg(Color::Rgb {
                r: 26,
                g: 27,
                b: 38,
            }), // Dark bg
    );
    m.insert(
        groups::SELECTION,
        Style::new().bg(Color::Rgb {
            r: 40,
            g: 52,
            b: 87,
        }), // #283457
    );
    m.insert(
        groups::LINE_NUMBER,
        Style::new().fg(Color::Rgb {
            r: 59,
            g: 66,
            b: 97,
        }), // #3b4261
    );
    m.insert(
        groups::LINE_NUMBER_ACTIVE,
        Style::new().fg(Color::Rgb {
            r: 255,
            g: 158,
            b: 100,
        }), // Orange
    );

    // Statusline
    m.insert(
        groups::STATUSLINE_BG,
        Style::new().bg(Color::Rgb {
            r: 22,
            g: 22,
            b: 30,
        }), // #16161e
    );
    m.insert(
        groups::STATUSLINE_FG,
        Style::new().fg(Color::Rgb {
            r: 169,
            g: 177,
            b: 214,
        }), // #a9b1d6
    );

    // Mode indicators (Orange theme accents)
    m.insert(
        groups::MODE_NORMAL,
        Style::new()
            .fg(Color::Rgb {
                r: 26,
                g: 27,
                b: 38,
            })
            .bg(Color::Rgb {
                r: 255,
                g: 158,
                b: 100,
            }) // Orange
            .bold(),
    );
    m.insert(
        groups::MODE_INSERT,
        Style::new()
            .fg(Color::Rgb {
                r: 26,
                g: 27,
                b: 38,
            })
            .bg(Color::Rgb {
                r: 158,
                g: 206,
                b: 106,
            }) // Green
            .bold(),
    );
    m.insert(
        groups::MODE_VISUAL,
        Style::new()
            .fg(Color::Rgb {
                r: 26,
                g: 27,
                b: 38,
            })
            .bg(Color::Rgb {
                r: 187,
                g: 154,
                b: 247,
            }) // Purple
            .bold(),
    );
    m.insert(
        groups::MODE_COMMAND,
        Style::new()
            .fg(Color::Rgb {
                r: 26,
                g: 27,
                b: 38,
            })
            .bg(Color::Rgb {
                r: 224,
                g: 175,
                b: 104,
            }) // Yellow
            .bold(),
    );

    // UI elements
    m.insert(
        groups::BORDER,
        Style::new().fg(Color::Rgb {
            r: 59,
            g: 66,
            b: 97,
        }), // #3b4261
    );
    m.insert(
        groups::POPUP_BG,
        Style::new().bg(Color::Rgb {
            r: 22,
            g: 22,
            b: 30,
        }), // #16161e
    );
    m.insert(
        groups::POPUP_FG,
        Style::new().fg(Color::Rgb {
            r: 169,
            g: 177,
            b: 214,
        }), // #a9b1d6
    );
    m.insert(
        groups::MENU_SELECTED,
        Style::new().bg(Color::Rgb {
            r: 40,
            g: 52,
            b: 87,
        }), // #283457
    );
    m.insert(
        groups::SEARCH_MATCH,
        Style::new()
            .bg(Color::Rgb {
                r: 255,
                g: 158,
                b: 100,
            }) // Orange
            .fg(Color::Rgb {
                r: 26,
                g: 27,
                b: 38,
            }), // Dark bg
    );

    // -------------------------------------------------------------------------
    // Diagnostics (4 groups)
    // -------------------------------------------------------------------------
    m.insert(
        groups::DIAGNOSTIC_ERROR,
        Style::new().fg(Color::Rgb {
            r: 247,
            g: 118,
            b: 142,
        }), // Red
    );
    m.insert(
        groups::DIAGNOSTIC_WARN,
        Style::new().fg(Color::Rgb {
            r: 224,
            g: 175,
            b: 104,
        }), // Yellow
    );
    m.insert(
        groups::DIAGNOSTIC_INFO,
        Style::new().fg(Color::Rgb {
            r: 122,
            g: 162,
            b: 247,
        }), // Blue
    );
    m.insert(
        groups::DIAGNOSTIC_HINT,
        Style::new().fg(Color::Rgb {
            r: 115,
            g: 218,
            b: 202,
        }), // Teal
    );

    m
});

// =============================================================================
// Public API
// =============================================================================

/// Get the palette for a builtin theme variant.
///
/// Returns a reference to the lazily-initialized palette `HashMap`.
pub fn get_palette(variant: super::BuiltinTheme) -> &'static HashMap<&'static str, Style> {
    match variant {
        super::BuiltinTheme::Dark => &DARK_PALETTE,
        super::BuiltinTheme::Light => &LIGHT_PALETTE,
        super::BuiltinTheme::TokyoNightOrange => &TOKYO_NIGHT_ORANGE_PALETTE,
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::highlight::Attributes};

    #[test]
    fn test_dark_palette_all_groups() {
        let palette = &DARK_PALETTE;
        for group in groups::ALL_GROUPS {
            assert!(palette.contains_key(group), "Dark palette missing group: {group}");
        }
        assert_eq!(palette.len(), groups::ALL_GROUPS.len());
    }

    #[test]
    fn test_light_palette_all_groups() {
        let palette = &LIGHT_PALETTE;
        for group in groups::ALL_GROUPS {
            assert!(palette.contains_key(group), "Light palette missing group: {group}");
        }
        assert_eq!(palette.len(), groups::ALL_GROUPS.len());
    }

    #[test]
    fn test_tokyo_night_palette_all_groups() {
        let palette = &TOKYO_NIGHT_ORANGE_PALETTE;
        for group in groups::ALL_GROUPS {
            assert!(palette.contains_key(group), "TokyoNightOrange palette missing group: {group}");
        }
        assert_eq!(palette.len(), groups::ALL_GROUPS.len());
    }

    #[test]
    fn test_get_palette_returns_correct_variant() {
        let dark = get_palette(super::super::BuiltinTheme::Dark);
        let light = get_palette(super::super::BuiltinTheme::Light);
        let tokyo = get_palette(super::super::BuiltinTheme::TokyoNightOrange);

        // Each should be different (they have different colors)
        let dark_keyword = dark.get(groups::KEYWORD).unwrap();
        let light_keyword = light.get(groups::KEYWORD).unwrap();
        let tokyo_keyword = tokyo.get(groups::KEYWORD).unwrap();

        assert_ne!(dark_keyword.fg, light_keyword.fg);
        assert_ne!(dark_keyword.fg, tokyo_keyword.fg);
    }

    #[test]
    fn test_dark_theme_has_bold_keywords() {
        let palette = &DARK_PALETTE;
        let keyword = palette.get(groups::KEYWORD).unwrap();
        assert!(keyword.attributes.contains(Attributes::BOLD));
    }

    #[test]
    fn test_all_themes_have_italic_comments() {
        for variant in super::super::BuiltinTheme::all() {
            let palette = get_palette(*variant);
            let comment = palette.get(groups::COMMENT).unwrap();
            assert!(
                comment.attributes.contains(Attributes::ITALIC),
                "{} theme should have italic comments",
                variant.name()
            );
        }
    }
}
