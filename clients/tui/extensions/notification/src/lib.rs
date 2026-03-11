//! Notification toast TUI extension.
//!
//! Displays stacked toast notifications in the top-right corner.
//! Each toast auto-dismisses after a configurable timeout (default 4s).
//! Progress notifications remain until dismissed by the server.
//!
//! This crate is a self-contained TUI extension: it owns its state,
//! parses notifications, and renders through `RenderBackend`.
//! The engine has ZERO knowledge of this crate.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use {
    reovim_arch::{
        Color,
        clock::{Clock, SystemClock},
    },
    reovim_driver_display::{
        Style,
        popup_utils::render_box_border,
        render_backend::{RenderBackend, TuiExtension},
        ui::truncate_end,
    },
};

/// Default auto-dismiss timeout for non-progress toasts.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(4);

/// Maximum number of visible toasts.
const MAX_VISIBLE: usize = 5;

/// Toast width (fixed, right-aligned).
const TOAST_WIDTH: u16 = 40;

/// Notification level (client-side mirror).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Info,
    Success,
    Warning,
    Error,
}

/// Client-side toast entry with display lifecycle.
#[derive(Debug)]
struct Toast {
    /// Server-assigned unique ID.
    id: u64,
    /// Severity level.
    level: Level,
    /// Short title.
    title: String,
    /// Optional body text.
    body: String,
    /// Optional progress (percent, detail).
    progress: Option<(u8, String)>,
    /// When this toast was first displayed.
    displayed_at: Instant,
    /// Whether this is a progress toast (no auto-dismiss).
    is_progress: bool,
}

/// Notification toast TUI extension.
///
/// # State Machine
///
/// ```text
/// No toasts: is_active() = false
///   |
///   | apply_notification(entries with new IDs)
///   v
/// Has toasts: is_active() = true
///   |--- tick(): removes expired non-progress toasts
///   |--- apply_notification(empty entries) -> removes all toasts
///   v
/// No toasts: is_active() = false
/// ```
pub struct NotificationExtension {
    /// Active toasts being displayed.
    toasts: Vec<Toast>,
    /// Auto-dismiss timeout.
    timeout: Duration,
    /// Clock source for deterministic testing.
    clock: Arc<dyn Clock>,
}

impl NotificationExtension {
    /// Create a new notification extension with default 4s timeout.
    #[must_use]
    pub fn new() -> Self {
        Self {
            toasts: Vec::new(),
            timeout: DEFAULT_TIMEOUT,
            clock: Arc::new(SystemClock),
        }
    }

    /// Create with custom timeout and clock (for testing).
    #[must_use]
    pub fn with_clock(clock: Arc<dyn Clock>, timeout: Duration) -> Self {
        Self {
            toasts: Vec::new(),
            timeout,
            clock,
        }
    }

    /// Parse level string from JSON.
    fn parse_level(s: &str) -> Level {
        match s {
            "success" => Level::Success,
            "warning" => Level::Warning,
            "error" => Level::Error,
            _ => Level::Info,
        }
    }

    /// Get border color for a notification level.
    const fn level_color(level: Level) -> Color {
        match level {
            Level::Info => Color::Cyan,
            Level::Success => Color::Green,
            Level::Warning => Color::Yellow,
            Level::Error => Color::Red,
        }
    }

    /// Get icon character for a notification level.
    const fn level_icon(level: Level) -> char {
        match level {
            Level::Info => 'i',
            Level::Success => '+',
            Level::Warning => '!',
            Level::Error => 'x',
        }
    }
}

