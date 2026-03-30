#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Option picker module for reovim.
//!
//! Provides a picker to browse and inspect editor options.
//! Registers `OptionsPicker` in the `PickerRegistry` during module init.
//! The preview panel shows detailed option metadata (type, value,
//! default, constraint, scope, owner).

use std::sync::Arc;

use {
    reovim_driver_picker::{
        Picker, PickerAction, PickerContext, PickerData, PickerItem, PickerRegistry, PreviewContent,
    },
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

// ============================================================================
// OptionsPicker
// ============================================================================

/// Picker that lists registered editor options.
///
/// Reads `ctx.options` and produces items for each option.
/// The preview panel shows full metadata: type, value, default,
/// constraint, scope, and owner module.
pub struct OptionsPicker;

impl OptionsPicker {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for OptionsPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for OptionsPicker {
    fn name(&self) -> &'static str {
        "options"
    }

    fn title(&self) -> &'static str {
        "Options"
    }

    fn items(
        &self,
        ctx: &PickerContext,
        _services: &reovim_kernel::api::v1::ServiceRegistry,
    ) -> Vec<PickerItem> {
        ctx.options
            .iter()
            .map(|opt| {
                let display = opt
                    .owner
                    .as_ref()
                    .map_or_else(|| opt.name.clone(), |owner| format!("[{owner}] {}", opt.name));
                let detail = format!("{} = {}", opt.type_name, opt.current_value);
                // Encode full metadata into PickerData::Text for preview().
                let data = build_preview_data(opt);
                PickerItem {
                    display,
                    detail: Some(detail),
                    data: PickerData::Text(data),
                    icon: None,
                }
            })
            .collect()
    }

    fn on_select(&self, _item: &PickerItem) -> PickerAction {
        PickerAction::Close
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn preview(
        &self,
        item: &PickerItem,
        _services: &reovim_kernel::api::v1::ServiceRegistry,
    ) -> Option<PreviewContent> {
        let PickerData::Text(ref data) = item.data else {
            return None;
        };

        let lines: Vec<String> = data.lines().map(String::from).collect();
        if lines.is_empty() {
            return None;
        }

        Some(PreviewContent {
            lines,
            highlight_line: Some(0),
            file_path: None,
            ..Default::default()
        })
    }
}

// ============================================================================
// Preview data builder
// ============================================================================

/// Build a multi-line preview string from option metadata.
///
/// Each line is a key-value pair showing the option's full specification.
/// This string is stored in `PickerData::Text` and split back into lines
/// by `preview()` for display.
fn build_preview_data(opt: &reovim_driver_picker::OptionInfo) -> String {
    let mut lines = Vec::with_capacity(10);

    lines.push(format!("  {}", opt.name));
    if let Some(ref short) = opt.short_form {
        lines.push(format!("  alias: {short}"));
    }
    lines.push(String::new());
    lines.push(format!("  {}", opt.description));
    lines.push(String::new());
    lines.push(format!("  Type:     {}", opt.type_name));
    lines.push(format!("  Value:    {}", opt.current_value));
    lines.push(format!("  Default:  {}", opt.default_value));
    lines.push(format!("  Scope:    {}", opt.scope));

    if let Some(ref constraint) = opt.constraint {
        lines.push(format!("  Range:    {constraint}"));
    }

    if let Some(ref owner) = opt.owner {
        lines.push(format!("  Module:   {owner}"));
    }

    if let Some(ref choices) = opt.choices {
        lines.push(format!("  Choices:  {}", choices.join(", ")));
    }

    lines.join("\n")
}

// ============================================================================
// Module implementation
// ============================================================================

/// Option picker module.
///
/// Registers `OptionsPicker` in `PickerRegistry` during init.
pub struct PickerOptionsModule;

impl PickerOptionsModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PickerOptionsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PickerOptionsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("picker-options")
    }

    fn name(&self) -> &'static str {
        "Option Picker"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<PickerRegistry>();
        registry.register(Arc::new(OptionsPicker));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(PickerOptionsModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
