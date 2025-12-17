//! Treesitter capture to style mapping

use std::collections::HashMap;

use reovim_sys::style::Color;

use crate::highlight::Style;

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
        let mut captures = HashMap::new();

        // Keywords
        captures.insert("keyword", Style::new().fg(Color::Magenta).bold());
        captures.insert("keyword.function", Style::new().fg(Color::Magenta).bold());
        captures.insert("keyword.return", Style::new().fg(Color::Magenta).bold());
        captures.insert("keyword.operator", Style::new().fg(Color::Magenta));
        captures.insert("keyword.import", Style::new().fg(Color::Magenta));
        captures.insert("keyword.conditional", Style::new().fg(Color::Magenta).bold());
        captures.insert("keyword.repeat", Style::new().fg(Color::Magenta).bold());

        // Types
        captures.insert("type", Style::new().fg(Color::Yellow));
        captures.insert("type.builtin", Style::new().fg(Color::Yellow).italic());
        captures.insert("type.qualifier", Style::new().fg(Color::Magenta));

        // Functions
        captures.insert("function", Style::new().fg(Color::Blue));
        captures.insert("function.method", Style::new().fg(Color::Blue));
        captures.insert("function.builtin", Style::new().fg(Color::Cyan));
        captures.insert("function.macro", Style::new().fg(Color::Cyan).bold());

        // Variables
        captures.insert("variable", Style::new());
        captures.insert("variable.parameter", Style::new().fg(Color::Red).italic());
        captures.insert("variable.builtin", Style::new().fg(Color::Red));
        captures.insert("variable.member", Style::new().fg(Color::Red));

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

        // Operators
        captures.insert("operator", Style::new().fg(Color::White));

        // Punctuation
        captures.insert("punctuation", Style::new().fg(Color::White));
        captures.insert("punctuation.bracket", Style::new().fg(Color::White));
        captures.insert("punctuation.delimiter", Style::new().fg(Color::White));
        captures.insert("punctuation.special", Style::new().fg(Color::Cyan));

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
}
