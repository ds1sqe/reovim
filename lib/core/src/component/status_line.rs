//! Status line display component
//!
//! Renders mode indicator, pending keys, filename, filetype, and cursor position.
//!
//! Format: `[MODE_ICON MODE] [pending/cmd] ... [FILENAME][+] [FILETYPE] Ln X, Col Y`

use crate::{
    buffer::Buffer,
    component::{DisplayComponent, RenderContext},
    frame::FrameBuffer,
    interactor::InteractorId,
    modd::{EditMode, ModeState, SubMode},
    screen::LayerBounds,
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

/// Status line display component
///
/// Displays current mode, pending keys, file info, and cursor position
/// at the bottom of the screen.
#[derive(Debug)]
pub struct StatusLineComponent<'a> {
    /// Current mode state
    pub mode: &'a ModeState,
    /// Current buffer (for filename, cursor position)
    pub buffer: Option<&'a Buffer>,
    /// Pending key sequence
    pub pending_keys: &'a str,
    /// Last executed command
    pub last_command: &'a str,
}

impl<'a> StatusLineComponent<'a> {
    /// Create a new status line component
    #[must_use]
    pub const fn new(
        mode: &'a ModeState,
        buffer: Option<&'a Buffer>,
        pending_keys: &'a str,
        last_command: &'a str,
    ) -> Self {
        Self {
            mode,
            buffer,
            pending_keys,
            last_command,
        }
    }

    /// Get mode icon for the current mode state
    fn mode_icon(&self) -> &'static str {
        // Sub-modes take precedence
        match &self.mode.sub_mode {
            SubMode::Command => return icons::COMMAND,
            SubMode::OperatorPending { .. } => return icons::OPERATOR,
            SubMode::Leap { .. } => return icons::LEAP,
            SubMode::None => {}
        }

        // Then check interactor
        if self.mode.interactor_id == InteractorId::TELESCOPE {
            return icons::TELESCOPE;
        }
        if self.mode.interactor_id == InteractorId::EXPLORER {
            return icons::EXPLORER;
        }
        if self.mode.interactor_id == InteractorId::SETTINGS {
            return icons::SETTINGS;
        }

        // Editor modes
        match &self.mode.edit_mode {
            EditMode::Normal => icons::NORMAL,
            EditMode::Insert(_) => icons::INSERT,
            EditMode::Visual(_) => icons::VISUAL,
        }
    }

    /// Get style for current mode
    fn get_mode_style<'t>(
        &self,
        theme: &'t crate::highlight::Theme,
    ) -> &'t crate::highlight::Style {
        // Sub-modes
        match &self.mode.sub_mode {
            SubMode::Command => return &theme.statusline.mode.command,
            SubMode::OperatorPending { .. } | SubMode::Leap { .. } => {
                return &theme.statusline.mode.normal;
            }
            SubMode::None => {}
        }

        // Interactor-specific styles
        if self.mode.interactor_id == InteractorId::TELESCOPE
            || self.mode.interactor_id == InteractorId::SETTINGS
        {
            return &theme.statusline.mode.command;
        }
        if self.mode.interactor_id == InteractorId::EXPLORER {
            return &theme.statusline.mode.explorer;
        }

        // Editor edit modes
        match &self.mode.edit_mode {
            EditMode::Normal => &theme.statusline.mode.normal,
            EditMode::Insert(_) => &theme.statusline.mode.insert,
            EditMode::Visual(_) => &theme.statusline.mode.visual,
        }
    }
}

/// Get filetype from file path extension
#[allow(dead_code)] // Will be used in future enhanced status line
fn filetype_from_path(path: &str) -> &'static str {
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
        _ => "",
    }
}

impl DisplayComponent for StatusLineComponent<'_> {
    fn component_id(&self) -> &'static str {
        "status_line"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render_to_frame(&self, buffer: &mut FrameBuffer, context: &RenderContext<'_>) {
        let mode_str = self.mode.display_string();
        let icon = self.mode_icon();
        let theme = context.theme;
        let status_row = context.status_line_row();

        // Get mode-specific style
        let mode_style = self.get_mode_style(theme);

        // Get buffer info
        let buffer_name = self
            .buffer
            .and_then(|b| b.file_path.as_ref())
            .map_or("[No Name]", String::as_str);
        let is_modified = self.buffer.is_some_and(|b| b.modified);
        let (cursor_line, cursor_col) = self.buffer.map_or((1, 1), |b| (b.cur.y + 1, b.cur.x + 1));

        // Format pending keys section
        let cmd_section = if self.pending_keys.is_empty() {
            String::from(" ")
        } else {
            format!(" {} ", self.pending_keys)
        };

        // Build right side content
        let right_content = format!(
            "{buffer_name}{} Ln {cursor_line}, Col {cursor_col} ",
            if is_modified { "[+]" } else { "" }
        );

        // Render mode indicator
        let mode_text = format!(" {icon}{mode_str} ");
        let mut x = buffer.write_str(0, status_row, &mode_text, mode_style);

        // Render pending keys
        x += buffer.write_str(x, status_row, &cmd_section, &theme.statusline.background);

        // Calculate fill space
        let right_start = context
            .screen_width
            .saturating_sub(right_content.len() as u16);
        let fill_style = theme.statusline.background.clone();

        // Fill middle
        for col in x..right_start {
            buffer.put_char(col, status_row, ' ', &fill_style);
        }

        // Render right side content
        buffer.write_str(right_start, status_row, &right_content, &theme.statusline.background);
    }

    fn bounds(&self, context: &RenderContext<'_>) -> LayerBounds {
        LayerBounds {
            x: 0,
            y: context.status_line_row(),
            width: context.screen_width,
            height: 1,
        }
    }
}
