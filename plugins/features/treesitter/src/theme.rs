//! Treesitter capture to style mapping

use std::collections::HashMap;

use reovim_core::highlight::{Style, ThemeName};

/// Theme mapping from treesitter capture names to styles
pub struct TreesitterTheme {
    captures: HashMap<&'static str, Style>,
}

impl Default for TreesitterTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl TreesitterTheme {
    /// Create a new theme with default capture mappings
    #[must_use]
    pub fn new() -> Self {
        use reovim_sys::style::Color;

        let mut captures = HashMap::new();

        // Keywords
        captures.insert("keyword", Style::new().fg(Color::Magenta).bold());
        captures.insert("keyword.function", Style::new().fg(Color::Magenta).bold());
        captures.insert("keyword.return", Style::new().fg(Color::Magenta).bold());
        captures.insert("keyword.operator", Style::new().fg(Color::Magenta));
        captures.insert("keyword.import", Style::new().fg(Color::Magenta));
        captures.insert("keyword.conditional", Style::new().fg(Color::Magenta).bold());
        captures.insert("keyword.repeat", Style::new().fg(Color::Magenta).bold());
        captures.insert("keyword.storage", Style::new().fg(Color::Magenta)); // let, const, static
        captures.insert("keyword.type", Style::new().fg(Color::Magenta).bold()); // type, typedef
        captures.insert("keyword.struct", Style::new().fg(Color::Magenta).bold()); // struct, class, trait
        captures.insert("keyword.enum", Style::new().fg(Color::Magenta).bold()); // enum
        captures.insert("keyword.exception", Style::new().fg(Color::Red).bold()); // try, catch, throw

        // Types
        captures.insert("type", Style::new().fg(Color::Yellow));
        captures.insert("type.builtin", Style::new().fg(Color::Yellow).italic());
        captures.insert("type.qualifier", Style::new().fg(Color::Magenta));
        captures.insert("type.class", Style::new().fg(Color::Yellow)); // class types
        captures.insert("type.interface", Style::new().fg(Color::Yellow).italic()); // trait/interface
        captures.insert("type.enum", Style::new().fg(Color::Yellow)); // enum types
        captures.insert("type.parameter", Style::new().fg(Color::Yellow).italic()); // generic <T>

        // Functions
        captures.insert("function", Style::new().fg(Color::Blue));
        captures.insert("function.method", Style::new().fg(Color::Blue));
        captures.insert("function.builtin", Style::new().fg(Color::Cyan));
        captures.insert("function.macro", Style::new().fg(Color::Cyan).bold());
        captures.insert("function.definition", Style::new().fg(Color::Blue).bold()); // fn name at def
        captures.insert("function.call", Style::new().fg(Color::Blue)); // fn name at call
        captures.insert("function.special", Style::new().fg(Color::Blue).italic()); // __init__, main

        // Variables
        captures.insert("variable", Style::new());
        captures.insert("variable.parameter", Style::new().fg(Color::Red).italic());
        captures.insert("variable.builtin", Style::new().fg(Color::Red));
        captures.insert("variable.member", Style::new().fg(Color::Red));
        captures.insert("variable.definition", Style::new()); // var at definition
        captures.insert("variable.field", Style::new().fg(Color::Red)); // struct fields

        // Constants
        captures.insert("constant", Style::new().fg(Color::Cyan));
        captures.insert("constant.builtin", Style::new().fg(Color::Cyan).bold());

        // Strings
        captures.insert("string", Style::new().fg(Color::Green));
        captures.insert("string.escape", Style::new().fg(Color::Cyan));
        captures.insert("string.special", Style::new().fg(Color::Cyan));

        // Numbers
        captures.insert("number", Style::new().fg(Color::Cyan));
        captures.insert("number.float", Style::new().fg(Color::Cyan));

        // Boolean
        captures.insert("boolean", Style::new().fg(Color::Cyan).bold());

        // Comments
        captures.insert("comment", Style::new().fg(Color::DarkGrey).italic());
        captures.insert("comment.documentation", Style::new().fg(Color::DarkGrey).italic());
        captures.insert("comment.line", Style::new().fg(Color::DarkGrey).italic()); // single-line //
        captures.insert("comment.block", Style::new().fg(Color::DarkGrey).italic()); // block /* */
        captures.insert("comment.todo", Style::new().fg(Color::Yellow).bold()); // TODO/FIXME

        // Operators
        captures.insert("operator", Style::new().fg(Color::White));

        // Punctuation
        captures.insert("punctuation", Style::new().fg(Color::White));
        captures.insert("punctuation.bracket", Style::new().fg(Color::White));
        captures.insert("punctuation.delimiter", Style::new().fg(Color::White));
        captures.insert("punctuation.special", Style::new().fg(Color::Cyan));
        // Specific bracket types (base colors, can be overridden by rainbow)
        captures.insert("punctuation.bracket.round", Style::new().fg(Color::White)); // ()
        captures.insert("punctuation.bracket.square", Style::new().fg(Color::White)); // []
        captures.insert("punctuation.bracket.curly", Style::new().fg(Color::White)); // {}
        captures.insert("punctuation.bracket.angle", Style::new().fg(Color::White)); // <>

        // Labels and tags
        captures.insert("label", Style::new().fg(Color::Yellow));
        captures.insert("tag", Style::new().fg(Color::Red));
        captures.insert("tag.attribute", Style::new().fg(Color::Yellow));

        // Attributes/Decorators
        captures.insert("attribute", Style::new().fg(Color::Yellow));

        // Namespace/Module
        captures.insert("namespace", Style::new().fg(Color::Yellow).italic());
        captures.insert("module", Style::new().fg(Color::Yellow).italic());

        // Constructor
        captures.insert("constructor", Style::new().fg(Color::Yellow));

        // Property
        captures.insert("property", Style::new().fg(Color::Red));

        // Markdown-specific
        captures.insert("markup.heading", Style::new().fg(Color::Blue).bold());
        captures.insert("markup.bold", Style::new().bold());
        captures.insert("markup.italic", Style::new().italic());
        captures.insert("markup.link", Style::new().fg(Color::Cyan).underline());
        captures.insert("markup.link.url", Style::new().fg(Color::Blue).underline());
        captures.insert("markup.raw", Style::new().fg(Color::Green));
        captures.insert("markup.list", Style::new().fg(Color::Magenta));

        Self { captures }
    }

