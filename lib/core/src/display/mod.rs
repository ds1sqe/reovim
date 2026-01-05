//! Display information registry for plugin-provided mode display strings and icons
//!
//! This module provides a registry where plugins can register custom display strings
//! and icons for different modes and components. This allows plugins to define their
//! own visual representations without modifying core code.

mod builder;

pub use builder::DisplayInfoBuilder;

use std::collections::HashMap;

use crate::modd::{ComponentId, ModeState};

/// Display information for a mode/component
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    /// Display string shown in status line (e.g., " NORMAL ", " INSERT ")
    pub display_string: &'static str,
    /// Icon for compact display (e.g., "󰆾 ", "󰙅 ")
    pub icon: &'static str,
    /// Style for this component's status line appearance
    pub style: crate::highlight::Style,
}

impl DisplayInfo {
    /// Create new display info with display string, icon, and style
    #[must_use]
    pub const fn new(
        display_string: &'static str,
        icon: &'static str,
        style: crate::highlight::Style,
    ) -> Self {
        Self {
            display_string,
            icon,
            style,
        }
    }
}

// Removed EditModeKey and SubModeKey enums - no longer needed
// since status line shows [INTERACTOR][MODE] as separate sections

/// Registry for plugin-provided display information
///
/// This registry allows plugins to register custom display information
/// for their components (e.g., Explorer shows " EXPLORER " with orange color).
///
/// The status line displays: [INTERACTOR][MODE] as two separate sections,
/// so each component only needs one display registration.
#[derive(Debug, Default)]
pub struct DisplayRegistry {
    /// Display info for each component (`ComponentId` -> `DisplayInfo`)
    interactors: HashMap<ComponentId, DisplayInfo>,
}

impl DisplayRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register display info for a component
    ///
    /// This is shown in the INTERACTOR section of the status line.
    /// The MODE section will separately show the edit mode (Normal/Insert/Visual).
    pub fn register_interactor(&mut self, id: ComponentId, info: DisplayInfo) {
        self.interactors.insert(id, info);
    }

    /// Get display info for a component
    ///
    /// Simply looks up the component in the interactors map.
    #[must_use]
    pub fn get_display(&self, mode: &ModeState) -> Option<&DisplayInfo> {
        self.interactors.get(&mode.interactor_id)
    }

    /// Get display string for a mode, with fallback to hierarchical display
    #[must_use]
    pub fn display_string(&self, mode: &ModeState) -> String {
        self.get_display(mode)
            .map_or_else(|| mode.hierarchical_display(), |info| info.display_string.to_string())
    }

    /// Get icon for a mode, with fallback
    #[must_use]
    pub fn icon(&self, mode: &ModeState) -> &'static str {
        self.get_display(mode).map_or(" ", |info| info.icon)
    }

    /// Register all built-in display info
    ///
    /// This registers the default display info for the core editor component.
    /// Plugins should register their own display info via `PluginContext`.
    ///
    /// Note: Editor modes (Normal/Insert/Visual) are shown in the MODE section
    /// of the status line, not in the interactor section, so they don't need
    /// to be registered here.
    pub fn register_builtins(&mut self, theme: &crate::highlight::Theme) {
        // Editor component
        // Mode (Normal/Insert/Visual) will be shown separately in MODE section
        self.register_interactor(
            ComponentId::EDITOR,
            DisplayInfo::new(" EDITOR ", "󰈸 ", theme.statusline.interactor.clone()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_info_new() {
        let style = crate::highlight::Style::new();
        let info = DisplayInfo::new(" TEST ", "󰆾 ", style);
        assert_eq!(info.display_string, " TEST ");
        assert_eq!(info.icon, "󰆾 ");
    }

    #[test]
    fn test_registry_lookup() {
        let mut registry = DisplayRegistry::new();
        let style = crate::highlight::Style::new();

        // Register a component
        registry.register_interactor(
            ComponentId::EDITOR,
            DisplayInfo::new(" EDITOR ", "󰈸 ", style),
        );

        // Check that we can retrieve it
        let mode = ModeState::normal();
        let display = registry.get_display(&mode);
        assert!(display.is_some());
        assert_eq!(display.unwrap().display_string, " EDITOR ");
        assert_eq!(display.unwrap().icon, "󰈸 ");
    }

    #[test]
    fn test_builtins_registration() {
        let mut registry = DisplayRegistry::new();
        let theme = crate::highlight::Theme::default();
        registry.register_builtins(&theme);

        // Check that editor component is registered
        let normal = ModeState::normal();
        let display = registry.get_display(&normal);
        assert!(display.is_some());
        assert_eq!(display.unwrap().display_string, " EDITOR ");
        assert_eq!(display.unwrap().icon, "󰈸 ");

        // Display string should use plugin-provided text
        assert_eq!(registry.display_string(&normal).as_str(), " EDITOR ");
        assert_eq!(registry.icon(&normal), "󰈸 ");
    }

    #[test]
    fn test_fallback_for_unregistered() {
        let registry = DisplayRegistry::new();

        // Unregistered component should return None
        let mode = ModeState::normal();
        assert!(registry.get_display(&mode).is_none());

        // display_string should fall back to hierarchical display
        let display_str = registry.display_string(&mode);
        // hierarchical_display() format is "component" or "component.submode"
        assert!(!display_str.is_empty());
        assert!(display_str.to_lowercase().contains("editor") || display_str.contains("editor"));

        // icon should return default
        assert_eq!(registry.icon(&mode), " ");
    }
}
