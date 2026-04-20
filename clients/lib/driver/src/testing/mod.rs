//! Testing utilities for `ClientModule` implementations.
//!
//! Provides mock types and assertion helpers so module authors don't need to
//! reimplement surface/capabilities/server mocks in every test file.
//!
//! Available when `cfg(test)` (this crate's own tests) or when consumers
//! enable the `testing` feature in their dev-dependencies.

mod capabilities;
mod context;
mod registry;
mod server;
mod surface;
mod theme;

pub use {
    capabilities::MockPlatformCapabilities,
    context::{TestModuleContext, TestModuleContextBuilder},
    registry::MockModuleRegistry,
    server::MockServerHandle,
    surface::{RecordingSurface, WriteSurface},
    theme::MockThemeProvider,
};

#[cfg(test)]
mod tests;