    /// Create a Tokyo Night theme with orange accents
    #[must_use]
    pub fn tokyo_night_orange() -> Self {
        use reovim_sys::style::Color;

        let mut captures = HashMap::new();

        // Tokyo Night color palette with orange accents
        let orange = Color::Rgb {
            r: 255,
            g: 158,
            b: 100,
        }; // #ff9e64
        let purple = Color::Rgb {
            r: 187,
            g: 154,
            b: 247,
        }; // #bb9af7
        let blue = Color::Rgb {
            r: 122,
            g: 162,
            b: 247,
        }; // #7aa2f7
        let cyan = Color::Rgb {
            r: 125,
            g: 207,
            b: 255,
        }; // #7dcfff
        let green = Color::Rgb {
            r: 158,
            g: 206,
            b: 106,
        }; // #9ece6a
        let yellow = Color::Rgb {
            r: 224,
            g: 175,
            b: 104,
        }; // #e0af68
        let red = Color::Rgb {
            r: 247,
            g: 118,
            b: 142,
        }; // #f7768e
        let comment = Color::Rgb {
            r: 199,
            g: 199,
            b: 199,
        }; // #c7c7c7
        let fg = Color::Rgb {
            r: 192,
            g: 202,
            b: 245,
        }; // #c0caf5
        let magenta = Color::Rgb {
            r: 255,
            g: 0,
            b: 127,
        }; // #ff007f

        // Keywords - purple, bold, italic
        captures.insert("keyword", Style::new().fg(purple).bold().italic());
        captures.insert("keyword.function", Style::new().fg(purple).bold().italic());
        captures.insert("keyword.return", Style::new().fg(purple).bold().italic());
        captures.insert("keyword.operator", Style::new().fg(purple).italic());
        captures.insert("keyword.import", Style::new().fg(purple).italic());
        captures.insert("keyword.conditional", Style::new().fg(purple).bold().italic());
        captures.insert("keyword.repeat", Style::new().fg(purple).bold().italic());
        captures.insert("keyword.storage", Style::new().fg(purple).italic()); // let, const, static
        captures.insert("keyword.type", Style::new().fg(purple).bold().italic()); // type, typedef
        captures.insert("keyword.struct", Style::new().fg(purple).bold().italic()); // struct, class, trait
        captures.insert("keyword.enum", Style::new().fg(purple).bold().italic()); // enum
        captures.insert("keyword.exception", Style::new().fg(red).bold()); // try, catch, throw

        // Types - cyan
        captures.insert("type", Style::new().fg(cyan));
        captures.insert("type.builtin", Style::new().fg(cyan).italic());
        captures.insert("type.qualifier", Style::new().fg(purple));
        captures.insert("type.class", Style::new().fg(cyan)); // class types
        captures.insert("type.interface", Style::new().fg(cyan).italic()); // trait/interface
        captures.insert("type.enum", Style::new().fg(cyan)); // enum types
        captures.insert("type.parameter", Style::new().fg(cyan).italic()); // generic <T>

        // Functions - blue
        captures.insert("function", Style::new().fg(blue));
        captures.insert("function.method", Style::new().fg(blue));
        captures.insert("function.builtin", Style::new().fg(cyan));
        captures.insert("function.macro", Style::new().fg(cyan).bold());
        captures.insert("function.definition", Style::new().fg(blue).bold()); // fn name at def
        captures.insert("function.call", Style::new().fg(blue)); // fn name at call
        captures.insert("function.special", Style::new().fg(blue).italic()); // __init__, main

        // Variables - foreground, with special colors for parameters
        captures.insert("variable", Style::new().fg(fg));
        captures.insert("variable.parameter", Style::new().fg(orange).italic());
        captures.insert("variable.builtin", Style::new().fg(red));
        captures.insert("variable.member", Style::new().fg(blue));
        captures.insert("variable.definition", Style::new().fg(fg)); // var at definition
        captures.insert("variable.field", Style::new().fg(blue)); // struct fields

        // Constants - orange (accent color!)
        captures.insert("constant", Style::new().fg(orange));
        captures.insert("constant.builtin", Style::new().fg(orange).bold());

        // Strings - green
        captures.insert("string", Style::new().fg(green));
        captures.insert("string.escape", Style::new().fg(cyan));
        captures.insert("string.special", Style::new().fg(cyan));

        // Numbers - orange (accent color!)
        captures.insert("number", Style::new().fg(orange));
        captures.insert("number.float", Style::new().fg(orange));

        // Boolean - orange (accent color!)
        captures.insert("boolean", Style::new().fg(orange).bold());

        // Comments - light gray, italic
        captures.insert("comment", Style::new().fg(comment).italic());
        captures.insert("comment.documentation", Style::new().fg(comment).italic());
        captures.insert("comment.line", Style::new().fg(comment).italic()); // single-line //
        captures.insert("comment.block", Style::new().fg(comment).italic()); // block /* */
        captures.insert("comment.todo", Style::new().fg(yellow).bold()); // TODO/FIXME

        // Operators - foreground
        captures.insert("operator", Style::new().fg(cyan));

        // Punctuation - foreground
        captures.insert("punctuation", Style::new().fg(fg));
        captures.insert("punctuation.bracket", Style::new().fg(fg));
        captures.insert("punctuation.delimiter", Style::new().fg(fg));
        captures.insert("punctuation.special", Style::new().fg(cyan));
        // Specific bracket types (base colors, can be overridden by rainbow)
        captures.insert("punctuation.bracket.round", Style::new().fg(fg)); // ()
        captures.insert("punctuation.bracket.square", Style::new().fg(fg)); // []
        captures.insert("punctuation.bracket.curly", Style::new().fg(fg)); // {}
        captures.insert("punctuation.bracket.angle", Style::new().fg(fg)); // <>

        // Labels and tags
        captures.insert("label", Style::new().fg(yellow));
        captures.insert("tag", Style::new().fg(red));
        captures.insert("tag.attribute", Style::new().fg(yellow));

        // Attributes/Decorators - yellow
        captures.insert("attribute", Style::new().fg(yellow));

        // Namespace/Module - yellow
        captures.insert("namespace", Style::new().fg(blue).italic());
        captures.insert("module", Style::new().fg(blue).italic());

        // Constructor - yellow
        captures.insert("constructor", Style::new().fg(cyan));

        // Property - magenta
        captures.insert("property", Style::new().fg(magenta));

        // Markdown-specific
        captures.insert("markup.heading", Style::new().fg(blue).bold());
        captures.insert("markup.bold", Style::new().bold());
        captures.insert("markup.italic", Style::new().italic());
        captures.insert("markup.link", Style::new().fg(cyan).underline());
        captures.insert("markup.link.url", Style::new().fg(blue).underline());
        captures.insert("markup.raw", Style::new().fg(green));
        captures.insert("markup.list", Style::new().fg(orange));

        Self { captures }
    }

    /// Get the style for a capture name
    ///
    /// Falls back to parent scope if exact match not found
    /// (e.g., "keyword.function" falls back to "keyword")
    #[must_use]
    pub fn style_for_capture(&self, capture_name: &str) -> Option<&Style> {
        // Try exact match first
        if let Some(style) = self.captures.get(capture_name) {
            return Some(style);
        }

        // Try parent scope (e.g., "keyword.function" -> "keyword")
        if let Some((parent, _)) = capture_name.rsplit_once('.') {
            return self.captures.get(parent);
        }

        None
    }

    /// Create a treesitter theme matching the given UI theme name
    #[must_use]
    pub fn from_theme_name(name: ThemeName) -> Self {
        match name {
            ThemeName::TokyoNightOrange => Self::tokyo_night_orange(),
            ThemeName::Dark | ThemeName::Light => Self::new(),
        }
    }
}
