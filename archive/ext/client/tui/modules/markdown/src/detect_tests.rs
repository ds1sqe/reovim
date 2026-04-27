use super::*;

#[test]
fn test_is_table_line_valid() {
    assert!(is_table_line("| a | b |"));
    assert!(is_table_line("|---|---|"));
    assert!(is_table_line("| x |"));
}

#[test]
fn test_is_table_line_invalid() {
    assert!(!is_table_line("plain text"));
    assert!(!is_table_line(""));
    assert!(!is_table_line("   "));
}

#[test]
fn test_is_delimiter_row_valid() {
    assert!(is_delimiter_row("|---|---|"));
    assert!(is_delimiter_row("| --- | --- |"));
    assert!(is_delimiter_row("|:---|---:|"));
}

#[test]
fn test_is_delimiter_row_invalid() {
    assert!(!is_delimiter_row("| a | b |"));
    assert!(!is_delimiter_row("plain text"));
    assert!(!is_delimiter_row("---")); // No leading pipe
}

#[test]
fn test_strip_bold() {
    assert_eq!(strip_inline_markdown("**bold**"), "bold");
}

#[test]
fn test_strip_strikethrough() {
    assert_eq!(strip_inline_markdown("~~strike~~"), "strike");
}

#[test]
fn test_strip_backtick() {
    assert_eq!(strip_inline_markdown("`code`"), "code");
}

#[test]
fn test_strip_mixed() {
    assert_eq!(strip_inline_markdown("**a** and ~~b~~"), "a and b");
}

#[test]
fn test_strip_plain() {
    assert_eq!(strip_inline_markdown("plain text"), "plain text");
}

#[test]
fn test_strip_single_star_preserved() {
    assert_eq!(strip_inline_markdown("*italic*"), "*italic*");
}

#[test]
fn test_parse_cells_basic() {
    let cells = parse_cells("| a | b | c |");
    assert_eq!(cells, vec!["a", "b", "c"]);
}

#[test]
fn test_parse_cells_strips_markdown() {
    let cells = parse_cells("| **a** | ~~b~~ |");
    assert_eq!(cells, vec!["a", "b"]);
}

#[test]
fn test_parse_cells_empty() {
    let cells = parse_cells("| | |");
    assert_eq!(cells, vec!["", ""]);
}

#[test]
fn test_parse_cells_no_pipes() {
    assert!(parse_cells("plain text").is_empty());
}

#[test]
fn test_parse_cells_no_trailing_pipe() {
    assert!(parse_cells("| a | b").is_empty());
}

#[test]
fn test_get_column_widths() {
    let widths = get_column_widths("| abc | de |");
    assert_eq!(widths, vec![3, 2]);
}

#[test]
fn test_detect_tables_single() {
    let lines = vec![
        "text".to_string(),
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
        "more".to_string(),
    ];
    let tables = detect_tables(&lines);
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].start_line, 1);
    assert_eq!(tables[0].end_line, 3);
    assert_eq!(tables[0].delimiter_line, 2);
}

#[test]
fn test_detect_tables_multiple() {
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
        "gap".to_string(),
        "| X | Y |".to_string(),
        "|---|---|".to_string(),
        "| 3 | 4 |".to_string(),
    ];
    let tables = detect_tables(&lines);
    assert_eq!(tables.len(), 2);
    assert_eq!(tables[0].start_line, 0);
    assert_eq!(tables[0].end_line, 2);
    assert_eq!(tables[1].start_line, 4);
    assert_eq!(tables[1].end_line, 6);
}

#[test]
fn test_detect_tables_no_table() {
    let lines = vec!["hello".to_string(), "world".to_string()];
    assert!(detect_tables(&lines).is_empty());
}

#[test]
fn test_detect_tables_no_delimiter() {
    let lines = vec!["| A | B |".to_string(), "| 1 | 2 |".to_string()];
    assert!(detect_tables(&lines).is_empty());
}

#[test]
fn test_detect_tables_has_borders() {
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    let tables = detect_tables(&lines);
    assert!(!tables[0].top_border.is_empty());
    assert!(tables[0].top_border.starts_with('┌'));
    assert!(!tables[0].bottom_border.is_empty());
    assert!(tables[0].bottom_border.starts_with('└'));
}
