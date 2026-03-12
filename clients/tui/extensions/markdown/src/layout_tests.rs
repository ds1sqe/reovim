use super::*;

#[test]
fn test_calculate_max_column_widths() {
    let lines = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| long | x |".to_string(),
    ];
    let widths = calculate_max_column_widths(&lines, 0, 2);
    assert_eq!(widths, vec![4, 1]); // "long" = 4, "B" and "x" = 1
}

#[test]
fn test_calculate_max_column_widths_skips_delimiter() {
    let lines = vec!["| a | b |".to_string(), "|---|---|".to_string()];
    let widths = calculate_max_column_widths(&lines, 0, 1);
    assert_eq!(widths, vec![1, 1]);
}

#[test]
fn test_generate_border_top() {
    let border = generate_border(&[3, 5], '┌', '┬', '┐');
    assert_eq!(border, "┌─────┬───────┐");
}

#[test]
fn test_generate_border_mid() {
    let border = generate_border(&[3, 5], '├', '┼', '┤');
    assert_eq!(border, "├─────┼───────┤");
}

#[test]
fn test_generate_border_bottom() {
    let border = generate_border(&[3, 5], '└', '┴', '┘');
    assert_eq!(border, "└─────┴───────┘");
}

#[test]
fn test_generate_border_empty_widths() {
    assert_eq!(generate_border(&[], '┌', '┬', '┐'), "");
}

#[test]
fn test_generate_border_single_column() {
    let border = generate_border(&[4], '┌', '┬', '┐');
    assert_eq!(border, "┌──────┐");
}

#[test]
fn test_build_expanded_row_header_centered() {
    let row = build_expanded_row("| A | B |", &[5, 5], true);
    assert!(row.starts_with('│'));
    assert!(row.ends_with('│'));
    assert!(row.contains("  A  "));
    assert!(row.contains("  B  "));
}

#[test]
fn test_build_expanded_row_data_left_aligned() {
    let row = build_expanded_row("| x | y |", &[5, 5], false);
    assert!(row.starts_with('│'));
    assert!(row.ends_with('│'));
    assert!(row.contains(" x"));
}

#[test]
fn test_build_expanded_row_empty_cells() {
    let row = build_expanded_row("| | |", &[3, 3], false);
    assert!(row.starts_with('│'));
    assert!(row.ends_with('│'));
}

#[test]
fn test_build_expanded_row_no_cells() {
    let row = build_expanded_row("plain text", &[5], false);
    assert_eq!(row, "plain text");
}
