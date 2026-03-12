//! Conceal application logic for text replacement.
//!
//! Handles applying conceal decorations to line content, generating
//! the display text and column mappings for cursor positioning.

#![allow(clippy::cast_possible_truncation)]

use crate::highlight::Style;

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
        let char_count = content.chars().count();
        Self {
            text: content.to_string(),
            col_mapping: (0..=char_count).map(|i| i as u16).collect(),
            styles: vec![None; char_count],
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

    // Build char→byte lookup for safe indexing with multi-byte content.
    // Columns from CachedToken are CHARACTER-based (via byte_to_position),
    // so we must map them to byte offsets before slicing into `content`.
    let char_byte_offsets: Vec<usize> = content
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(content.len()))
        .collect();
    let char_count = char_byte_offsets.len().saturating_sub(1);

    // Collect applicable conceal/hide regions (columns are character-based)
    let mut regions: Vec<ConcealRegion> = decorations
        .iter()
        .filter_map(|d| match d {
            Decoration::Conceal {
                span,
                replacement,
                style,
            } if span.affects_line(line) => {
                let (start_col, end_col) = span_cols_for_line(span, line, char_count as u32);
                Some(ConcealRegion {
                    start_col: start_col as usize,
                    end_col: end_col as usize,
                    replacement: Some(replacement.clone()),
                    style: style.clone(),
                })
            }
            Decoration::Hide { span } if span.affects_line(line) => {
                let (start_col, end_col) = span_cols_for_line(span, line, char_count as u32);
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

    // Sort by start column (character-based)
    regions.sort_by_key(|r| r.start_col);

    // Helper: convert character column to byte offset
    let char_to_byte = |col: usize| -> usize {
        char_byte_offsets
            .get(col.min(char_count))
            .copied()
            .unwrap_or(content.len())
    };

    // Build the transformed text
    let mut result_text = String::with_capacity(content.len());
    let mut col_mapping = Vec::with_capacity(content.len() + 1);
    let mut styles = Vec::with_capacity(content.len());

    // source_char tracks the current CHARACTER position (not byte)
    let mut source_char = 0;

    for region in &regions {
        // Skip if this region starts before current position (overlapping regions)
        if region.start_col < source_char {
            continue;
        }

        // Add unchanged content before this region
        let unchanged_end_char = region.start_col.min(char_count);
        if source_char < unchanged_end_char {
            let start_byte = char_to_byte(source_char);
            let end_byte = char_to_byte(unchanged_end_char);
            let slice = &content[start_byte..end_byte];
            for (i, _) in slice.chars().enumerate() {
                col_mapping.push((source_char + i) as u16);
                styles.push(None);
            }
            result_text.push_str(slice);
        }

        // Apply the conceal/hide
        if let Some(replacement) = &region.replacement {
            // Conceal: add replacement text, mapped to region start CHAR column
            for _ in replacement.chars() {
                col_mapping.push(region.start_col as u16);
                styles.push(region.style.clone());
            }
            result_text.push_str(replacement);
        }
        // Hide: don't add anything (no output)

        // Advance past the concealed/hidden region (in character units)
        source_char = region.end_col.min(char_count);
    }

    // Add remaining content after last region
    if source_char < char_count {
        let start_byte = char_to_byte(source_char);
        let slice = &content[start_byte..];
        for (i, _) in slice.chars().enumerate() {
            col_mapping.push((source_char + i) as u16);
            styles.push(None);
        }
        result_text.push_str(slice);
    }

    // Add final mapping for end position
    col_mapping.push(char_count as u16);

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
#[path = "conceal_tests.rs"]
mod tests;
