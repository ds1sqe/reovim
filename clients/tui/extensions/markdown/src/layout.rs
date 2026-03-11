//! Table layout computation.
//!
//! Pure functions for calculating column widths, generating borders,
//! and building expanded table rows with box-drawing characters.

use super::detect::{get_column_widths, is_delimiter_row, parse_cells};

/// Calculate max column widths across all rows in a table range.
pub fn calculate_max_column_widths(lines: &[String], start: usize, end: usize) -> Vec<usize> {
    let mut max_widths: Vec<usize> = Vec::new();
    for line in lines.iter().take(end + 1).skip(start) {
        if is_delimiter_row(line) {
            continue;
        }
        let widths = get_column_widths(line);
        for (i, w) in widths.into_iter().enumerate() {
            if i >= max_widths.len() {
                max_widths.push(w);
            } else if w > max_widths[i] {
                max_widths[i] = w;
            }
        }
    }
    max_widths
}

/// Generate a horizontal border from column widths.
///
/// Each cell segment is `width + 2` chars (1 space padding on each side).
pub fn generate_border(widths: &[usize], start: char, mid: char, end: char) -> String {
    if widths.is_empty() {
        return String::new();
    }
    let mut result = String::new();
    result.push(start);
    for (i, &w) in widths.iter().enumerate() {
        for _ in 0..(w + 2) {
            result.push('─');
        }
        if i < widths.len() - 1 {
            result.push(mid);
        }
    }
    result.push(end);
    result
}

/// Build an expanded table row with box-drawing borders.
///
/// Headers are centered, data rows are left-aligned.
pub fn build_expanded_row(line: &str, max_widths: &[usize], is_header: bool) -> String {
    let cells = parse_cells(line);
    if cells.is_empty() {
        return line.to_string();
    }
    let mut result = String::from("│");
    for (i, cell) in cells.iter().enumerate() {
        let cell_len = cell.chars().count();
        let width = max_widths.get(i).copied().unwrap_or(cell_len);
        let padded = if is_header {
            let total_pad = width.saturating_sub(cell_len);
            let left_pad = total_pad / 2;
            let right_pad = total_pad - left_pad;
            format!(" {}{cell}{} ", " ".repeat(left_pad), " ".repeat(right_pad))
        } else {
            format!(" {cell:width$} ")
        };
        result.push_str(&padded);
        result.push('│');
    }
    result
}

#[cfg(test)]
mod tests {
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
}
