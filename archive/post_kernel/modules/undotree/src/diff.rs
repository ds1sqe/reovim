//! Diff formatting for undo node edits.
//!
//! Converts `Vec<Edit>` to unified diff format for preview display.
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_undotree::diff::{format_edits_as_diff, DiffLine};
//! use reovim_kernel::api::v1::{Edit, Position};
//!
//! let edits = vec![Edit::insert(Position::new(0, 0), "Hello")];
//! let lines = format_edits_as_diff(&edits);
//! assert!(lines.iter().any(|l| l.text.contains("+Hello")));
//! ```

use reovim_kernel::api::v1::Edit;

/// A line in the diff output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// The text content.
    pub text: String,
    /// The line type (context, insert, delete, header).
    pub line_type: DiffLineType,
}

impl DiffLine {
    /// Create a new diff line.
    #[must_use]
    pub const fn new(text: String, line_type: DiffLineType) -> Self {
        Self { text, line_type }
    }

    /// Create a header line.
    #[must_use]
    pub fn header(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            line_type: DiffLineType::Header,
        }
    }

    /// Create an insert line.
    #[must_use]
    pub fn insert(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            line_type: DiffLineType::Insert,
        }
    }

    /// Create a delete line.
    #[must_use]
    pub fn delete(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            line_type: DiffLineType::Delete,
        }
    }
}

/// Type of diff line for styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineType {
    /// Header line (@@, ---, +++).
    Header,
    /// Context line (unchanged).
    Context,
    /// Insertion (+).
    Insert,
    /// Deletion (-).
    Delete,
}

/// Format edits as diff lines.
///
/// Groups edits by position and formats them in unified diff style.
/// Each edit becomes one or more diff lines depending on whether
/// the text contains newlines.
///
/// # Arguments
///
/// * `edits` - The edits to format
///
/// # Returns
///
/// A vector of diff lines suitable for display.
#[must_use]
pub fn format_edits_as_diff(edits: &[Edit]) -> Vec<DiffLine> {
    if edits.is_empty() {
        return vec![DiffLine::header("(no changes)")];
    }

    let mut lines = Vec::new();

    // Add header
    lines.push(DiffLine::header("--- (before)"));
    lines.push(DiffLine::header("+++ (after)"));

    for edit in edits {
        let pos = edit.position();
        let text = edit.text();

        // Add position marker for each edit
        lines.push(DiffLine::header(format!("@@ L{},C{} @@", pos.line + 1, pos.column + 1)));

        let (prefix, line_type) = if edit.is_insert() {
            ("+", DiffLineType::Insert)
        } else {
            ("-", DiffLineType::Delete)
        };

        // Handle multi-line text
        if text.is_empty() {
            lines.push(DiffLine::new(format!("{prefix}(empty)"), line_type));
        } else {
            for line_text in text.lines() {
                lines.push(DiffLine::new(format!("{prefix}{line_text}"), line_type));
            }
            // If text ends with newline, the last line is empty but we want to show it
            if text.ends_with('\n') {
                lines.push(DiffLine::new(prefix.to_string(), line_type));
            }
        }
    }

    lines
}

/// Get summary of edit count.
///
/// # Arguments
///
/// * `edits` - The edits to summarize
///
/// # Returns
///
/// A string like "2 insert(s), 1 delete(s)".
#[must_use]
pub fn edit_summary(edits: &[Edit]) -> String {
    let inserts = edits.iter().filter(|e| e.is_insert()).count();
    let deletes = edits.iter().filter(|e| e.is_delete()).count();
    format!("{inserts} insert(s), {deletes} delete(s)")
}

/// Options for diff formatting.
#[derive(Debug, Clone)]
pub struct DiffOptions {
    /// Maximum lines per edit to show before truncating.
    pub max_lines_per_edit: usize,
    /// Text to show when truncating.
    pub truncation_text: &'static str,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            max_lines_per_edit: 10,
            truncation_text: "...(truncated)",
        }
    }
}

