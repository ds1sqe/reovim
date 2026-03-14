//! Conceal system for text concealment and column mapping.
//!
//! Handles hiding/replacing spans of text (e.g., markdown link syntax)
//! and tracking the mapping between source and display columns.

use crate::Style;

// =============================================================================
// Types
// =============================================================================

/// A syntax token with position info (line, column range, category).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxToken {
    /// Line number (0-indexed).
    pub line: u32,
    /// Start column (0-indexed, in characters).
    pub start_col: u32,
    /// End column (exclusive, in characters).
    pub end_col: u32,
    /// Token category (e.g., `"keyword"`, `"function.builtin"`).
    pub category: String,
}

/// A decoration that conceals or hides a span of text.
///
/// Used by the viewport renderer to determine which tokens to replace
/// or hide during display.
#[derive(Debug, Clone)]
pub struct ConcealDecoration {
    /// Start column (character-based, inclusive).
    pub start_col: usize,
    /// End column (character-based, exclusive).
    pub end_col: usize,
    /// Replacement text (`None` = hide completely).
    pub replacement: Option<String>,
    /// Style for the replacement text.
    pub style: Option<Style>,
}

/// Result of applying conceals to a line of text.
///
/// Contains the display text, per-character styles, and a mapping from
/// display columns back to source columns (for cursor positioning).
#[derive(Debug, Clone)]
pub struct ConcealedLine {
    /// The display text with conceals applied.
    pub text: String,
    /// Mapping from display column to source column.
    ///
    /// `col_mapping[display_col]` gives the source column.
    /// Length is `text.len() + 1` (extra entry for end position).
    pub col_mapping: Vec<u16>,
    /// Styles to apply at each display position.
    ///
    /// `None` means use the default/syntax style.
    pub styles: Vec<Option<Style>>,
}

impl ConcealedLine {
    /// Create an identity mapping (no conceals applied).
    #[must_use]
    pub fn identity(content: &str) -> Self {
        let char_count = content.chars().count();
        #[allow(clippy::cast_possible_truncation)]
        let col_mapping: Vec<u16> = (0..=char_count).map(|i| i as u16).collect();
        let styles = vec![None; char_count];
        Self {
            text: content.to_owned(),
            col_mapping,
            styles,
        }
    }
}

// =============================================================================
// Conceal application
// =============================================================================

/// Internal region for conceal processing.
struct ConcealRegion {
    start_col: usize,
    end_col: usize,
    replacement: Option<String>,
    style: Option<Style>,
}

/// Apply conceals and hides to a line's content.
///
/// Takes the original line content and a list of conceal decorations,
/// and produces the display text with appropriate column mappings.
///
/// Columns are character-based (not byte-based). Multi-byte UTF-8
/// content is handled correctly via char indexing.
#[must_use]
pub fn apply_conceals(content: &str, decorations: &[ConcealDecoration]) -> ConcealedLine {
    if decorations.is_empty() {
        return ConcealedLine::identity(content);
    }

    let char_count = content.chars().count();

    // Build sorted regions from decorations.
    let mut regions: Vec<ConcealRegion> = decorations
        .iter()
        .map(|d| ConcealRegion {
            start_col: d.start_col,
            end_col: d.end_col.min(char_count),
            replacement: d.replacement.clone(),
            style: d.style.clone(),
        })
        .collect();
    regions.sort_by_key(|r| r.start_col);

    // Build char→byte lookup for safe slicing.
    let char_byte_offsets: Vec<usize> = content
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(content.len()))
        .collect();

    let char_to_byte = |col: usize| -> usize {
        char_byte_offsets
            .get(col.min(char_count))
            .copied()
            .unwrap_or(content.len())
    };

    let mut result_text = String::with_capacity(content.len());
    let mut col_mapping = Vec::with_capacity(content.len() + 1);
    let mut styles = Vec::with_capacity(content.len());

    let mut source_char = 0;

    for region in &regions {
        // Skip overlapping regions.
        if region.start_col < source_char {
            continue;
        }

        // Add unchanged content before this region.
        let unchanged_end = region.start_col.min(char_count);
        if source_char < unchanged_end {
            let start_byte = char_to_byte(source_char);
            let end_byte = char_to_byte(unchanged_end);
            let slice = &content[start_byte..end_byte];
            for (i, _) in slice.chars().enumerate() {
                #[allow(clippy::cast_possible_truncation)]
                col_mapping.push((source_char + i) as u16);
                styles.push(None);
            }
            result_text.push_str(slice);
        }

        // Apply the conceal/hide.
        if let Some(replacement) = &region.replacement {
            #[allow(clippy::cast_possible_truncation)]
            let start = region.start_col as u16;
            for _ in replacement.chars() {
                col_mapping.push(start);
                styles.push(region.style.clone());
            }
            result_text.push_str(replacement);
        }
        // Hide: don't add anything.

        source_char = region.end_col.min(char_count);
    }

    // Add remaining content after last region.
    if source_char < char_count {
        let start_byte = char_to_byte(source_char);
        let slice = &content[start_byte..];
        for (i, _) in slice.chars().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            col_mapping.push((source_char + i) as u16);
            styles.push(None);
        }
        result_text.push_str(slice);
    }

    // Final mapping for end position.
    #[allow(clippy::cast_possible_truncation)]
    col_mapping.push(char_count as u16);

    ConcealedLine {
        text: result_text,
        col_mapping,
        styles,
    }
}

