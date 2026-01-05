//! Treesitter capture to style mapping
//!
//! Reo Color System - A unified palette merging:
//! - Tokyo Night (blue/purple spectrum)
//! - Catppuccin Mocha (warm pastels)
//! - One Dark (high contrast)
//!
//! See `docs/color-system.md` for color science principles.

use std::collections::HashMap;

use {
    reovim_core::highlight::{Style, ThemeName},
    reovim_sys::style::Color,
};

// =============================================================================
// REO COLOR PALETTE
// 45 distinct colors organized by HSL hue clusters
// =============================================================================

/// Reo palette - HSL-organized color system
mod palette {
    use reovim_sys::style::Color;

    // -------------------------------------------------------------------------
    // WARM CLUSTER (0-60° Hue) - Danger, constants, parameters
    // -------------------------------------------------------------------------

    /// #f38ba8 - HSL(343°, 81%, 75%) - Danger, mut, unsafe
    pub const RED: Color = Color::Rgb {
        r: 243,
        g: 139,
        b: 168,
    };
    /// #ff6b6b - HSL(0°, 100%, 71%) - Errors, critical
    pub const RED_BRIGHT: Color = Color::Rgb {
        r: 255,
        g: 107,
        b: 107,
    };
    /// #eba0ac - HSL(350°, 65%, 77%) - Alt red, warnings
    pub const MAROON: Color = Color::Rgb {
        r: 235,
        g: 160,
        b: 172,
    };
    /// #ff8a80 - HSL(5°, 100%, 75%) - HTML tags
    pub const CORAL: Color = Color::Rgb {
        r: 255,
        g: 138,
        b: 128,
    };
    /// #fab387 - HSL(23°, 92%, 75%) - Constants, numbers
    pub const ORANGE: Color = Color::Rgb {
        r: 250,
        g: 179,
        b: 135,
    };
    /// #ffb86c - HSL(27°, 100%, 71%) - Float numbers
    pub const PEACH: Color = Color::Rgb {
        r: 255,
        g: 184,
        b: 108,
    };
    /// #f9e2af - HSL(41°, 86%, 83%) - Integers, booleans
    pub const GOLD: Color = Color::Rgb {
        r: 249,
        g: 226,
        b: 175,
    };
    /// #e5c07b - HSL(39°, 67%, 69%) - Parameters, type params
    pub const YELLOW: Color = Color::Rgb {
        r: 229,
        g: 192,
        b: 123,
    };
    /// #f0d9a0 - HSL(42°, 71%, 78%) - Patterns
    pub const YELLOW_SOFT: Color = Color::Rgb {
        r: 240,
        g: 217,
        b: 160,
    };

    // -------------------------------------------------------------------------
    // GREEN CLUSTER (90-150° Hue) - Strings, properties, success
    // -------------------------------------------------------------------------

    /// #98c379 - HSL(95°, 38%, 62%) - Strings
    pub const GREEN: Color = Color::Rgb {
        r: 152,
        g: 195,
        b: 121,
    };
    /// #a6e3a1 - HSL(115°, 54%, 76%) - Success, diff add
    pub const GREEN_BRIGHT: Color = Color::Rgb {
        r: 166,
        g: 227,
        b: 161,
    };
    /// #c3e88d - HSL(82°, 68%, 73%) - String interpolation
    pub const LIME: Color = Color::Rgb {
        r: 195,
        g: 232,
        b: 141,
    };
    /// #56b6c2 - HSL(187°, 47%, 55%) - Properties, fields
    pub const TEAL: Color = Color::Rgb {
        r: 86,
        g: 182,
        b: 194,
    };
    /// #1abc9c - HSL(168°, 76%, 42%) - Lifetimes, hints
    pub const TEAL_DARK: Color = Color::Rgb {
        r: 26,
        g: 188,
        b: 156,
    };
    /// #94e2d5 - HSL(170°, 57%, 73%) - Character literals
    pub const MINT: Color = Color::Rgb {
        r: 148,
        g: 226,
        b: 213,
    };

    // -------------------------------------------------------------------------
    // COOL CLUSTER (170-240° Hue) - Types, functions, imports
    // -------------------------------------------------------------------------