/// Format edits as diff lines with options.
///
/// Like [`format_edits_as_diff`] but with configurable truncation for large edits.
///
/// # Arguments
///
/// * `edits` - The edits to format
/// * `options` - Formatting options (truncation limits)
///
/// # Returns
///
/// A vector of diff lines suitable for display.
#[must_use]
pub fn format_edits_as_diff_with_options(edits: &[Edit], options: &DiffOptions) -> Vec<DiffLine> {
    if edits.is_empty() {
        return vec![DiffLine::header("(no changes)")];
    }

    let mut lines = Vec::new();

    // Add header
    lines.push(DiffLine::header("--- (before)"));
    lines.push(DiffLine::header("+++ (after)"));

    for edit in edits {
        let pos = edit.position();
        let text = edit.text();

        // Add position marker for each edit
        lines.push(DiffLine::header(format!("@@ L{},C{} @@", pos.line + 1, pos.column + 1)));

        let (prefix, line_type) = if edit.is_insert() {
            ("+", DiffLineType::Insert)
        } else {
            ("-", DiffLineType::Delete)
        };

        // Handle multi-line text with truncation
        if text.is_empty() {
            lines.push(DiffLine::new(format!("{prefix}(empty)"), line_type));
        } else {
            let text_lines: Vec<&str> = text.lines().collect();
            let total_lines = text_lines.len() + usize::from(text.ends_with('\n'));

            for (i, line_text) in text_lines.iter().enumerate() {
                if i >= options.max_lines_per_edit {
                    let remaining = total_lines - i;
                    lines.push(DiffLine::new(
                        format!("{prefix}{} (+{remaining} more lines)", options.truncation_text),
                        line_type,
                    ));
                    break;
                }
                lines.push(DiffLine::new(format!("{prefix}{line_text}"), line_type));
            }

            // If text ends with newline and not truncated, show the empty line
            if text.ends_with('\n') && text_lines.len() < options.max_lines_per_edit {
                lines.push(DiffLine::new(prefix.to_string(), line_type));
            }
        }
    }

    lines
}

