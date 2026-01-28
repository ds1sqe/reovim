//! Built-in UI icons
//!
//! Provides icons for:
//! - Diagnostics (error, warn, hint, info)
//! - Git status (added, modified, removed)
//! - Tree drawing (branch, vertical, etc.)
//! - LSP completion kinds
//! - Spinners

use super::{IconSet, SpinnerDef, registry::IconProvider};

/// Built-in UI icon provider (priority 0)
pub struct BuiltinUiIconProvider;

impl IconProvider for BuiltinUiIconProvider {
    #[allow(clippy::too_many_lines, clippy::match_same_arms)]
    fn icon(&self, key: &str, set: IconSet) -> Option<&'static str> {
        let icon = match key {
            // Diagnostics
            "diag.error" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "✖ ",
                IconSet::Ascii => "E ",
            },
            "diag.warn" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "⚠ ",
                IconSet::Ascii => "W ",
            },
            "diag.hint" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "💡 ",
                IconSet::Ascii => "H ",
            },
            "diag.info" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "ℹ ",
                IconSet::Ascii => "I ",
            },

            // Git status
            "git.added" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "+ ",
                IconSet::Ascii => "+ ",
            },
            "git.modified" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "~ ",
                IconSet::Ascii => "~ ",
            },
            "git.removed" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "- ",
                IconSet::Ascii => "- ",
            },
            "git.renamed" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "→ ",
                IconSet::Ascii => "R ",
            },
            "git.untracked" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "? ",
                IconSet::Ascii => "? ",
            },
            "git.ignored" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "◌ ",
                IconSet::Ascii => "I ",
            },

            // Tree drawing
            "tree.branch" => match set {
                IconSet::Nerd => "├── ",
                IconSet::Unicode => "├── ",
                IconSet::Ascii => "|-- ",
            },
            "tree.last_branch" => match set {
                IconSet::Nerd => "└── ",
                IconSet::Unicode => "└── ",
                IconSet::Ascii => "`-- ",
            },
            "tree.vertical" => match set {
                IconSet::Nerd => "│   ",
                IconSet::Unicode => "│   ",
                IconSet::Ascii => "|   ",
            },
            "tree.indent" => match set {
                IconSet::Nerd => "    ",
                IconSet::Unicode => "    ",
                IconSet::Ascii => "    ",
            },

            // Expand/collapse indicators
            "node.expanded" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "▼ ",
                IconSet::Ascii => "v ",
            },
            "node.collapsed" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "▶ ",
                IconSet::Ascii => "> ",
            },

            // UI elements
            "checkbox.checked" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "☑ ",
                IconSet::Ascii => "[x]",
            },
            "checkbox.unchecked" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "☐ ",
                IconSet::Ascii => "[ ]",
            },
            "indicator.selected" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "● ",
                IconSet::Ascii => "* ",
            },
            "indicator.unselected" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "○ ",
                IconSet::Ascii => "  ",
            },
            "arrow.right" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "→ ",
                IconSet::Ascii => "->",
            },
            "arrow.left" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "← ",
                IconSet::Ascii => "<-",
            },

            _ => return None,
        };
        Some(icon)
    }

    #[allow(clippy::too_many_lines, clippy::match_same_arms)]
    fn kind_icon(&self, kind: &str, set: IconSet) -> Option<&'static str> {
        let icon = match kind {
            // From user's icons.lua
            "Array" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "[] ",
                IconSet::Ascii => "[] ",
            },
            "Boolean" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "⊤ ",
                IconSet::Ascii => "bl ",
            },
            "Class" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "◆ ",
                IconSet::Ascii => "cl ",
            },
            "Color" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🎨 ",
                IconSet::Ascii => "co ",
            },
            "Constant" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "π ",
                IconSet::Ascii => "cn ",
            },
            "Constructor" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "⬡ ",
                IconSet::Ascii => "ct ",
            },
            "Enum" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "∈ ",
                IconSet::Ascii => "en ",
            },
            "EnumMember" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "∋ ",
                IconSet::Ascii => "em ",
            },
            "Event" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "⚡ ",
                IconSet::Ascii => "ev ",
            },
            "Field" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "□ ",
                IconSet::Ascii => "fd ",
            },
            "File" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "📄 ",
                IconSet::Ascii => "fi ",
            },
            "Folder" => match set {
                IconSet::Nerd => "󰉋 ",
                IconSet::Unicode => "📁 ",
                IconSet::Ascii => "fo ",
            },
            "Function" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "ƒ ",
                IconSet::Ascii => "fn ",
            },
            "Interface" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "◇ ",
                IconSet::Ascii => "if ",
            },
            "Key" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🔑 ",
                IconSet::Ascii => "ky ",
            },
            "Keyword" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "⌘ ",
                IconSet::Ascii => "kw ",
            },
            "Method" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "ƒ ",
                IconSet::Ascii => "mt ",
            },
            "Module" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "□ ",
                IconSet::Ascii => "md ",
            },
            "Namespace" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "⬡ ",
                IconSet::Ascii => "ns ",
            },
            "Null" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "∅ ",
                IconSet::Ascii => "nu ",
            },
            "Number" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "# ",
                IconSet::Ascii => "nm ",
            },
            "Object" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "◎ ",
                IconSet::Ascii => "ob ",
            },
            "Operator" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "± ",
                IconSet::Ascii => "op ",
            },
            "Package" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "📦 ",
                IconSet::Ascii => "pk ",
            },
            "Property" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "□ ",
                IconSet::Ascii => "pr ",
            },
            "Reference" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "& ",
                IconSet::Ascii => "rf ",
            },
            "Snippet" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "✂ ",
                IconSet::Ascii => "sn ",
            },
            "String" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "\" ",
                IconSet::Ascii => "st ",
            },
            "Struct" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "◇ ",
                IconSet::Ascii => "sr ",
            },
            "Text" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "T ",
                IconSet::Ascii => "tx ",
            },
            "TypeParameter" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "T ",
                IconSet::Ascii => "tp ",
            },
            "Unit" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "() ",
                IconSet::Ascii => "un ",
            },
            "Value" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "= ",
                IconSet::Ascii => "vl ",
            },
            "Variable" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "𝑥 ",
                IconSet::Ascii => "vr ",
            },

            _ => return None,
        };
        Some(icon)
    }

    fn priority(&self) -> u32 {
        0 // Built-in, lowest priority
    }

    fn name(&self) -> &'static str {
        "builtin-ui"
    }
}

