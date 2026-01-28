//! Command line popup UI rendering.
//!
//! Provides rendering logic for the floating command line popup.
//! Renders border, prompt character, input text, and cursor.

use reovim_driver_session::CmdlinePrompt;

/// Border characters for the popup (rounded corners).
pub mod border {
    /// Top-left corner.
    pub const TOP_LEFT: char = '\u{256D}'; // ╭
    /// Top-right corner.
    pub const TOP_RIGHT: char = '\u{256E}'; // ╮
    /// Bottom-left corner.
    pub const BOTTOM_LEFT: char = '\u{2570}'; // ╰
    /// Bottom-right corner.
    pub const BOTTOM_RIGHT: char = '\u{256F}'; // ╯
    /// Horizontal line.
    pub const HORIZONTAL: char = '\u{2500}'; // ─
    /// Vertical line.
    pub const VERTICAL: char = '\u{2502}'; // │
}

/// Cursor character (solid block).
pub const CURSOR_CHAR: char = '\u{2588}'; // █

/// Rendered line content for the popup.
///
/// Each line is a vector of characters with their positions.
#[derive(Debug, Clone)]
pub struct RenderedLine {
    /// Characters on this line.
    pub chars: Vec<char>,
}

impl RenderedLine {
    /// Create a new rendered line.
    #[must_use]
    pub const fn new() -> Self {
        Self { chars: Vec::new() }
    }

    /// Create from a string.
    #[must_use]
    pub fn from_text(s: &str) -> Self {
        Self {
            chars: s.chars().collect(),
        }
    }

    /// Get the line as a string.
    #[must_use]
    pub fn as_string(&self) -> String {
        self.chars.iter().collect()
    }
}

