//! Window layout management for split views

use {super::window::Anchor, crate::ui_component::ComponentId};

/// Type of window content
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowType {
    /// Regular file editor window
    #[default]
    Editor,
    /// Plugin window (explorer, telescope, etc.)
    Plugin(ComponentId),
}

impl WindowType {
    /// Check if this is a plugin window
    #[must_use]
    pub const fn is_plugin(&self) -> bool {
        matches!(self, Self::Plugin(_))
    }

    /// Get the plugin's component ID if this is a plugin window
    #[must_use]
    pub const fn plugin_id(&self) -> Option<ComponentId> {
        match self {
            Self::Plugin(id) => Some(*id),
            Self::Editor => None,
        }
    }
}

/// Reserved base for plugin window IDs
const PLUGIN_ID_BASE: usize = usize::MAX - 1000;

/// Generate a stable window ID for a plugin based on its `ComponentId`.
///
/// This uses a reserved range starting from `usize::MAX - 1000` to avoid
/// collision with regular editor window IDs which start from 0.
#[must_use]
pub fn plugin_window_id(id: ComponentId) -> usize {
    // Use a simple hash of the component ID string
    let hash: usize =
        id.0.bytes()
            .fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(usize::from(b)));
    // Map to reserved range (usize::MAX - 1000 .. usize::MAX)
    PLUGIN_ID_BASE.wrapping_add(hash % 1000)
}

/// Check if a window ID is a plugin window ID (in the reserved range)
#[must_use]
pub const fn is_plugin_window_id(window_id: usize) -> bool {
    window_id >= PLUGIN_ID_BASE
}

/// Calculated window dimensions after layout
#[derive(Clone, Copy, Debug)]
pub struct WindowLayout {
    pub anchor: Anchor,
    pub width: u16,
    pub height: u16,
}

/// Manages window layout with optional sidebars
pub struct LayoutManager {
    /// Total screen width
    screen_width: u16,
    /// Total screen height (excluding status line)
    screen_height: u16,
    /// ID of the currently focused window
    active_window_id: usize,
    /// Currently focused plugin window (if any)
    focused_plugin: Option<ComponentId>,
}

impl LayoutManager {
    /// Create a new layout manager
    #[must_use]
    pub const fn new(screen_width: u16, screen_height: u16) -> Self {
        Self {
            screen_width,
            screen_height,
            active_window_id: 0,
            focused_plugin: None,
        }
    }

    /// Update screen dimensions (e.g., on terminal resize)
    pub const fn set_screen_size(&mut self, width: u16, height: u16) {
        self.screen_width = width;
        self.screen_height = height;
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

    // === Generic plugin focus API ===

    /// Focus a plugin window by component ID
    pub const fn focus_plugin(&mut self, id: ComponentId) {
        self.focused_plugin = Some(id);
    }

    /// Unfocus any plugin window, returning focus to editor
    pub const fn unfocus_plugin(&mut self) {
        self.focused_plugin = None;
    }

    /// Check if a specific plugin window is focused
    #[must_use]
    pub fn is_plugin_focused(&self, id: ComponentId) -> bool {
        self.focused_plugin.is_some_and(|focused| focused == id)
    }

    /// Check if any plugin window is focused
    #[must_use]
    pub const fn has_plugin_focus(&self) -> bool {
        self.focused_plugin.is_some()
    }

    /// Get the currently focused plugin component ID
    #[must_use]
    pub const fn focused_plugin(&self) -> Option<ComponentId> {
        self.focused_plugin
    }

    /// Focus the editor window
    pub const fn focus_editor(&mut self) {
        self.focused_plugin = None;
    }

    /// Get the editor area layout (full screen minus status line)
    #[must_use]
    pub const fn editor_layout(&self) -> WindowLayout {
        WindowLayout {
            anchor: Anchor { x: 0, y: 0 },
            width: self.screen_width,
            // Subtract 1 for status line
            height: self.screen_height.saturating_sub(1),
        }
    }
}
