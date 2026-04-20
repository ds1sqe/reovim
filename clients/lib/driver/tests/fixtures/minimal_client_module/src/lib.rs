//! Minimal client module fixture for flight #724 integration tests.
//!
//! Built as a `cdylib` so the T1 (FFI symbol resolution), T3 (discovery
//! filters), and T5 (`load_from_path` error paths) tests have a real `.so` to
//! load via `ClientModuleHandle::load_from_path`.
//!
//! This fixture intentionally exercises ONE of each optional dependency slot
//! so tests can assert that the leaked-slice accessors on the handle return
//! the expected content.
//!
//! # Locating the built library
//!
//! ```sh
//! cargo build -p reovim-minimal-client-module
//! # Produces target/debug/libreovim_minimal_client_module.so (Linux)
//! ```
//!
//! The tests use the same `test_module_path` helper pattern that
//! `ext/server/drivers/module-loader/tests/integration.rs` uses — walking up
//! from `CARGO_MANIFEST_DIR` to find the workspace root.

use reovim_client_driver::{ClientModule, ClientModuleError, ModuleContext, ProbeResult, Version};

/// Minimal client module: one required dep, one optional dep, no chrome,
/// no buffer contribution, no annotations.
pub struct MinimalClientModule {
    initialized: bool,
}

impl Default for MinimalClientModule {
    fn default() -> Self {
        Self::new()
    }
}

impl MinimalClientModule {
    #[must_use]
    pub const fn new() -> Self {
        Self { initialized: false }
    }
}

impl ClientModule for MinimalClientModule {
    fn id(&self) -> &'static str {
        "minimal-client"
    }

    fn kind(&self) -> &'static str {
        "minimal-client"
    }

    fn name(&self) -> &'static str {
        "Minimal Client Module"
    }

    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }

    fn dependencies(&self) -> &[&str] {
        &["required-dep"]
    }

    fn optional_dependencies(&self) -> &[&str] {
        &["optional-dep"]
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        self.initialized = true;
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        self.initialized = false;
        Ok(())
    }
}

reovim_module_macros::declare_client_module!(MinimalClientModule);