    /// #7dcfff - HSL(197°, 100%, 74%) - Attributes, imports
    pub const CYAN: Color = Color::Rgb {
        r: 125,
        g: 207,
        b: 255,
    };
    /// #89dceb - HSL(189°, 71%, 73%) - Constructors
    pub const CYAN_BRIGHT: Color = Color::Rgb {
        r: 137,
        g: 220,
        b: 235,
    };
    /// #74c7ec - HSL(199°, 76%, 69%) - Info
    pub const SKY: Color = Color::Rgb {
        r: 116,
        g: 199,
        b: 236,
    };
    /// #2ac3de - HSL(189°, 71%, 52%) - Types (custom)
    pub const AQUA: Color = Color::Rgb {
        r: 42,
        g: 195,
        b: 222,
    };
    /// #89ddff - HSL(191°, 100%, 77%) - Operators
    pub const TURQUOISE: Color = Color::Rgb {
        r: 137,
        g: 221,
        b: 255,
    };
    /// #7aa2f7 - HSL(220°, 89%, 72%) - Functions
    pub const BLUE: Color = Color::Rgb {
        r: 122,
        g: 162,
        b: 247,
    };
    /// #89b4fa - HSL(217°, 92%, 76%) - Function definitions
    pub const BLUE_BRIGHT: Color = Color::Rgb {
        r: 137,
        g: 180,
        b: 250,
    };
    /// #3d59a1 - HSL(222°, 45%, 44%) - Modules, muted blue
    pub const BLUE_DARK: Color = Color::Rgb {
        r: 61,
        g: 89,
        b: 161,
    };
    /// #6366f1 - HSL(239°, 84%, 67%) - Deep functions
    pub const INDIGO: Color = Color::Rgb {
        r: 99,
        g: 102,
        b: 241,
    };

    // -------------------------------------------------------------------------
    // PURPLE CLUSTER (260-320° Hue) - Keywords, control flow, macros
    // -------------------------------------------------------------------------

    /// #9d7cd8 - HSL(267°, 53%, 67%) - Keywords
    pub const PURPLE: Color = Color::Rgb {
        r: 157,
        g: 124,
        b: 216,
    };
    /// #bb9af7 - HSL(267°, 84%, 79%) - Control flow
    pub const PURPLE_BRIGHT: Color = Color::Rgb {
        r: 187,
        g: 154,
        b: 247,
    };
    /// #c7b0fa - HSL(259°, 88%, 84%) - Type keywords
    pub const LAVENDER: Color = Color::Rgb {
        r: 199,
        g: 176,
        b: 250,
    };
    /// #cba6f7 - HSL(267°, 84%, 81%) - Alt keywords
    pub const MAUVE: Color = Color::Rgb {
        r: 203,
        g: 166,
        b: 247,
    };
    /// #f5c2e7 - HSL(316°, 72%, 86%) - Macros
    pub const MAGENTA: Color = Color::Rgb {
        r: 245,
        g: 194,
        b: 231,
    };
    /// #ff79c6 - HSL(326°, 100%, 74%) - Async/loops
    pub const PINK: Color = Color::Rgb {
        r: 255,
        g: 121,
        b: 198,
    };
    /// #f2cdcd - HSL(0°, 59%, 88%) - Decorators
    pub const FLAMINGO: Color = Color::Rgb {
        r: 242,
        g: 205,
        b: 205,
    };
    /// #f5e0dc - HSL(10°, 56%, 91%) - Special strings
    pub const ROSEWATER: Color = Color::Rgb {
        r: 245,
        g: 224,
        b: 220,
    };

    // -------------------------------------------------------------------------
    // NEUTRAL CLUSTER (Desaturated) - Text, backgrounds, UI
    // -------------------------------------------------------------------------

