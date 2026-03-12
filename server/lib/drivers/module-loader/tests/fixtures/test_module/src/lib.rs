//! Test dynamic module for integration testing.
//!
//! This module is compiled as a `cdylib` and loaded by the module-loader
//! integration tests to exercise the full FFI load-probe-init-exit cycle.

use reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version};

/// A minimal test module for dynamic loading.
pub struct TestDynamicModule {
    initialized: bool,
}

impl Default for TestDynamicModule {
    fn default() -> Self {
        Self::new()
    }
}

impl TestDynamicModule {
    /// Create new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self { initialized: false }
    }
}

impl Module for TestDynamicModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("test-dynamic")
    }

    fn name(&self) -> &'static str {
        "Test Dynamic Module"
    }

    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        self.initialized = true;
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        self.initialized = false;
        Ok(())
    }
}

reovim_module_macros::declare_module!(TestDynamicModule);
