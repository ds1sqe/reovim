#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Interactive settings panel module for reovim.
//!
//! Provides a floating panel for browsing and modifying editor options.
//! Reads from the kernel `OptionRegistry` and delegates mutations back
//! through `options.set()` for validation and subscriber notifications.

mod bridge;
pub mod state;

use {
    reovim_driver_text_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

use crate::bridge::SettingsBridge;

/// Extension kind string.
const KIND: &str = "settings";
/// Module ID constant.
const MODULE_ID: &str = "settings";

/// Interactive settings panel module.
///
/// Registers [`SettingsBridge`] during `init()` so that panel state
/// is streamed to TUI/web clients via the extension bridge system.
pub struct SettingsModule;

impl SettingsModule {
    /// Create a new settings module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for SettingsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for SettingsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new(MODULE_ID)
    }

    fn name(&self) -> &'static str {
        "Interactive Settings"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(SettingsBridge);

        tracing::info!("SettingsModule: registered settings extension bridge");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(SettingsModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