impl Default for NotificationExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiExtension for NotificationExtension {
    fn kind(&self) -> &'static str {
        reovim_extension_kinds::NOTIFICATION
    }

    fn is_active(&self) -> bool {
        !self.toasts.is_empty()
    }

    fn apply_notification(&mut self, data: &str) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
            return;
        };

        let Some(entries) = json.get("entries").and_then(serde_json::Value::as_array) else {
            return;
        };

        // Collect server-side IDs for reconciliation
        let mut server_ids: Vec<u64> = Vec::new();

        for entry in entries {
            let Some(id) = entry.get("id").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            server_ids.push(id);

            // Update progress on existing toast if present
            if let Some(toast) = self.toasts.iter_mut().find(|t| t.id == id) {
                if let Some(progress) = entry.get("progress") {
                    let percent = u8::try_from(
                        progress
                            .get("percent")
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(0),
                    )
                    .unwrap_or(100);
                    let detail = progress
                        .get("detail")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    toast.progress = Some((percent, detail));
                }
                continue;
            }

            // New toast — parse and add
            let level_str = entry
                .get("level")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("info");
            let title = entry
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            let body = entry
                .get("body")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();

            let progress = entry.get("progress").map(|p| {
                let pct = u8::try_from(
                    p.get("percent")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0),
                )
                .unwrap_or(100);
                let detail = p
                    .get("detail")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string();
                (pct, detail)
            });

            let is_progress = progress.is_some();

            self.toasts.push(Toast {
                id,
                level: Self::parse_level(level_str),
                title,
                body,
                progress,
                displayed_at: self.clock.now(),
                is_progress,
            });
        }

        // Remove toasts whose IDs are no longer in server state
        // (server dismissed them, e.g., progress completed)
        self.toasts.retain(|t| server_ids.contains(&t.id));
    }

    fn tick(&mut self) -> bool {
        let now = self.clock.now();
        let before = self.toasts.len();

        // Auto-dismiss non-progress toasts after timeout
        self.toasts.retain(|toast| {
            if toast.is_progress {
                return true;
            }
            now.duration_since(toast.displayed_at) < self.timeout
        });

        self.toasts.len() != before
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(&self, backend: &mut dyn RenderBackend) {
        if self.toasts.is_empty() {
            return;
        }

        let (width, _height) = backend.size();

        // Position: top-right corner with 1-col margin
        let toast_w = TOAST_WIDTH.min(width.saturating_sub(2));
        let toast_x = width.saturating_sub(toast_w + 1);
        let mut y: u16 = 1;

        // Show newest toasts first (reverse), capped at MAX_VISIBLE
        let visible: Vec<&Toast> = self.toasts.iter().rev().take(MAX_VISIBLE).collect();

        for toast in &visible {
            let has_body = !toast.body.is_empty();
            let has_progress = toast.progress.is_some();
            // Height: 1 (title) + optional body + optional progress bar
            let content_lines = 1 + u16::from(has_body) + u16::from(has_progress);
            let toast_height = content_lines + 2; // +2 for borders

            let border_color = Self::level_color(toast.level);
            let border_style = Style::default().fg(border_color);

            // Draw border using shared utility
            render_box_border(backend, toast_x, y, toast_w, toast_height, &border_style);

            let content_x = toast_x + 2;
            let content_width = toast_w.saturating_sub(4);

            // Clear interior
            let clear_style = Style::default();
            for row in 1..toast_height.saturating_sub(1) {
                for col in 0..content_width {
                    backend.set_cell(content_x + col, y + row, ' ', &clear_style);
                }
            }

            // Title line with level icon
            let icon = Self::level_icon(toast.level);
            let icon_style = Style::default().fg(border_color);
            backend.set_cell(content_x, y + 1, icon, &icon_style);
            backend.set_cell(content_x + 1, y + 1, ' ', &clear_style);

            let title_style = Style::default().fg(Color::White);
            let max_title_len = content_width.saturating_sub(2) as usize;
            let title_display = truncate_end(&toast.title, max_title_len);
            backend.write_str(content_x + 2, y + 1, &title_display, &title_style);

            let mut current_row = y + 2;

            // Body line
            if has_body {
                let body_style = Style::default().fg(Color::DarkGrey);
                let body_display = truncate_end(&toast.body, content_width as usize);
                backend.write_str(content_x, current_row, &body_display, &body_style);
                current_row += 1;
            }

            // Progress bar
            if let Some((percent, ref detail)) = toast.progress {
                render_progress_bar(
                    backend,
                    content_x,
                    current_row,
                    content_width,
                    percent,
                    detail,
                );
            }

            y += toast_height + 1; // 1-row gap between toasts
        }
    }
}

/// Render a horizontal progress bar.
///
/// Layout: `NNN% [bar...] detail`
/// If detail is non-empty, the bar shrinks to make room.
#[allow(clippy::cast_possible_truncation)]
fn render_progress_bar(
    backend: &mut dyn RenderBackend,
    x: u16,
    y: u16,
    width: u16,
    percent: u8,
    detail: &str,
) {
    // "100% " = 5 chars for the percentage label
    let label_width: u16 = 5;
    let available = width.saturating_sub(label_width);
    if available == 0 {
        return;
    }

    // Reserve space for " detail" if detail is non-empty
    let detail_cols = if detail.is_empty() {
        0u16
    } else {
        // 1 space + detail text length, capped to half the available space
        let needed = 1 + detail.len() as u16;
        needed.min(available / 2)
    };
    let bar_width = u32::from(available.saturating_sub(detail_cols));
    // Invariant: detail_cols <= available/2, so bar_width >= ceil(available/2) >= 1
    debug_assert!(bar_width > 0);

    let filled = (u32::from(percent.min(100)) * bar_width / 100) as u16;

    // Percentage label
    let pct_str = format!("{percent:>3}%");
    let pct_style = Style::default().fg(Color::White);
    backend.write_str(x, y, &pct_str, &pct_style);
    backend.set_cell(x + 4, y, ' ', &Style::default());

    // Bar
    let bar_x = x + label_width;
    let filled_style = Style::default().fg(Color::Green);
    let empty_style = Style::default().fg(Color::DarkGrey);

    for col in 0..bar_width as u16 {
        if col < filled {
            backend.set_cell(bar_x + col, y, '\u{2588}', &filled_style);
        } else {
            backend.set_cell(bar_x + col, y, '\u{2591}', &empty_style);
        }
    }

    // Detail text after bar
    if !detail.is_empty() && detail_cols > 1 {
        let detail_x = bar_x + bar_width as u16 + 1;
        let max_detail = (detail_cols - 1) as usize;
        let detail_display = truncate_end(detail, max_detail);
        let detail_style = Style::default().fg(Color::DarkGrey);
        backend.write_str(detail_x, y, &detail_display, &detail_style);
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
