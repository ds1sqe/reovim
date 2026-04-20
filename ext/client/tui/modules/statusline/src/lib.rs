#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Statusline chrome module.
//!
//! Renders a lualine-style statusline with six sections:
//!
//! ```text
//! [A: Mode] [B: Branch] [C: Filename]     [X: Diagnostics] [Y: Filetype] [Z: Pos]
//! ```
//!
//! Data flows from server via `ClientModule` callbacks and `ExtensionUpdated`
//! notifications. Sections are populated incrementally:
//! - Phase 1: A (mode) + Z (position + progress)
//! - Phase 2: C (filename, modified, filetype) + Y (encoding)
//! - Phase 3: B (git branch)
//! - Phase 4: X (diagnostic counts)

use {
    reovim_client_driver::{
        BufferId, ChromePosition, ClientModule, ClientModuleError, ModuleContext,
        PlatformCapabilities, ProbeResult, Rect, RenderSurface, Style, ThemeProvider, Version,
        types::{BufferUpdateEvent, Color},
    },
    serde::Deserialize,
};

/// JSON payload for buffer metadata notifications from the TUI (#661).
#[derive(Debug, Deserialize)]
struct BufferMetadataPayload {
    filename: Option<String>,
    filetype: Option<String>,
    encoding: Option<String>,
    modified: Option<bool>,
    readonly: Option<bool>,
    diagnostics: Option<DiagnosticCountsPayload>,
}

/// Diagnostic counts embedded in buffer metadata.
#[derive(Debug, Deserialize)]
struct DiagnosticCountsPayload {
    #[serde(default)]
    error: u32,
    #[serde(default)]
    warning: u32,
    #[serde(default)]
    info: u32,
    #[serde(default)]
    hint: u32,
}

/// Separator character between right-side sections.
const SECTION_SEP: &str = " \u{2502} ";
/// Separator width in columns (space + box-drawing + space).
const SECTION_SEP_WIDTH: usize = 3;

// Inline icon constants (no dependency on reovim-driver-display).
const GIT_BRANCH_ICON: &str = "\u{e0a0}";
const MODIFIED_ICON: &str = "\u{25cf}";
const READONLY_ICON: &str = "\u{f033e}";
const ERROR_ICON: &str = "\u{f015a}";
const WARNING_ICON: &str = "\u{f002a}";
const INFO_ICON: &str = "\u{f02fd}";
const HINT_ICON: &str = "\u{f0336}";

/// Statusline chrome module.
///
/// Renders a lualine-style statusline with mode badge, file info, git branch,
/// diagnostics, and cursor position organized in six sections (A-Z).
pub struct StatuslineModule {
    // Section A: Mode
    mode: String,

    // Section Z: Cursor position + progress
    cursor_line: usize,
    cursor_col: usize,
    total_lines: usize,
    has_cursor: bool,

    // Section C: Buffer info (Phase 2)
    filename: Option<String>,
    modified: bool,
    readonly: bool,

    // Section Y: Filetype + encoding (Phase 2)
    filetype: Option<String>,
    encoding: Option<String>,

    // Section B: Git branch (Phase 3)
    git_branch: Option<String>,

    // Section X: Diagnostic counts (Phase 4)
    diag_error: u32,
    diag_warning: u32,
    diag_info: u32,
    diag_hint: u32,

    // Theme-cached styles
    bg_style: Style,
    sep_style: Style,
}

impl StatuslineModule {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode: String::new(),
            cursor_line: 0,
            cursor_col: 0,
            total_lines: 0,
            has_cursor: false,
            filename: None,
            modified: false,
            readonly: false,
            filetype: None,
            encoding: None,
            git_branch: None,
            diag_error: 0,
            diag_warning: 0,
            diag_info: 0,
            diag_hint: 0,
            bg_style: Style::new().bg(Color::DarkGrey).fg(Color::White),
            sep_style: Style::new().fg(Color::Grey).bg(Color::DarkGrey),
        }
    }

    fn cache_theme(&mut self, theme: &dyn ThemeProvider) {
        let background = theme.highlight("statusline_bg");
        let foreground = theme.highlight("statusline_fg");
        self.bg_style = Style::new()
            .bg(background.bg.unwrap_or(Color::DarkGrey))
            .fg(foreground.fg.unwrap_or(Color::White));
        self.sep_style = Style::new()
            .fg(Color::Grey)
            .bg(background.bg.unwrap_or(Color::DarkGrey));
    }
}

