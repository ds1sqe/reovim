//! Status line display component
//!
//! Renders mode indicator, pending keys, filename, filetype, and cursor position.
//!
//! Format with powerline separators:
//! `[MODE_ICON MODE] [pending/cmd] ... [FILENAME][+] [FILETYPE] Ln X, Col Y`

use crate::{
    buffer::Buffer,
    component::RenderContext,
    frame::FrameBuffer,
    highlight::Style,
    modd::{EditMode, ModeState, SubMode},
};

/// Mode icons (Nerd Font) - core modes only
/// Plugin-specific icons are provided via `DisplayRegistry`
mod icons {
    pub const NORMAL: &str = "󰆾 ";
    pub const INSERT: &str = "󰙏 ";
    pub const VISUAL: &str = "󰒉 ";
    pub const COMMAND: &str = "󰘳 ";
    pub const OPERATOR: &str = "󰦒 ";
    #[allow(dead_code)] // Will be used when Replace mode is implemented
    pub const REPLACE: &str = "󰛔 ";
    pub const INTERACTOR: &str = "󰆾 "; // Generic icon for plugin interactors
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
    /// Returns core mode icons only - plugins provide their own via `DisplayRegistry`
    fn mode_icon(&self) -> &'static str {
        // Sub-modes take precedence
        match &self.mode.sub_mode {
            SubMode::Command => return icons::COMMAND,
            SubMode::OperatorPending { .. } => return icons::OPERATOR,
            SubMode::Interactor(_) => return icons::INTERACTOR,
            SubMode::None => {}
        }

        // Non-editor interactors use generic icon
        if self.mode.interactor_id.0 != "editor" {
            return icons::INTERACTOR;
        }

        // Editor modes
        match &self.mode.edit_mode {
            EditMode::Normal => icons::NORMAL,
            EditMode::Insert(_) => icons::INSERT,
            EditMode::Visual(_) => icons::VISUAL,
        }
    }

    /// Get style for current mode
    /// Uses generic styles - plugins can override via theme customization
    fn get_mode_style<'t>(
        &self,
        theme: &'t crate::highlight::Theme,
    ) -> &'t crate::highlight::Style {
        // Sub-modes
        match &self.mode.sub_mode {
            SubMode::Command => return &theme.statusline.mode.command,
            SubMode::OperatorPending { .. } => return &theme.statusline.mode.operator_pending,
            SubMode::Interactor(_) => return &theme.statusline.mode.normal,
            SubMode::None => {}
        }

        // Non-editor interactors use normal style
        if self.mode.interactor_id.0 != "editor" {
            return &theme.statusline.mode.normal;
        }

        // Editor edit modes
        match &self.mode.edit_mode {
            EditMode::Normal => &theme.statusline.mode.normal,
            EditMode::Insert(_) => &theme.statusline.mode.insert,
            EditMode::Visual(_) => &theme.statusline.mode.visual,
        }
    }

    /// Get the short mode name for display
    const fn mode_name(&self) -> &'static str {
        // Sub-modes take precedence
        match &self.mode.sub_mode {
            SubMode::Command => return "COMMAND",
            SubMode::OperatorPending { .. } => return "OPERATOR",
            SubMode::Interactor(_) => return "INTERACTOR",
            SubMode::None => {}
        }

        // Editor modes
        match &self.mode.edit_mode {
            EditMode::Normal => "NORMAL",
            EditMode::Insert(_) => "INSERT",
            EditMode::Visual(_) => "VISUAL",
        }
    }
}

/// Get filetype from file path extension
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

