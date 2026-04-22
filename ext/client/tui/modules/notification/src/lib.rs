#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![cfg_attr(coverage_nightly, allow(unused_features))]
//! Notification toast chrome module.
//!
//! Displays stacked toast notifications in the top-right corner.
//! Each toast auto-dismisses after a configurable timeout (default 4s).
//! Progress notifications remain until dismissed by the server.
//! Native `ClientModule` implementation (no `TuiExtension` bridge).

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use reovim_client_driver::{
    ChromePosition, ChromeSurface, ClientModule, ClientModuleError, ModuleContext,
    PlatformCapabilities, ProbeResult, Rect, Style, Version, types::Color,
};

// Re-export the Clock trait from arch for constructor usage.
pub use reovim_client_driver::reovim_arch::clock::{Clock, SystemClock};

/// Default auto-dismiss timeout for non-progress toasts.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(4);

/// Maximum number of visible toasts.
const MAX_VISIBLE: usize = 5;

/// Toast width (fixed, right-aligned).
const TOAST_WIDTH: u16 = 40;

const KIND: &str = "notification";

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
    /// Optional producer name for grouped display (#691).
    source: Option<String>,
}

/// Notification toast chrome module.
pub struct NotificationModule {
    /// Active toasts being displayed.
    toasts: Vec<Toast>,
    /// Auto-dismiss timeout.
    timeout: Duration,
    /// Clock source for deterministic testing.
    clock: Arc<dyn Clock>,
}

impl NotificationModule {
    /// Create a new notification module with default 4s timeout.
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

    /// Get Nerd Font icon for a notification level.
    const fn level_icon(level: Level) -> &'static str {
        match level {
            Level::Info => "\u{f02fd}",    // 󰋽 nf-md-information
            Level::Success => "\u{f012c}", // 󰄬 nf-md-check
            Level::Warning => "\u{f002a}", // 󰀪 nf-md-alert
            Level::Error => "\u{f0159}",   // 󰅙 nf-md-close_circle
        }
    }
}

impl Default for NotificationModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for NotificationModule {
    fn id(&self) -> &'static str {
        KIND
    }

    fn kind(&self) -> &'static str {
        KIND
    }

    fn name(&self) -> &'static str {
        "Notification"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn has_chrome(&self) -> bool {
        true
    }

    fn chrome_position(&self) -> ChromePosition {
        ChromePosition::Overlay
    }

    fn chrome_priority(&self) -> u16 {
        40
    }

    fn on_notification(&mut self, data: &str) {
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

            // New toast - parse and add
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

            let source = entry
                .get("source")
                .and_then(serde_json::Value::as_str)
                .map(String::from);

            self.toasts.push(Toast {
                id,
                level: Self::parse_level(level_str),
                title,
                body,
                progress,
                displayed_at: self.clock.now(),
                is_progress,
                source,
            });
        }

        // Remove toasts whose IDs are no longer in server state
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
    fn chrome_render(
        &self,
        surface: &mut dyn ChromeSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        if self.toasts.is_empty() {
            return;
        }

        let width = bounds.width;
        let toast_w = TOAST_WIDTH.min(width.saturating_sub(2));
        let toast_x = width.saturating_sub(toast_w + 1);
        let mut y: u16 = 1;

        // Collect visible toasts (newest first), capped at MAX_VISIBLE
        let visible: Vec<&Toast> = self.toasts.iter().rev().take(MAX_VISIBLE).collect();

        // Partition into groups: grouped by source, or standalone (None source)
        let mut groups: Vec<(Option<&str>, Vec<&Toast>)> = Vec::new();
        for toast in &visible {
            if let Some(ref source) = toast.source {
                if let Some(group) = groups.iter_mut().find(|(s, _)| *s == Some(source.as_str())) {
                    group.1.push(toast);
                } else {
                    groups.push((Some(source.as_str()), vec![toast]));
                }
            } else {
                // Standalone toast (no source) — each gets its own "group"
                groups.push((None, vec![toast]));
            }
        }

        for (source, toasts) in &groups {
            if source.is_some() {
                y = render_grouped_box(self, surface, toast_x, y, toast_w, *source, toasts);
            } else {
                for toast in toasts {
                    y = render_standalone_toast(self, surface, toast_x, y, toast_w, toast);
                }
            }
        }
    }
}

