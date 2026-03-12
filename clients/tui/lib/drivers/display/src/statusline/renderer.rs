//! Statusline renderer.
//!
//! Renders sections to a frame buffer with support for:
//! - Left/right section alignment
//! - Per-section styling
//! - Separator characters between sections
//! - Multi-row layout (when height > 1)

use crate::{
    Style,
    frame::{FrameBuffer, char_width},
};

use super::{Section, SectionPosition};

/// Separator characters for powerline-style statuslines.
#[derive(Debug, Clone)]
pub struct StatuslineSeparator {
    /// Left separator (used between left sections).
    pub left: &'static str,
    /// Right separator (used between right sections).
    pub right: &'static str,
}

impl StatuslineSeparator {
    /// Powerline solid arrow separators.
    pub const POWERLINE_ARROW: Self = Self {
        left: "\u{e0b0}",  //
        right: "\u{e0b2}", //
    };

    /// Powerline round separators.
    pub const POWERLINE_ROUND: Self = Self {
        left: "\u{e0b4}",  //
        right: "\u{e0b6}", //
    };

    /// Simple pipe separators (ASCII fallback).
    pub const PIPE: Self = Self {
        left: "|",
        right: "|",
    };

    /// No separators.
    pub const NONE: Self = Self {
        left: "",
        right: "",
    };
}

impl Default for StatuslineSeparator {
    fn default() -> Self {
        Self::POWERLINE_ARROW
    }
}

/// Configuration for the statusline renderer.
#[derive(Debug, Clone)]
pub struct StatuslineRendererConfig {
    /// Separator style between sections.
    pub separator: StatuslineSeparator,
    /// Default style for the statusline background.
    pub default_style: Style,
    /// Whether to use powerline-style separator coloring.
    pub powerline_coloring: bool,
}

impl Default for StatuslineRendererConfig {
    fn default() -> Self {
        Self {
            separator: StatuslineSeparator::POWERLINE_ARROW,
            default_style: Style::default(),
            powerline_coloring: true,
        }
    }
}

/// Render sections to a frame buffer row.
///
/// # Arguments
///
/// * `buffer` - Frame buffer to render to
/// * `y` - Y position of the statusline row
/// * `sections` - Sections to render
/// * `config` - Renderer configuration
pub fn render_sections(
    buffer: &mut FrameBuffer,
    y: u16,
    sections: &[Section],
    config: &StatuslineRendererConfig,
) {
    let width = buffer.width();

    // Fill background with default style
    for x in 0..width {
        buffer.put_char(x, y, ' ', &config.default_style);
    }

    // Separate left and right sections
    let left_sections: Vec<_> = sections
        .iter()
        .filter(|s| s.position() == SectionPosition::Left && !s.is_empty())
        .collect();
    let right_sections: Vec<_> = sections
        .iter()
        .filter(|s| s.position() == SectionPosition::Right && !s.is_empty())
        .collect();

    // Calculate total widths (left_width reserved for future separator width calculation)
    let _left_width: usize = left_sections.iter().map(|s| s.display_width()).sum();
    let right_width: usize = right_sections.iter().map(|s| s.display_width()).sum();

    // Render left sections (from x=0)
    let mut x: u16 = 0;
    for (i, section) in left_sections.iter().enumerate() {
        // Add separator between sections (not before first)
        if i > 0 && !config.separator.left.is_empty() {
            let prev_section = left_sections.get(i.saturating_sub(1));
            x = render_separator(
                buffer,
                x,
                y,
                prev_section.map(|s| &s.style),
                Some(&section.style),
                config.separator.left,
                config.powerline_coloring,
            );
        }

        // Render section content
        x = render_section_content(buffer, x, y, section, width);
    }

    // Render right sections (from right edge)
    let right_start = (width as usize).saturating_sub(right_width);
    // SAFETY: right_start <= width (u16), so truncation cannot occur
    #[allow(clippy::cast_possible_truncation)]
    let mut x = right_start as u16;

    for (i, section) in right_sections.iter().enumerate() {
        // Add separator before section (except first)
        if i > 0 && !config.separator.right.is_empty() {
            let prev_section = right_sections.get(i.saturating_sub(1));
            x = render_separator(
                buffer,
                x,
                y,
                prev_section.map(|s| &s.style),
                Some(&section.style),
                config.separator.right,
                config.powerline_coloring,
            );
        }

        // Render section content
        x = render_section_content(buffer, x, y, section, width);
    }
}

/// Render a single section's content.
///
/// Returns the next x position after the section.
fn render_section_content(
    buffer: &mut FrameBuffer,
    start_x: u16,
    y: u16,
    section: &Section,
    max_width: u16,
) -> u16 {
    let mut x = start_x;

    for ch in section.text.chars() {
        if x >= max_width {
            break;
        }
        buffer.put_char(x, y, ch, &section.style);
        x += u16::from(char_width(ch));
    }

    x
}

