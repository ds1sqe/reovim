#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod chrome_utils;
pub mod conceal;
pub mod discovery;
pub mod domain;
pub mod ffi;
pub mod handle;
pub mod loader;
#[cfg(feature = "serde")]
pub mod notification;
pub mod scoped_surface;
pub mod services;
// Testing utilities: active during `cargo test` (this crate) or when consumers
// enable the `testing` feature in their dev-dependencies. Downstream crates
// must use `features = ["testing"]` — `cfg(test)` only activates for the
// crate being tested, not its dependencies.
pub mod projection;
pub mod render;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod traits;
pub mod types;
pub mod ui;
pub mod viewport;

pub use {
    conceal::*, loader::*, scoped_surface::ScopedSurface, services::ClientServiceRegistry,
    traits::*, types::*,
};

// Re-export reovim-arch for native modules that need Clock etc.
pub use reovim_arch;