impl Default for StatuslineModule {
    fn default() -> Self {
        let mut m = Self::new();
        "NORMAL".clone_into(&mut m.mode);
        m
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ClientModule for StatuslineModule {
    fn id(&self) -> &'static str {
        "statusline"
    }

    fn kind(&self) -> &'static str {
        "statusline"
    }

    fn name(&self) -> &'static str {
        "Statusline"
    }

    fn version(&self) -> Version {
        Version::new(0, 2, 0)
    }

    fn server_kinds(&self) -> Vec<&'static str> {
        vec![]
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        if self.mode.is_empty() {
            "NORMAL".clone_into(&mut self.mode);
        }
        self.cache_theme(ctx.theme);
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn has_chrome(&self) -> bool {
        true
    }

    fn chrome_position(&self) -> ChromePosition {
        ChromePosition::Bottom
    }

    fn chrome_requested_size(&self, _caps: &dyn PlatformCapabilities) -> u16 {
        1
    }

    fn chrome_priority(&self) -> u16 {
        100
    }

    fn on_mode_change(&mut self, mode: &str) {
        self.mode = mode.to_uppercase();
    }

    fn on_cursor_update(&mut self, _buffer_id: BufferId, line: usize, col: usize) {
        self.cursor_line = line;
        self.cursor_col = col;
        self.has_cursor = true;
    }

    fn on_buffer_update(&mut self, event: &BufferUpdateEvent) {
        self.total_lines = event.total_lines;
    }

    fn on_theme_changed(&mut self, theme: &dyn ThemeProvider) {
        self.cache_theme(theme);
    }

    fn on_notification(&mut self, data: &str) {
        if let Ok(payload) = serde_json::from_str::<BufferMetadataPayload>(data) {
            if let Some(name) = payload.filename {
                self.filename = if name.is_empty() { None } else { Some(name) };
            }
            if let Some(ft) = payload.filetype {
                self.filetype = if ft.is_empty() { None } else { Some(ft) };
            }
            if let Some(enc) = payload.encoding {
                self.encoding = if enc.is_empty() { None } else { Some(enc) };
            }
            if let Some(m) = payload.modified {
                self.modified = m;
            }
            if let Some(r) = payload.readonly {
                self.readonly = r;
            }
            if let Some(diag) = payload.diagnostics {
                self.diag_error = diag.error;
                self.diag_warning = diag.warning;
                self.diag_info = diag.info;
                self.diag_hint = diag.hint;
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        let width = bounds.width as usize;
        if width == 0 {
            return;
        }

        // Fill background
        surface.fill(bounds, ' ', self.bg_style.clone());

        // --- Left sections: A | B | C ---
        let mut x = bounds.x;

        // Section A: Mode badge
        let mode_style = mode_style(&self.mode);
        let mode_str = format!(" {} ", self.mode);
        x += surface.write_styled(x, bounds.y, &mode_str, mode_style);

        // Section B: Git branch (Phase 3)
        if let Some(branch) = &self.git_branch {
            let branch_str = format!(" {GIT_BRANCH_ICON} {branch} ");
            x += surface.write_styled(x, bounds.y, &branch_str, self.bg_style.clone());
        }

        // Section C: Filename + modified/readonly (Phase 2)
        if let Some(name) = &self.filename {
            let mut file_str = format!(" {name}");
            if self.modified {
                file_str.push(' ');
                file_str.push_str(MODIFIED_ICON);
            }
            if self.readonly {
                file_str.push(' ');
                file_str.push_str(READONLY_ICON);
            }
            file_str.push(' ');
            x += surface.write_styled(x, bounds.y, &file_str, self.bg_style.clone());
        }

        // Suppress unused assignment warning — x marks left-section end
        let _ = x;

        // --- Right sections: X | Y | Z (rendered right-to-left) ---
        let right_sections = self.build_right_sections();
        let total_right_width: usize = right_sections
            .iter()
            .map(|(text, _)| text.len())
            .sum::<usize>()
            + right_sections.len().saturating_sub(1) * SECTION_SEP_WIDTH;

        if total_right_width < width {
            let mut rx = bounds.x + bounds.width - total_right_width as u16;
            for (i, (text, style)) in right_sections.iter().enumerate() {
                if i > 0 {
                    rx += surface.write_styled(rx, bounds.y, SECTION_SEP, self.sep_style.clone());
                }
                rx += surface.write_styled(rx, bounds.y, text, style.clone());
            }
        }
    }
}

impl StatuslineModule {
    /// Build right-side section content: X (diagnostics), Y (filetype+encoding), Z (pos+progress).
    /// Returns vec of (text, style) pairs in left-to-right order.
    fn build_right_sections(&self) -> Vec<(String, Style)> {
        let mut sections = Vec::new();

        // Section X: Diagnostic counts (Phase 4)
        let diag_text = self.build_diagnostic_text();
        if !diag_text.is_empty() {
            sections.push((diag_text, self.bg_style.clone()));
        }

        // Section Y: filetype | encoding (Phase 2)
        if let Some(ft) = &self.filetype {
            let y_text = self
                .encoding
                .as_ref()
                .map_or_else(|| ft.clone(), |enc| format!("{ft} {enc}"));
            sections.push((y_text, self.bg_style.clone()));
        }

        // Section Z: progress + line:col
        let z_text = if self.has_cursor {
            let progress = progress_indicator(self.cursor_line, self.total_lines);
            format!("{progress} {}:{}", self.cursor_line + 1, self.cursor_col + 1)
        } else {
            String::from("?:?")
        };
        sections.push((z_text, self.bg_style.clone()));

        sections
    }

    /// Build diagnostic count text for Section X.
    /// Only includes counts > 0.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn build_diagnostic_text(&self) -> String {
        let mut parts = Vec::new();
        if self.diag_error > 0 {
            parts.push(format!("{ERROR_ICON} {}", self.diag_error));
        }
        if self.diag_warning > 0 {
            parts.push(format!("{WARNING_ICON} {}", self.diag_warning));
        }
        if self.diag_info > 0 {
            parts.push(format!("{INFO_ICON} {}", self.diag_info));
        }
        if self.diag_hint > 0 {
            parts.push(format!("{HINT_ICON} {}", self.diag_hint));
        }
        parts.join(" ")
    }
}

