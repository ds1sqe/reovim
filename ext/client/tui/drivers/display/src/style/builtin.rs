//! Built-in theme color definitions.
//!
//! Each theme provides complete color definitions for all 42 built-in highlight groups.
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
    // Syntax sub-categories (30 groups) - visually distinct variants
    // -------------------------------------------------------------------------
    // keyword.* -- inherit purple from KEYWORD unless distinct
    m.insert(
        groups::KEYWORD_CONTROL,
        Style::new()
            .fg(Color::Rgb {
                r: 198,
                g: 120,
                b: 221,
            })
            .bold(),
    ); // Purple (same)
    m.insert(
        groups::KEYWORD_FUNCTION,
        Style::new()
            .fg(Color::Rgb {
                r: 198,
                g: 120,
                b: 221,
            })
            .bold(),
    ); // Purple (same)
    m.insert(
        groups::KEYWORD_TYPE,
        Style::new()
            .fg(Color::Rgb {
                r: 198,
                g: 120,
                b: 221,
            })
            .bold(),
    ); // Purple (same)
    m.insert(
        groups::KEYWORD_OPERATOR,
        Style::new().fg(Color::Rgb {
            r: 198,
            g: 120,
            b: 221,
        }),
    ); // Purple (no bold)
    // type.builtin -- italic to distinguish from user types
    m.insert(
        groups::TYPE_BUILTIN,
        Style::new()
            .fg(Color::Rgb {
                r: 229,
                g: 192,
                b: 123,
            })
            .italic(),
    ); // Yellow italic
    // function.* -- blue family
    m.insert(
        groups::FUNCTION_BUILTIN,
        Style::new()
            .fg(Color::Rgb {
                r: 97,
                g: 175,
                b: 239,
            })
            .bold(),
    ); // Blue bold
    m.insert(
        groups::FUNCTION_MACRO,
        Style::new()
            .fg(Color::Rgb {
                r: 86,
                g: 182,
                b: 194,
            })
            .italic(),
    ); // Cyan italic
    m.insert(
        groups::FUNCTION_METHOD,
        Style::new().fg(Color::Rgb {
            r: 97,
            g: 175,
            b: 239,
        }),
    ); // Blue (same)
    // variable.* -- red family
    m.insert(
        groups::VARIABLE_BUILTIN,
        Style::new().fg(Color::Rgb {
            r: 209,
            g: 154,
            b: 102,
        }),
    ); // Orange
    m.insert(
        groups::VARIABLE_PARAMETER,
        Style::new()
            .fg(Color::Rgb {
                r: 224,
                g: 108,
                b: 117,
            })
            .italic(),
    ); // Red italic
    m.insert(
        groups::VARIABLE_FIELD,
        Style::new().fg(Color::Rgb {
            r: 224,
            g: 108,
            b: 117,
        }),
    ); // Red (same as VARIABLE)
    // string.escape -- orange to stand out in green strings
    m.insert(
        groups::STRING_ESCAPE,
        Style::new().fg(Color::Rgb {
            r: 209,
            g: 154,
            b: 102,
        }),
    ); // Orange
    // comment.doc -- slightly brighter grey, bold
    m.insert(
        groups::COMMENT_DOC,
        Style::new()
            .fg(Color::Rgb {
                r: 108,
                g: 115,
                b: 128,
            })
            .italic()
            .bold(),
    ); // Brighter grey
    // punctuation.* -- foreground color
    m.insert(
        groups::PUNCTUATION_BRACKET,
        Style::new().fg(Color::Rgb {
            r: 171,
            g: 178,
            b: 191,
        }),
    ); // Foreground
    m.insert(
        groups::PUNCTUATION_DELIMITER,
        Style::new().fg(Color::Rgb {
            r: 171,
            g: 178,
            b: 191,
        }),
    ); // Foreground
    // standalone sub-categories
    m.insert(
        groups::NAMESPACE,
        Style::new().fg(Color::Rgb {
            r: 97,
            g: 175,
            b: 239,
        }),
    ); // Blue
    m.insert(
        groups::CONSTRUCTOR,
        Style::new()
            .fg(Color::Rgb {
                r: 229,
                g: 192,
                b: 123,
            })
            .bold(),
    ); // Yellow bold
    m.insert(
        groups::LABEL,
        Style::new().fg(Color::Rgb {
            r: 198,
            g: 120,
            b: 221,
        }),
    ); // Purple
    m.insert(
        groups::BOOLEAN,
        Style::new().fg(Color::Rgb {
            r: 209,
            g: 154,
            b: 102,
        }),
    ); // Orange
    m.insert(
        groups::CHARACTER,
        Style::new().fg(Color::Rgb {
            r: 152,
            g: 195,
            b: 121,
        }),
    ); // Green
    // markup.* -- document formatting
    m.insert(
        groups::MARKUP_HEADING,
        Style::new()
            .fg(Color::Rgb {
                r: 97,
                g: 175,
                b: 239,
            })
            .bold(),
    ); // Blue bold
    m.insert(groups::MARKUP_BOLD, Style::new().bold());
    m.insert(groups::MARKUP_ITALIC, Style::new().italic());
    m.insert(
        groups::MARKUP_STRIKETHROUGH,
        Style::new().fg(Color::Rgb {
            r: 92,
            g: 99,
            b: 112,
        }),
    ); // Grey
    m.insert(
        groups::MARKUP_LINK,
        Style::new()
            .fg(Color::Rgb {
                r: 86,
                g: 182,
                b: 194,
            })
            .underline(),
    ); // Cyan underline
    m.insert(
        groups::MARKUP_LINK_URL,
        Style::new().fg(Color::Rgb {
            r: 86,
            g: 182,
            b: 194,
        }),
    ); // Cyan
    m.insert(
        groups::MARKUP_LIST,
        Style::new().fg(Color::Rgb {
            r: 198,
            g: 120,
            b: 221,
        }),
    ); // Purple
    m.insert(
        groups::MARKUP_RAW,
        Style::new().fg(Color::Rgb {
            r: 152,
            g: 195,
            b: 121,
        }),
    ); // Green
    m.insert(
        groups::MARKUP_RAW_INLINE,
        Style::new().fg(Color::Rgb {
            r: 152,
            g: 195,
            b: 121,
        }),
    ); // Green
    m.insert(
        groups::EMBEDDED,
        Style::new().fg(Color::Rgb {
            r: 171,
            g: 178,
            b: 191,
        }),
    ); // Foreground
    m.insert(
        groups::SPECIAL,
        Style::new().fg(Color::Rgb {
            r: 86,
            g: 182,
            b: 194,
        }),
    ); // Cyan

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
    m.insert(
        groups::MODE_REPLACE,
        Style::new()
            .fg(Color::Rgb {
                r: 40,
                g: 44,
                b: 52,
            })
            .bg(Color::Rgb {
                r: 224,
                g: 108,
                b: 117,
            }) // Red
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

    // -------------------------------------------------------------------------
    // Gutter/Annotation colors (8 groups) - Issue #455
    // -------------------------------------------------------------------------
    m.insert(groups::SIGN_COLUMN, Style::new()); // Inherit background
    m.insert(
        groups::GUTTER_SEPARATOR,
        Style::new().fg(Color::Rgb {
            r: 60,
            g: 64,
            b: 72,
        }), // #3c4048 Subtle separator
    );
    m.insert(
        groups::GIT_ADD,
        Style::new().fg(Color::Rgb {
            r: 152,
            g: 195,
            b: 121,
        }), // #98c379 Green (matches STRING)
    );
    m.insert(
        groups::GIT_CHANGE,
        Style::new().fg(Color::Rgb {
            r: 229,
            g: 192,
            b: 123,
        }), // #e5c07b Yellow (matches TYPE)
    );
    m.insert(
        groups::GIT_DELETE,
        Style::new().fg(Color::Rgb {
            r: 224,
            g: 108,
            b: 117,
        }), // #e06c75 Red (matches DIAGNOSTIC_ERROR)
    );
    m.insert(
        groups::FOLD_OPEN,
        Style::new().fg(Color::Rgb {
            r: 92,
            g: 99,
            b: 112,
        }), // #5c6370 Grey (matches COMMENT)
    );
    m.insert(
        groups::FOLD_CLOSED,
        Style::new().fg(Color::Rgb {
            r: 97,
            g: 175,
            b: 239,
        }), // #61afef Blue (matches FUNCTION)
    );
    m.insert(
        groups::BOOKMARK,
        Style::new().fg(Color::Rgb {
            r: 198,
            g: 120,
            b: 221,
        }), // #c678dd Purple (matches KEYWORD)
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
    // Syntax sub-categories (30 groups) - Light theme variants
    // -------------------------------------------------------------------------
    m.insert(
        groups::KEYWORD_CONTROL,
        Style::new()
            .fg(Color::Rgb {
                r: 166,
                g: 38,
                b: 164,
            })
            .bold(),
    ); // Magenta
    m.insert(
        groups::KEYWORD_FUNCTION,
        Style::new()
            .fg(Color::Rgb {
                r: 166,
                g: 38,
                b: 164,
            })
            .bold(),
    ); // Magenta
    m.insert(
        groups::KEYWORD_TYPE,
        Style::new()
            .fg(Color::Rgb {
                r: 166,
                g: 38,
                b: 164,
            })
            .bold(),
    ); // Magenta
    m.insert(
        groups::KEYWORD_OPERATOR,
        Style::new().fg(Color::Rgb {
            r: 166,
            g: 38,
            b: 164,
        }),
    ); // Magenta (no bold)
    m.insert(
        groups::TYPE_BUILTIN,
        Style::new()
            .fg(Color::Rgb {
                r: 193,
                g: 132,
                b: 1,
            })
            .italic(),
    ); // Yellow italic
    m.insert(
        groups::FUNCTION_BUILTIN,
        Style::new()
            .fg(Color::Rgb {
                r: 64,
                g: 120,
                b: 242,
            })
            .bold(),
    ); // Blue bold
    m.insert(
        groups::FUNCTION_MACRO,
        Style::new()
            .fg(Color::Rgb {
                r: 1,
                g: 132,
                b: 188,
            })
            .italic(),
    ); // Cyan italic
    m.insert(
        groups::FUNCTION_METHOD,
        Style::new().fg(Color::Rgb {
            r: 64,
            g: 120,
            b: 242,
        }),
    ); // Blue
    m.insert(
        groups::VARIABLE_BUILTIN,
        Style::new().fg(Color::Rgb {
            r: 152,
            g: 104,
            b: 1,
        }),
    ); // Orange
    m.insert(
        groups::VARIABLE_PARAMETER,
        Style::new()
            .fg(Color::Rgb {
                r: 228,
                g: 86,
                b: 73,
            })
            .italic(),
    ); // Red italic
    m.insert(
        groups::VARIABLE_FIELD,
        Style::new().fg(Color::Rgb {
            r: 228,
            g: 86,
            b: 73,
        }),
    ); // Red
    m.insert(
        groups::STRING_ESCAPE,
        Style::new().fg(Color::Rgb {
            r: 152,
            g: 104,
            b: 1,
        }),
    ); // Orange
    m.insert(
        groups::COMMENT_DOC,
        Style::new()
            .fg(Color::Rgb {
                r: 130,
                g: 131,
                b: 137,
            })
            .italic()
            .bold(),
    ); // Darker grey
    m.insert(
        groups::PUNCTUATION_BRACKET,
        Style::new().fg(Color::Rgb {
            r: 56,
            g: 58,
            b: 66,
        }),
    ); // Foreground
    m.insert(
        groups::PUNCTUATION_DELIMITER,
        Style::new().fg(Color::Rgb {
            r: 56,
            g: 58,
            b: 66,
        }),
    ); // Foreground
    m.insert(
        groups::NAMESPACE,
        Style::new().fg(Color::Rgb {
            r: 64,
            g: 120,
            b: 242,
        }),
    ); // Blue
    m.insert(
        groups::CONSTRUCTOR,
        Style::new()
            .fg(Color::Rgb {
                r: 193,
                g: 132,
                b: 1,
            })
            .bold(),
    ); // Yellow bold
    m.insert(
        groups::LABEL,
        Style::new().fg(Color::Rgb {
            r: 166,
            g: 38,
            b: 164,
        }),
    ); // Magenta
    m.insert(
        groups::BOOLEAN,
        Style::new().fg(Color::Rgb {
            r: 152,
            g: 104,
            b: 1,
        }),
    ); // Orange
    m.insert(
        groups::CHARACTER,
        Style::new().fg(Color::Rgb {
            r: 80,
            g: 161,
            b: 79,
        }),
    ); // Green
    m.insert(
        groups::MARKUP_HEADING,
        Style::new()
            .fg(Color::Rgb {
                r: 64,
                g: 120,
                b: 242,
            })
            .bold(),
    ); // Blue bold
    m.insert(groups::MARKUP_BOLD, Style::new().bold());
    m.insert(groups::MARKUP_ITALIC, Style::new().italic());
    m.insert(
        groups::MARKUP_STRIKETHROUGH,
        Style::new().fg(Color::Rgb {
            r: 160,
            g: 161,
            b: 167,
        }),
    ); // Grey
    m.insert(
        groups::MARKUP_LINK,
        Style::new()
            .fg(Color::Rgb {
                r: 1,
                g: 132,
                b: 188,
            })
            .underline(),
    ); // Cyan underline
    m.insert(
        groups::MARKUP_LINK_URL,
        Style::new().fg(Color::Rgb {
            r: 1,
            g: 132,
            b: 188,
        }),
    ); // Cyan
    m.insert(
        groups::MARKUP_LIST,
        Style::new().fg(Color::Rgb {
            r: 166,
            g: 38,
            b: 164,
        }),
    ); // Magenta
    m.insert(
        groups::MARKUP_RAW,
        Style::new().fg(Color::Rgb {
            r: 80,
            g: 161,
            b: 79,
        }),
    ); // Green
    m.insert(
        groups::MARKUP_RAW_INLINE,
        Style::new().fg(Color::Rgb {
            r: 80,
            g: 161,
            b: 79,
        }),
    ); // Green
    m.insert(
        groups::EMBEDDED,
        Style::new().fg(Color::Rgb {
            r: 56,
            g: 58,
            b: 66,
        }),
    ); // Foreground
    m.insert(
        groups::SPECIAL,
        Style::new().fg(Color::Rgb {
            r: 1,
            g: 132,
            b: 188,
        }),
    ); // Cyan

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
    m.insert(
        groups::MODE_REPLACE,
        Style::new()
            .fg(Color::Rgb {
                r: 250,
                g: 250,
                b: 250,
            })
            .bg(Color::Rgb { r: 204, g: 0, b: 0 }) // Red
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

    // -------------------------------------------------------------------------
    // Gutter/Annotation colors (8 groups) - Issue #455
    // -------------------------------------------------------------------------
    m.insert(groups::SIGN_COLUMN, Style::new()); // Inherit background
    m.insert(
        groups::GUTTER_SEPARATOR,
        Style::new().fg(Color::Rgb {
            r: 200,
            g: 200,
            b: 200,
        }), // Light grey separator
    );
    m.insert(
        groups::GIT_ADD,
        Style::new().fg(Color::Rgb {
            r: 80,
            g: 161,
            b: 79,
        }), // #50a14f Green
    );
    m.insert(
        groups::GIT_CHANGE,
        Style::new().fg(Color::Rgb {
            r: 193,
            g: 132,
            b: 1,
        }), // #c18401 Yellow/Orange
    );
    m.insert(
        groups::GIT_DELETE,
        Style::new().fg(Color::Rgb {
            r: 228,
            g: 86,
            b: 73,
        }), // #e45649 Red
    );
    m.insert(
        groups::FOLD_OPEN,
        Style::new().fg(Color::Rgb {
            r: 160,
            g: 160,
            b: 160,
        }), // Grey
    );
    m.insert(
        groups::FOLD_CLOSED,
        Style::new().fg(Color::Rgb {
            r: 64,
            g: 120,
            b: 242,
        }), // #4078f2 Blue
    );
    m.insert(
        groups::BOOKMARK,
        Style::new().fg(Color::Rgb {
            r: 166,
            g: 38,
            b: 164,
        }), // #a626a4 Purple
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
    // Syntax sub-categories (30 groups) - Tokyo Night Orange variants
    // -------------------------------------------------------------------------
    m.insert(
        groups::KEYWORD_CONTROL,
        Style::new()
            .fg(Color::Rgb {
                r: 187,
                g: 154,
                b: 247,
            })
            .bold(),
    ); // Purple
    m.insert(
        groups::KEYWORD_FUNCTION,
        Style::new()
            .fg(Color::Rgb {
                r: 187,
                g: 154,
                b: 247,
            })
            .bold(),
    ); // Purple
    m.insert(
        groups::KEYWORD_TYPE,
        Style::new()
            .fg(Color::Rgb {
                r: 187,
                g: 154,
                b: 247,
            })
            .bold(),
    ); // Purple
    m.insert(
        groups::KEYWORD_OPERATOR,
        Style::new().fg(Color::Rgb {
            r: 187,
            g: 154,
            b: 247,
        }),
    ); // Purple (no bold)
    m.insert(
        groups::TYPE_BUILTIN,
        Style::new()
            .fg(Color::Rgb {
                r: 255,
                g: 158,
                b: 100,
            })
            .italic(),
    ); // Orange italic
    m.insert(
        groups::FUNCTION_BUILTIN,
        Style::new()
            .fg(Color::Rgb {
                r: 122,
                g: 162,
                b: 247,
            })
            .bold(),
    ); // Blue bold
    m.insert(
        groups::FUNCTION_MACRO,
        Style::new()
            .fg(Color::Rgb {
                r: 137,
                g: 221,
                b: 255,
            })
            .italic(),
    ); // Cyan italic
    m.insert(
        groups::FUNCTION_METHOD,
        Style::new().fg(Color::Rgb {
            r: 122,
            g: 162,
            b: 247,
        }),
    ); // Blue
    m.insert(
        groups::VARIABLE_BUILTIN,
        Style::new().fg(Color::Rgb {
            r: 255,
            g: 158,
            b: 100,
        }),
    ); // Orange
    m.insert(
        groups::VARIABLE_PARAMETER,
        Style::new()
            .fg(Color::Rgb {
                r: 247,
                g: 118,
                b: 142,
            })
            .italic(),
    ); // Red italic
    m.insert(
        groups::VARIABLE_FIELD,
        Style::new().fg(Color::Rgb {
            r: 115,
            g: 218,
            b: 202,
        }),
    ); // Teal
    m.insert(
        groups::STRING_ESCAPE,
        Style::new().fg(Color::Rgb {
            r: 255,
            g: 158,
            b: 100,
        }),
    ); // Orange
    m.insert(
        groups::COMMENT_DOC,
        Style::new()
            .fg(Color::Rgb {
                r: 96,
                g: 105,
                b: 147,
            })
            .italic()
            .bold(),
    ); // Brighter grey
    m.insert(
        groups::PUNCTUATION_BRACKET,
        Style::new().fg(Color::Rgb {
            r: 169,
            g: 177,
            b: 214,
        }),
    ); // Foreground
    m.insert(
        groups::PUNCTUATION_DELIMITER,
        Style::new().fg(Color::Rgb {
            r: 169,
            g: 177,
            b: 214,
        }),
    ); // Foreground
    m.insert(
        groups::NAMESPACE,
        Style::new().fg(Color::Rgb {
            r: 122,
            g: 162,
            b: 247,
        }),
    ); // Blue
    m.insert(
        groups::CONSTRUCTOR,
        Style::new()
            .fg(Color::Rgb {
                r: 224,
                g: 175,
                b: 104,
            })
            .bold(),
    ); // Yellow bold
    m.insert(
        groups::LABEL,
        Style::new().fg(Color::Rgb {
            r: 187,
            g: 154,
            b: 247,
        }),
    ); // Purple
    m.insert(
        groups::BOOLEAN,
        Style::new().fg(Color::Rgb {
            r: 255,
            g: 158,
            b: 100,
        }),
    ); // Orange
    m.insert(
        groups::CHARACTER,
        Style::new().fg(Color::Rgb {
            r: 158,
            g: 206,
            b: 106,
        }),
    ); // Green
    m.insert(
        groups::MARKUP_HEADING,
        Style::new()
            .fg(Color::Rgb {
                r: 122,
                g: 162,
                b: 247,
            })
            .bold(),
    ); // Blue bold
    m.insert(groups::MARKUP_BOLD, Style::new().bold());
    m.insert(groups::MARKUP_ITALIC, Style::new().italic());
    m.insert(
        groups::MARKUP_STRIKETHROUGH,
        Style::new().fg(Color::Rgb {
            r: 86,
            g: 95,
            b: 137,
        }),
    ); // Grey
    m.insert(
        groups::MARKUP_LINK,
        Style::new()
            .fg(Color::Rgb {
                r: 125,
                g: 207,
                b: 255,
            })
            .underline(),
    ); // Cyan underline
    m.insert(
        groups::MARKUP_LINK_URL,
        Style::new().fg(Color::Rgb {
            r: 125,
            g: 207,
            b: 255,
        }),
    ); // Cyan
    m.insert(
        groups::MARKUP_LIST,
        Style::new().fg(Color::Rgb {
            r: 187,
            g: 154,
            b: 247,
        }),
    ); // Purple
    m.insert(
        groups::MARKUP_RAW,
        Style::new().fg(Color::Rgb {
            r: 158,
            g: 206,
            b: 106,
        }),
    ); // Green
    m.insert(
        groups::MARKUP_RAW_INLINE,
        Style::new().fg(Color::Rgb {
            r: 158,
            g: 206,
            b: 106,
        }),
    ); // Green
    m.insert(
        groups::EMBEDDED,
        Style::new().fg(Color::Rgb {
            r: 169,
            g: 177,
            b: 214,
        }),
    ); // Foreground
    m.insert(
        groups::SPECIAL,
        Style::new().fg(Color::Rgb {
            r: 137,
            g: 221,
            b: 255,
        }),
    ); // Cyan

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
    m.insert(
        groups::MODE_REPLACE,
        Style::new()
            .fg(Color::Rgb {
                r: 26,
                g: 27,
                b: 38,
            })
            .bg(Color::Rgb {
                r: 247,
                g: 118,
                b: 142,
            }) // Pink-red
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

    // -------------------------------------------------------------------------
    // Gutter/Annotation colors (8 groups) - Issue #455
    // -------------------------------------------------------------------------
    m.insert(groups::SIGN_COLUMN, Style::new()); // Inherit background
    m.insert(
        groups::GUTTER_SEPARATOR,
        Style::new().fg(Color::Rgb {
            r: 50,
            g: 52,
            b: 66,
        }), // #323442 Subtle separator
    );
    m.insert(
        groups::GIT_ADD,
        Style::new().fg(Color::Rgb {
            r: 158,
            g: 206,
            b: 106,
        }), // #9ece6a Green
    );
    m.insert(
        groups::GIT_CHANGE,
        Style::new().fg(Color::Rgb {
            r: 255,
            g: 158,
            b: 100,
        }), // #ff9e64 Orange
    );
    m.insert(
        groups::GIT_DELETE,
        Style::new().fg(Color::Rgb {
            r: 247,
            g: 118,
            b: 142,
        }), // #f7768e Red
    );
    m.insert(
        groups::FOLD_OPEN,
        Style::new().fg(Color::Rgb {
            r: 86,
            g: 95,
            b: 137,
        }), // #565f89 Grey-blue
    );
    m.insert(
        groups::FOLD_CLOSED,
        Style::new().fg(Color::Rgb {
            r: 125,
            g: 207,
            b: 255,
        }), // #7dcfff Cyan
    );
    m.insert(
        groups::BOOKMARK,
        Style::new().fg(Color::Rgb {
            r: 187,
            g: 154,
            b: 247,
        }), // #bb9af7 Purple
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
#[path = "builtin_tests.rs"]
mod tests;
