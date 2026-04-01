//! Buffer capability flags — re-exported from kernel.
//!
//! `BufferCapabilities` is now defined in `reovim-kernel::api` as part of
//! the `BufferOps` contract. This module re-exports it for backward
//! compatibility with existing driver/module consumers.

pub use reovim_kernel::api::v1::BufferCapabilities;

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