/// Calculate the progress indicator string (Vim-style).
///
/// Returns "Top" when at line 0, "Bot" when at last line, "All" when the
/// buffer fits in one screen (`total_lines` <= 1), or "N%" for the percentage
/// position.
fn progress_indicator(cursor_line: usize, total_lines: usize) -> &'static str {
    if total_lines <= 1 {
        return "All";
    }
    if cursor_line == 0 {
        return "Top";
    }
    let last_line = total_lines - 1;
    if cursor_line >= last_line {
        return "Bot";
    }
    // Return a percentage bucket string.
    // We use a static table to avoid allocation.
    let pct = cursor_line * 100 / last_line;
    PCT_STRINGS[pct.min(99)]
}

/// Static table of percentage strings "1%" through "99%".
/// Index 0 is "0%" (unreachable since `cursor_line==0` returns "Top").
const PCT_STRINGS: [&str; 100] = [
    "0%", "1%", "2%", "3%", "4%", "5%", "6%", "7%", "8%", "9%", "10%", "11%", "12%", "13%", "14%",
    "15%", "16%", "17%", "18%", "19%", "20%", "21%", "22%", "23%", "24%", "25%", "26%", "27%",
    "28%", "29%", "30%", "31%", "32%", "33%", "34%", "35%", "36%", "37%", "38%", "39%", "40%",
    "41%", "42%", "43%", "44%", "45%", "46%", "47%", "48%", "49%", "50%", "51%", "52%", "53%",
    "54%", "55%", "56%", "57%", "58%", "59%", "60%", "61%", "62%", "63%", "64%", "65%", "66%",
    "67%", "68%", "69%", "70%", "71%", "72%", "73%", "74%", "75%", "76%", "77%", "78%", "79%",
    "80%", "81%", "82%", "83%", "84%", "85%", "86%", "87%", "88%", "89%", "90%", "91%", "92%",
    "93%", "94%", "95%", "96%", "97%", "98%", "99%",
];

/// Get the style for a mode indicator badge.
fn mode_style(mode: &str) -> Style {
    let mode_lower = mode.to_lowercase();
    let (fg, bg) = if mode_lower.contains("insert") {
        (Color::Black, Color::Green)
    } else if mode_lower.contains("visual") {
        (Color::Black, Color::Magenta)
    } else if mode_lower.contains("command") || mode_lower.contains("cmdline") {
        (Color::Black, Color::Yellow)
    } else if mode_lower.contains("replace") {
        (Color::Black, Color::Red)
    } else {
        // Normal mode (default)
        (Color::Black, Color::Blue)
    };
    Style::new().fg(fg).bg(bg)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
