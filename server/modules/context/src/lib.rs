#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Scope context provider module for reovim.
//!
//! Provides statusline breadcrumbs and scope hierarchy data by consuming
//! `SyntaxDriver::scopes()`. Language modules register `context.scm` queries;
//! this module reads the resulting scope hierarchy and formats it for display.
//!
//! # Architecture
//!
//! ```text
//! SyntaxDriver::scopes()  -->  ContextModule  -->  BreadcrumbComponent (statusline)
//!                                             -->  ContextBridge (TUI/web extensions)
//! ```

mod breadcrumb;
mod bridge;
mod state;

pub use {
    breadcrumb::BreadcrumbComponent,
    bridge::ContextBridge,
    state::{ContextOptions, ContextSessionState},
};

use std::sync::Arc;

use {
    reovim_driver_session::bridges::BridgeProvider,
    reovim_driver_statusline::{ComponentDataProviderKey, ComponentDataProviderRegistry},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// Scope context provider module.
///
/// Registers a `ContextBridge` for TUI/web extensions and a
/// `BreadcrumbComponent` data provider for the statusline.
pub struct ContextModule;

impl ContextModule {
    /// Create a new context module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ContextModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ContextModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("context")
    }

    fn name(&self) -> &'static str {
        "Context"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register extension bridge
        let bridge_provider = ctx.services.get_or_create::<BridgeProvider>();
        bridge_provider.register(ContextBridge);

        // Register breadcrumb data provider (display-side wraps with Style)
        let component_registry = ctx
            .services
            .get_or_create::<ComponentDataProviderRegistry>();
        component_registry
            .register(ComponentDataProviderKey::new("breadcrumb"), Arc::new(BreadcrumbComponent));

        tracing::info!("ContextModule: registered context bridge and breadcrumb component");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("ContextModule: exiting");
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(ContextModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
