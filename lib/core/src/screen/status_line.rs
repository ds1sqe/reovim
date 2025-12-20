//! Status line and command line rendering
//!
//! Enhanced statusline format:
//! `[MODE_ICON MODE] [pending/cmd] ... [FILENAME][+] [FILETYPE] Ln X, Col Y`

// Allow deprecated Focus enum during transition to FocusId
#![allow(deprecated)]

use {
    crate::{
        buffer::Buffer,
        command_line::CommandLine,
        constants::RESET_STYLE,
        highlight::{ColorMode, Theme},
        modd::{EditMode, Focus, ModeState, SubMode},
    },
    reovim_sys::{cursor::MoveTo, queue, style::Print},
    std::io::Write,
};

/// Mode icons (Nerd Font)
mod icons {
    pub const NORMAL: &str = "󰆾 ";
    pub const INSERT: &str = "󰙏 ";
    pub const VISUAL: &str = "󰒉 ";
    pub const COMMAND: &str = "󰘳 ";
    pub const EXPLORER: &str = "󰙅 ";
    pub const LEAP: &str = "󱐋 ";
    pub const OPERATOR: &str = "󰦒 ";
    pub const TELESCOPE: &str = "󰭎 ";
    pub const SETTINGS: &str = "󰒓 ";
}

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

/// Get mode icon for the current mode state
const fn mode_icon(mode: &ModeState) -> &'static str {
    match (&mode.sub_mode, &mode.focus, &mode.edit_mode) {
        (SubMode::Command, _, _) => icons::COMMAND,
        (SubMode::OperatorPending { .. }, _, _) => icons::OPERATOR,
        (SubMode::Leap { .. }, _, _) => icons::LEAP,
        (SubMode::None, Focus::Telescope, _) => icons::TELESCOPE,
        (SubMode::None, Focus::Explorer, _) => icons::EXPLORER,
        (SubMode::None, Focus::SettingsMenu, _) => icons::SETTINGS,
        (SubMode::None, Focus::Editor, EditMode::Normal) => icons::NORMAL,
        (SubMode::None, Focus::Editor, EditMode::Insert(_)) => icons::INSERT,
        (SubMode::None, Focus::Editor, EditMode::Visual(_)) => icons::VISUAL,
    }
}

/// Get filetype from file path extension
fn filetype_from_path(path: &str) -> &'static str {
    // Extract extension (everything after last dot)
    let ext = path.rsplit('.').next().unwrap_or("");

    match ext.to_lowercase().as_str() {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "jsx" => "javascriptreact",
        "tsx" => "typescriptreact",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "go" => "go",
        "lua" => "lua",
        "sh" | "bash" => "bash",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "html" | "htm" => "html",
        "css" => "css",
        "sql" => "sql",
        "vim" => "vim",
        "txt" => "text",
        _ => "", // Unknown extension - don't display
    }
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
    let icon = mode_icon(mode);

    // Get mode-specific style
    let mode_style = match (&mode.sub_mode, &mode.focus, &mode.edit_mode) {
        // Command sub-mode, Telescope, or SettingsMenu uses command style
        (SubMode::Command, _, _) | (SubMode::None, Focus::Telescope | Focus::SettingsMenu, _) => {
            &theme.statusline.mode.command
        }
        // Operator-pending, Leap, and normal mode in Editor
        (SubMode::OperatorPending { .. } | SubMode::Leap { .. }, _, _)
        | (SubMode::None, Focus::Editor, EditMode::Normal) => &theme.statusline.mode.normal,
        // Editor insert mode
        (SubMode::None, Focus::Editor, EditMode::Insert(_)) => &theme.statusline.mode.insert,
        // Editor visual mode
        (SubMode::None, Focus::Editor, EditMode::Visual(_)) => &theme.statusline.mode.visual,
        // Explorer
        (SubMode::None, Focus::Explorer, _) => &theme.statusline.mode.explorer,
    };

    // Get buffer info
    let buffer_name = buffer
        .and_then(|b| b.file_path.as_ref())
        .map_or("[No Name]", String::as_str);
    let is_modified = buffer.is_some_and(|b| b.modified);
    let (cursor_line, cursor_col) = buffer.map_or((1, 1), |b| (b.cur.y + 1, b.cur.x + 1));
    let filetype = buffer
        .and_then(|b| b.file_path.as_ref())
        .map_or("", |p| filetype_from_path(p));

    // Format pending keys or last command section
    let cmd_section = if !pending_keys.is_empty() {
        format!(" {pending_keys} ")
    } else if !last_command.is_empty() {
        format!(" [{last_command}] ")
    } else {
        String::from(" ")
    };

    // Build right side: [filename][+] [filetype] Ln X, Col Y
    let modified_indicator = if is_modified { "[+]" } else { "" };
    let position_str = format!("Ln {cursor_line}, Col {cursor_col}");
    let right_side = if filetype.is_empty() {
        format!("{buffer_name}{modified_indicator} {position_str} ")
    } else {
        format!("{buffer_name}{modified_indicator} {filetype} {position_str} ")
    };

    // Calculate spacing
    let status_row = screen_height.saturating_sub(1);
    // Mode section includes icon + mode_str
    let mode_display = format!("{icon}{mode_str}");
    let left_len = mode_display.chars().count() + cmd_section.len();
    let right_len = right_side.chars().count();
    let width = screen_width as usize;

    let spacing = if left_len + right_len < width {
        width - left_len - right_len
    } else {
        0
    };

    queue!(out, MoveTo(0, status_row))?;

    // Render mode indicator with icon and mode style
    if !mode_str.is_empty() || !icon.is_empty() {
        let mode_ansi = mode_style.to_ansi_start(color_mode);
        queue!(out, Print(&mode_ansi))?;
        queue!(out, Print(icon))?;
        queue!(out, Print(mode_str))?;
        queue!(out, Print(RESET_STYLE))?;
    }

    // Render rest of status line with statusline.background style
    let status_ansi = theme.statusline.background.to_ansi_start(color_mode);
    queue!(out, Print(&status_ansi))?;
    queue!(out, Print(&cmd_section))?;
    queue!(out, Print(format!("{:spacing$}", "", spacing = spacing)))?;

    // Render filename with optional modified indicator
    let filename_ansi = theme.statusline.filename.to_ansi_start(color_mode);
    queue!(out, Print(&filename_ansi))?;
    queue!(out, Print(buffer_name))?;
    queue!(out, Print(RESET_STYLE))?;

    // Render modified indicator if modified
    if is_modified {
        let modified_ansi = theme.statusline.modified.to_ansi_start(color_mode);
        queue!(out, Print(&modified_ansi))?;
        queue!(out, Print("[+]"))?;
        queue!(out, Print(RESET_STYLE))?;
    }

    // Render filetype and position
    queue!(out, Print(&status_ansi))?;
    if !filetype.is_empty() {
        queue!(out, Print(" "))?;
        let filetype_ansi = theme.statusline.filetype.to_ansi_start(color_mode);
        queue!(out, Print(&filetype_ansi))?;
        queue!(out, Print(filetype))?;
        queue!(out, Print(RESET_STYLE))?;
        queue!(out, Print(&status_ansi))?;
    }

    // Position with position style
    queue!(out, Print(" "))?;
    let position_ansi = theme.statusline.position.to_ansi_start(color_mode);
    queue!(out, Print(&position_ansi))?;
    queue!(out, Print(&position_str))?;
    queue!(out, Print(" "))?;
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
