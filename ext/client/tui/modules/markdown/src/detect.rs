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
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|') && !trimmed.is_empty()
}

/// Check if a line is a delimiter row (`|---|---|`).
#[cfg_attr(coverage_nightly, coverage(off))]
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
/// delimiters. Single `*` (italic) is preserved -- ambiguous with list markers.
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
                    let top_border = super::layout::generate_border(
                        &col_widths,
                        '\u{250C}',
                        '\u{252C}',
                        '\u{2510}',
                    );
                    let bottom_border = super::layout::generate_border(
                        &col_widths,
                        '\u{2514}',
                        '\u{2534}',
                        '\u{2518}',
                    );
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
#[path = "detect_tests.rs"]
mod tests;
