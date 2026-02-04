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

/// Render the tab line at the top of the screen.
///
/// # Arguments
///
/// * `buffer` - Frame buffer to render to
/// * `tabs` - List of tab information
/// * `active` - Index of the active tab
/// * `y` - Y position to render at
/// * `width` - Width of the tab line
/// * `tab_style` - Style for inactive tabs
/// * `active_style` - Style for the active tab
/// * `fill_style` - Style for the remaining space
#[allow(clippy::too_many_arguments)]
pub fn render_tabline(
    buffer: &mut FrameBuffer,
    tabs: &[TabInfo],
    active: usize,
    y: u16,
    width: u16,
    tab_style: &Style,
    active_style: &Style,
    fill_style: &Style,
) {
    let mut x: u16 = 0;

    for (idx, tab) in tabs.iter().enumerate() {
        if x >= width {
            break;
        }

        let text = tab.display_text();
        let style = if idx == active {
            active_style
        } else {
            tab_style
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
        buffer.put_char(x, y, ' ', fill_style);
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
        if x >= right_x || x >= width {
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
mod tests {
    use super::*;

    #[test]
    fn test_tab_info_new() {
        let tab = TabInfo::new("test.rs");
        assert_eq!(tab.name, "test.rs");
        assert!(!tab.modified);
        assert!(tab.path.is_none());
    }

    #[test]
    fn test_tab_info_builder() {
        let tab = TabInfo::new("test.rs")
            .modified(true)
            .path("/home/user/test.rs");

        assert!(tab.modified);
        assert_eq!(tab.path, Some("/home/user/test.rs".to_string()));
    }

    #[test]
    fn test_tab_info_display_text() {
        let tab = TabInfo::new("test.rs");
        assert_eq!(tab.display_text(), " test.rs ");

        let modified_tab = TabInfo::new("test.rs").modified(true);
        assert_eq!(modified_tab.display_text(), " test.rs [+] ");
    }

    #[test]
    fn test_render_tabline() {
        let mut buffer = FrameBuffer::new(80, 24);
        let tabs = vec![
            TabInfo::new("file1.rs"),
            TabInfo::new("file2.rs").modified(true),
        ];

        render_tabline(
            &mut buffer,
            &tabs,
            0,
            0,
            80,
            &Style::default(),
            &Style::default(),
            &Style::default(),
        );

        // Check first tab content
        assert_eq!(buffer.get(1, 0).unwrap().char, 'f');
        assert_eq!(buffer.get(2, 0).unwrap().char, 'i');
    }

    #[test]
    fn test_render_tabline_empty() {
        let mut buffer = FrameBuffer::new(80, 24);

        render_tabline(
            &mut buffer,
            &[],
            0,
            0,
            80,
            &Style::default(),
            &Style::default(),
            &Style::default(),
        );

        // Should be filled with spaces
        assert_eq!(buffer.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_render_statusline() {
        let mut buffer = FrameBuffer::new(80, 24);

        render_statusline(&mut buffer, 23, "NORMAL", "file.rs", "1:1", 80, &Style::default());

        // Check left section
        assert_eq!(buffer.get(0, 23).unwrap().char, 'N');

        // Check right section (at end)
        assert_eq!(buffer.get(77, 23).unwrap().char, '1');
        assert_eq!(buffer.get(78, 23).unwrap().char, ':');
        assert_eq!(buffer.get(79, 23).unwrap().char, '1');
    }

    #[test]
    fn test_render_statusline_simple() {
        let mut buffer = FrameBuffer::new(80, 24);

        render_statusline_simple(&mut buffer, 23, "LEFT", "RIGHT", 80, &Style::default());

        assert_eq!(buffer.get(0, 23).unwrap().char, 'L');
        assert_eq!(buffer.get(75, 23).unwrap().char, 'R');
    }
}
