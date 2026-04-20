#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Buffer picker module for reovim.
//!
//! Provides a picker to switch between open buffers.
//! Registers `BuffersPicker` in the `PickerRegistry` during module init.

use std::sync::Arc;

use {
    reovim_driver_picker::{
        Picker, PickerAction, PickerContext, PickerData, PickerItem, PickerRegistry, SessionRuntime,
    },
    reovim_driver_text_session::{BufferApi, WindowApi},
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
                let icon = if buf.modified {
                    Some('+')
                } else {
                    reovim_driver_picker::icon_for_path(&buf.name)
                };
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
            runtime.set_active_buffer(Some(buf));
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
#[path = "lib_tests.rs"]
mod tests;
