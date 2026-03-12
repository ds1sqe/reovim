//! Line rendering with syntax highlighting.
//!
//! Renders a single line of text to a frame buffer, applying
//! style spans for syntax highlighting.

use crate::{compositor::Style, frame::FrameBuffer};

/// Render a line of text with highlights to a frame buffer.
///
/// # Arguments
///
/// * `buffer` - The frame buffer to render to
/// * `x` - Starting X position
/// * `y` - Y position (row)
/// * `text` - The text to render
/// * `highlights` - Style spans: (`start_col`, `end_col`, style)
/// * `max_width` - Maximum width to render
/// * `default_style` - Default style for unhighlighted text
///
/// # Returns
///
/// The number of columns used.
pub fn render_line(
    buffer: &mut FrameBuffer,
    x: u16,
    y: u16,
    text: &str,
    highlights: &[(usize, usize, Style)],
    max_width: u16,
    default_style: &Style,
) -> u16 {
    if max_width == 0 {
        return 0;
    }

    let mut col: u16 = 0;
    let chars: Vec<char> = text.chars().collect();

    for (char_idx, &ch) in chars.iter().enumerate() {
        if col >= max_width {
            break;
        }

        // Find applicable style from highlights
        let style = find_style_for_column(char_idx, highlights, default_style);

        // Write the character
        let char_width = crate::frame::char_width(ch);
        buffer.put_char(x + col, y, ch, style);
        col += u16::from(char_width);
    }

    // Fill remaining width with spaces
    while col < max_width {
        buffer.put_char(x + col, y, ' ', default_style);
        col += 1;
    }

    col
}

/// Find the style for a given column from the highlights list.
fn find_style_for_column<'a>(
    col: usize,
    highlights: &'a [(usize, usize, Style)],
    default: &'a Style,
) -> &'a Style {
    for (start, end, style) in highlights {
        if col >= *start && col < *end {
            return style;
        }
    }
    default
}

/// Render a line with a single style (no highlights).
///
/// Convenience function when no syntax highlighting is needed.
pub fn render_line_simple(
    buffer: &mut FrameBuffer,
    x: u16,
    y: u16,
    text: &str,
    max_width: u16,
    style: &Style,
) -> u16 {
    render_line(buffer, x, y, text, &[], max_width, style)
}

#[cfg(test)]
#[path = "line_tests.rs"]
mod tests;