    /// #c0caf5 - HSL(227°, 70%, 85%) - Default text
    pub const FG: Color = Color::Rgb {
        r: 192,
        g: 202,
        b: 245,
    };
    /// #a9b1d6 - HSL(227°, 36%, 75%) - Secondary text
    pub const FG_DIM: Color = Color::Rgb {
        r: 169,
        g: 177,
        b: 214,
    };
    /// #9399b2 - HSL(227°, 20%, 65%) - Tertiary text
    pub const FG_MUTED: Color = Color::Rgb {
        r: 147,
        g: 153,
        b: 178,
    };
    /// #565f89 - HSL(225°, 23%, 44%) - Comments
    pub const COMMENT: Color = Color::Rgb {
        r: 86,
        g: 95,
        b: 137,
    };
    /// #6c7086 - HSL(227°, 12%, 47%) - Muted elements
    pub const OVERLAY: Color = Color::Rgb {
        r: 108,
        g: 112,
        b: 134,
    };
    /// #3b4261 - HSL(224°, 25%, 30%) - Line numbers
    pub const GUTTER: Color = Color::Rgb {
        r: 59,
        g: 66,
        b: 97,
    };
    /// #45475a - HSL(225°, 13%, 31%) - Elevated surfaces
    pub const SURFACE2: Color = Color::Rgb {
        r: 69,
        g: 71,
        b: 90,
    };
    /// #313244 - HSL(231°, 16%, 23%) - UI backgrounds
    pub const SURFACE1: Color = Color::Rgb {
        r: 49,
        g: 50,
        b: 68,
    };
    /// #1e1e2e - HSL(240°, 23%, 15%) - Base background
    pub const SURFACE0: Color = Color::Rgb {
        r: 30,
        g: 30,
        b: 46,
    };
    /// #181825 - HSL(240°, 21%, 12%) - Deepest background
    pub const BLACK: Color = Color::Rgb {
        r: 24,
        g: 24,
        b: 37,
    };

    // -------------------------------------------------------------------------
    // SEMANTIC STATUS - Errors, warnings, diffs
    // -------------------------------------------------------------------------

    /// #f38ba8 - Error highlights (same as RED)
    pub const ERROR: Color = RED;
    /// #f9e2af - Warnings (same as GOLD)
    pub const WARNING: Color = GOLD;
    /// #89b4fa - Info messages (same as BLUE_BRIGHT)
    pub const INFO: Color = BLUE_BRIGHT;
    /// #a6e3a1 - Hints (same as GREEN_BRIGHT)
    pub const HINT: Color = GREEN_BRIGHT;
    /// #a6e3a1 - Diff additions
    pub const ADD: Color = GREEN_BRIGHT;
    /// #89b4fa - Diff changes
    pub const CHANGE: Color = BLUE_BRIGHT;
    /// #f38ba8 - Diff deletions
    pub const DELETE: Color = RED;
}

/// Theme mapping from treesitter capture names to styles
pub struct TreesitterTheme {
    captures: HashMap<&'static str, Style>,
}

impl Default for TreesitterTheme {
    fn default() -> Self {
        Self::reo()
    }
}

impl TreesitterTheme {
    /// Create a new theme with default capture mappings (Reo)
    #[must_use]
    pub fn new() -> Self {
        Self::reo()
    }

    /// Create the Reo theme with full color palette
    ///
    /// This is the primary theme with complete syntax highlighting support.
    /// Uses 45 colors + style modifiers for 120+ unique appearances.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn reo() -> Self {
        use palette::*;

        let mut captures = HashMap::new();

        // =====================================================================
        // PATTERNS (Yellow/Soft Spectrum) - NEW!
        // =====================================================================

        captures.insert("pattern", Style::new().fg(YELLOW_SOFT));
        captures.insert("pattern.or", Style::new().fg(YELLOW));
        captures.insert("pattern.tuple", Style::new().fg(YELLOW_SOFT).italic());
        captures.insert("pattern.struct", Style::new().fg(GOLD));
        captures.insert("pattern.tuple_struct", Style::new().fg(PEACH));
        captures.insert("pattern.slice", Style::new().fg(YELLOW).italic());
        captures.insert("pattern.range", Style::new().fg(ORANGE));
        captures.insert("pattern.ref", Style::new().fg(TURQUOISE).italic());
        captures.insert("pattern.mut", Style::new().fg(RED).bold());
        captures.insert("pattern.capture", Style::new().fg(MAUVE));
        captures.insert("pattern.literal", Style::new().fg(GOLD).italic());
        captures.insert("pattern.identifier", Style::new().fg(FG).italic());
        captures.insert("pattern.wildcard", Style::new().fg(OVERLAY));
        captures.insert("pattern.rest", Style::new().fg(OVERLAY).bold());