/// Render a grouped notification box for toasts sharing the same source (#691).
///
/// Layout:
/// ```text
/// ┌─ rust-analyzer ──────────────────┐
/// │  ✓ Language server ready         │
/// │  42% ████████░░░░ Indexing       │
/// └──────────────────────────────────┘
/// ```
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::cast_possible_truncation)]
fn render_grouped_box(
    module: &NotificationModule,
    surface: &mut dyn ChromeSurface,
    x: u16,
    y: u16,
    width: u16,
    source: Option<&str>,
    toasts: &[&Toast],
) -> u16 {
    // Calculate total content height: one line per toast entry
    let content_lines: u16 = toasts
        .iter()
        .map(|t| {
            let has_body = !t.body.is_empty();
            let has_progress = t.progress.is_some();
            1 + u16::from(has_body) + u16::from(has_progress)
        })
        .sum();
    let box_height = content_lines + 2; // +2 for top/bottom borders

    // Use highest-priority level color for the group border
    let group_level = toasts
        .iter()
        .map(|t| t.level)
        .max_by_key(|l| match l {
            Level::Error => 3,
            Level::Warning => 2,
            Level::Success => 1,
            Level::Info => 0,
        })
        .unwrap_or(Level::Info);
    let border_color = NotificationModule::level_color(group_level);
    let border_style = Style::new().fg(border_color);

    // Draw border with source name in top border
    reovim_client_driver::chrome_utils::render_box_border(
        surface,
        x,
        y,
        width,
        box_height,
        &border_style,
    );

    // Write source name into top border
    if let Some(name) = source {
        let label = format!(" {name} ");
        let max_label = (width.saturating_sub(4)) as usize;
        let display = reovim_client_driver::ui::truncate_end(&label, max_label);
        let label_style = Style::new().fg(border_color).bold();
        surface.write_styled(x + 2, y, &display, label_style);
    }

    let content_x = x + 2;
    let content_width = width.saturating_sub(4);

    // Clear interior
    for row in 1..box_height.saturating_sub(1) {
        surface.fill(
            Rect {
                x: content_x,
                y: y + row,
                width: content_width,
                height: 1,
            },
            ' ',
            Style::new(),
        );
    }

    // Render each toast entry inside the box
    let mut current_row = y + 1;
    for toast in toasts {
        render_toast_content(module, surface, content_x, current_row, content_width, toast);
        current_row += 1;
        if !toast.body.is_empty() {
            current_row += 1;
        }
        if toast.progress.is_some() {
            current_row += 1;
        }
    }

    y + box_height + 1 // 1-row gap
}

/// Render a standalone toast (no source grouping — backward compat).
#[allow(clippy::cast_possible_truncation)]
fn render_standalone_toast(
    module: &NotificationModule,
    surface: &mut dyn ChromeSurface,
    x: u16,
    y: u16,
    width: u16,
    toast: &Toast,
) -> u16 {
    let has_body = !toast.body.is_empty();
    let has_progress = toast.progress.is_some();
    let content_lines = 1 + u16::from(has_body) + u16::from(has_progress);
    let toast_height = content_lines + 2;

    let border_color = NotificationModule::level_color(toast.level);
    let border_style = Style::new().fg(border_color);

    reovim_client_driver::chrome_utils::render_box_border(
        surface,
        x,
        y,
        width,
        toast_height,
        &border_style,
    );

    let content_x = x + 2;
    let content_width = width.saturating_sub(4);

    for row in 1..toast_height.saturating_sub(1) {
        surface.fill(
            Rect {
                x: content_x,
                y: y + row,
                width: content_width,
                height: 1,
            },
            ' ',
            Style::new(),
        );
    }

    render_toast_content(module, surface, content_x, y + 1, content_width, toast);

    y + toast_height + 1
}

/// Render the content of a single toast entry (icon + title + optional body + progress).
#[allow(clippy::cast_possible_truncation)]
fn render_toast_content(
    _module: &NotificationModule,
    surface: &mut dyn ChromeSurface,
    x: u16,
    y: u16,
    width: u16,
    toast: &Toast,
) {
    let border_color = NotificationModule::level_color(toast.level);
    let icon = NotificationModule::level_icon(toast.level);
    let icon_style = Style::new().fg(border_color);
    let icon_written = surface.write_styled(x, y, icon, icon_style);
    surface.write_styled(x + icon_written, y, " ", Style::new());

    let title_style = Style::new().fg(Color::White);
    let max_title_len = width.saturating_sub(2) as usize;
    let title_display = reovim_client_driver::ui::truncate_end(&toast.title, max_title_len);
    surface.write_styled(x + 2, y, &title_display, title_style);

    let mut current_row = y + 1;

    if !toast.body.is_empty() {
        let body_style = Style::new().fg(Color::DarkGrey);
        let body_display = reovim_client_driver::ui::truncate_end(&toast.body, width as usize);
        surface.write_styled(x, current_row, &body_display, body_style);
        current_row += 1;
    }

    if let Some((percent, ref detail)) = toast.progress {
        render_progress_bar(surface, x, current_row, width, percent, detail);
    }
}

/// Render a horizontal progress bar.
///
/// Layout: `NNN% [bar...] detail`
/// If detail is non-empty, the bar shrinks to make room.
#[allow(clippy::cast_possible_truncation)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn render_progress_bar(
    surface: &mut dyn ChromeSurface,
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
        let needed = 1 + detail.len() as u16;
        needed.min(available / 2)
    };
    let bar_width = u32::from(available.saturating_sub(detail_cols));
    debug_assert!(bar_width > 0);

    let filled = (u32::from(percent.min(100)) * bar_width / 100) as u16;

    // Percentage label
    let pct_str = format!("{percent:>3}%");
    let pct_style = Style::new().fg(Color::White);
    surface.write_styled(x, y, &pct_str, pct_style);
    surface.write_styled(x + 4, y, " ", Style::new());

    // Bar
    let bar_x = x + label_width;
    let filled_style = Style::new().fg(Color::Green);
    let empty_style = Style::new().fg(Color::DarkGrey);

    for col in 0..bar_width as u16 {
        if col < filled {
            surface.write_styled(bar_x + col, y, "\u{2588}", filled_style.clone());
        } else {
            surface.write_styled(bar_x + col, y, "\u{2591}", empty_style.clone());
        }
    }

    // Detail text after bar
    if !detail.is_empty() && detail_cols > 1 {
        let detail_x = bar_x + bar_width as u16 + 1;
        let max_detail = (detail_cols - 1) as usize;
        let detail_display = reovim_client_driver::ui::truncate_end(detail, max_detail);
        let detail_style = Style::new().fg(Color::DarkGrey);
        surface.write_styled(detail_x, y, &detail_display, detail_style);
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
