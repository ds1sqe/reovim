//! Style group registry for module-provided style defaults.
//!
//! This module provides a mechanism for modules to register their own style groups
//! with default styles. The `ThemeManager` uses this registry as a fallback when
//! the current theme doesn't define a requested group.
//!
//! # Architecture
//!
//! This follows mechanism/policy separation:
//! - **Mechanism** (this driver): `StyleGroupRegistry` and registration API
//! - **Policy** (modules): Group names and default styles
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::style::StyleGroupRegistry;
//!
//! // Module defines its groups
//! const RAINBOW_1: &str = "rainbow.bracket.1";
//! let default_style = Style::new().fg(Color::Red);
//!
//! // In module init():
//! let registry = ctx.services.get_or_create::<StyleGroupRegistry>();
//! registry.register(RAINBOW_1, default_style);
//! ```

use std::collections::HashMap;

use {reovim_arch::sync::RwLock, reovim_kernel::api::v1::Service};

use crate::highlight::Style;

/// Registry for module-provided style group defaults.
///
/// Modules register their style groups during `init()`. The `ThemeManager`
/// uses this registry as a fallback when the current theme doesn't
/// define a requested group.
///
/// # Lookup Order (in `ThemeManager`)
///
/// ```text
/// 1. User overrides (set_override)
/// 2. Current theme (ThemeProvider.get_style)
/// 3. Module defaults (StyleGroupRegistry)  <-- this registry
/// 4. ThemeProvider.default_style()
/// ```
pub struct StyleGroupRegistry {
    /// Map from group name to default style.
    groups: RwLock<HashMap<&'static str, Style>>,
}

impl StyleGroupRegistry {
    /// Create a new empty style group registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            groups: RwLock::new(HashMap::new()),
        }
    }

    /// Register a single style group with its default style.
    ///
    /// # Arguments
    ///
    /// * `group` - The group name (e.g., `"rainbow.bracket.1"`)
    /// * `default_style` - The style to use when theme doesn't define this group
    pub fn register(&self, group: &'static str, default_style: Style) {
        self.groups.write().insert(group, default_style);
    }

    /// Register multiple style groups at once.
    ///
    /// Convenience method for modules that define many groups.
    pub fn register_batch(&self, registrations: &[(&'static str, Style)]) {
        let mut groups = self.groups.write();
        for (group, style) in registrations {
            groups.insert(*group, style.clone());
        }
    }

    /// Get the default style for a group.
    ///
    /// Returns `None` if the group is not registered.
    #[must_use]
    pub fn get(&self, group: &str) -> Option<Style> {
        self.groups.read().get(group).cloned()
    }

    /// List all registered groups.
    #[must_use]
    pub fn registered_groups(&self) -> Vec<&'static str> {
        self.groups.read().keys().copied().collect()
    }

    /// Check if a group is registered.
    #[must_use]
    pub fn contains(&self, group: &str) -> bool {
        self.groups.read().contains_key(group)
    }

    /// Get the number of registered groups.
    #[must_use]
    pub fn len(&self) -> usize {
        self.groups.read().len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.groups.read().is_empty()
    }
}

impl Default for StyleGroupRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for StyleGroupRegistry {}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
