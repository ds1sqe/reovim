//! Driver compat tests for the loader module re-export.
//!
//! The full loader test suite lives in `reovim-client-subsys-module::loader_tests`.
//! These tests verify that the driver's re-export path is accessible.

use crate::loader::{ClientModuleLoader, ClientModuleLoaderError, ClientModuleState};

#[test]
fn loader_types_accessible_through_driver() {
    // Verify the re-export surface is present. Compilation = coverage.
    let _ = std::mem::size_of::<ClientModuleLoader>();
    let _ = std::mem::size_of::<ClientModuleLoaderError>();
    let _ = std::mem::size_of::<ClientModuleState>();
}
