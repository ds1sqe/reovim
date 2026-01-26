//! Window Layout Module
//!
//! This module provides window tiling and focus navigation for reovim.
//!
//! # Architecture
//!
//! Following the kernel's "mechanism vs policy" principle:
//! - **Mechanism** (display driver): `RootCompositor`, `WindowLayerCompositor`, `TiledLayer` traits
//! - **Policy** (this module): `HybridCompositor`, `DefaultLayer`, `TilingLayout` implementations
//!
//! # Components
//!
//! - [`HybridCompositor`]: Multi-layer compositor implementing `RootCompositor`
//! - [`DefaultLayer`]: Single-layer compositor implementing `WindowLayerCompositor`
//! - [`TilingLayout`]: Window arrangement using a split tree, implements `TiledLayer`
//! - [`SplitTree`]: Binary tree for managing window splits
//!
//! # Commands
//!
//! This module provides window management commands (see [`commands`] module):
//! - Split: horizontal, vertical
//! - Close: current window, other windows
//! - Navigate: left, right, up, down
//! - Cycle: next, previous
//! - Resize: increase/decrease height/width, equalize

pub mod commands;
mod compositor;
mod floatzone;
mod focus;
pub mod ids;
mod layer;
mod split;
mod tiling;

pub use {
    commands::all_commands,
    compositor::HybridCompositor,
    floatzone::FloatZone,
    focus::VimFocusPolicy,
    layer::DefaultLayer,
    split::{SplitNode, SplitTree},
    tiling::TilingLayout,
};

use std::sync::Arc;

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_driver_display::layout::{CompositorKey, CompositorRegistry},
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version, pr_info,
    },
};

/// Window layout module.
///
/// Manages window tiling, splits, and focus navigation.
/// Provides compositor and commands for window management.
pub struct LayoutModule;

impl LayoutModule {
    /// Create a new layout module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LayoutModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for LayoutModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("layout")
    }

    fn name(&self) -> &'static str {
        "Window Layout"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register compositor with typed key (Epic #417)
        let compositor_registry = ctx.services.get_or_create::<CompositorRegistry>();
        compositor_registry
            .register(CompositorKey::Root, Arc::new(HybridCompositor::with_main_layer()));

        // Epic #438: Self-register window commands
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::all_commands() {
            command_store.add(handler);
        }

        pr_info!("Layout module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Layout module exiting");
        Ok(())
    }
}

impl CommandProvider for LayoutModule {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        commands::all_commands()
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(LayoutModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = LayoutModule::new();
        assert_eq!(module.id().as_str(), "layout");
    }

    #[test]
    fn test_module_name() {
        let module = LayoutModule::new();
        assert_eq!(module.name(), "Window Layout");
    }

    #[test]
    fn test_module_version() {
        let module = LayoutModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }
}
