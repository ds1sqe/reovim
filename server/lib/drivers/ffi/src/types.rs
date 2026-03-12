//! FFI type re-exports and documentation.
//!
//! This module re-exports `#[repr(C)]` types from the kernel for use by external
//! modules. These types have stable ABI guarantees.
//!
//! # Memory Layout Guarantees
//!
//! All types in this module are `#[repr(C)]` with documented sizes:
//!
//! | Type | Size (bytes) | Alignment |
//! |------|--------------|-----------|
//! | `Version` | 12 | 4 |
//! | `ModuleProbe` | 1308 | 4 |
//!
//! # Stability
//!
//! Breaking changes to these layouts will bump the ABI version.

use reovim_kernel::api::v1::{ModuleProbe, Version};

/// Verify that `Version` has expected FFI-safe layout.
///
/// This is a compile-time assertion that will fail if the layout changes.
const _: () = {
    assert!(std::mem::size_of::<Version>() == 12);
    assert!(std::mem::align_of::<Version>() == 4);
};

/// Verify that `ModuleProbe` has expected FFI-safe layout.
const _: () = {
    assert!(std::mem::size_of::<ModuleProbe>() == 1308);
    assert!(std::mem::align_of::<ModuleProbe>() == 4);
};

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
