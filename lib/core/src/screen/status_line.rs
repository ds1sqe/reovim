//! Status line and command line rendering

use crate::buffer::Buffer;
use crate::command_line::CommandLine;
use crate::modd::Mod;
use reovim_sys::{cursor::MoveTo, queue, style::Print};
use std::io::Write;

/// Trait for rendering status line and command line
pub trait StatusLineRenderer {
    /// Render the status line showing current mode, pending keys, last command, and buffer name
    fn render_status_line(
        &mut self,
        mode: &Mod,
        buffer: Option<&Buffer>,
        pending_keys: &str,
        last_command: &str,
    ) -> std::result::Result<(), std::io::Error>;

    /// Render the command line input (shown in Command mode)
    fn render_command_line(
        &mut self,
        cmd_line: &CommandLine,
    ) -> std::result::Result<(), std::io::Error>;
}

/// Render status line to a write stream
#[allow(clippy::cast_possible_truncation)]
pub fn render_status_line_to<W: Write>(
    out: &mut W,
    screen_width: u16,
    screen_height: u16,
    mode: &Mod,
    buffer: Option<&Buffer>,
    pending_keys: &str,
    last_command: &str,
) -> std::result::Result<(), std::io::Error> {
    let mode_str = match mode {
        Mod::Normal => "-- NORMAL --",
        Mod::Insert(_) => "-- INSERT --",
        Mod::Visual(_) => "-- VISUAL --",
        Mod::Command => "", // Command mode shows command line instead
        Mod::Explorer => "-- EXPLORER --",
        Mod::ExplorerInput => "-- EXPLORER INPUT --",
    };

    // Get buffer name (file path or [No Name])
    let buffer_name = buffer
        .and_then(|b| b.file_path.as_ref())
        .map_or("[No Name]", String::as_str);

    // Format pending keys or last command section
    let cmd_section = if !pending_keys.is_empty() {
        format!(" {pending_keys}")
    } else if !last_command.is_empty() {
        format!(" [{last_command}]")
    } else {
        String::new()
    };

    // Calculate spacing to right-align buffer name
    let status_row = screen_height.saturating_sub(1);
    let left_len = mode_str.len() + cmd_section.len();
    let name_len = buffer_name.len();
    let width = screen_width as usize;

    // Build status line: mode + cmd on left, buffer name on right
    let spacing = if left_len + name_len < width {
        width - left_len - name_len
    } else {
        1
    };

    let status_line = format!(
        "{}{}{:spacing$}{}",
        mode_str,
        cmd_section,
        "",
        buffer_name,
        spacing = spacing
    );

    queue!(out, MoveTo(0, status_row))?;
    queue!(out, Print(status_line))
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
