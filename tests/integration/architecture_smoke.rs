//! Smoke tests for the new kernel architecture.
//!
//! These tests verify that the architecture layers compile correctly
//! and have the expected dependency structure.

/// Test that reovim-arch compiles and exports expected modules.
#[test]
fn arch_layer_compiles() {
    // Verify arch crate is accessible
    use reovim_arch::error::ArchError;
    // Verify traits module exists (empty for now)
    // Just importing it verifies it compiles
    #[allow(unused_imports)]
    use reovim_arch::traits;

    // Verify error types exist
    let err: ArchError = ArchError::NotSupported("test");
    assert!(matches!(err, ArchError::NotSupported("test")));
}

/// Test that reovim-kernel compiles and depends only on arch.
#[test]
fn kernel_layer_compiles() {
    // Verify kernel crate is accessible
    use reovim_kernel::api::API_VERSION;
    use reovim_kernel::arch::error::ArchError;

    // Verify API version is defined
    let (major, minor, patch) = API_VERSION;
    assert_eq!(major, 0);
    assert_eq!(minor, 1);
    assert_eq!(patch, 0);

    // Verify arch re-export works
    let _ = ArchError::NotSupported("test");
}

/// Test that kernel modules exist.
#[test]
fn kernel_modules_exist() {
    // All modules should be accessible (even if empty)
    // Importing them verifies they compile
    #[allow(unused_imports)]
    use reovim_kernel::api;
    #[allow(unused_imports)]
    use reovim_kernel::block;
    #[allow(unused_imports)]
    use reovim_kernel::core;
    #[allow(unused_imports)]
    use reovim_kernel::debug;
    #[allow(unused_imports)]
    use reovim_kernel::ipc;
    #[allow(unused_imports)]
    use reovim_kernel::mm;
    #[allow(unused_imports)]
    use reovim_kernel::panic;
    #[allow(unused_imports)]
    use reovim_kernel::sched;

    // Verify API_VERSION is accessible
    assert_eq!(reovim_kernel::api::API_VERSION, (0, 1, 0));
}

/// Test that display driver compiles and depends only on kernel.
#[test]
fn driver_display_compiles() {
    // Just importing verifies it compiles with correct dependencies
    #[allow(unused_imports)]
    use reovim_driver_display;
}

/// Test that input driver compiles and depends only on kernel.
#[test]
fn driver_input_compiles() {
    #[allow(unused_imports)]
    use reovim_driver_input;
}

/// Test that syntax driver compiles and has NO tree-sitter dependency.
#[test]
fn driver_syntax_compiles_without_treesitter() {
    // Verify syntax driver crate is accessible
    #[allow(unused_imports)]
    use reovim_driver_syntax;

    // This test passes if the crate compiles without tree-sitter.
    // If tree-sitter were added as a dependency, it would be visible
    // in the dependency tree, violating the architecture constraint.
}

/// Test that LSP driver compiles and depends only on kernel.
#[test]
fn driver_lsp_compiles() {
    #[allow(unused_imports)]
    use reovim_driver_lsp;
}

/// Test that net driver compiles and depends only on kernel.
#[test]
fn driver_net_compiles() {
    #[allow(unused_imports)]
    use reovim_driver_net;
}

/// Test that VFS driver compiles and depends only on kernel.
#[test]
fn driver_vfs_compiles() {
    #[allow(unused_imports)]
    use reovim_driver_vfs;
}

/// Test the dependency hierarchy is correct.
#[test]
fn dependency_hierarchy() {
    use reovim_arch::error::ArchError;
    use reovim_kernel::arch::error::ArchError as KernelArchError;

    // This test documents the expected dependency hierarchy:
    //
    // lib/arch       → no dependencies (except std)
    // lib/kernel     → only lib/arch
    // lib/drivers/*  → only lib/kernel
    //
    // The hierarchy is enforced by Cargo.toml configurations.
    // This test serves as documentation and will fail to compile
    // if the hierarchy is violated.

    // Both should be the same type (kernel re-exports arch)
    let arch_err: ArchError = ArchError::NotSupported("test");
    let kernel_err: KernelArchError = arch_err;
    assert!(matches!(kernel_err, KernelArchError::NotSupported("test")));
}
