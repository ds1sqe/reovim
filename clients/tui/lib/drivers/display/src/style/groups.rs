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
//! Groups are organized into four categories:
//! - **Syntax** (13 groups): Code elements like keywords, functions, strings
//! - **UI** (17 groups): Editor chrome like statusline, line numbers, borders
//! - **Diagnostic** (4 groups): LSP diagnostics like errors and warnings
//! - **Gutter** (8 groups): Annotation system groups (signs, git, folds)
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
// Syntax Sub-Category Groups
// ============================================================================
//
// These sub-categories inherit from their parent via ThemeManager's
// hierarchical fallback. For example, "keyword.function" falls back
// to "keyword" if no explicit theme entry exists.

/// Control flow keywords (if, else, while, for, return, break)
pub const KEYWORD_CONTROL: &str = "keyword.control";

/// Function definition keywords (fn, def, func)
pub const KEYWORD_FUNCTION: &str = "keyword.function";

/// Type definition keywords (struct, enum, trait, class)
pub const KEYWORD_TYPE: &str = "keyword.type";

/// Keyword operators (and, or, not, in, as)
pub const KEYWORD_OPERATOR: &str = "keyword.operator";

/// Built-in types (i32, str, bool, Vec, String)
pub const TYPE_BUILTIN: &str = "type.builtin";

/// Built-in functions (println, format, len)
pub const FUNCTION_BUILTIN: &str = "function.builtin";

/// Macro invocations (println!, vec!, derive)
pub const FUNCTION_MACRO: &str = "function.macro";

/// Method calls
pub const FUNCTION_METHOD: &str = "function.method";

/// Built-in variables (self, this, super)
pub const VARIABLE_BUILTIN: &str = "variable.builtin";

/// Function parameters
pub const VARIABLE_PARAMETER: &str = "variable.parameter";

/// Struct/object field access
pub const VARIABLE_FIELD: &str = "variable.field";

/// String escape sequences (\\n, \\t, etc.)
pub const STRING_ESCAPE: &str = "string.escape";

/// Doc comments (/// or /** */)
pub const COMMENT_DOC: &str = "comment.doc";

/// Brackets: (), {}, []
pub const PUNCTUATION_BRACKET: &str = "punctuation.bracket";

/// Delimiters: , ; :
pub const PUNCTUATION_DELIMITER: &str = "punctuation.delimiter";

/// Module/package namespaces
pub const NAMESPACE: &str = "namespace";

/// Constructors
pub const CONSTRUCTOR: &str = "constructor";

/// Labels (loop labels, goto targets)
pub const LABEL: &str = "label";

/// Boolean literals (true, false)
pub const BOOLEAN: &str = "boolean";

/// Character literals ('a')
pub const CHARACTER: &str = "character";

/// Markup headings
pub const MARKUP_HEADING: &str = "markup.heading";

/// Bold text
pub const MARKUP_BOLD: &str = "markup.bold";

/// Italic text
pub const MARKUP_ITALIC: &str = "markup.italic";

/// Strikethrough text
pub const MARKUP_STRIKETHROUGH: &str = "markup.strikethrough";

/// Link text
pub const MARKUP_LINK: &str = "markup.link";

/// Link URLs
pub const MARKUP_LINK_URL: &str = "markup.link.url";

/// List markers
pub const MARKUP_LIST: &str = "markup.list";

/// Raw/code blocks
pub const MARKUP_RAW: &str = "markup.raw";

/// Inline code
pub const MARKUP_RAW_INLINE: &str = "markup.raw.inline";

/// Embedded content (e.g., SQL in strings)
pub const EMBEDDED: &str = "embedded";

/// Special tokens
pub const SPECIAL: &str = "special";

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

/// Warning diagnostic.
///
/// Uses `"diagnostic.warning"` to match the server-side
/// `HighlightCategory::DIAGNOSTIC_WARNING` and tree-sitter convention.
pub const DIAGNOSTIC_WARN: &str = "diagnostic.warning";

/// Info diagnostic
pub const DIAGNOSTIC_INFO: &str = "diagnostic.info";

/// Hint diagnostic
pub const DIAGNOSTIC_HINT: &str = "diagnostic.hint";

// ============================================================================
// Gutter/Annotation Groups (8 groups) - Issue #455
// ============================================================================

/// Sign column background (where signs/annotations appear)
pub const SIGN_COLUMN: &str = "sign_column";

/// Separator between gutter and content area
pub const GUTTER_SEPARATOR: &str = "gutter_separator";

/// Git added line indicator
pub const GIT_ADD: &str = "git.add";

/// Git changed line indicator
pub const GIT_CHANGE: &str = "git.change";

/// Git deleted line indicator
pub const GIT_DELETE: &str = "git.delete";

/// Fold marker - region is open/expanded
pub const FOLD_OPEN: &str = "fold.open";

/// Fold marker - region is closed/collapsed
pub const FOLD_CLOSED: &str = "fold.closed";

/// Bookmark indicator
pub const BOOKMARK: &str = "bookmark";

// ============================================================================
// All Groups (for testing completeness)
// ============================================================================

/// All built-in highlight groups (73 total).
///
/// Used by tests to verify theme completeness.
/// Additional groups (e.g., rainbow brackets) are registered by modules
/// via `StyleGroupRegistry` at runtime.
pub const ALL_GROUPS: &[&str] = &[
    // Syntax base (13)
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
    // Syntax sub-categories (30)
    KEYWORD_CONTROL,
    KEYWORD_FUNCTION,
    KEYWORD_TYPE,
    KEYWORD_OPERATOR,
    TYPE_BUILTIN,
    FUNCTION_BUILTIN,
    FUNCTION_MACRO,
    FUNCTION_METHOD,
    VARIABLE_BUILTIN,
    VARIABLE_PARAMETER,
    VARIABLE_FIELD,
    STRING_ESCAPE,
    COMMENT_DOC,
    PUNCTUATION_BRACKET,
    PUNCTUATION_DELIMITER,
    NAMESPACE,
    CONSTRUCTOR,
    LABEL,
    BOOLEAN,
    CHARACTER,
    MARKUP_HEADING,
    MARKUP_BOLD,
    MARKUP_ITALIC,
    MARKUP_STRIKETHROUGH,
    MARKUP_LINK,
    MARKUP_LINK_URL,
    MARKUP_LIST,
    MARKUP_RAW,
    MARKUP_RAW_INLINE,
    EMBEDDED,
    SPECIAL,
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
    // Gutter/Annotation (8) - Issue #455
    SIGN_COLUMN,
    GUTTER_SEPARATOR,
    GIT_ADD,
    GIT_CHANGE,
    GIT_DELETE,
    FOLD_OPEN,
    FOLD_CLOSED,
    BOOKMARK,
];

#[cfg(test)]
#[path = "groups_tests.rs"]
mod tests;
