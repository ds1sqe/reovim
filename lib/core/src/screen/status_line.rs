//! Status line and command line rendering

use crate::buffer::Buffer;
use crate::command_line::CommandLine;
use crate::constants::RESET_STYLE;
use crate::highlight::{ColorMode, Theme};
use crate::modd::{EditMode, Focus, ModeState, SubMode};
use reovim_sys::{cursor::MoveTo, queue, style::Print};
use std::io::Write;

/// Trait for rendering status line and command line
pub trait StatusLineRenderer {
    /// Render the status line showing current mode, pending keys, last command, and buffer name
    fn render_status_line(
        &mut self,
        mode: &ModeState,
        buffer: Option<&Buffer>,
        pending_keys: &str,
        last_command: &str,
        theme: &Theme,
        color_mode: ColorMode,
    ) -> std::result::Result<(), std::io::Error>;

    /// Render the command line input (shown in Command mode)
    fn render_command_line(
        &mut self,
        cmd_line: &CommandLine,
    ) -> std::result::Result<(), std::io::Error>;
}

/// Render status line to a write stream
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::too_many_arguments)]
pub fn render_status_line_to<W: Write>(
    out: &mut W,
    screen_width: u16,
    screen_height: u16,
    mode: &ModeState,
    buffer: Option<&Buffer>,
    pending_keys: &str,
    last_command: &str,
    theme: &Theme,
    color_mode: ColorMode,
) -> std::result::Result<(), std::io::Error> {
    let mode_str = mode.display_string();

    // Get mode-specific style
    let mode_style = match (&mode.sub_mode, &mode.focus, &mode.edit_mode) {
        // Command sub-mode or Telescope uses command style
        (SubMode::Command, _, _) | (SubMode::None, Focus::Telescope, _) => {
            &theme.status_line_mode.command
        }
        // Operator-pending and normal mode in Editor
        (SubMode::OperatorPending { .. }, _, _)
        | (SubMode::None, Focus::Editor, EditMode::Normal) => &theme.status_line_mode.normal,
        // Editor insert mode
        (SubMode::None, Focus::Editor, EditMode::Insert(_)) => &theme.status_line_mode.insert,
        // Editor visual mode
        (SubMode::None, Focus::Editor, EditMode::Visual(_)) => &theme.status_line_mode.visual,
        // Explorer
        (SubMode::None, Focus::Explorer, _) => &theme.status_line_mode.explorer,
    };

    // Get buffer name (file path or [No Name])
    let buffer_name = buffer
        .and_then(|b| b.file_path.as_ref())
        .map_or("[No Name]", String::as_str);

    // Format pending keys or last command section
    let cmd_section = if !pending_keys.is_empty() {
        format!(" {pending_keys} ")
    } else if !last_command.is_empty() {
        format!(" [{last_command}] ")
    } else {
        String::from(" ")
    };

    // Calculate spacing to right-align buffer name
    let status_row = screen_height.saturating_sub(1);
    let left_len = mode_str.len() + cmd_section.len();
    let name_len = buffer_name.len() + 1; // +1 for trailing space
    let width = screen_width as usize;

    // Build status line: mode + cmd on left, buffer name on right
    let spacing = if left_len + name_len < width {
        width - left_len - name_len
    } else {
        0
    };

    queue!(out, MoveTo(0, status_row))?;

    // Render mode indicator with mode style
    if !mode_str.is_empty() {
        let mode_ansi = mode_style.to_ansi_start(color_mode);
        queue!(out, Print(&mode_ansi))?;
        queue!(out, Print(mode_str))?;
        queue!(out, Print(RESET_STYLE))?;
    }

    // Render rest of status line with status_line style
    let status_ansi = theme.status_line.to_ansi_start(color_mode);
    queue!(out, Print(&status_ansi))?;
    queue!(out, Print(&cmd_section))?;
    queue!(out, Print(format!("{:spacing$}", "", spacing = spacing)))?;
    queue!(out, Print(format!("{buffer_name} ")))?;
    queue!(out, Print(RESET_STYLE))
}

/// Render command line to a write stream
#[allow(clippy::cast_possible_truncation)]
pub fn render_command_line_to<W: Write>(
    out: &mut W,
    screen_height: u16,
    cmd_line: &CommandLine,
) -> std::result::Result<(), std::io::Error> {
    let status_row = screen_height.saturating_sub(1);
    queue!(out, MoveTo(0, status_row))?;
    let display = format!(":{}", cmd_line.input);
    queue!(out, Print(display))?;
    // Position cursor after the input
    let cursor_col = 1 + cmd_line.cursor as u16;
    queue!(out, MoveTo(cursor_col, status_row))
}
