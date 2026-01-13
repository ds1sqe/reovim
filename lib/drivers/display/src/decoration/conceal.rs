//! Conceal application logic for text replacement.
//!
//! Handles applying conceal decorations to line content, generating
//! the display text and column mappings for cursor positioning.

#![allow(clippy::cast_possible_truncation)]

use reovim_core::highlight::Style;

use super::types::{Decoration, Span};

/// Result of applying conceals to a line.
///
/// Contains the transformed display text and mappings between
/// display positions and source positions.
#[derive(Debug, Clone)]
pub struct ConcealedLine {
    /// The display text with conceals applied
    pub text: String,
    /// Mapping from display column to source column
    ///
    /// `col_mapping[display_col]` gives the source column.
    /// Used for cursor positioning.
    pub col_mapping: Vec<u16>,
    /// Styles to apply at each display position
    ///
    /// `None` means use the default/syntax style.
    pub styles: Vec<Option<Style>>,
}

impl ConcealedLine {
    /// Create a new concealed line with no transformations.
    #[must_use]
    pub fn identity(content: &str) -> Self {
        let len = content.len();
        Self {
            text: content.to_string(),
            col_mapping: (0..=len).map(|i| i as u16).collect(),
            styles: vec![None; len],
        }
    }

    /// Get the display width of the concealed line.
    #[must_use]
    pub const fn display_width(&self) -> usize {
        self.text.len()
    }

    /// Get the source width (original content length).
    #[must_use]
    pub fn source_width(&self) -> usize {
        self.col_mapping.last().copied().unwrap_or(0) as usize
    }
}

/// Apply conceals and hides to a line's content.
///
/// Takes the original line content and a list of conceal/hide decorations,
/// and produces the display text with appropriate column mappings.
///
/// # Arguments
///
/// * `content` - The original line content
/// * `line` - The line number (for filtering span-based decorations)
/// * `decorations` - Conceal and hide decorations to apply
///
/// # Returns
///
/// A `ConcealedLine` with the transformed text and mappings.
#[must_use]
pub fn apply_conceals(content: &str, line: u32, decorations: &[&Decoration]) -> ConcealedLine {
    if decorations.is_empty() {
        return ConcealedLine::identity(content);
    }

    // Collect applicable conceal/hide regions
    let mut regions: Vec<ConcealRegion> = decorations
        .iter()
        .filter_map(|d| match d {
            Decoration::Conceal {
                span,
                replacement,
                style,
            } if span.affects_line(line) => {
                let (start_col, end_col) = span_cols_for_line(span, line, content.len() as u32);
                Some(ConcealRegion {
                    start_col: start_col as usize,
                    end_col: end_col as usize,
                    replacement: Some(replacement.clone()),
                    style: style.clone(),
                })
            }
            Decoration::Hide { span } if span.affects_line(line) => {
                let (start_col, end_col) = span_cols_for_line(span, line, content.len() as u32);
                Some(ConcealRegion {
                    start_col: start_col as usize,
                    end_col: end_col as usize,
                    replacement: None,
                    style: None,
                })
            }
            _ => None,
        })
        .collect();

    if regions.is_empty() {
        return ConcealedLine::identity(content);
    }

    // Sort by start column
    regions.sort_by_key(|r| r.start_col);

    // Build the transformed text
    let mut result_text = String::with_capacity(content.len());
    let mut col_mapping = Vec::with_capacity(content.len() + 1);
    let mut styles = Vec::with_capacity(content.len());

    let mut source_col = 0;

    for region in &regions {
        // Skip if this region starts before current position (overlapping regions)
        if region.start_col < source_col {
            continue;
        }

        // Add unchanged content before this region
        let unchanged_end = region.start_col.min(content.len());
        if source_col < unchanged_end {
            let slice = &content[source_col..unchanged_end];
            for (i, _) in slice.char_indices() {
                col_mapping.push((source_col + i) as u16);
                styles.push(None);
            }
            result_text.push_str(slice);
        }

        // Apply the conceal/hide
        if let Some(replacement) = &region.replacement {
            // Conceal: add replacement text
            for _ in replacement.chars() {
                // Map all replacement chars to the start of the concealed region
                col_mapping.push(region.start_col as u16);
                styles.push(region.style.clone());
            }
            result_text.push_str(replacement);
        }
        // Hide: don't add anything (no output)

        // Advance past the concealed/hidden region
        source_col = region.end_col.min(content.len());
    }

    // Add remaining content after last region
    if source_col < content.len() {
        let slice = &content[source_col..];
        for (i, _) in slice.char_indices() {
            col_mapping.push((source_col + i) as u16);
            styles.push(None);
        }
        result_text.push_str(slice);
    }

    // Add final mapping for end position
    col_mapping.push(content.len() as u16);

    ConcealedLine {
        text: result_text,
        col_mapping,
        styles,
    }
}