/// Check if edits should be summarized due to size.
///
/// Returns true if the edits are too large and should use truncation
/// or summarization.
///
/// # Arguments
///
/// * `edits` - The edits to check
///
/// # Returns
///
/// `true` if edits exceed size thresholds.
#[must_use]
pub fn should_summarize(edits: &[Edit]) -> bool {
    const MAX_EDITS: usize = 20;
    const MAX_TOTAL_TEXT: usize = 1000;

    edits.len() > MAX_EDITS || edits.iter().map(|e| e.text().len()).sum::<usize>() > MAX_TOTAL_TEXT
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::Position};

    #[test]
    fn test_format_empty_edits() {
        let edits: Vec<Edit> = vec![];
        let lines = format_edits_as_diff(&edits);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "(no changes)");
        assert_eq!(lines[0].line_type, DiffLineType::Header);
    }

    #[test]
    fn test_format_single_insert() {
        let edits = vec![Edit::insert(Position::new(0, 0), "Hello")];
        let lines = format_edits_as_diff(&edits);

        // Should have: header (---), header (+++), position (@@), content (+Hello)
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].text, "--- (before)");
        assert_eq!(lines[1].text, "+++ (after)");
        assert_eq!(lines[2].text, "@@ L1,C1 @@");
        assert_eq!(lines[3].text, "+Hello");
        assert_eq!(lines[3].line_type, DiffLineType::Insert);
    }

    #[test]
    fn test_format_single_delete() {
        let edits = vec![Edit::delete(Position::new(2, 5), "World")];
        let lines = format_edits_as_diff(&edits);

        assert_eq!(lines.len(), 4);
        assert_eq!(lines[2].text, "@@ L3,C6 @@");
        assert_eq!(lines[3].text, "-World");
        assert_eq!(lines[3].line_type, DiffLineType::Delete);
    }

    #[test]
    fn test_format_multiline_text() {
        let edits = vec![Edit::insert(Position::new(0, 0), "Line1\nLine2\nLine3")];
        let lines = format_edits_as_diff(&edits);

        // header (2) + position (1) + 3 lines
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[3].text, "+Line1");
        assert_eq!(lines[4].text, "+Line2");
        assert_eq!(lines[5].text, "+Line3");
    }

    #[test]
    fn test_format_text_ending_with_newline() {
        let edits = vec![Edit::insert(Position::new(0, 0), "Hello\n")];
        let lines = format_edits_as_diff(&edits);

        // header (2) + position (1) + line + empty line marker
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[3].text, "+Hello");
        assert_eq!(lines[4].text, "+");
    }

    #[test]
    fn test_format_empty_text() {
        let edits = vec![Edit::insert(Position::new(0, 0), "")];
        let lines = format_edits_as_diff(&edits);

        assert_eq!(lines.len(), 4);
        assert_eq!(lines[3].text, "+(empty)");
    }

    #[test]
    fn test_format_mixed_edits() {
        let edits = vec![
            Edit::delete(Position::new(0, 0), "old"),
            Edit::insert(Position::new(0, 0), "new"),
        ];
        let lines = format_edits_as_diff(&edits);

        // header (2) + 2*(position + content)
        assert_eq!(lines.len(), 6);
        assert!(lines.iter().any(|l| l.text == "-old"));
        assert!(lines.iter().any(|l| l.text == "+new"));
    }

    #[test]
    fn test_edit_summary_empty() {
        let edits: Vec<Edit> = vec![];
        assert_eq!(edit_summary(&edits), "0 insert(s), 0 delete(s)");
    }

    #[test]
    fn test_edit_summary_mixed() {
        let edits = vec![
            Edit::insert(Position::new(0, 0), "a"),
            Edit::insert(Position::new(0, 0), "b"),
            Edit::delete(Position::new(0, 0), "c"),
        ];
        assert_eq!(edit_summary(&edits), "2 insert(s), 1 delete(s)");
    }

    #[test]
    fn test_diff_line_constructors() {
        let header = DiffLine::header("test");
        assert_eq!(header.line_type, DiffLineType::Header);

        let insert = DiffLine::insert("+foo");
        assert_eq!(insert.line_type, DiffLineType::Insert);

        let delete = DiffLine::delete("-bar");
        assert_eq!(delete.line_type, DiffLineType::Delete);
    }

    #[test]
    fn test_diff_options_default() {
        let options = DiffOptions::default();
        assert_eq!(options.max_lines_per_edit, 10);
        assert_eq!(options.truncation_text, "...(truncated)");
    }

    #[test]
    fn test_format_with_options_truncation() {
        let long_text = (0..15)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let edits = vec![Edit::insert(Position::new(0, 0), long_text)];

        let options = DiffOptions {
            max_lines_per_edit: 5,
            truncation_text: "...cut...",
        };

        let lines = format_edits_as_diff_with_options(&edits, &options);

        // header (2) + position (1) + 5 lines + truncation message
        assert_eq!(lines.len(), 9);
        assert!(lines[8].text.contains("...cut..."));
        assert!(lines[8].text.contains("+10 more lines"));
    }

    #[test]
    fn test_format_with_options_no_truncation() {
        let short_text = "line1\nline2\nline3";
        let edits = vec![Edit::insert(Position::new(0, 0), short_text)];

        let options = DiffOptions::default();
        let lines = format_edits_as_diff_with_options(&edits, &options);

        // header (2) + position (1) + 3 lines (no truncation needed)
        assert_eq!(lines.len(), 6);
        assert!(!lines.iter().any(|l| l.text.contains("truncated")));
    }

    #[test]
    fn test_should_summarize_below_threshold() {
        let edits = vec![Edit::insert(Position::new(0, 0), "short")];
        assert!(!should_summarize(&edits));
    }

    #[test]
    fn test_should_summarize_many_edits() {
        let edits: Vec<Edit> = (0..25)
            .map(|i| Edit::insert(Position::new(i, 0), "x"))
            .collect();
        assert!(should_summarize(&edits));
    }

    #[test]
    fn test_should_summarize_large_text() {
        let large_text = "x".repeat(1500);
        let edits = vec![Edit::insert(Position::new(0, 0), large_text)];
        assert!(should_summarize(&edits));
    }
}
