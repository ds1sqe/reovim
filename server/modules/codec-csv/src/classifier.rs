//! CSV/TSV/PSV content classifier.
//!
//! Detects delimiter-separated value files by analyzing delimiter
//! consistency across lines. Supports comma, tab, pipe, and semicolon.

use reovim_driver_codec::{ContentClassifier, ContentType};

/// Content type for comma-separated values.
pub const CSV: &str = "text/csv";

/// Content type for tab-separated values.
pub const TSV: &str = "text/tsv";

/// Content type for pipe-separated values.
pub const PSV: &str = "text/psv";

/// Content type for semicolon-separated values (European CSV).
pub const SCSV: &str = "text/scsv";

/// Minimum number of rows to consider as CSV (including header).
const MIN_ROWS: usize = 2;

/// Minimum number of columns (delimiters + 1 per line).
const MIN_COLUMNS: usize = 2;

/// Maximum bytes to sample for delimiter detection.
const SAMPLE_SIZE: usize = 8192;

/// Known CSV/TSV/PSV file extensions (fast-path).
const CSV_EXTENSIONS: &[(&str, &str)] = &[("csv", CSV), ("tsv", TSV), ("tab", TSV), ("psv", PSV)];

/// Delimiters to try, in order of preference.
const DELIMITERS: &[(u8, &str)] = &[
    (b',', CSV),
    (b'\t', TSV),
    (b'|', PSV),
    (b';', SCSV), // Semicolon is common in European CSV
];

/// CSV/TSV/PSV content classifier (priority 15).
///
/// Runs after binary detection (20) but before UTF-8 (10). CSV files
/// are always valid text, so the binary classifier won't claim them.
/// This classifier intercepts tabular data that would otherwise be
/// treated as plain text.
pub struct CsvClassifier;

impl CsvClassifier {
    /// Create a new CSV classifier.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for CsvClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentClassifier for CsvClassifier {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn classify(&self, raw: &[u8], path: &str) -> Option<ContentType> {
        // Fast-path: known extensions
        if let Some(ct) = extension_content_type(path) {
            return Some(ContentType::new(ct));
        }

        // Only classify valid UTF-8 text
        let sample = if raw.len() > SAMPLE_SIZE {
            &raw[..SAMPLE_SIZE]
        } else {
            raw
        };
        let text = std::str::from_utf8(sample).ok()?;

        // Must have enough lines
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() < MIN_ROWS {
            return None;
        }

        // Try each delimiter
        for &(delim, content_type) in DELIMITERS {
            if is_consistent_delimiter(&lines, delim) {
                return Some(ContentType::new(content_type));
            }
        }

        None
    }

    fn priority(&self) -> u8 {
        15
    }

    fn name(&self) -> &'static str {
        "csv"
    }
}

/// Check if a delimiter produces consistent column counts across lines.
///
/// Returns true if every non-empty line has at least `MIN_COLUMNS`
/// columns and the column count is identical on every line.
fn is_consistent_delimiter(lines: &[&str], delim: u8) -> bool {
    let delim_char = delim as char;
    let mut expected_count = 0;
    let mut valid_lines = 0;

    for line in lines {
        if line.is_empty() {
            continue;
        }

        let count = line.matches(delim_char).count() + 1;
        if count < MIN_COLUMNS {
            return false;
        }

        if valid_lines == 0 {
            expected_count = count;
        } else if count != expected_count {
            return false;
        }

        valid_lines += 1;
    }

    valid_lines >= MIN_ROWS
}

/// Look up content type from file extension.
fn extension_content_type(path: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(path).extension()?;
    let ext_str = ext.to_str()?;
    let lower = ext_str.to_ascii_lowercase();
    CSV_EXTENSIONS
        .iter()
        .find(|(e, _)| *e == lower.as_str())
        .map(|(_, ct)| *ct)
}

#[cfg(test)]
#[path = "classifier_tests.rs"]
mod tests;