        // =====================================================================
        // KEYWORDS (Purple/Mauve Spectrum)
        // =====================================================================

        captures.insert("keyword", Style::new().fg(PURPLE).italic());
        captures.insert("keyword.function", Style::new().fg(MAGENTA));
        captures.insert("keyword.type", Style::new().fg(LAVENDER).italic());
        captures.insert("keyword.struct", Style::new().fg(LAVENDER).bold());
        captures.insert("keyword.enum", Style::new().fg(MAUVE).italic());
        captures.insert("keyword.trait", Style::new().fg(MAUVE).bold());
        captures.insert("keyword.control", Style::new().fg(PURPLE_BRIGHT));
        captures.insert("keyword.control.conditional", Style::new().fg(PURPLE_BRIGHT).italic());
        captures.insert("keyword.control.repeat", Style::new().fg(PINK));
        captures.insert("keyword.control.return", Style::new().fg(PURPLE));
        captures.insert("keyword.control.import", Style::new().fg(CYAN));
        captures.insert("keyword.control.exception", Style::new().fg(PINK).bold());
        captures.insert("keyword.storage", Style::new().fg(PURPLE).bold());
        captures.insert("keyword.storage.modifier", Style::new().fg(LAVENDER));
        captures.insert("keyword.storage.modifier.mut", Style::new().fg(RED).bold());
        captures.insert("keyword.storage.modifier.ref", Style::new().fg(TURQUOISE).italic());
        captures.insert("keyword.special", Style::new().fg(RED).bold());
        captures.insert("keyword.operator", Style::new().fg(PINK));
        captures.insert("keyword.where", Style::new().fg(MAUVE).italic());
        captures.insert("keyword.async", Style::new().fg(PINK));
        captures.insert("keyword.async.block", Style::new().fg(PINK).italic());
        captures.insert("keyword.await", Style::new().fg(PINK).bold());
        captures.insert("keyword.try", Style::new().fg(MAROON));
        captures.insert("keyword.unsafe", Style::new().fg(RED).bold());
        captures.insert("keyword.unsafe.block", Style::new().fg(RED_BRIGHT));

        // =====================================================================
        // TYPES (Cyan/Aqua Spectrum)
        // =====================================================================

        captures.insert("type", Style::new().fg(AQUA));
        captures.insert("type.builtin", Style::new().fg(CYAN).italic());
        captures.insert("type.qualifier", Style::new().fg(LAVENDER).italic());
        captures.insert("type.class", Style::new().fg(SKY).bold());
        captures.insert("type.interface", Style::new().fg(TEAL).italic());
        captures.insert("type.enum", Style::new().fg(CYAN_BRIGHT));
        captures.insert("type.parameter", Style::new().fg(YELLOW).italic());
        captures.insert("type.enum.variant", Style::new().fg(PEACH));
        captures.insert("type.enum.variant.builtin", Style::new().fg(GOLD).bold());
        captures.insert("type.array", Style::new().fg(AQUA).italic());
        captures.insert("type.pointer", Style::new().fg(RED).italic());
        captures.insert("type.pointer.mut", Style::new().fg(RED_BRIGHT).italic());
        captures.insert("type.reference", Style::new().fg(TURQUOISE));
        captures.insert("type.reference.mut", Style::new().fg(RED));
        captures.insert("type.function", Style::new().fg(BLUE).italic());
        captures.insert("type.generic", Style::new().fg(AQUA));
        captures.insert("type.turbofish", Style::new().fg(CYAN_BRIGHT));
        captures.insert("type.dynamic", Style::new().fg(TEAL).italic());
        captures.insert("type.qualified", Style::new().fg(SKY));
        captures.insert("type.never", Style::new().fg(RED_BRIGHT).bold());
        captures.insert("type.impl_trait", Style::new().fg(TEAL_DARK).italic());

