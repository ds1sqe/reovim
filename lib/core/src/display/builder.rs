//! Builder pattern for registering display information
//!
//! Provides a fluent API for registering display info across multiple scopes.

use crate::{
    display::{DisplayInfo, EditModeKey, SubModeKey},
    modd::{EditMode, SubMode},
    plugin::PluginContext,
    ui_component::ComponentId,
};

/// Builder for registering display information with a fluent API
///
/// # Examples
///
/// ```ignore
/// // Register default and mode-specific displays
/// ctx.display_info(COMPONENT_ID)
///     .default(" EXPLORER ", "󰙅 ")
///     .when_mode(EditMode::Insert(InsertVariant::Standard), " EXPLORER | INSERT ", "󰙅 ")
///     .register();
/// ```
pub struct DisplayInfoBuilder<'a> {
    ctx: &'a mut PluginContext,
    component_id: ComponentId,
    default_info: Option<DisplayInfo>,
    mode_info: Vec<(EditModeKey, DisplayInfo)>,
    sub_mode_info: Vec<(SubModeKey, DisplayInfo)>,
}

impl<'a> DisplayInfoBuilder<'a> {
    /// Create a new builder for the given component
    #[must_use]
    pub const fn new(ctx: &'a mut PluginContext, component_id: ComponentId) -> Self {
        Self {
            ctx,
            component_id,
            default_info: None,
            mode_info: Vec::new(),
            sub_mode_info: Vec::new(),
        }
    }

    /// Set the default display info for this component
    ///
    /// This is shown when the component is focused but no specific mode display is registered.
    #[must_use]
    pub const fn default(mut self, name: &'static str, icon: &'static str) -> Self {
        self.default_info = Some(DisplayInfo::new(name, icon));
        self
    }

    /// Add display info for a specific edit mode
    ///
    /// For example, showing " EXPLORER | INSERT " when in insert mode within Explorer.
    #[must_use]
    pub fn when_mode(mut self, mode: &EditMode, name: &'static str, icon: &'static str) -> Self {
        self.mode_info
            .push((EditModeKey::from(mode), DisplayInfo::new(name, icon)));
        self
    }

    /// Add display info for a sub-mode
    ///
    /// Sub-modes take priority over component/edit mode displays.
    #[must_use]
    pub fn when_sub_mode(
        mut self,
        sub_mode: &SubMode,
        name: &'static str,
        icon: &'static str,
    ) -> Self {
        self.sub_mode_info
            .push((SubModeKey::from(sub_mode), DisplayInfo::new(name, icon)));
        self
    }

    /// Register all configured display information
    ///
    /// Consumes the builder and registers all display info with the plugin context.
    pub fn register(self) {
        // Register default display for component
        if let Some(info) = self.default_info {
            self.ctx.register_display(self.component_id, info);
        }

        // Register component + mode combinations
        for (mode_key, info) in self.mode_info {
            self.ctx
                .register_component_mode_display(self.component_id, mode_key, info);
        }

        // Register sub-modes
        for (sub_mode_key, info) in self.sub_mode_info {
            self.ctx.register_sub_mode_display(sub_mode_key, info);
        }
    }
}
