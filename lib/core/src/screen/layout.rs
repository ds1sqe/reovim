//! Window layout management for split views

use super::window::Anchor;

/// Type of window content
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowType {
    /// Regular file editor window
    #[default]
    Editor,
    /// File explorer sidebar
    Explorer,
}

/// Calculated window dimensions after layout
#[derive(Clone, Copy, Debug)]
pub struct WindowLayout {
    pub anchor: Anchor,
    pub width: u16,
    pub height: u16,
}

/// Manages window layout with optional explorer sidebar
pub struct LayoutManager {
    /// Total screen width
    screen_width: u16,
    /// Total screen height (excluding status line)
    screen_height: u16,
    /// Explorer sidebar width (when visible)
    explorer_width: u16,
    /// Whether explorer is currently visible
    explorer_visible: bool,
    /// ID of the currently focused window
    active_window_id: usize,
}

impl LayoutManager {
    /// Default explorer sidebar width
    pub const DEFAULT_EXPLORER_WIDTH: u16 = 30;
    /// Minimum explorer width
    pub const MIN_EXPLORER_WIDTH: u16 = 20;
    /// Maximum explorer width (fraction of screen)
    pub const MAX_EXPLORER_WIDTH_RATIO: f32 = 0.5;

    /// Create a new layout manager
    #[must_use]
    pub const fn new(screen_width: u16, screen_height: u16) -> Self {
        Self {
            screen_width,
            screen_height,
            explorer_width: Self::DEFAULT_EXPLORER_WIDTH,
            explorer_visible: false,
            active_window_id: 0,
        }
    }

    /// Update screen dimensions (e.g., on terminal resize)
    pub fn set_screen_size(&mut self, width: u16, height: u16) {
        self.screen_width = width;
        self.screen_height = height;
        // Clamp explorer width if screen got smaller
        let max_width = self.max_explorer_width();
        if self.explorer_width > max_width {
            self.explorer_width = max_width;
        }
    }

    /// Toggle explorer visibility
    pub const fn toggle_explorer(&mut self) {
        self.explorer_visible = !self.explorer_visible;
    }

    /// Show explorer
    pub const fn show_explorer(&mut self) {
        self.explorer_visible = true;
    }

    /// Hide explorer
    pub const fn hide_explorer(&mut self) {
        self.explorer_visible = false;
    }

    /// Check if explorer is visible
    #[must_use]
    pub const fn is_explorer_visible(&self) -> bool {
        self.explorer_visible
    }

    /// Get explorer width
    #[must_use]
    pub const fn explorer_width(&self) -> u16 {
        self.explorer_width
    }

    /// Get explorer height (same as screen height when visible)
    #[must_use]
    pub const fn explorer_height(&self) -> u16 {
        self.screen_height
    }

    /// Set explorer width with bounds checking
    pub fn set_explorer_width(&mut self, width: u16) {
        let max = self.max_explorer_width();
        self.explorer_width = width.clamp(Self::MIN_EXPLORER_WIDTH, max);
    }

    /// Increase explorer width by delta
    pub fn grow_explorer(&mut self, delta: u16) {
        self.set_explorer_width(self.explorer_width.saturating_add(delta));
    }

    /// Decrease explorer width by delta
    pub fn shrink_explorer(&mut self, delta: u16) {
        self.set_explorer_width(self.explorer_width.saturating_sub(delta));
    }

    /// Maximum allowed explorer width based on screen size
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    fn max_explorer_width(&self) -> u16 {
        (f32::from(self.screen_width) * Self::MAX_EXPLORER_WIDTH_RATIO) as u16
    }

    /// Get the active window ID
    #[must_use]
    pub const fn active_window_id(&self) -> usize {
        self.active_window_id
    }

    /// Set the active window ID
    pub const fn set_active_window(&mut self, window_id: usize) {
        self.active_window_id = window_id;
    }

    /// Focus the explorer window (window ID 0 when visible)
    pub const fn focus_explorer(&mut self) {
        if self.explorer_visible {
            self.active_window_id = 0;
        }
    }

    /// Focus the editor window
    pub const fn focus_editor(&mut self) {
        // Editor is window 1 when explorer is visible, 0 otherwise
        self.active_window_id = if self.explorer_visible { 1 } else { 0 };
    }

    /// Check if explorer is currently focused
    #[must_use]
    pub const fn is_explorer_focused(&self) -> bool {
        self.explorer_visible && self.active_window_id == 0
    }

    /// Calculate layout for explorer window (if visible)
    #[must_use]
    pub const fn explorer_layout(&self) -> Option<WindowLayout> {
        if !self.explorer_visible {
            return None;
        }

        Some(WindowLayout {
            anchor: Anchor { x: 0, y: 0 },
            width: self.explorer_width,
            height: self.screen_height,
        })
    }

    /// Calculate layout for main editor window
    #[must_use]
    pub const fn editor_layout(&self) -> WindowLayout {
        if self.explorer_visible {
            // Editor is to the right of explorer
            WindowLayout {
                anchor: Anchor {
                    x: self.explorer_width,
                    y: 0,
                },
                width: self.screen_width.saturating_sub(self.explorer_width),
                height: self.screen_height,
            }
        } else {
            // Editor takes full width
            WindowLayout {
                anchor: Anchor { x: 0, y: 0 },
                width: self.screen_width,
                height: self.screen_height,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_without_explorer() {
        let layout = LayoutManager::new(100, 50);
        assert!(!layout.is_explorer_visible());

        let editor = layout.editor_layout();
        assert_eq!(editor.anchor.x, 0);
        assert_eq!(editor.width, 100);
        assert_eq!(editor.height, 50);

        assert!(layout.explorer_layout().is_none());
    }

    #[test]
    fn test_layout_with_explorer() {
        let mut layout = LayoutManager::new(100, 50);
        layout.toggle_explorer();
        assert!(layout.is_explorer_visible());

        let explorer = layout.explorer_layout().unwrap();
        assert_eq!(explorer.anchor.x, 0);
        assert_eq!(explorer.width, LayoutManager::DEFAULT_EXPLORER_WIDTH);
        assert_eq!(explorer.height, 50);

        let editor = layout.editor_layout();
        assert_eq!(editor.anchor.x, LayoutManager::DEFAULT_EXPLORER_WIDTH);
        assert_eq!(editor.width, 100 - LayoutManager::DEFAULT_EXPLORER_WIDTH);
    }

    #[test]
    fn test_explorer_width_bounds() {
        let mut layout = LayoutManager::new(100, 50);

        layout.set_explorer_width(10); // Below minimum
        assert_eq!(layout.explorer_width(), LayoutManager::MIN_EXPLORER_WIDTH);

        layout.set_explorer_width(80); // Above maximum (50% of 100)
        assert_eq!(layout.explorer_width(), 50);
    }

    #[test]
    fn test_focus_management() {
        let mut layout = LayoutManager::new(100, 50);

        // Without explorer, active window is editor (0)
        assert_eq!(layout.active_window_id(), 0);
        assert!(!layout.is_explorer_focused());

        // With explorer, focus explorer
        layout.toggle_explorer();
        layout.focus_explorer();
        assert_eq!(layout.active_window_id(), 0);
        assert!(layout.is_explorer_focused());

        // Focus editor
        layout.focus_editor();
        assert_eq!(layout.active_window_id(), 1);
        assert!(!layout.is_explorer_focused());
    }
}