        // =====================================================================
        // FUNCTIONS (Blue Spectrum)
        // =====================================================================

        captures.insert("function", Style::new().fg(BLUE));
        captures.insert("function.method", Style::new().fg(BLUE).italic());
        captures.insert("function.builtin", Style::new().fg(BLUE_BRIGHT));
        captures.insert("function.macro", Style::new().fg(MAGENTA).bold());
        captures.insert("function.definition", Style::new().fg(BLUE_BRIGHT).bold());
        captures.insert("function.call", Style::new().fg(BLUE).italic());
        captures.insert("function.special", Style::new().fg(INDIGO));

        // =====================================================================
        // VARIABLES (Warm Spectrum)
        // =====================================================================

        captures.insert("variable", Style::new().fg(FG));
        captures.insert("variable.parameter", Style::new().fg(ORANGE).italic());
        captures.insert("variable.builtin", Style::new().fg(RED));
        captures.insert("variable.member", Style::new().fg(TEAL));
        captures.insert("variable.field", Style::new().fg(GREEN));
        captures.insert("variable.definition", Style::new().fg(FG_DIM));

        // =====================================================================
        // CONSTANTS (Orange/Gold Spectrum)
        // =====================================================================

        captures.insert("constant", Style::new().fg(ORANGE));
        captures.insert("constant.builtin", Style::new().fg(GOLD));
        captures.insert("constant.character", Style::new().fg(MINT));
        captures.insert("constant.character.escape", Style::new().fg(PINK));
        captures.insert("constant.numeric", Style::new().fg(ORANGE));
        captures.insert("constant.numeric.integer", Style::new().fg(GOLD));
        captures.insert("constant.numeric.float", Style::new().fg(PEACH));
        captures.insert("constant.builtin.boolean", Style::new().fg(GOLD).italic());

        // =====================================================================
        // STRINGS (Green Spectrum)
        // =====================================================================

        captures.insert("string", Style::new().fg(GREEN));
        captures.insert("string.escape", Style::new().fg(PINK));
        captures.insert("string.special", Style::new().fg(ROSEWATER));
        captures.insert("string.regexp", Style::new().fg(FLAMINGO).italic());
        captures.insert("string.interpolation", Style::new().fg(LIME));

        // =====================================================================
        // NUMBERS (Legacy - prefer constant.numeric.*)
        // =====================================================================

        captures.insert("number", Style::new().fg(GOLD));
        captures.insert("number.float", Style::new().fg(PEACH));
        captures.insert("boolean", Style::new().fg(GOLD).italic());

        // =====================================================================
        // COMMENTS (Gray Spectrum)
        // =====================================================================

        captures.insert("comment", Style::new().fg(COMMENT).italic());
        captures.insert("comment.documentation", Style::new().fg(TEAL_DARK).italic());
        captures.insert("comment.line", Style::new().fg(COMMENT).italic());
        captures.insert("comment.block", Style::new().fg(OVERLAY).italic());
        captures.insert("comment.todo", Style::new().fg(PINK).bold());
        captures.insert("comment.unused", Style::new().fg(FG_MUTED).italic());

        // =====================================================================
        // OPERATORS (Turquoise Spectrum)
        // =====================================================================

        captures.insert("operator", Style::new().fg(TURQUOISE));
        captures.insert("operator.binary", Style::new().fg(TURQUOISE));
        captures.insert("operator.logical", Style::new().fg(PINK));
        captures.insert("operator.bitwise", Style::new().fg(CYAN));
        captures.insert("operator.unary", Style::new().fg(TURQUOISE).bold());
        captures.insert("operator.deref", Style::new().fg(RED));
        captures.insert("operator.ref", Style::new().fg(TURQUOISE).italic());
        captures.insert("operator.assignment", Style::new().fg(LAVENDER));
        captures.insert("operator.compound", Style::new().fg(LAVENDER).italic());

        // =====================================================================
        // PUNCTUATION
        // =====================================================================

