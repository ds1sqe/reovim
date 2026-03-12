//! Fluent builder for display info registration.
//!
//! Provides a builder pattern for registering component display information
//! with the display registry.

use std::{any::Any, sync::Arc};

use crate::highlight::Style;

use super::{
    registry::DisplayRegistry,
    types::{ComponentId, DisplayInfo, DynamicDisplayFn},
};

/// Fluent builder for registering display information.
///
/// # Example
///
/// ```ignore
/// display_registry.builder(ComponentId::new(1))
///     .default(" EXPLORER ", "󰙅 ", explorer_style)
///     .dynamic(|ctx| {
///         // Compute display based on context
///         Some(format!(" EXPLORER ({}) ", file_count))
///     })
///     .register();
/// ```
pub struct DisplayInfoBuilder<'a> {
    registry: &'a mut DisplayRegistry,
    component_id: ComponentId,
    info: Option<DisplayInfo>,
    dynamic_fn: Option<DynamicDisplayFn>,
}

impl<'a> DisplayInfoBuilder<'a> {
    /// Create a new builder for the given component ID.
    pub(crate) fn new(registry: &'a mut DisplayRegistry, component_id: ComponentId) -> Self {
        Self {
            registry,
            component_id,
            info: None,
            dynamic_fn: None,
        }
    }

    /// Set the default display information.
    ///
    /// This is the static display info used when no dynamic function is set
    /// or when the dynamic function returns `None`.
    #[must_use]
    pub const fn default(
        mut self,
        display_string: &'static str,
        icon: &'static str,
        style: Style,
    ) -> Self {
        self.info = Some(DisplayInfo::new(display_string, icon, style));
        self
    }

    /// Set a dynamic display function.
    ///
    /// The function receives context and can return a custom display string.
    /// If it returns `None`, the default `display_string` is used.
    ///
    /// # Note
    ///
    /// This method must be called after `default()` to ensure fallback behavior.
    #[must_use]
    pub fn dynamic<F>(mut self, f: F) -> Self
    where
        F: Fn(&dyn Any) -> Option<String> + Send + Sync + 'static,
    {
        self.dynamic_fn = Some(Arc::new(f));
        self
    }

    /// Register the display info with the registry.
    ///
    /// Consumes the builder and adds the entry to the registry.
    ///
    /// # Panics
    ///
    /// Panics if `default()` was not called before `register()`.
    pub fn register(self) {
        let info = self
            .info
            .expect("DisplayInfoBuilder::default() must be called before register()");

        self.registry
            .register_entry(self.component_id, info, self.dynamic_fn);
    }
}

#[cfg(test)]
#[path = "info_builder_tests.rs"]
mod tests;
