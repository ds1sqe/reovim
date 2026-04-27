//! Server layout mirror for TUI.
//!
//! This module provides a passive data structure that mirrors window layout
//! state from server `layout_changed` notifications. Unlike implementing
//! `RootCompositor` (20+ methods), this is a simple storage layer.
//!
//! # Design Philosophy (Phase 11.2)
//!
//! - **Server owns layout**: Window lifecycle managed by server
//! - **Client mirrors state**: TUI passively stores placements
//! - **Mechanism/Policy separation**: Server decides, client renders
//!
//! # Usage
//!
//! ```ignore
//! let mut mirror = ServerLayoutMirror::new(80, 24);
//!
//! // On layout_changed notification:
//! mirror.apply_layout_changed(focused_id, &windows);
//!
//! // During render:
//! for placement in mirror.placements() {
//!     render_window(placement);
//! }
//! ```

use reovim_protocol::v3::WindowInfo;

/// Mirror of server-side window layout.
///
/// Passive data structure that stores placements from `layout_changed`
/// notifications. Does NOT manage window lifecycle - that's the server's job.
#[derive(Debug, Default)]
pub struct ServerLayoutMirror {
    /// Window placements from server.
    windows: Vec<WindowPlacement>,
    /// Focused window ID.
    focused_id: Option<u64>,
    /// Screen width for bounds checking.
    screen_width: u16,
    /// Screen height for bounds checking.
    screen_height: u16,
}

/// Client-side window placement (derived from server notification).
#[derive(Debug, Clone)]
pub struct WindowPlacement {
    /// Window ID.
    pub window_id: u64,
    /// Buffer ID displayed in this window (None if window has no buffer).
    /// Phase #479: Changed to Option to eliminate ID ambiguity.
    pub buffer_id: Option<u64>,
    /// X position (column).
    pub x: u16,
    /// Y position (row).
    pub y: u16,
    /// Width in columns.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
    /// Whether this window is focused.
    pub focused: bool,
    /// Layer opacity (0.0 = transparent, 1.0 = opaque).
    /// #400: Deserialized from `optional float` with absent = 1.0 (fully opaque).
    pub opacity: f32,
}

impl ServerLayoutMirror {
    /// Create a new mirror with screen dimensions.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            windows: Vec::new(),
            focused_id: None,
            screen_width: width,
            screen_height: height,
        }
    }

    /// Apply a `layout_changed` notification.
    ///
    /// Replaces all stored placements with the new layout from server.
    /// Note: Server sends x=0, y=0 for all windows, so we calculate positions
    /// based on actual screen size. For single window, use full screen minus statusline.
    pub fn apply_layout_changed(&mut self, focused_id: u64, windows: &[WindowInfo]) {
        self.focused_id = Some(focused_id);

        // Reserve 1 row for statusline
        let content_height = self.screen_height.saturating_sub(1);

        // For now, simple single-window layout: full width, height minus statusline
        // TODO: Handle multi-window splits properly
        self.windows = windows
            .iter()
            .enumerate()
            .map(|(idx, w)| {
                // For single window, use full screen
                // For multiple windows, we'd need to calculate splits
                let (x, y, width, height) = if windows.len() == 1 {
                    (0, 0, self.screen_width, content_height)
                } else {
                    // Fallback: use server rect if available, or stack windows
                    w.rect.as_ref().map_or_else(
                        || {
                            // Stack windows vertically as fallback
                            #[allow(clippy::cast_possible_truncation)]
                            let h = content_height / windows.len() as u16;
                            #[allow(clippy::cast_possible_truncation)]
                            (0, (idx as u16) * h, self.screen_width, h)
                        },
                        |r| {
                            #[allow(clippy::cast_possible_truncation)]
                            (r.x as u16, r.y as u16, r.width as u16, r.height as u16)
                        },
                    )
                };

                WindowPlacement {
                    window_id: w.window_id,
                    buffer_id: w.buffer_id,
                    x,
                    y,
                    width,
                    height,
                    focused: w.focused,
                    // #400: absent = fully opaque (proto3 optional float)
                    opacity: w.opacity.unwrap_or(1.0),
                }
            })
            .collect();
    }

    /// Update screen dimensions (on terminal resize).
    pub const fn set_screen(&mut self, width: u16, height: u16) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Get all placements for rendering.
    #[must_use]
    pub fn placements(&self) -> &[WindowPlacement] {
        &self.windows
    }

    /// Get focused window ID.
    #[must_use]
    pub const fn focused_id(&self) -> Option<u64> {
        self.focused_id
    }

    /// Get the focused placement.
    #[must_use]
    pub fn focused_placement(&self) -> Option<&WindowPlacement> {
        let fid = self.focused_id?;
        self.windows.iter().find(|p| p.window_id == fid)
    }

    /// Get placement by window ID.
    #[must_use]
    pub fn get_placement(&self, window_id: u64) -> Option<&WindowPlacement> {
        self.windows.iter().find(|p| p.window_id == window_id)
    }

    /// Get window count.
    #[must_use]
    pub const fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Check if there are multiple windows (for separator drawing).
    #[must_use]
    pub const fn has_multiple_windows(&self) -> bool {
        self.windows.len() > 1
    }

    /// Get screen dimensions.
    #[must_use]
    pub const fn screen_size(&self) -> (u16, u16) {
        (self.screen_width, self.screen_height)
    }
}

#[cfg(test)]
#[path = "layout_mirror_tests.rs"]
mod tests;