        captures.insert("punctuation", Style::new().fg(FG_DIM));
        captures.insert("punctuation.bracket", Style::new().fg(FG_DIM));
        captures.insert("punctuation.bracket.round", Style::new().fg(FG_MUTED));
        captures.insert("punctuation.bracket.square", Style::new().fg(TURQUOISE));
        captures.insert("punctuation.bracket.curly", Style::new().fg(YELLOW));
        captures.insert("punctuation.bracket.angle", Style::new().fg(CYAN));
        captures.insert("punctuation.delimiter", Style::new().fg(CYAN));
        captures.insert("punctuation.special", Style::new().fg(MAGENTA));

        // =====================================================================
        // LABELS & LIFETIMES
        // =====================================================================

        captures.insert("label", Style::new().fg(TEAL_DARK));
        captures.insert("label.special", Style::new().fg(PINK).bold());
        captures.insert("lifetime", Style::new().fg(TEAL_DARK).italic());
        captures.insert("lifetime.static", Style::new().fg(RED).italic());

        // =====================================================================
        // TAGS (HTML/XML)
        // =====================================================================

        captures.insert("tag", Style::new().fg(CORAL));
        captures.insert("tag.attribute", Style::new().fg(PEACH));
        captures.insert("tag.delimiter", Style::new().fg(FG_DIM));

        // =====================================================================
        // ATTRIBUTES/DECORATORS
        // =====================================================================

        captures.insert("attribute", Style::new().fg(CYAN).italic());
        captures.insert("attribute.builtin", Style::new().fg(BLUE_BRIGHT).italic());
        captures.insert("attribute.derive", Style::new().fg(MAGENTA).italic());

        // =====================================================================
        // NAMESPACE/MODULE
        // =====================================================================

        captures.insert("namespace", Style::new().fg(CYAN));
        captures.insert("module", Style::new().fg(BLUE_DARK));
        captures.insert("module.builtin", Style::new().fg(INDIGO));

        // =====================================================================
        // CONSTRUCTOR & PROPERTY
        // =====================================================================

        captures.insert("constructor", Style::new().fg(CYAN_BRIGHT).bold());
        captures.insert("property", Style::new().fg(TEAL));
        captures.insert("property.declaration", Style::new().fg(TEAL).bold());
        captures.insert("special", Style::new().fg(CYAN));

        // =====================================================================
        // MARKUP (Rainbow Headings)
        // =====================================================================

        captures.insert("markup.heading", Style::new().fg(BLUE).bold());
        captures.insert("markup.heading.1", Style::new().fg(RED).bold());
        captures.insert("markup.heading.2", Style::new().fg(ORANGE).bold());
        captures.insert("markup.heading.3", Style::new().fg(YELLOW).bold());
        captures.insert("markup.heading.4", Style::new().fg(GREEN).bold());
        captures.insert("markup.heading.5", Style::new().fg(SKY).bold());
        captures.insert("markup.heading.6", Style::new().fg(LAVENDER).bold());
        captures.insert("markup.bold", Style::new().fg(GOLD).bold());
        captures.insert("markup.italic", Style::new().fg(MAGENTA).italic());
        captures.insert("markup.strikethrough", Style::new().fg(OVERLAY));
        captures.insert("markup.link", Style::new().fg(BLUE).underline());
        captures.insert("markup.link.label", Style::new().fg(CYAN));
        captures.insert("markup.link.text", Style::new().fg(LAVENDER));
        captures.insert("markup.link.url", Style::new().fg(BLUE_BRIGHT).underline());
        captures.insert("markup.raw", Style::new().fg(GREEN));
        captures.insert("markup.raw.inline", Style::new().fg(TEAL));
        captures.insert("markup.quote", Style::new().fg(GREEN_BRIGHT).italic());
        captures.insert("markup.list", Style::new().fg(CORAL).bold());

        // =====================================================================
        // DIFF
        // =====================================================================

        captures.insert("diff.plus", Style::new().fg(ADD));
        captures.insert("diff.minus", Style::new().fg(DELETE));
        captures.insert("diff.delta", Style::new().fg(CHANGE));
        captures.insert("diff.delta.moved", Style::new().fg(BLUE));

        // =====================================================================
        // DIAGNOSTICS
        // =====================================================================