/// Render a separator between two sections.
///
/// Returns the next x position after the separator.
fn render_separator(
    buffer: &mut FrameBuffer,
    x: u16,
    y: u16,
    left_style: Option<&Style>,
    right_style: Option<&Style>,
    separator: &str,
    powerline_coloring: bool,
) -> u16 {
    let mut pos = x;

    let style = if powerline_coloring {
        // Powerline style: separator fg = left bg, separator bg = right bg
        let fg = left_style.and_then(|s| s.bg).unwrap_or_default();
        let bg = right_style.and_then(|s| s.bg).unwrap_or_default();
        Style::new().fg(fg).bg(bg)
    } else {
        // Simple style: use right section's style
        right_style.cloned().unwrap_or_default()
    };

    for ch in separator.chars() {
        if pos >= buffer.width() {
            break;
        }
        buffer.put_char(pos, y, ch, &style);
        pos += u16::from(char_width(ch));
    }

    pos
}

/// Simple statusline renderer for basic mode + position display.
///
/// This is a convenience function for quick rendering without
/// the full section system.
///
/// # Arguments
///
/// * `buffer` - Frame buffer to render to
/// * `y` - Y position of the statusline row
/// * `mode` - Current mode string (e.g., "NORMAL")
/// * `position` - Cursor position string (e.g., "42:15")
/// * `mode_style` - Style for the mode section
/// * `position_style` - Style for the position section
/// * `fill_style` - Style for the middle fill area
pub fn render_statusline_simple(
    buffer: &mut FrameBuffer,
    y: u16,
    mode: &str,
    position: &str,
    mode_style: &Style,
    position_style: &Style,
    fill_style: &Style,
) {
    let width = buffer.width();

    // Fill with fill_style
    for x in 0..width {
        buffer.put_char(x, y, ' ', fill_style);
    }

    // Render mode on the left
    let mut x: u16 = 0;
    for ch in mode.chars() {
        if x >= width {
            break;
        }
        buffer.put_char(x, y, ch, mode_style);
        x += u16::from(char_width(ch));
    }

    // Render position on the right
    let pos_width = position
        .chars()
        .map(|c| u16::from(char_width(c)))
        .sum::<u16>();
    let pos_start = width.saturating_sub(pos_width);
    let mut x = pos_start;
    for ch in position.chars() {
        if x >= width {
            break;
        }
        buffer.put_char(x, y, ch, position_style);
        x += u16::from(char_width(ch));
    }
}

/// Truncation indicator.
const TRUNCATION_INDICATOR: &str = "…";
const TRUNCATION_WIDTH: usize = 1;

/// Truncate sections to fit within the available width.
///
/// Sections are truncated from lowest to highest priority.
/// The truncation indicator ("…") is added to truncated sections.
///
/// # Arguments
///
/// * `sections` - Sections to potentially truncate
/// * `available_width` - Maximum width available
///
/// # Returns
///
/// A new vector of sections, potentially with some truncated.
#[must_use]
pub fn truncate_sections(sections: &[Section], available_width: usize) -> Vec<Section> {
    let total_width: usize = sections.iter().map(Section::display_width).sum();

    // No truncation needed
    if total_width <= available_width {
        return sections.to_vec();
    }

    // Calculate how much we need to remove
    let excess = total_width - available_width;

    // Sort sections by priority (lowest first for truncation)
    let mut indexed_sections: Vec<(usize, &Section)> = sections.iter().enumerate().collect();
    indexed_sections.sort_by_key(|(_, s)| s.priority);

    // Track which sections to truncate and by how much
    let mut truncation_amounts: Vec<usize> = vec![0; sections.len()];
    let mut remaining_excess = excess;

    for (idx, section) in &indexed_sections {
        if remaining_excess == 0 {
            break;
        }

        let section_width = section.display_width();
        if section_width <= TRUNCATION_WIDTH {
            // Section is too small to truncate
            continue;
        }

        // Maximum we can truncate from this section (leave room for indicator)
        let max_truncation = section_width.saturating_sub(TRUNCATION_WIDTH);
        let truncation = remaining_excess.min(max_truncation);

        truncation_amounts[*idx] = truncation;
        remaining_excess = remaining_excess.saturating_sub(truncation);
    }

    // Apply truncations
    sections
        .iter()
        .enumerate()
        .map(|(idx, section)| {
            let truncation = truncation_amounts[idx];
            if truncation == 0 {
                section.clone()
            } else {
                truncate_section(section, truncation)
            }
        })
        .collect()
}

/// Truncate a single section by the specified amount.
fn truncate_section(section: &Section, amount: usize) -> Section {
    let current_width = section.display_width();
    let target_width = current_width.saturating_sub(amount);

    if target_width <= TRUNCATION_WIDTH {
        // Section would be too small, just use the indicator
        return Section::with_priority(
            section.id,
            TRUNCATION_INDICATOR,
            section.style.clone(),
            section.priority,
        );
    }

    // Truncate text to fit target width (leaving room for indicator)
    let text_target = target_width.saturating_sub(TRUNCATION_WIDTH);
    let truncated_text = truncate_text_to_width(&section.text, text_target);

    Section::with_priority(
        section.id,
        format!("{truncated_text}{TRUNCATION_INDICATOR}"),
        section.style.clone(),
        section.priority,
    )
}

/// Truncate text to fit within the specified display width.
fn truncate_text_to_width(text: &str, max_width: usize) -> String {
    let mut result = String::new();
    let mut current_width = 0;

    for ch in text.chars() {
        let ch_width = char_width(ch);
        if current_width + usize::from(ch_width) > max_width {
            break;
        }
        result.push(ch);
        current_width += usize::from(ch_width);
    }

    result
}

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod tests;
