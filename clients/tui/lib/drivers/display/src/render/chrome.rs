//! Chrome rendering (tab line, status line).
//!
//! Renders the "chrome" - the non-content UI elements like
//! the tab line at the top and status line at the bottom.

use crate::{compositor::Style, frame::FrameBuffer};

/// Information about a tab for rendering.
#[derive(Debug, Clone)]
pub struct TabInfo {
    /// Tab display name (filename or buffer name).
    pub name: String,
    /// Whether the buffer has unsaved changes.
    pub modified: bool,
    /// Full file path (for tooltip/display).
    pub path: Option<String>,
}

impl TabInfo {
    /// Create a new tab info.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            modified: false,
            path: None,
        }
    }

    /// Set the modified flag.
    #[must_use]
    pub const fn modified(mut self, modified: bool) -> Self {
        self.modified = modified;
        self
    }

    /// Set the path.
    #[must_use]
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Get display text for the tab.
    #[must_use]
    pub fn display_text(&self) -> String {
        if self.modified {
            format!(" {} [+] ", self.name)
        } else {
            format!(" {} ", self.name)
        }
    }
}

/// Styles for tabline rendering.
#[derive(Debug, Clone)]
pub struct TablineStyles<'a> {
    /// Style for inactive tabs.
    pub tab: &'a Style,
    /// Style for the active tab.
    pub active: &'a Style,
    /// Style for the remaining (fill) space.
    pub fill: &'a Style,
}

impl<'a> TablineStyles<'a> {
    /// Create a new tabline styles descriptor.
    #[must_use]
    pub const fn new(tab: &'a Style, active: &'a Style, fill: &'a Style) -> Self {
        Self { tab, active, fill }
    }
}

/// Render the tab line at the top of the screen.
///
/// # Arguments
///
/// * `buffer` - Frame buffer to render to
/// * `tabs` - List of tab information
/// * `active` - Index of the active tab
/// * `y` - Y position to render at
/// * `width` - Width of the tab line
/// * `styles` - Styles for inactive tabs, active tab, and fill space
pub fn render_tabline(
    buffer: &mut FrameBuffer,
    tabs: &[TabInfo],
    active: usize,
    y: u16,
    width: u16,
    styles: &TablineStyles<'_>,
) {
    let mut x: u16 = 0;

    for (idx, tab) in tabs.iter().enumerate() {
        if x >= width {
            break;
        }

        let text = tab.display_text();
        let style = if idx == active {
            styles.active
        } else {
            styles.tab
        };

        for ch in text.chars() {
            if x >= width {
                break;
            }
            buffer.put_char(x, y, ch, style);
            x += u16::from(crate::frame::char_width(ch));
        }
    }

    // Fill remaining space
    while x < width {
        buffer.put_char(x, y, ' ', styles.fill);
        x += 1;
    }
}

/// Render the status line at the bottom of the screen.
///
/// The status line has three sections: left, center, and right.
/// The center is centered in the available space, and the right
/// is right-aligned.
///
/// # Arguments
///
/// * `buffer` - Frame buffer to render to
/// * `y` - Y position to render at
/// * `left` - Left-aligned content
/// * `center` - Centered content
/// * `right` - Right-aligned content
/// * `width` - Width of the status line
/// * `style` - Style for the status line
#[allow(clippy::cast_possible_truncation)]
pub fn render_statusline(
    buffer: &mut FrameBuffer,
    y: u16,
    left: &str,
    center: &str,
    right: &str,
    width: u16,
    style: &Style,
) {
    // Calculate positions (truncation safe: status line won't exceed u16 max)
    let center_len = center.chars().count() as u16;
    let right_len = right.chars().count() as u16;

    // Center position
    let center_x = (width.saturating_sub(center_len)) / 2;
    // Right position
    let right_x = width.saturating_sub(right_len);

    // First, fill the entire line with spaces
    for x in 0..width {
        buffer.put_char(x, y, ' ', style);
    }

    // Render left section
    let mut x: u16 = 0;
    for ch in left.chars() {
        if x >= center_x.min(right_x) {
            break;
        }
        buffer.put_char(x, y, ch, style);
        x += u16::from(crate::frame::char_width(ch));
    }

    // Render center section
    x = center_x;
    for ch in center.chars() {
        if x >= right_x {
            break;
        }
        buffer.put_char(x, y, ch, style);
        x += u16::from(crate::frame::char_width(ch));
    }

    // Render right section
    x = right_x;
    for ch in right.chars() {
        if x >= width {
            break;
        }
        buffer.put_char(x, y, ch, style);
        x += u16::from(crate::frame::char_width(ch));
    }
}

/// Render a simple status line with just left and right sections.
///
/// Convenience function for common status line layout.
pub fn render_statusline_simple(
    buffer: &mut FrameBuffer,
    y: u16,
    left: &str,
    right: &str,
    width: u16,
    style: &Style,
) {
    render_statusline(buffer, y, left, "", right, width, style);
}

#[cfg(test)]
#[path = "chrome_tests.rs"]
mod tests;
