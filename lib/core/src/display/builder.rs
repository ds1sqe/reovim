//! Builder pattern for registering display information
//!
//! Provides a fluent API for registering display info across multiple scopes.

use std::sync::Arc;

use crate::{
    display::{DisplayContext, DisplayInfo},
    modd::ComponentId,
    plugin::PluginContext,
};

/// Builder for registering display information with a fluent API
///
/// # Examples
///
/// ```ignore
/// // Register display info with custom color
/// use reovim_core::highlight::{Style, Color};
/// let style = Style::new().fg(fg_color).bg(bg_color).bold();
/// ctx.display_info(COMPONENT_ID)
///     .default(" EXPLORER ", "󰙅 ", style)
///     .register();
///
/// // Register with dynamic display callback
/// ctx.display_info(COMPONENT_ID)
///     .default(" EXPLORER ", "󰙅 ", style)
///     .dynamic(|ctx| {
///         ctx.plugin_state.with::<ExplorerState, _, _>(|state| {
///             format!(" EXPLORER ({}) ", state.file_count)
///         }).unwrap_or_else(|| " EXPLORER ".to_string())
///     })
///     .register();
/// ```
pub struct DisplayInfoBuilder<'a> {
    ctx: &'a mut PluginContext,
    component_id: ComponentId,
    default_info: Option<DisplayInfo>,
}

impl<'a> DisplayInfoBuilder<'a> {
    /// Create a new builder for the given component
    #[must_use]
    pub const fn new(ctx: &'a mut PluginContext, component_id: ComponentId) -> Self {
        Self {
            ctx,
            component_id,
            default_info: None,
        }
    }

    /// Set the default display info for this component
    ///
    /// This is shown when the component is focused.
    /// The MODE section will separately show the edit mode (Normal/Insert/Visual).
    #[must_use]
    pub fn default(
        mut self,
        name: &'static str,
        icon: &'static str,
        style: crate::highlight::Style,
    ) -> Self {
        self.default_info = Some(DisplayInfo::new(name, icon, style));
        self
    }

    /// Add a dynamic display callback
    ///
    /// When set, this callback is invoked during rendering to generate the display string.
    /// The callback takes precedence over the static `display_string` set via `.default()`.
    ///
    /// Note: This method must be called after `.default()` since it modifies the existing
    /// display info.
    #[must_use]
    pub fn dynamic<F>(mut self, f: F) -> Self
    where
        F: Fn(&DisplayContext<'_>) -> String + Send + Sync + 'static,
    {
        if let Some(ref mut info) = self.default_info {
            info.dynamic_display = Some(Arc::new(f));
        }
        self
    }

    /// Register the display information
    ///
    /// Consumes the builder and registers the display info with the plugin context.
    pub fn register(self) {
        // Register default display for component
        if let Some(info) = self.default_info {
            self.ctx.register_display(self.component_id, info);
        }
    }
}
