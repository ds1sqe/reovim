//! Table detection in buffer content.
//!
//! Pure functions that analyze buffer lines to find markdown table regions.

/// A detected table region in the buffer.
#[derive(Debug)]
pub struct TableRegion {
    /// First table row (header).
    pub start_line: usize,
    /// Last table row (inclusive).
    pub end_line: usize,
    /// Delimiter row index.
    pub delimiter_line: usize,
    /// Max column widths across all rows.
    pub col_widths: Vec<usize>,
    /// Precomputed top border string.
    pub top_border: String,
    /// Precomputed bottom border string.
    pub bottom_border: String,
}

/// Check if a line looks like a table row (contains pipes).
pub fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|') && !trimmed.is_empty()
}

/// Check if a line is a delimiter row (`|---|---|`).
pub fn is_delimiter_row(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c == '|' || c == '-' || c == ':' || c == ' ')
        && trimmed.contains('-')
}

/// Strip inline markdown delimiters from text.
///
/// Removes `**` (bold), `~~` (strikethrough), and `` ` `` (inline code)
/// delimiters. Single `*` (italic) is preserved — ambiguous with list markers.
pub fn strip_inline_markdown(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
            }
            '~' if chars.peek() == Some(&'~') => {
                chars.next();
            }
            '`' => {}
            _ => result.push(ch),
        }
    }
    result
}

/// Parse cells from a table row (content between pipes).
///
/// Inline markdown delimiters (`**`, `~~`, `` ` ``) are stripped from cell
/// content so that display text (column widths, expanded rows) shows clean text.
pub fn parse_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return Vec::new();
    }
    trimmed[1..trimmed.len() - 1]
        .split('|')
        .map(|s| strip_inline_markdown(s.trim()))
        .collect()
}

/// Get column widths from a line (character count of each cell).
pub fn get_column_widths(line: &str) -> Vec<usize> {
    parse_cells(line)
        .iter()
        .map(|cell| cell.chars().count())
        .collect()
}

/// Detect table regions in buffer lines.
pub fn detect_tables(lines: &[String]) -> Vec<TableRegion> {
    let mut tables = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if is_table_line(&lines[i]) {
            let start = i;
            let mut end = i;
            let mut delimiter = None;
            while end < lines.len() && is_table_line(&lines[end]) {
                if delimiter.is_none() && is_delimiter_row(&lines[end]) {
                    delimiter = Some(end);
                }
                end += 1;
            }
            if let Some(delim) = delimiter.filter(|_| end > start) {
                let col_widths = super::layout::calculate_max_column_widths(lines, start, end - 1);
                if !col_widths.is_empty() {
                    let top_border = super::layout::generate_border(&col_widths, '┌', '┬', '┐');
                    let bottom_border = super::layout::generate_border(&col_widths, '└', '┴', '┘');
                    tables.push(TableRegion {
                        start_line: start,
                        end_line: end - 1,
                        delimiter_line: delim,
                        col_widths,
                        top_border,
                        bottom_border,
                    });
                }
            }
            i = end;
        } else {
            i += 1;
        }
    }
    tables
}

#[cfg(test)]
mod tests {
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
}
