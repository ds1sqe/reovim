//! Display information types for component registration.
//!
//! Provides types for components to register their visual representation
//! (icons, names, styles) in the display registry.

use std::{any::Any, sync::Arc};

use reovim_core::highlight::Style;

/// Display information for a component or mode.
///
/// Contains the visual representation that will be shown in UI elements
/// like the status line, tab bar, or command line.
#[derive(Debug, Clone, Default)]
pub struct DisplayInfo {
    /// The display string shown in UI (e.g., " EDITOR ", " EXPLORER ")
    pub display_string: &'static str,

    /// Icon character (typically Nerd Font) (e.g., "󰈸 ", "󰙅 ")
    pub icon: &'static str,

    /// Style applied to the display string
    pub style: Style,
}

impl DisplayInfo {
    /// Create a new display info with the given values.
    #[must_use]
    pub const fn new(display_string: &'static str, icon: &'static str, style: Style) -> Self {
        Self {
            display_string,
            icon,
            style,
        }
    }
}

/// Component identifier for display registry lookups.
///
/// Each component (mode, interactor, plugin) gets a unique ID for registration.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct ComponentId(u64);

impl ComponentId {
    /// Create a new component ID.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl From<u64> for ComponentId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// Dynamic display function for runtime-computed display text.
///
/// Allows components to provide context-aware display strings that can
/// change based on state (e.g., file count in explorer, cursor position).
///
/// The function receives an opaque context reference and returns an optional
/// display string. If `None` is returned, the static `display_string` is used.
pub type DynamicDisplayFn = Arc<dyn Fn(&dyn Any) -> Option<String> + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_id_creation() {
        let id = ComponentId::new(42);
        assert_eq!(id.as_u64(), 42);
    }

    #[test]
    fn test_component_id_from_u64() {
        let id: ComponentId = 123_u64.into();
        assert_eq!(id.as_u64(), 123);
    }

    #[test]
    fn test_component_id_equality() {
        let id1 = ComponentId::new(42);
        let id2 = ComponentId::new(42);
        let id3 = ComponentId::new(43);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_display_info_default() {
        let info = DisplayInfo::default();
        assert_eq!(info.display_string, "");
        assert_eq!(info.icon, "");
    }
}
