use super::*;

#[test]
fn test_all_groups_count() {
    // 13 syntax base + 31 syntax sub-categories + 17 UI + 4 diagnostic + 8 gutter = 73 groups
    // Module-specific groups (e.g., rainbow brackets) are registered via StyleGroupRegistry
    assert_eq!(ALL_GROUPS.len(), 73);
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