impl Default for RenderedLine {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the top border line.
///
/// Format: `╭───...───╮`
#[must_use]
pub fn render_top_border(width: usize) -> RenderedLine {
    let mut line = RenderedLine::new();

    if width < 2 {
        return line;
    }

    line.chars.push(border::TOP_LEFT);
    for _ in 0..(width.saturating_sub(2)) {
        line.chars.push(border::HORIZONTAL);
    }
    line.chars.push(border::TOP_RIGHT);

    line
}

/// Render the bottom border line.
///
/// Format: `╰───...───╯`
#[must_use]
pub fn render_bottom_border(width: usize) -> RenderedLine {
    let mut line = RenderedLine::new();

    if width < 2 {
        return line;
    }

    line.chars.push(border::BOTTOM_LEFT);
    for _ in 0..(width.saturating_sub(2)) {
        line.chars.push(border::HORIZONTAL);
    }
    line.chars.push(border::BOTTOM_RIGHT);

    line
}

/// Render the content line with prompt, input, and cursor.
///
/// Format: `│ :input█ ... │`
///
/// # Arguments
///
/// * `width` - Total line width including borders
/// * `prompt` - Prompt type (determines prompt character)
/// * `input` - Current input text
/// * `cursor` - Cursor position within input
#[must_use]
pub fn render_content_line(
    width: usize,
    prompt: CmdlinePrompt,
    input: &str,
    cursor: usize,
) -> RenderedLine {
    let mut line = RenderedLine::new();

    if width < 4 {
        return line;
    }

    // Left border
    line.chars.push(border::VERTICAL);
    line.chars.push(' ');

    // Prompt character
    line.chars.push(prompt.char());

    // Available space for input and cursor (width - 2 borders - 2 spaces - 1 prompt)
    let available = width.saturating_sub(5);

    // Calculate visible portion of input (handle horizontal scrolling for long input)
    let input_chars: Vec<char> = input.chars().collect();
    let input_len = input_chars.len();

    // Determine scroll offset to keep cursor visible
    let scroll_offset = if cursor > available.saturating_sub(1) {
        cursor.saturating_sub(available.saturating_sub(1))
    } else {
        0
    };

    // Render visible portion of input
    let mut rendered_chars = 0;
    for (i, ch) in input_chars.iter().enumerate().skip(scroll_offset) {
        if rendered_chars >= available {
            break;
        }

        if i == cursor {
            // Cursor position: render block cursor instead of character
            line.chars.push(CURSOR_CHAR);
        } else {
            line.chars.push(*ch);
        }
        rendered_chars += 1;
    }

    // If cursor is at end of input
    if cursor >= input_len && rendered_chars < available {
        line.chars.push(CURSOR_CHAR);
        rendered_chars += 1;
    }

    // Fill remaining space
    while rendered_chars < available {
        line.chars.push(' ');
        rendered_chars += 1;
    }

    // Right border
    line.chars.push(' ');
    line.chars.push(border::VERTICAL);

    line
}

/// Render the complete popup as a list of lines.
///
/// Returns three lines: top border, content, bottom border.
#[must_use]
pub fn render_popup(
    width: usize,
    prompt: CmdlinePrompt,
    input: &str,
    cursor: usize,
) -> Vec<RenderedLine> {
    vec![
        render_top_border(width),
        render_content_line(width, prompt, input, cursor),
        render_bottom_border(width),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_empty_input() {
        let line = render_content_line(20, CmdlinePrompt::Command, "", 0);
        let s = line.as_string();

        // Should contain: │ :[cursor]... │
        assert!(s.starts_with("│ :"));
        assert!(s.contains(CURSOR_CHAR));
        assert!(s.ends_with(" │"));
    }

    #[test]
    fn test_render_with_input() {
        let line = render_content_line(30, CmdlinePrompt::Command, "set number", 10);
        let s = line.as_string();

        // Should contain the input text
        assert!(s.contains("set number"));
        assert!(s.starts_with("│ :"));
    }

    #[test]
    fn test_render_cursor_at_end() {
        let line = render_content_line(20, CmdlinePrompt::Command, "abc", 3);
        let s = line.as_string();

        // Cursor should be after "abc"
        assert!(s.contains("abc"));
        assert!(s.contains(CURSOR_CHAR));
    }

    #[test]
    fn test_render_cursor_in_middle() {
        let line = render_content_line(20, CmdlinePrompt::Command, "abc", 1);
        let s = line.as_string();

        // Cursor should be at position 1 (replacing 'b')
        // Actually, character at cursor position is replaced with cursor char
        assert!(s.contains('a'));
        assert!(s.contains(CURSOR_CHAR));
        assert!(s.contains('c'));
    }

    #[test]
    fn test_render_prompt_colon() {
        let line = render_content_line(20, CmdlinePrompt::Command, "", 0);
        let s = line.as_string();
        assert!(s.contains(':'));
    }

    #[test]
    fn test_render_prompt_slash() {
        let line = render_content_line(20, CmdlinePrompt::SearchForward, "", 0);
        let s = line.as_string();
        assert!(s.contains('/'));
    }

    #[test]
    fn test_render_prompt_question() {
        let line = render_content_line(20, CmdlinePrompt::SearchBackward, "", 0);
        let s = line.as_string();
        assert!(s.contains('?'));
    }

    #[test]
    fn test_render_border_characters() {
        let top = render_top_border(10);
        let bottom = render_bottom_border(10);

        let top_s = top.as_string();
        let bottom_s = bottom.as_string();

        assert!(top_s.starts_with(border::TOP_LEFT));
        assert!(top_s.ends_with(border::TOP_RIGHT));
        assert!(top_s.contains(border::HORIZONTAL));

        assert!(bottom_s.starts_with(border::BOTTOM_LEFT));
        assert!(bottom_s.ends_with(border::BOTTOM_RIGHT));
        assert!(bottom_s.contains(border::HORIZONTAL));
    }

    #[test]
    fn test_render_popup_three_lines() {
        let lines = render_popup(30, CmdlinePrompt::Command, "test", 4);

        assert_eq!(lines.len(), 3);

        // First line is top border
        assert!(lines[0].as_string().starts_with(border::TOP_LEFT));

        // Second line is content
        assert!(lines[1].as_string().contains("test"));

        // Third line is bottom border
        assert!(lines[2].as_string().starts_with(border::BOTTOM_LEFT));
    }

    #[test]
    fn test_border_minimum_width() {
        let top = render_top_border(2);
        let s = top.as_string();

        // Minimum: just corners (2 characters, but more bytes due to Unicode)
        assert_eq!(s.chars().count(), 2);
        assert_eq!(s.chars().next(), Some(border::TOP_LEFT));
        assert_eq!(s.chars().nth(1), Some(border::TOP_RIGHT));
    }

    #[test]
    fn test_border_too_small() {
        let top = render_top_border(1);
        assert!(top.chars.is_empty());

        let top = render_top_border(0);
        assert!(top.chars.is_empty());
    }

    #[test]
    fn test_rendered_line_from_text() {
        let line = RenderedLine::from_text("hello");
        assert_eq!(line.as_string(), "hello");
        assert_eq!(line.chars.len(), 5);
    }
}
