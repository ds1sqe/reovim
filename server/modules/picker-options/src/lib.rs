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
mod tests {
    use std::path::PathBuf;

    use reovim_driver_picker::OptionInfo;

    use super::*;

    fn services() -> reovim_kernel::api::v1::ServiceRegistry {
        reovim_kernel::api::v1::ServiceRegistry::new()
    }

    fn empty_ctx() -> PickerContext {
        PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![],
            commands: vec![],
            options: vec![],
        }
    }

    #[test]
    fn name_and_title() {
        let picker = OptionsPicker::new();
        assert_eq!(picker.name(), "options");
        assert_eq!(picker.title(), "Options");
    }

    #[test]
    fn is_static() {
        let picker = OptionsPicker::new();
        assert!(picker.is_static());
    }

    #[test]
    fn default_prompt() {
        let picker = OptionsPicker::new();
        assert_eq!(picker.prompt(), "> ");
    }

    #[test]
    fn items_empty_context() {
        let picker = OptionsPicker::new();
        let items = picker.items(&empty_ctx(), &services());
        assert!(items.is_empty());
    }

    #[test]
    fn items_with_options() {
        let picker = OptionsPicker::new();
        let ctx = PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![],
            commands: vec![],
            options: vec![
                OptionInfo {
                    name: "number".to_owned(),
                    short_form: Some("nu".to_owned()),
                    description: "Show line numbers".to_owned(),
                    type_name: "bool".to_owned(),
                    current_value: "false".to_owned(),
                    default_value: "false".to_owned(),
                    constraint: None,
                    scope: "window".to_owned(),
                    owner: Some("options".to_owned()),
                    choices: None,
                },
                OptionInfo {
                    name: "picker_height".to_owned(),
                    short_form: None,
                    description: "Maximum height of the picker window".to_owned(),
                    type_name: "integer".to_owned(),
                    current_value: "15".to_owned(),
                    default_value: "15".to_owned(),
                    constraint: Some("3..50".to_owned()),
                    scope: "global".to_owned(),
                    owner: Some("microscope".to_owned()),
                    choices: None,
                },
            ],
        };
        let items = picker.items(&ctx, &services());
        assert_eq!(items.len(), 2);

        // Option with owner shows "[module] name" format.
        assert_eq!(items[0].display, "[options] number");
        assert_eq!(items[0].detail.as_deref(), Some("bool = false"));

        assert_eq!(items[1].display, "[microscope] picker_height");
        assert_eq!(items[1].detail.as_deref(), Some("integer = 15"));
    }

    #[test]
    fn items_without_owner() {
        let picker = OptionsPicker::new();
        let ctx = PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![],
            commands: vec![],
            options: vec![OptionInfo {
                name: "orphan".to_owned(),
                short_form: None,
                description: "No owner".to_owned(),
                type_name: "string".to_owned(),
                current_value: "value".to_owned(),
                default_value: "default".to_owned(),
                constraint: None,
                scope: "global".to_owned(),
                owner: None,
                choices: None,
            }],
        };
        let items = picker.items(&ctx, &services());
        assert_eq!(items[0].display, "orphan");
    }

    #[test]
    fn on_select_closes() {
        let picker = OptionsPicker::new();
        let item = PickerItem {
            display: "number".to_owned(),
            detail: None,
            data: PickerData::Text("number".to_owned()),
            icon: None,
        };
        assert!(matches!(picker.on_select(&item), PickerAction::Close));
    }

    #[test]
    fn preview_text_data() {
        let picker = OptionsPicker::new();
        let data = build_preview_data(&OptionInfo {
            name: "number".to_owned(),
            short_form: Some("nu".to_owned()),
            description: "Show line numbers".to_owned(),
            type_name: "bool".to_owned(),
            current_value: "false".to_owned(),
            default_value: "false".to_owned(),
            constraint: None,
            scope: "window".to_owned(),
            owner: Some("options".to_owned()),
            choices: None,
        });
        let item = PickerItem {
            display: "[options] number".to_owned(),
            detail: Some("bool = false".to_owned()),
            data: PickerData::Text(data),
            icon: None,
        };
        let preview = picker.preview(&item, &services());
        assert!(preview.is_some());
        let preview = preview.unwrap();
        assert!(preview.lines[0].contains("number"));
        assert!(preview.lines.iter().any(|l| l.contains("Type:")));
        assert!(preview.lines.iter().any(|l| l.contains("alias: nu")));
        assert!(preview.lines.iter().any(|l| l.contains("Module:")));
        assert_eq!(preview.highlight_line, Some(0));
    }

    #[test]
    fn preview_wrong_data() {
        let picker = OptionsPicker::new();
        let item = PickerItem {
            display: "x".to_owned(),
            detail: None,
            data: PickerData::BufferId(1),
            icon: None,
        };
        assert!(picker.preview(&item, &services()).is_none());
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn default_impl() {
        let picker = OptionsPicker::default();
        assert_eq!(picker.name(), "options");
    }

    #[test]
    fn build_preview_data_full() {
        let info = OptionInfo {
            name: "picker_border".to_owned(),
            short_form: None,
            description: "Border style for picker window".to_owned(),
            type_name: "choice".to_owned(),
            current_value: "rounded".to_owned(),
            default_value: "rounded".to_owned(),
            constraint: None,
            scope: "global".to_owned(),
            owner: Some("microscope".to_owned()),
            choices: Some(vec![
                "none".to_owned(),
                "single".to_owned(),
                "double".to_owned(),
                "rounded".to_owned(),
            ]),
        };
        let data = build_preview_data(&info);
        assert!(data.contains("picker_border"));
        assert!(data.contains("Type:"));
        assert!(data.contains("choice"));
        assert!(data.contains("Choices:"));
        assert!(data.contains("none, single, double, rounded"));
        assert!(data.contains("Module:"));
        assert!(data.contains("microscope"));
    }

    #[test]
    fn build_preview_data_minimal() {
        let info = OptionInfo {
            name: "simple".to_owned(),
            short_form: None,
            description: "A simple option".to_owned(),
            type_name: "bool".to_owned(),
            current_value: "true".to_owned(),
            default_value: "true".to_owned(),
            constraint: None,
            scope: "global".to_owned(),
            owner: None,
            choices: None,
        };
        let data = build_preview_data(&info);
        assert!(data.contains("simple"));
        assert!(!data.contains("alias:"));
        assert!(!data.contains("Range:"));
        assert!(!data.contains("Module:"));
        assert!(!data.contains("Choices:"));
    }

    #[test]
    fn build_preview_data_with_constraint() {
        let info = OptionInfo {
            name: "height".to_owned(),
            short_form: Some("h".to_owned()),
            description: "Height setting".to_owned(),
            type_name: "integer".to_owned(),
            current_value: "15".to_owned(),
            default_value: "15".to_owned(),
            constraint: Some("3..50".to_owned()),
            scope: "global".to_owned(),
            owner: None,
            choices: None,
        };
        let data = build_preview_data(&info);
        assert!(data.contains("alias: h"));
        assert!(data.contains("Range:    3..50"));
    }

    // -- Module tests --

    #[test]
    fn module_id() {
        let module = PickerOptionsModule::new();
        assert_eq!(module.id().as_str(), "picker-options");
    }

    #[test]
    fn module_name() {
        let module = PickerOptionsModule::new();
        assert_eq!(module.name(), "Option Picker");
    }

    #[test]
    fn module_version() {
        let module = PickerOptionsModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn module_default() {
        let module = PickerOptionsModule::default();
        assert_eq!(module.id().as_str(), "picker-options");
    }

    #[test]
    fn module_exit() {
        let mut module = PickerOptionsModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn module_init_registers_picker() {
        let services = Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
        let ctx = ModuleContext::new(
            reovim_kernel::api::v1::KernelContext::default(),
            services.clone(),
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp"),
        );

        let mut module = PickerOptionsModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        let registry = services.get::<PickerRegistry>();
        assert!(registry.is_some());
        let reg = registry.unwrap();
        assert!(reg.get("options").is_some());
    }
}
