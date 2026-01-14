//! Notification popup window
//!
//! Plugin window that renders notifications and progress bars.

use std::sync::Arc;

use reovim_core::{
    frame::FrameBuffer,
    highlight::Theme,
    plugin::{EditorContext, PluginStateRegistry, PluginWindow, Rect, WindowConfig},
};

use crate::state::{NotificationPosition, SharedNotificationManager};

/// Plugin window for notification display
///
/// This window accesses the `SharedNotificationManager` from the plugin state
/// registry to ensure it uses the same instance that RPC handlers update.
pub struct NotificationPluginWindow;

impl NotificationPluginWindow {
    /// Create a new notification window
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for NotificationPluginWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to get the manager from the state registry
fn get_manager(state: &Arc<PluginStateRegistry>) -> Option<Arc<SharedNotificationManager>> {
    let result = state.with::<Arc<SharedNotificationManager>, _, _>(Clone::clone);
    if result.is_none() {
        tracing::warn!("NotificationPluginWindow: manager not found in state registry");
    }
    result
}

impl PluginWindow for NotificationPluginWindow {
    #[allow(clippy::cast_possible_truncation)]
    fn window_config(
        &self,
        state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        tracing::debug!("NotificationPluginWindow::window_config called");

        let manager = get_manager(state)?;
        tracing::debug!("NotificationPluginWindow: got manager");

        // Cleanup expired notifications first
        manager.cleanup_expired();

        let has_visible = manager.has_visible();
        tracing::debug!("NotificationPluginWindow: has_visible={}", has_visible);

        if !has_visible {
            return None;
        }

        let config = manager.config();
        let notifications = manager.notifications();
        let progress_items = manager.progress_items();

        // Calculate total height needed
        let notif_height = notifications.len();
        let progress_height = progress_items.len() * 2; // Each progress is 2 lines
        let total_height = (notif_height + progress_height).min(10) as u16;

        if total_height == 0 {
            return None;
        }

        // Calculate width (max 50 chars)
        let max_notif_width = notifications
            .iter()
            .map(|n| n.message.len() + n.level.icon().len() + 2)
            .max()
            .unwrap_or(20);
        let max_progress_width = progress_items
            .iter()
            .map(|p| p.title.len() + p.source.len() + 25) // Include progress bar
            .max()
            .unwrap_or(20);
        let popup_width = max_notif_width.max(max_progress_width).clamp(20, 50) as u16;

        // Calculate position based on config
        let (x, y) = calculate_position(
            config.position,
            popup_width,
            total_height,
            ctx.screen_width,
            ctx.screen_height,
        );

        Some(WindowConfig {
            bounds: Rect::new(x, y, popup_width, total_height),
            z_order: 500, // Notifications appear above most things
            visible: true,
        })
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(
        &self,
        state: &Arc<PluginStateRegistry>,
        _ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        bounds: Rect,
        _theme: &Theme,
    ) {
        let Some(manager) = get_manager(state) else {
            return;
        };

        let styles = manager.styles();
        let config = manager.config();
        let notifications = manager.notifications();
        let progress_items = manager.progress_items();

        let mut row = bounds.y;

        // Render notifications
        for notif in notifications.iter().take(bounds.height as usize) {
            if row >= bounds.y + bounds.height {
                break;
            }

            let style = styles.for_level(notif.level);
            let icon = notif.level.icon();
            let mut col = bounds.x;

            // Render icon
            for ch in icon.chars() {
                if col < bounds.x + bounds.width {
                    buffer.put_char(col, row, ch, style);
                    col += 1;
                }
            }

            // Render message
            for ch in notif.message.chars() {
                if col < bounds.x + bounds.width {
                    buffer.put_char(col, row, ch, style);
                    col += 1;
                }
            }

            // Fill rest with spaces
            while col < bounds.x + bounds.width {
                buffer.put_char(col, row, ' ', style);
                col += 1;
            }

            row += 1;
        }

        // Render progress items
        for progress in &progress_items {
            if row >= bounds.y + bounds.height {
                break;
            }

            // Line 1: Title + Source
            let mut col = bounds.x;
            let title_style = &styles.info;

            // Title
            for ch in progress.title.chars() {
                if col < bounds.x + bounds.width {
                    buffer.put_char(col, row, ch, title_style);
                    col += 1;
                }
            }

            // Space
            if col < bounds.x + bounds.width {
                buffer.put_char(col, row, ' ', title_style);
                col += 1;
            }

            // Source (dimmed)
            for ch in format!("[{}]", progress.source).chars() {
                if col < bounds.x + bounds.width {
                    buffer.put_char(col, row, ch, &styles.source);
                    col += 1;
                }
            }

            // Fill rest
            while col < bounds.x + bounds.width {
                buffer.put_char(col, row, ' ', title_style);
                col += 1;
            }

            row += 1;

            // Line 2: Progress bar + Detail
            if row >= bounds.y + bounds.height {
                break;
            }

            col = bounds.x;

            // Render progress bar
            let bar = config.progress_bar.render(progress.progress);
            for ch in bar.chars() {
                if col < bounds.x + bounds.width {
                    let style = if progress.progress.is_some() {
                        &styles.progress_bar
                    } else {
                        &styles.progress_track
                    };
                    buffer.put_char(col, row, ch, style);
                    col += 1;
                }
            }

            // Space + detail
            if let Some(ref detail) = progress.detail {
                if col < bounds.x + bounds.width {
                    buffer.put_char(col, row, ' ', &styles.source);
                    col += 1;
                }
                for ch in detail.chars() {
                    if col < bounds.x + bounds.width {
                        buffer.put_char(col, row, ch, &styles.source);
                        col += 1;
                    }
                }
            }

            // Fill rest
            while col < bounds.x + bounds.width {
                buffer.put_char(col, row, ' ', &styles.source);
                col += 1;
            }

            row += 1;
        }
    }
}

/// Calculate position based on notification position setting
const fn calculate_position(
    position: NotificationPosition,
    width: u16,
    height: u16,
    screen_width: u16,
    screen_height: u16,
) -> (u16, u16) {
    const MARGIN: u16 = 1;

    match position {
        NotificationPosition::TopRight => {
            (screen_width.saturating_sub(width).saturating_sub(MARGIN), MARGIN)
        }
        NotificationPosition::TopLeft => (MARGIN, MARGIN),
        NotificationPosition::BottomRight => (
            screen_width.saturating_sub(width).saturating_sub(MARGIN),
            screen_height
                .saturating_sub(height)
                .saturating_sub(MARGIN + 1), // +1 for statusline
        ),
        NotificationPosition::BottomLeft => (
            MARGIN,
            screen_height
                .saturating_sub(height)
                .saturating_sub(MARGIN + 1),
        ),
        NotificationPosition::TopCenter => ((screen_width.saturating_sub(width)) / 2, MARGIN),
        NotificationPosition::BottomCenter => (
            (screen_width.saturating_sub(width)) / 2,
            screen_height
                .saturating_sub(height)
                .saturating_sub(MARGIN + 1),
        ),
    }
}