/// Region to be concealed or hidden.
struct ConcealRegion {
    start_col: usize,
    end_col: usize,
    replacement: Option<String>,
    style: Option<Style>,
}

/// Get the column range for a span on a specific line.
const fn span_cols_for_line(span: &Span, line: u32, line_len: u32) -> (u32, u32) {
    if span.is_single_line() {
        (span.start_col, span.end_col)
    } else if line == span.start_line {
        // First line: from start_col to end of line
        (span.start_col, line_len)
    } else if line == span.end_line {
        // Last line: from start of line to end_col
        (0, span.end_col)
    } else {
        // Middle line: entire line
        (0, line_len)
    }
}

/// Map display column back to source column.
///
/// # Arguments
///
/// * `concealed` - The concealed line result
/// * `display_col` - Column in the display text
///
/// # Returns
///
/// The corresponding column in the source text.
#[must_use]
pub fn display_to_source_col(concealed: &ConcealedLine, display_col: usize) -> usize {
    concealed
        .col_mapping
        .get(display_col)
        .copied()
        .unwrap_or_else(|| concealed.col_mapping.last().copied().unwrap_or(0)) as usize
}

/// Map source column to display column.
///
/// # Arguments
///
/// * `concealed` - The concealed line result
/// * `source_col` - Column in the source text
///
/// # Returns
///
/// The corresponding column in the display text.
#[must_use]
pub fn source_to_display_col(concealed: &ConcealedLine, source_col: usize) -> usize {
    // Find the first display position that maps to >= source_col
    for (display_col, &mapped_source) in concealed.col_mapping.iter().enumerate() {
        if mapped_source as usize >= source_col {
            return display_col;
        }
    }
    concealed.text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_no_decorations() {
        let content = "Hello, World!";
        let result = apply_conceals(content, 0, &[]);

        assert_eq!(result.text, content);
        assert_eq!(result.col_mapping.len(), content.len() + 1);
    }

    #[test]
    fn test_simple_conceal() {
        let content = "[text](url)";
        let decorations = [&Decoration::conceal(
            Span::line(0, 0, content.len() as u32),
            "[link]",
            None,
        )];

        let result = apply_conceals(content, 0, &decorations);

        assert_eq!(result.text, "[link]");
    }

    #[test]
    fn test_simple_hide() {
        let content = "Hello hidden World";
        // Hide "hidden " (positions 6-13)
        let decorations = [&Decoration::hide(Span::line(0, 6, 13))];

        let result = apply_conceals(content, 0, &decorations);

        assert_eq!(result.text, "Hello World");
    }

    #[test]
    fn test_display_to_source_mapping() {
        let content = "abc123def";
        // Replace "123" with "X"
        let decorations = [&Decoration::conceal(Span::line(0, 3, 6), "X", None)];

        let result = apply_conceals(content, 0, &decorations);

        assert_eq!(result.text, "abcXdef");

        // Position 0-2 map to themselves
        assert_eq!(display_to_source_col(&result, 0), 0);
        assert_eq!(display_to_source_col(&result, 2), 2);

        // Position 3 (X) maps to start of concealed region
        assert_eq!(display_to_source_col(&result, 3), 3);

        // Position 4 (d) maps to 6 (after concealed region)
        assert_eq!(display_to_source_col(&result, 4), 6);
    }

    #[test]
    fn test_source_to_display_mapping() {
        let content = "abc123def";
        // Replace "123" with "X"
        let decorations = [&Decoration::conceal(Span::line(0, 3, 6), "X", None)];

        let result = apply_conceals(content, 0, &decorations);

        // Source 0-2 map to themselves
        assert_eq!(source_to_display_col(&result, 0), 0);
        assert_eq!(source_to_display_col(&result, 2), 2);

        // Source 3-5 (concealed region) maps to 3 (X position)
        assert_eq!(source_to_display_col(&result, 3), 3);

        // Source 6 (d) maps to 4
        assert_eq!(source_to_display_col(&result, 6), 4);
    }
}
