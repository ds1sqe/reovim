//! Highlight group constants.
//!
//! Defines the standard highlight groups used by themes. Groups are string-based
//! for flexibility; themes map group names to styles.
//!
//! # Architecture
//!
//! This is part of the **mechanism layer**. It defines WHAT highlight groups exist,
//! but not HOW they are styled. Themes (in `builtin.rs`) provide the actual colors.
//!
//! # Groups
//!
//! Groups are organized into three categories:
//! - **Syntax** (13 groups): Code elements like keywords, functions, strings
//! - **UI** (17 groups): Editor chrome like statusline, line numbers, borders
//! - **Diagnostic** (4 groups): LSP diagnostics like errors and warnings
//!
//! Additional groups (e.g., rainbow brackets) are registered by modules via
//! `StyleGroupRegistry` at runtime.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::style::groups;
//!
//! let style = theme.get_style(groups::KEYWORD);
//! ```

// ============================================================================
// Syntax Groups (13 groups)
// ============================================================================

/// Keyword highlight (if, else, fn, let, match, loop, etc.)
pub const KEYWORD: &str = "keyword";

/// Function names
pub const FUNCTION: &str = "function";

/// Type names (struct, enum, trait)
pub const TYPE: &str = "type";

/// String literals ("hello")
pub const STRING: &str = "string";

/// Numeric literals (42, 3.14, 0xff)
pub const NUMBER: &str = "number";

/// Comments (// and /* */)
pub const COMMENT: &str = "comment";

/// Operators (+, -, *, /, =, ==)
pub const OPERATOR: &str = "operator";

/// Punctuation ((), {}, [], ;, :)
pub const PUNCTUATION: &str = "punctuation";

/// Variable names
pub const VARIABLE: &str = "variable";

/// Constants (`CONSTANT_NAME`)
pub const CONSTANT: &str = "constant";

/// Attributes (#[derive], @decorator)
pub const ATTRIBUTE: &str = "attribute";

/// Object/struct properties
pub const PROPERTY: &str = "property";

/// HTML/XML tags
pub const TAG: &str = "tag";

// ============================================================================
// UI Groups (17 groups)
// ============================================================================

/// Default background color
pub const BACKGROUND: &str = "background";

/// Default foreground color
pub const FOREGROUND: &str = "foreground";

/// Cursor color
pub const CURSOR: &str = "cursor";

/// Visual selection highlight
pub const SELECTION: &str = "selection";

/// Line number column
pub const LINE_NUMBER: &str = "line_number";

/// Active/cursor line number
pub const LINE_NUMBER_ACTIVE: &str = "line_number_active";

/// Statusline background
pub const STATUSLINE_BG: &str = "statusline_bg";

/// Statusline foreground
pub const STATUSLINE_FG: &str = "statusline_fg";

/// Normal mode indicator
pub const MODE_NORMAL: &str = "mode_normal";

/// Insert mode indicator
pub const MODE_INSERT: &str = "mode_insert";

/// Visual mode indicator
pub const MODE_VISUAL: &str = "mode_visual";

/// Command mode indicator
pub const MODE_COMMAND: &str = "mode_command";

/// UI borders
pub const BORDER: &str = "border";

/// Popup/floating window background
pub const POPUP_BG: &str = "popup_bg";

/// Popup/floating window foreground
pub const POPUP_FG: &str = "popup_fg";

/// Menu selected item
pub const MENU_SELECTED: &str = "menu_selected";

/// Search match highlight
pub const SEARCH_MATCH: &str = "search_match";

// ============================================================================
// Diagnostic Groups (4 groups)
// ============================================================================

/// Error diagnostic
pub const DIAGNOSTIC_ERROR: &str = "diagnostic.error";

/// Warning diagnostic
pub const DIAGNOSTIC_WARN: &str = "diagnostic.warn";

/// Info diagnostic
pub const DIAGNOSTIC_INFO: &str = "diagnostic.info";

/// Hint diagnostic
pub const DIAGNOSTIC_HINT: &str = "diagnostic.hint";

// ============================================================================
// All Groups (for testing completeness)
// ============================================================================

/// All built-in highlight groups (34 total).
///
/// Used by tests to verify theme completeness.
/// Additional groups (e.g., rainbow brackets) are registered by modules
/// via `StyleGroupRegistry` at runtime.
pub const ALL_GROUPS: &[&str] = &[
    // Syntax (13)
    KEYWORD,
    FUNCTION,
    TYPE,
    STRING,
    NUMBER,
    COMMENT,
    OPERATOR,
    PUNCTUATION,
    VARIABLE,
    CONSTANT,
    ATTRIBUTE,
    PROPERTY,
    TAG,
    // UI (17)
    BACKGROUND,
    FOREGROUND,
    CURSOR,
    SELECTION,
    LINE_NUMBER,
    LINE_NUMBER_ACTIVE,
    STATUSLINE_BG,
    STATUSLINE_FG,
    MODE_NORMAL,
    MODE_INSERT,
    MODE_VISUAL,
    MODE_COMMAND,
    BORDER,
    POPUP_BG,
    POPUP_FG,
    MENU_SELECTED,
    SEARCH_MATCH,
    // Diagnostic (4)
    DIAGNOSTIC_ERROR,
    DIAGNOSTIC_WARN,
    DIAGNOSTIC_INFO,
    DIAGNOSTIC_HINT,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_groups_count() {
        // 13 syntax + 17 UI + 4 diagnostic = 34 groups
        // Module-specific groups (e.g., rainbow brackets) are registered via StyleGroupRegistry
        assert_eq!(ALL_GROUPS.len(), 34);
    }

    #[test]
    fn test_syntax_groups_count() {
        let syntax_groups = [
            KEYWORD,
            FUNCTION,
            TYPE,
            STRING,
            NUMBER,
            COMMENT,
            OPERATOR,
            PUNCTUATION,
            VARIABLE,
            CONSTANT,
            ATTRIBUTE,
            PROPERTY,
            TAG,
        ];
        assert_eq!(syntax_groups.len(), 13);
    }

    #[test]
    fn test_ui_groups_count() {
        let ui_groups = [
            BACKGROUND,
            FOREGROUND,
            CURSOR,
            SELECTION,
            LINE_NUMBER,
            LINE_NUMBER_ACTIVE,
            STATUSLINE_BG,
            STATUSLINE_FG,
            MODE_NORMAL,
            MODE_INSERT,
            MODE_VISUAL,
            MODE_COMMAND,
            BORDER,
            POPUP_BG,
            POPUP_FG,
            MENU_SELECTED,
            SEARCH_MATCH,
        ];
        assert_eq!(ui_groups.len(), 17);
    }

    #[test]
    fn test_diagnostic_groups_count() {
        let diagnostic_groups = [
            DIAGNOSTIC_ERROR,
            DIAGNOSTIC_WARN,
            DIAGNOSTIC_INFO,
            DIAGNOSTIC_HINT,
        ];
        assert_eq!(diagnostic_groups.len(), 4);
    }

    #[test]
    fn test_no_duplicate_groups() {
        use std::collections::HashSet;
        let unique: HashSet<_> = ALL_GROUPS.iter().collect();
        assert_eq!(unique.len(), ALL_GROUPS.len(), "duplicate group names found");
    }
}