// Spinner definitions

/// Simple braille spinner
pub const SPINNER_SIMPLE: SpinnerDef = SpinnerDef {
    nerd: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
    unicode: &["◐", "◓", "◑", "◒"],
    ascii: &["-", "\\", "|", "/"],
};

/// Dots spinner
pub const SPINNER_DOTS: SpinnerDef = SpinnerDef {
    nerd: &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"],
    unicode: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
    ascii: &[".", "..", "...", ".."],
};

/// Bouncing bar spinner
pub const SPINNER_BOUNCE: SpinnerDef = SpinnerDef {
    nerd: &[
        "[    ]", "[=   ]", "[==  ]", "[=== ]", "[ ===]", "[  ==]", "[   =]", "[    ]",
    ],
    unicode: &[
        "▐    ▌",
        "▐=   ▌",
        "▐==  ▌",
        "▐=== ▌",
        "▐ ===▌",
        "▐  ==▌",
        "▐   =▌",
    ],
    ascii: &[
        "[    ]", "[=   ]", "[==  ]", "[=== ]", "[ ===]", "[  ==]", "[   =]",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_icons() {
        let provider = BuiltinUiIconProvider;
        assert!(provider.icon("diag.error", IconSet::Nerd).is_some());
        assert!(provider.icon("diag.warn", IconSet::Unicode).is_some());
        assert!(provider.icon("diag.hint", IconSet::Ascii).is_some());
    }

    #[test]
    fn test_kind_icons() {
        let provider = BuiltinUiIconProvider;
        assert!(provider.kind_icon("Function", IconSet::Nerd).is_some());
        assert!(provider.kind_icon("Variable", IconSet::Unicode).is_some());
        assert!(provider.kind_icon("Class", IconSet::Ascii).is_some());
    }

    #[test]
    fn test_spinner() {
        assert!(!SPINNER_SIMPLE.nerd.is_empty());
        assert!(!SPINNER_SIMPLE.unicode.is_empty());
        assert!(!SPINNER_SIMPLE.ascii.is_empty());
    }
}