        captures.insert("error", Style::new().fg(ERROR).bold());
        captures.insert("warning", Style::new().fg(WARNING));
        captures.insert("info", Style::new().fg(INFO));
        captures.insert("hint", Style::new().fg(HINT).italic());

        // =====================================================================
        // UI ELEMENTS
        // =====================================================================

        captures.insert("ui.text", Style::new().fg(FG));
        captures.insert("ui.text.inactive", Style::new().fg(OVERLAY).italic());
        captures.insert("ui.text.directory", Style::new().fg(CYAN).bold());
        captures.insert("ui.linenr", Style::new().fg(GUTTER));
        captures.insert("ui.linenr.selected", Style::new().fg(FG));
        captures.insert("ui.cursorline", Style::new().bg(SURFACE0));
        captures.insert("ui.virtual.ruler", Style::new().fg(SURFACE2));
        captures.insert("ui.virtual.whitespace", Style::new().fg(GUTTER));
        captures.insert("ui.virtual.inlay-hint", Style::new().fg(BLUE_DARK).italic());
        captures.insert("ui.virtual.jump-label", Style::new().fg(PINK).bold());
        captures.insert("ui.selection", Style::new().bg(SURFACE1));
        captures.insert("ui.menu", Style::new().bg(SURFACE1));
        captures.insert("ui.menu.selected", Style::new().bg(BLUE_DARK));
        captures.insert("ui.background", Style::new().bg(BLACK));
        captures.insert("ui.popup", Style::new().bg(SURFACE0));
        captures.insert("ui.popup.border", Style::new().fg(SURFACE2));