/// Map a source column to a display column.
///
/// Searches the `col_mapping` to find the first display position
/// that maps to a source column >= the given source column.
#[must_use]
pub fn source_to_display_col(concealed: &ConcealedLine, source_col: usize) -> usize {
    for (display_col, &mapped_source) in concealed.col_mapping.iter().enumerate() {
        if mapped_source as usize >= source_col {
            return display_col;
        }
    }
    concealed.text.len()
}

// =============================================================================
// Color blending
// =============================================================================

/// Linearly interpolate between two colors.
///
/// `t=0.0` returns `from`, `t=1.0` returns `to`.
#[must_use]
#[allow(clippy::many_single_char_names)]
fn lerp_color(from: reovim_arch::Color, to: reovim_arch::Color, t: f32) -> reovim_arch::Color {
    let (from_r, from_g, from_b) = color_to_rgb(from);
    let (to_r, to_g, to_b) = color_to_rgb(to);

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let red = (f32::from(to_r) - f32::from(from_r)).mul_add(t, f32::from(from_r)) as u8;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let green = (f32::from(to_g) - f32::from(from_g)).mul_add(t, f32::from(from_g)) as u8;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let blue = (f32::from(to_b) - f32::from(from_b)).mul_add(t, f32::from(from_b)) as u8;

    reovim_arch::Color::Rgb {
        r: red,
        g: green,
        b: blue,
    }
}

/// Convert a Color to RGB components.
///
/// Named colors are mapped to approximate RGB values.
#[allow(clippy::match_same_arms)]
fn color_to_rgb(color: reovim_arch::Color) -> (u8, u8, u8) {
    match color {
        reovim_arch::Color::Rgb { r, g, b } => (r, g, b),
        reovim_arch::Color::Black => (0, 0, 0),
        reovim_arch::Color::DarkGrey => (128, 128, 128),
        reovim_arch::Color::Red => (255, 0, 0),
        reovim_arch::Color::DarkRed => (139, 0, 0),
        reovim_arch::Color::Green => (0, 255, 0),
        reovim_arch::Color::DarkGreen => (0, 128, 0),
        reovim_arch::Color::Yellow => (255, 255, 0),
        reovim_arch::Color::DarkYellow => (128, 128, 0),
        reovim_arch::Color::Blue => (0, 0, 255),
        reovim_arch::Color::DarkBlue => (0, 0, 139),
        reovim_arch::Color::Magenta => (255, 0, 255),
        reovim_arch::Color::DarkMagenta => (128, 0, 128),
        reovim_arch::Color::Cyan => (0, 255, 255),
        reovim_arch::Color::DarkCyan => (0, 128, 128),
        reovim_arch::Color::White => (255, 255, 255),
        reovim_arch::Color::Grey => (192, 192, 192),
        reovim_arch::Color::AnsiValue(n) => ansi256_to_rgb(n),
        reovim_arch::Color::Reset => (0, 0, 0),
    }
}

/// Convert ANSI 256-color index to RGB.
#[allow(clippy::cast_possible_truncation)]
fn ansi256_to_rgb(n: u8) -> (u8, u8, u8) {
    match n {
        0..=15 => {
            // Standard colors — approximate
            let table: [(u8, u8, u8); 16] = [
                (0, 0, 0),       // 0 black
                (128, 0, 0),     // 1 red
                (0, 128, 0),     // 2 green
                (128, 128, 0),   // 3 yellow
                (0, 0, 128),     // 4 blue
                (128, 0, 128),   // 5 magenta
                (0, 128, 128),   // 6 cyan
                (192, 192, 192), // 7 white
                (128, 128, 128), // 8 bright black
                (255, 0, 0),     // 9 bright red
                (0, 255, 0),     // 10 bright green
                (255, 255, 0),   // 11 bright yellow
                (0, 0, 255),     // 12 bright blue
                (255, 0, 255),   // 13 bright magenta
                (0, 255, 255),   // 14 bright cyan
                (255, 255, 255), // 15 bright white
            ];
            table[n as usize]
        }
        16..=231 => {
            // 6x6x6 color cube
            let idx = n - 16;
            let r = idx / 36;
            let g = (idx % 36) / 6;
            let b = idx % 6;
            let to_val = |c: u8| if c == 0 { 0 } else { 55 + 40 * c };
            (to_val(r), to_val(g), to_val(b))
        }
        232..=255 => {
            // Grayscale ramp
            let v = 8 + 10 * (n - 232);
            (v, v, v)
        }
    }
}

/// Apply opacity dimming to a style.
///
/// Blends the style's foreground and background colors toward `default_bg`
/// proportionally to `(1 - opacity)`. Preserves text attributes.
///
/// - `opacity=1.0` → style unchanged
/// - `opacity=0.5` → colors halfway to background
/// - `opacity=0.0` → all colors become background
#[must_use]
pub fn dim_style(style: &Style, opacity: f32, default_bg: reovim_arch::Color) -> Style {
    if (opacity - 1.0).abs() < f32::EPSILON {
        return style.clone();
    }

    let blend_opt = |color: Option<reovim_arch::Color>| -> Option<reovim_arch::Color> {
        color.map(|c| lerp_color(default_bg, c, opacity))
    };

    Style {
        fg: blend_opt(style.fg),
        bg: blend_opt(style.bg),
        attributes: style.attributes,
    }
}

#[cfg(test)]
#[path = "conceal_tests.rs"]
mod tests;
