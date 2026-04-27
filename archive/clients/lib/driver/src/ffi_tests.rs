//! Driver compat tests for the ffi module re-export.
//!
//! The full ffi test suite lives in `reovim-client-subsys-module::ffi_tests`.
//! These tests verify that the driver's re-export path is accessible.

use super::{FfiColor, FfiStyle};

// Verify public FFI types are accessible through the driver re-export.
#[test]
fn ffi_types_accessible_through_driver() {
    // A successful compilation of this test is the coverage target.
    let _ = std::mem::size_of::<FfiColor>();
    let _ = std::mem::size_of::<FfiStyle>();
}