        Self { captures }
    }

    /// Alias for reo() - Tokyo Night style
    #[must_use]
    pub fn tokyo_night() -> Self {
        Self::reo()
    }

    /// Alias for reo() - Tokyo Night Orange variant
    #[must_use]
    pub fn tokyo_night_orange() -> Self {
        Self::reo()
    }

    /// Create a minimal dark theme (fallback for compatibility)
    #[must_use]
    pub fn dark() -> Self {
        let mut captures = HashMap::new();

        captures.insert("keyword", Style::new().fg(Color::Magenta).bold());
        captures.insert("type", Style::new().fg(Color::Yellow));
        captures.insert("function", Style::new().fg(Color::Blue));
        captures.insert("variable", Style::new().fg(Color::White));
        captures.insert("string", Style::new().fg(Color::Green));
        captures.insert("number", Style::new().fg(Color::Cyan));
        captures.insert("comment", Style::new().fg(Color::DarkGrey).italic());
        captures.insert("operator", Style::new().fg(Color::White));
        captures.insert("punctuation", Style::new().fg(Color::White));

        Self { captures }
    }

    /// Get the style for a capture name
    ///
    /// Falls back to parent scope if exact match not found
    /// (e.g., "keyword.control.conditional" -> "keyword.control" -> "keyword")
    #[must_use]
    pub fn style_for_capture(&self, capture_name: &str) -> Option<&Style> {
        // Try exact match first
        if let Some(style) = self.captures.get(capture_name) {
            return Some(style);
        }

        // Try parent scopes progressively
        let mut name = capture_name;
        while let Some((parent, _)) = name.rsplit_once('.') {
            if let Some(style) = self.captures.get(parent) {
                return Some(style);
            }
            name = parent;
        }

        None
    }

    /// Create a treesitter theme matching the given UI theme name
    #[must_use]
    pub fn from_theme_name(name: ThemeName) -> Self {
        match name {
            ThemeName::TokyoNightOrange => Self::reo(),
            ThemeName::Dark => Self::dark(),
            ThemeName::Light => Self::dark(), // Light theme TODO
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_reo() {
        let default = TreesitterTheme::default();
        let reo = TreesitterTheme::reo();

        assert!(default.captures.contains_key("keyword"));
        assert!(reo.captures.contains_key("keyword"));
    }

    #[test]
    fn test_fallback_chain() {
        let theme = TreesitterTheme::reo();

        assert!(theme.style_for_capture("keyword").is_some());
        assert!(
            theme
                .style_for_capture("keyword.control.conditional")
                .is_some()
        );
        assert!(
            theme
                .style_for_capture("keyword.control.conditional.if")
                .is_some()
        );
    }

    #[test]
    fn test_palette_colors() {
        use palette::*;

        // Verify key colors from each cluster
        assert!(matches!(
            RED,
            Color::Rgb {
                r: 243,
                g: 139,
                b: 168
            }
        ));
        assert!(matches!(
            GREEN,
            Color::Rgb {
                r: 152,
                g: 195,
                b: 121
            }
        ));
        assert!(matches!(
            BLUE,
            Color::Rgb {
                r: 122,
                g: 162,
                b: 247
            }
        ));
        assert!(matches!(
            PURPLE,
            Color::Rgb {
                r: 157,
                g: 124,
                b: 216
            }
        ));
    }

    #[test]
    fn test_new_pattern_captures() {
        let theme = TreesitterTheme::reo();

        assert!(theme.style_for_capture("pattern").is_some());
        assert!(theme.style_for_capture("pattern.struct").is_some());
        assert!(theme.style_for_capture("pattern.mut").is_some());
        assert!(theme.style_for_capture("pattern.wildcard").is_some());
    }

    #[test]
    fn test_new_operator_captures() {
        let theme = TreesitterTheme::reo();

        assert!(theme.style_for_capture("operator.binary").is_some());
        assert!(theme.style_for_capture("operator.logical").is_some());
        assert!(theme.style_for_capture("operator.deref").is_some());
        assert!(theme.style_for_capture("operator.assignment").is_some());
    }

    #[test]
    fn test_new_type_captures() {
        let theme = TreesitterTheme::reo();

        assert!(theme.style_for_capture("type.array").is_some());
        assert!(theme.style_for_capture("type.pointer").is_some());
        assert!(theme.style_for_capture("type.reference").is_some());
        assert!(theme.style_for_capture("type.never").is_some());
    }

    #[test]
    fn test_async_unsafe_captures() {
        let theme = TreesitterTheme::reo();

        assert!(theme.style_for_capture("keyword.async").is_some());
        assert!(theme.style_for_capture("keyword.await").is_some());
        assert!(theme.style_for_capture("keyword.unsafe").is_some());
        assert!(theme.style_for_capture("keyword.unsafe.block").is_some());
    }

    #[test]
    fn test_all_major_captures_defined() {
        let theme = TreesitterTheme::reo();

        // Keywords
        assert!(theme.style_for_capture("keyword").is_some());
        assert!(theme.style_for_capture("keyword.control").is_some());
        assert!(theme.style_for_capture("keyword.function").is_some());

        // Types
        assert!(theme.style_for_capture("type").is_some());
        assert!(theme.style_for_capture("type.builtin").is_some());
        assert!(theme.style_for_capture("type.enum.variant").is_some());

        // Functions
        assert!(theme.style_for_capture("function").is_some());
        assert!(theme.style_for_capture("function.builtin").is_some());
        assert!(theme.style_for_capture("function.macro").is_some());

        // Variables
        assert!(theme.style_for_capture("variable").is_some());
        assert!(theme.style_for_capture("variable.parameter").is_some());
        assert!(theme.style_for_capture("variable.builtin").is_some());

        // Constants
        assert!(theme.style_for_capture("constant").is_some());
        assert!(theme.style_for_capture("constant.character").is_some());
        assert!(theme.style_for_capture("constant.numeric").is_some());

        // Strings
        assert!(theme.style_for_capture("string").is_some());
        assert!(theme.style_for_capture("string.escape").is_some());

        // Comments
        assert!(theme.style_for_capture("comment").is_some());
        assert!(theme.style_for_capture("comment.documentation").is_some());

        // Operators & Punctuation
        assert!(theme.style_for_capture("operator").is_some());
        assert!(theme.style_for_capture("punctuation").is_some());

        // Markup
        assert!(theme.style_for_capture("markup.heading").is_some());
        assert!(theme.style_for_capture("markup.bold").is_some());
        assert!(theme.style_for_capture("markup.link").is_some());
    }
}
