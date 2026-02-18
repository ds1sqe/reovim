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

    #[test]
    fn test_render_tabline_overflow_tab_start() {
        // Width is very small so that a tab's x position starts beyond width
        let mut buffer = FrameBuffer::new(5, 2);
        let tabs = vec![TabInfo::new("longname1.rs"), TabInfo::new("longname2.rs")];

        // First tab " longname1.rs " is 14 chars wide, so second tab
        // starts at x=14 which is >= width(5). The loop should break at line 82.
        render_tabline(
            &mut buffer,
            &tabs,
            0,
            0,
            5,
            &Style::default(),
            &Style::default(),
            &Style::default(),
        );

        // First tab partially rendered, second tab not rendered at all
        assert_eq!(buffer.get(1, 0).unwrap().char, 'l');
    }

    #[test]
    fn test_render_tabline_overflow_mid_tab() {
        // Width cuts off in the middle of a tab's characters
        let mut buffer = FrameBuffer::new(4, 2);
        let tabs = vec![TabInfo::new("abcdefgh")];

        // Tab text is " abcdefgh " (10 chars). Width is 4, so chars after x=3
        // should trigger the inner break at line 94.
        render_tabline(
            &mut buffer,
            &tabs,
            0,
            0,
            4,
            &Style::default(),
            &Style::default(),
            &Style::default(),
        );

        // Only first 4 characters of " abcdefgh " should be rendered
        assert_eq!(buffer.get(0, 0).unwrap().char, ' ');
        assert_eq!(buffer.get(1, 0).unwrap().char, 'a');
        assert_eq!(buffer.get(2, 0).unwrap().char, 'b');
        assert_eq!(buffer.get(3, 0).unwrap().char, 'c');
    }

    #[test]
    fn test_render_statusline_left_overflow() {
        // Left text is long enough to be truncated by center/right position
        let mut buffer = FrameBuffer::new(10, 2);

        // With width=10, right "RIGHTRIGHT" starts at x=0, center "" at x=5
        // Left text "LEFTLEFTLEFT" should break when reaching center_x/right_x
        render_statusline(&mut buffer, 0, "LEFTLEFTLEFT", "", "RRRRRRRRRR", 10, &Style::default());

        // Left should be fully truncated since right fills the entire width
        // Right section starts at x=0 (10 - 10 = 0)
        assert_eq!(buffer.get(0, 0).unwrap().char, 'R');
    }

    #[test]
    fn test_render_statusline_center_hits_right_not_width() {
        // Center text reaches right_x (first condition true) but not width (second condition false)
        // Line 160: `if x >= right_x || x >= width` - exercise first true, second false
        let mut buffer = FrameBuffer::new(30, 2);

        // right "RR" at x=28, center text "CCCCCCCCCCCC" (12 chars) starts at (30-12)/2=9
        // Center rendering should stop at x=28 (right_x), not x=30 (width)
        render_statusline(&mut buffer, 0, "", "CCCCCCCCCCCC", "RR", 30, &Style::default());

        // Center chars should appear from x=9
        assert_eq!(buffer.get(9, 0).unwrap().char, 'C');
        // Right section should still appear at x=28
        assert_eq!(buffer.get(28, 0).unwrap().char, 'R');
        assert_eq!(buffer.get(29, 0).unwrap().char, 'R');
    }

    #[test]
    fn test_render_statusline_center_overflow() {
        // Center text overflows into right section area
        let mut buffer = FrameBuffer::new(20, 2);

        // right "RR" at x=18, center "CCCCCCCCCCCCCCCCCCCC" (20 chars) at ~x=0
        // The center loop should break when x >= right_x (line 161)
        render_statusline(&mut buffer, 0, "", "CCCCCCCCCCCCCCCCCCCC", "RR", 20, &Style::default());

        // Right section should still appear at x=18
        assert_eq!(buffer.get(18, 0).unwrap().char, 'R');
        assert_eq!(buffer.get(19, 0).unwrap().char, 'R');
    }

    #[test]
    fn test_render_statusline_right_overflow() {
        // Right text is wider than the buffer
        let mut buffer = FrameBuffer::new(5, 2);

        // right "RIGHTTEXT" (9 chars), width=5. right_x = 5-9 = 0 (saturating).
        // Right loop should break at x >= width (line 171)
        render_statusline(&mut buffer, 0, "", "", "RIGHTTEXT", 5, &Style::default());

        // Only first 5 chars of right text should render
        assert_eq!(buffer.get(0, 0).unwrap().char, 'R');
        assert_eq!(buffer.get(4, 0).unwrap().char, 'T');
    }
}
