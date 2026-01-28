//! Statusline module for reovim.
//!
//! Provides a lualine-inspired extensible statusline with:
//! - Sections: A, B, C (left) | X, Y, Z (right)
//! - Pluggable components (mode, filename, position, etc.)
//! - Mode-specific coloring
//! - Powerline-style separators
//!
//! # Architecture
//!
//! Following the mechanism/policy separation:
//! - **Mechanism**: `StatuslineProvider` trait (in `reovim-driver-display`)
//! - **Policy**: `DefaultStatuslineProvider` (this module) provides implementation
//!
//! # Section Layout
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │ A │ B │ C                              │ X │ Y │ Z │
//! │mode│git│filename                       │enc│ft │pos│
//! └────────────────────────────────────────────────────────────┘
//! ```
//!
//! - **A**: Mode indicator (NORMAL, INSERT, VISUAL, etc.)
//! - **B**: Git branch (future)
//! - **C**: Filename with modified indicator
//! - **X**: Encoding (future)
//! - **Y**: Filetype
//! - **Z**: Cursor position (line:col)

pub mod components;
mod config;
mod provider;
pub mod theme;

pub use {
    config::{ComponentCondition, ComponentConfig, ComponentId, StatuslineConfig},
    provider::DefaultStatuslineProvider,
    theme::{ModeColors, SectionColors, StatuslineTheme},
};

use std::sync::Arc;

use {
    reovim_driver_display::{StatuslineProviderKey, StatuslineProviderRegistry},
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version, pr_info,
    },
};

/// Statusline module instance.
///
/// Registers the default statusline provider with built-in components.
pub struct StatuslineModule {
    config: StatuslineConfig,
}

impl StatuslineModule {
    /// Create a new statusline module with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: StatuslineConfig::default(),
        }
    }

    /// Create a new statusline module with custom configuration.
    #[must_use]
    pub const fn with_config(config: StatuslineConfig) -> Self {
        Self { config }
    }
}

impl Default for StatuslineModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for StatuslineModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("statusline")
    }

    fn name(&self) -> &'static str {
        "Statusline"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register statusline provider
        let statusline_registry = ctx.services.get_or_create::<StatuslineProviderRegistry>();

        // Create provider with built-in components
        let provider = DefaultStatuslineProvider::new(self.config.clone());
        statusline_registry.register(StatuslineProviderKey::Main, Arc::new(provider));

        pr_info!("Statusline module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Statusline module exiting");
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(StatuslineModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = StatuslineModule::new();
        assert_eq!(module.id().as_str(), "statusline");
    }

    #[test]
    fn test_module_name() {
        let module = StatuslineModule::new();
        assert_eq!(module.name(), "Statusline");
    }

    #[test]
    fn test_module_version() {
        let module = StatuslineModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }
}
