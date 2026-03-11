//! Buffer picker module for reovim.
//!
//! Provides a picker to switch between open buffers.
//! Registers `BuffersPicker` in the `PickerRegistry` during module init.

use std::sync::Arc;

use {
    reovim_driver_picker::{
        Picker, PickerAction, PickerContext, PickerData, PickerItem, PickerRegistry, SessionRuntime,
    },
    reovim_driver_session::WindowApi,
    reovim_kernel::api::v1::{
        BufferId, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
};

// ============================================================================
// BuffersPicker
// ============================================================================

/// Picker that lists open buffers.
///
/// Reads `ctx.buffers` and produces items for each buffer.
/// Selecting a buffer switches to it via `PickerAction::SwitchBuffer`.
pub struct BuffersPicker;

impl BuffersPicker {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for BuffersPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for BuffersPicker {
    fn name(&self) -> &'static str {
        "buffers"
    }

    fn title(&self) -> &'static str {
        "Buffers"
    }

    fn items(
        &self,
        ctx: &PickerContext,
        _services: &reovim_kernel::api::v1::ServiceRegistry,
    ) -> Vec<PickerItem> {
        ctx.buffers
            .iter()
            .map(|buf| {
                let icon = if buf.modified { Some('+') } else { None };
                PickerItem {
                    display: buf.name.clone(),
                    detail: Some(format!("#{}", buf.id)),
                    data: PickerData::BufferId(buf.id),
                    icon,
                }
            })
            .collect()
    }

    fn on_select(&self, item: &PickerItem) -> PickerAction {
        match &item.data {
            PickerData::BufferId(id) => PickerAction::SwitchBuffer(*id),
            _ => PickerAction::Close,
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, action: PickerAction, runtime: &mut SessionRuntime<'_>) {
        if let PickerAction::SwitchBuffer(id) = action {
            let buf = BufferId::from_raw(id);
            if let Some(win) = runtime.active_window() {
                let _ = runtime.set_window_buffer(win, buf);
            }
        }
    }
}

// ============================================================================
// Module implementation
// ============================================================================

/// Buffer picker module.
///
/// Registers `BuffersPicker` in `PickerRegistry` during init.
pub struct PickerBuffersModule;

impl PickerBuffersModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PickerBuffersModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PickerBuffersModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("picker-buffers")
    }

    fn name(&self) -> &'static str {
        "Buffer Picker"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<PickerRegistry>();
        registry.register(Arc::new(BuffersPicker));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(PickerBuffersModule);

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use reovim_driver_picker::BufferInfo;

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
        }
    }

    #[test]
    fn name_and_title() {
        let picker = BuffersPicker::new();
        assert_eq!(picker.name(), "buffers");
        assert_eq!(picker.title(), "Buffers");
    }

    #[test]
    fn is_static() {
        let picker = BuffersPicker::new();
        assert!(picker.is_static());
    }

    #[test]
    fn default_prompt() {
        let picker = BuffersPicker::new();
        assert_eq!(picker.prompt(), "> ");
    }

    #[test]
    fn items_empty_context() {
        let picker = BuffersPicker::new();
        let items = picker.items(&empty_ctx(), &services());
        assert!(items.is_empty());
    }

    #[test]
    fn items_from_buffers() {
        let picker = BuffersPicker::new();
        let ctx = PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![
                BufferInfo {
                    id: 1,
                    name: "main.rs".to_owned(),
                    modified: false,
                },
                BufferInfo {
                    id: 2,
                    name: "lib.rs".to_owned(),
                    modified: true,
                },
            ],
            commands: vec![],
        };

        let items = picker.items(&ctx, &services());
        assert_eq!(items.len(), 2);

        assert_eq!(items[0].display, "main.rs");
        assert_eq!(items[0].detail.as_deref(), Some("#1"));
        assert!(items[0].icon.is_none());

        assert_eq!(items[1].display, "lib.rs");
        assert_eq!(items[1].detail.as_deref(), Some("#2"));
        assert_eq!(items[1].icon, Some('+'));
    }

    #[test]
    fn on_select_buffer_id() {
        let picker = BuffersPicker::new();
        let item = PickerItem {
            display: "test.rs".to_owned(),
            detail: None,
            data: PickerData::BufferId(42),
            icon: None,
        };
        let action = picker.on_select(&item);
        assert!(matches!(action, PickerAction::SwitchBuffer(42)));
    }

    #[test]
    fn on_select_wrong_data_closes() {
        let picker = BuffersPicker::new();
        let item = PickerItem {
            display: "test".to_owned(),
            detail: None,
            data: PickerData::Text("wrong".to_owned()),
            icon: None,
        };
        let action = picker.on_select(&item);
        assert!(matches!(action, PickerAction::Close));
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn default_impl() {
        let picker = BuffersPicker::default();
        assert_eq!(picker.name(), "buffers");
    }

    #[test]
    fn no_preview() {
        let picker = BuffersPicker::new();
        let item = PickerItem {
            display: "x".to_owned(),
            detail: None,
            data: PickerData::BufferId(1),
            icon: None,
        };
        assert!(picker.preview(&item, &services()).is_none());
    }

    // -- Module tests --

    #[test]
    fn module_id() {
        let module = PickerBuffersModule::new();
        assert_eq!(module.id().as_str(), "picker-buffers");
    }

    #[test]
    fn module_name() {
        let module = PickerBuffersModule::new();
        assert_eq!(module.name(), "Buffer Picker");
    }

    #[test]
    fn module_version() {
        let module = PickerBuffersModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn module_default() {
        let module = PickerBuffersModule::default();
        assert_eq!(module.id().as_str(), "picker-buffers");
    }

    #[test]
    fn module_exit() {
        let mut module = PickerBuffersModule::new();
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

        let mut module = PickerBuffersModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        let registry = services.get::<PickerRegistry>();
        assert!(registry.is_some());
        let reg = registry.unwrap();
        assert!(reg.get("buffers").is_some());
    }
}
