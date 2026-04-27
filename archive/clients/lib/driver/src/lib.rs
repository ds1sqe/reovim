#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod conceal;
pub mod discovery;
pub mod domain;
pub mod ffi;
pub mod handle;
pub mod loader;
#[cfg(feature = "serde")]
pub mod notification;
pub mod projection;
pub mod render;
pub mod services;
// Testing utilities: active during `cargo test` (this crate) or when consumers
// enable the `testing` feature in their dev-dependencies. Downstream crates
// must use `features = ["testing"]` — `cfg(test)` only activates for the
// crate being tested, not its dependencies.
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod traits;
pub mod types;
pub mod ui;
pub mod viewport;

// TODO(#753): Phase B compat stubs — removed in Phase F when
// `clients/lib/driver/` is decommissioned. The `ClientModule` trait family,
// `ClientServiceRegistry`, `ClientModuleLoader`, and FFI boundary types are now
// canonical in `reovim-client-subsys-module`. These re-exports at the crate
// root allow the 20 non-pilot ext modules to keep compiling without path changes
// until Phase F. Traits, services, ffi, and loader are still declared as
// `pub mod` above because they contain driver-local additions (e.g.,
// `CellGridClientModule`) or re-export subsys items transitively.
pub use reovim_client_subsys_module::{
    ChromeSurface, ClientModule, ClientModuleError, ClientModuleFactory, ClientModuleLoader,
    ClientModuleLoaderError, ClientModuleProbe, ClientModuleRegistry, ClientModuleState,
    ClientServiceRegistry, LayoutPolicy, LocalInputResult, ModuleContext, PlatformCapabilities,
    ProbeResult, ServerHandle, ThemeProvider, TokenProvider, ViewportRenderer,
};

// `conceal::SyntaxToken` is re-exported from `crate::types::SyntaxToken` (subsys).
// `types::*` re-exports all shared types from `reovim-client-subsys-module::types`.
pub use types::*;

// Re-export reovim-arch for native modules that need Clock etc.
pub use reovim_arch;

// TODO(#753): Phase C compat stubs — removed in Phase F when
// `clients/lib/driver/` is decommissioned. `chrome_utils`, `ScopedSurface`,
// `RenderTarget`, `RenderError`, conceal types, and viewport types are now
// canonical in `reovim-client-subsys-chrome` / `reovim-client-subsys-render`.
// These re-exports allow existing consumers to keep compiling.
pub use {
    reovim_client_subsys_chrome::{ScopedSurface, chrome_utils},
    reovim_client_subsys_render::{RenderError, RenderTarget},
};
