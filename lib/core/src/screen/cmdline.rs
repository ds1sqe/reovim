//! Command line rendering for the editor
//!
//! This module contains the command line rendering logic, displaying the
//! command prompt and user input when in command mode.

use crate::{command_line::CommandLine, frame::FrameBuffer, highlight::Theme};

use super::Screen;

impl Screen {
    /// Render command line to frame buffer
    ///
    /// Displays the command prompt (":") followed by user input on the
    /// bottom line of the screen.
    #[allow(clippy::cast_possible_truncation)]
    pub(super) fn render_command_line_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        cmd_line: &CommandLine,
        theme: &Theme,
    ) {
        let y = self.size.height.saturating_sub(1);
        let style = &theme.base.default;

        // Write colon prompt
        buffer.put_char(0, y, ':', style);

        // Write command text
        for (i, ch) in cmd_line.input.chars().enumerate() {
            let x = 1 + i as u16;
            if x < buffer.width() {
                buffer.put_char(x, y, ch, style);
            }
        }

        // Clear rest of line
        let input_len = cmd_line.input.len() as u16;
        for x in (1 + input_len)..buffer.width() {
            buffer.put_char(x, y, ' ', style);
        }
    }
}
