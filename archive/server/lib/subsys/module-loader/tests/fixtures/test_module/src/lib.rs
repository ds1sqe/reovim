//! Test dynamic module for integration testing.
//!
//! This module is compiled as a `cdylib` and loaded by the module-loader
//! integration tests to exercise the full FFI load-probe-init-exit cycle.

#![allow(unsafe_code)] // #[no_mangle] observation hooks for integration tests

use std::sync::atomic::{AtomicBool, Ordering};

use reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version};

/// Observable flag set by `Module::on_all_loaded` — used by the #725 P1-T2
/// integration test to verify the `reovim_module_on_all_loaded` FFI
/// trampoline actually fires.
static ON_ALL_LOADED_CALLED: AtomicBool = AtomicBool::new(false);

/// Exported symbol read by the integration test via `libloading::Symbol`.
/// Returns `true` after `Module::on_all_loaded` has been called at least
/// once on any instance.
#[unsafe(no_mangle)]
pub extern "C" fn test_module_on_all_loaded_flag() -> bool {
    ON_ALL_LOADED_CALLED.load(Ordering::Relaxed)
}

/// Reset the on-all-loaded flag (integration test helper).
///
/// Without this, flag state leaks across parallel tests in the same
/// process (both tests and the trampoline see the same static).
#[unsafe(no_mangle)]
pub extern "C" fn test_module_reset_on_all_loaded_flag() {
    ON_ALL_LOADED_CALLED.store(false, Ordering::Relaxed);
}

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

    fn on_all_loaded(&mut self, _ctx: &ModuleContext) {
        ON_ALL_LOADED_CALLED.store(true, Ordering::Relaxed);
    }
}

reovim_module_macros::declare_module!(TestDynamicModule);