impl StatusLineComponent<'_> {
    /// Render the status line to the frame buffer
    #[allow(clippy::cast_possible_truncation)]
    pub fn render_to_frame(&self, frame: &mut FrameBuffer, ctx: &RenderContext<'_>) {
        let theme = ctx.theme;
        let status_row = ctx.status_line_row();
        let width = ctx.screen_width as usize;

        // Get styles
        let mode_style = self.get_mode_style(theme);
        let bg_style = &theme.statusline.background;
        let separator = &theme.statusline.separator;

        // Get mode info
        let mode_icon = self.mode_icon();
        let mode_name = self.mode_name();

        // Get buffer info
        let (filename, is_modified, cursor_line, cursor_col, filetype) =
            self.buffer
                .map_or(("[No Name]", false, 1, 1, ""), |buffer| {
                    let path = buffer
                        .file_path
                        .as_ref()
                        .map_or("[No Name]", String::as_str);
                    let ft = filetype_from_path(path);
                    (path, buffer.modified, buffer.cur.y + 1, buffer.cur.x + 1, ft)
                });

        // Build sections
        let mode_text = format!(" {mode_icon}{mode_name} ");
        let pending_text = if self.pending_keys.is_empty() {
            String::new()
        } else {
            format!(" {} ", self.pending_keys)
        };

        // Right side sections
        let modified_indicator = if is_modified { " [+]" } else { "" };
        let filename_text = format!(" {filename}{modified_indicator} ");
        let filetype_text = if filetype.is_empty() {
            String::new()
        } else {
            format!(" {filetype} ")
        };
        let position_text = format!(" Ln {cursor_line}, Col {cursor_col} ");

        // Calculate right side widths for positioning
        let position_width = unicode_width(&position_text);
        let filetype_width = unicode_width(&filetype_text);
        let filename_width = unicode_width(&filename_text);
        let sep_right_width = unicode_width(separator.right);
        let section_sep_width = if filetype.is_empty() {
            0
        } else {
            unicode_width(separator.section)
        };

        // Calculate right side total
        let right_total =
            position_width + filetype_width + filename_width + sep_right_width + section_sep_width;

        // Render left side
        let mut x: u16 = 0;

        // Mode section with background
        x += frame.write_str(x, status_row, &mode_text, mode_style);

        // Left separator (powerline style: mode bg -> statusline bg)
        let sep_style = Self::create_separator_style(mode_style, bg_style);
        x += frame.write_str(x, status_row, separator.left, &sep_style);

        // Pending keys
        if !pending_text.is_empty() {
            x += frame.write_str(x, status_row, &pending_text, bg_style);
        }

        // Calculate where right side starts
        #[allow(clippy::cast_possible_truncation)]
        let right_start = width.saturating_sub(right_total) as u16;

        // Fill middle with background
        for col in x..right_start {
            frame.put_char(col, status_row, ' ', bg_style);
        }
        x = right_start;

        // Right separator (statusline bg -> filename section)
        x += frame.write_str(x, status_row, separator.right, &sep_style);

        // Filename section
        x += frame.write_str(x, status_row, &filename_text, &theme.statusline.filename);

        // Filetype section
        if !filetype_text.is_empty() {
            x += frame.write_str(x, status_row, separator.section, &separator.style);
            x += frame.write_str(x, status_row, &filetype_text, &theme.statusline.filetype);
        }

        // Position section
        frame.write_str(x, status_row, &position_text, &theme.statusline.position);
    }

    /// Create a separator style that transitions between two backgrounds
    fn create_separator_style(from: &Style, to: &Style) -> Style {
        // For powerline separators, fg is the "from" background, bg is the "to" background
        Style::new().fg_opt(from.bg).bg_opt(to.bg)
    }
}

/// Calculate the display width of a string (accounting for unicode)
fn unicode_width(s: &str) -> usize {
    // Simple approximation - each char is 1 width, except some emoji/icons are 2
    s.chars()
        .map(|c| {
            if c.is_ascii() {
                1
            } else {
                // Most Nerd Font icons and CJK chars are 2 wide
                2
            }
        })
        .sum()
}

/// Extension trait for Style to allow optional color setting
trait StyleExt {
    fn fg_opt(self, color: Option<reovim_sys::style::Color>) -> Self;
    fn bg_opt(self, color: Option<reovim_sys::style::Color>) -> Self;
}

impl StyleExt for Style {
    fn fg_opt(mut self, color: Option<reovim_sys::style::Color>) -> Self {
        self.fg = color;
        self
    }

    fn bg_opt(mut self, color: Option<reovim_sys::style::Color>) -> Self {
        self.bg = color;
        self
    }
}
