//! Central registry for component display information.
//!
//! The display registry stores visual representation data for components,
//! enabling consistent display across UI elements.

use std::{any::Any, collections::HashMap};

use super::{
    icons::fallback,
    info_builder::DisplayInfoBuilder,
    types::{ComponentId, DisplayInfo, DynamicDisplayFn},
};

/// Entry in the display registry.
struct DisplayEntry {
    /// Static display information
    pub info: DisplayInfo,
    /// Optional dynamic display function
    pub dynamic_fn: Option<DynamicDisplayFn>,
}

/// Central registry for component display information.
///
/// # Ownership
///
/// The runner creates and owns this registry. Modules register their display
/// information during initialization via the builder pattern.
///
/// # Example
///
/// ```ignore
/// // In runner initialization:
/// let mut display_registry = DisplayRegistry::new();
///
/// // Module registers during load:
/// display_registry.builder(ComponentId::new(module.id()))
///     .default(" EXPLORER ", "󰙅 ", explorer_style)
///     .dynamic(|ctx| {
///         // Compute display based on context
///         ctx.downcast_ref::<ExplorerState>()
///             .map(|state| format!(" EXPLORER ({}) ", state.file_count))
///     })
///     .register();
///
/// // Render uses registry:
/// let display = display_registry.display_string(component_id, Some(&state));
/// render_statusline(buffer, display, info.style);
/// ```
#[derive(Default)]
pub struct DisplayRegistry {
    entries: HashMap<ComponentId, DisplayEntry>,
}

impl DisplayRegistry {
    /// Create a new empty display registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Create a builder for registering display info for a component.
    pub fn builder(&mut self, id: ComponentId) -> DisplayInfoBuilder<'_> {
        DisplayInfoBuilder::new(self, id)
    }

    /// Register an entry directly (used by builder).
    pub(crate) fn register_entry(
        &mut self,
        id: ComponentId,
        info: DisplayInfo,
        dynamic_fn: Option<DynamicDisplayFn>,
    ) {
        self.entries.insert(id, DisplayEntry { info, dynamic_fn });
    }

    /// Get the static display info for a component.
    ///
    /// Returns `None` if the component is not registered.
    #[must_use]
    pub fn get(&self, id: ComponentId) -> Option<&DisplayInfo> {
        self.entries.get(&id).map(|entry| &entry.info)
    }

    /// Get the static display string for a component.
    ///
    /// Returns the static `display_string` from the registered `DisplayInfo`.
    /// For dynamic content that can change at runtime, use `display_string_owned`.
    ///
    /// Returns an empty string if the component is not registered.
    #[must_use]
    pub fn display_string(&self, id: ComponentId) -> &str {
        self.entries
            .get(&id)
            .map_or("", |entry| entry.info.display_string)
    }

    /// Get the display string for a component, returning owned string for dynamic content.
    ///
    /// This method handles dynamic display functions that compute strings at runtime.
    #[must_use]
    pub fn display_string_owned(&self, id: ComponentId, ctx: Option<&dyn Any>) -> String {
        if let Some(entry) = self.entries.get(&id) {
            // Try dynamic function first
            if let (Some(dynamic_fn), Some(ctx)) = (&entry.dynamic_fn, ctx)
                && let Some(dynamic_str) = dynamic_fn(ctx)
            {
                return dynamic_str;
            }
            // Fall back to static string
            entry.info.display_string.to_string()
        } else {
            String::new()
        }
    }

    /// Get the icon for a component.
    ///
    /// Returns the fallback icon if the component is not registered.
    #[must_use]
    pub fn icon(&self, id: ComponentId) -> &str {
        self.entries
            .get(&id)
            .map_or(fallback::NONE, |entry| entry.info.icon)
    }

    /// Check if a component is registered.
    #[must_use]
    pub fn contains(&self, id: ComponentId) -> bool {
        self.entries.contains_key(&id)
    }

    /// Get the number of registered components.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove a component from the registry.
    ///
    /// Returns the removed display info if the component was registered.
    pub fn remove(&mut self, id: ComponentId) -> Option<DisplayInfo> {
        self.entries.remove(&id).map(|entry| entry.info)
    }

    /// Clear all entries from the registry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
