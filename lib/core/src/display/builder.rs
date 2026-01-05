//! Builder pattern for registering display information
//!
//! Provides a fluent API for registering display info across multiple scopes.

use crate::{display::DisplayInfo, modd::ComponentId, plugin::PluginContext};

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
    pub const fn default(
        mut self,
        name: &'static str,
        icon: &'static str,
        style: crate::highlight::Style,
    ) -> Self {
        self.default_info = Some(DisplayInfo::new(name, icon, style));
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
