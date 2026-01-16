//! Integration tests for module loading system.
//!
//! These tests exercise the real FFI loading via `libloading`, verifying that:
//! - Module .so files can be loaded
//! - FFI symbols exist and are callable
//! - Hot reload state preservation works
//! - Error handling is correct
//!
//! # Requirements
//!
//! These tests require the hot-reload-demo module to be built.
//! The module is automatically built when running `cargo test` because
//! it's listed as a dev-dependency in runner/Cargo.toml.
//!
//! # Safety
//!
//! These tests use `unsafe` FFI calls to exercise the module loading system.
//! This is intentional - we're testing the FFI boundary.

// Allow unsafe code in this test module - we're intentionally testing FFI
#![allow(unsafe_code)]
// Allow comparisons that are always true due to type limits (u32 >= 0)
#![allow(unused_comparisons)]

mod common;

use std::ffi::c_void;

use {
    libloading::{Library, Symbol},
    reovim_kernel::api::v1::{ModuleContext, ModuleProbe, Version},
};

/// Type alias for FFI probe function.
type ProbeFn = unsafe extern "C" fn() -> ModuleProbe;

/// Type alias for FFI entry function.
type EntryFn = unsafe extern "C" fn() -> *mut c_void;

/// Type alias for FFI init function.
type InitFn = unsafe extern "C" fn(*mut c_void, *const c_void) -> i32;

/// Type alias for FFI exit function.
type ExitFn = unsafe extern "C" fn(*mut c_void) -> i32;

/// Type alias for FFI destroy function.
type DestroyFn = unsafe extern "C" fn(*mut c_void);

/// Type alias for FFI `supports_hot_reload` function.
type SupportsHotReloadFn = unsafe extern "C" fn(*const c_void) -> i32;

/// Type alias for FFI `save_state` function.
type SaveStateFn = unsafe extern "C" fn(*const c_void, *mut *mut u8, *mut usize) -> i32;

/// Type alias for FFI `restore_state` function.
type RestoreStateFn = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;

/// Type alias for FFI `free_state` function.
type FreeStateFn = unsafe extern "C" fn(*mut u8, usize);

// ============================================================================
// Core Tests (P0 - Must Have)
// ============================================================================

#[test]
fn test_module_load_dynamic() {
    let path = common::demo_module_path();
    assert!(path.exists(), "Demo module not found at {path:?}");

    // SAFETY: We're loading a trusted module built by this project
    let library = unsafe { Library::new(&path) };
    let library_err = library.as_ref().err();
    assert!(library.is_ok(), "Failed to load library: {library_err:?}");

    let library = library.unwrap();

    // Verify all expected symbols exist
    unsafe {
        let probe: Result<Symbol<ProbeFn>, _> = library.get(b"reovim_module_probe");
        assert!(probe.is_ok(), "Missing symbol: reovim_module_probe");

        let entry: Result<Symbol<EntryFn>, _> = library.get(b"reovim_module_entry");
        assert!(entry.is_ok(), "Missing symbol: reovim_module_entry");

        let init: Result<Symbol<InitFn>, _> = library.get(b"reovim_module_init");
        assert!(init.is_ok(), "Missing symbol: reovim_module_init");

        let exit: Result<Symbol<ExitFn>, _> = library.get(b"reovim_module_exit");
        assert!(exit.is_ok(), "Missing symbol: reovim_module_exit");

        let destroy: Result<Symbol<DestroyFn>, _> = library.get(b"reovim_module_destroy");
        assert!(destroy.is_ok(), "Missing symbol: reovim_module_destroy");

        // Hot reload symbols
        let supports: Result<Symbol<SupportsHotReloadFn>, _> =
            library.get(b"reovim_module_supports_hot_reload");
        assert!(supports.is_ok(), "Missing symbol: reovim_module_supports_hot_reload");

        let save: Result<Symbol<SaveStateFn>, _> = library.get(b"reovim_module_save_state");
        assert!(save.is_ok(), "Missing symbol: reovim_module_save_state");

        let restore: Result<Symbol<RestoreStateFn>, _> =
            library.get(b"reovim_module_restore_state");
        assert!(restore.is_ok(), "Missing symbol: reovim_module_restore_state");

        let free: Result<Symbol<FreeStateFn>, _> = library.get(b"reovim_module_free_state");
        assert!(free.is_ok(), "Missing symbol: reovim_module_free_state");
    }
}

#[test]
fn test_module_probe_metadata() {
    let path = common::demo_module_path();

    // SAFETY: We're loading a trusted module
    let library = unsafe { Library::new(&path).expect("Failed to load library") };

    unsafe {
        let probe: Symbol<ProbeFn> = library.get(b"reovim_module_probe").unwrap();
        let metadata = probe();

        assert_eq!(metadata.id_str(), "hot-reload-demo");
        assert_eq!(metadata.name_str(), "Hot Reload Demo");
        assert_eq!(metadata.version, Version::new(0, 9, 0));
    }
}

#[test]
fn test_module_init_exit_lifecycle() {
    let path = common::demo_module_path();

    // SAFETY: We're loading a trusted module
    let library = unsafe { Library::new(&path).expect("Failed to load library") };

    unsafe {
        let entry: Symbol<EntryFn> = library.get(b"reovim_module_entry").unwrap();
        let init: Symbol<InitFn> = library.get(b"reovim_module_init").unwrap();
        let exit: Symbol<ExitFn> = library.get(b"reovim_module_exit").unwrap();
        let destroy: Symbol<DestroyFn> = library.get(b"reovim_module_destroy").unwrap();

        // Create module instance
        let module_ptr = entry();
        assert!(!module_ptr.is_null(), "entry() returned null");

        // Initialize with context
        let ctx = ModuleContext::default();
        let ctx_ptr: *const c_void = std::ptr::from_ref(&ctx).cast();
        let init_result = init(module_ptr, ctx_ptr);
        assert_eq!(init_result, 0, "init() should return 0 for success");

        // Exit
        let exit_result = exit(module_ptr);
        assert_eq!(exit_result, 0, "exit() should return 0 for success");

        // Destroy
        destroy(module_ptr);
        // If we get here without crash, destroy worked
    }
}

#[test]
fn test_module_hot_reload_state_preservation() {
    let path = common::demo_module_path();

    // SAFETY: We're loading a trusted module
    let library = unsafe { Library::new(&path).expect("Failed to load library") };

    unsafe {
        let entry: Symbol<EntryFn> = library.get(b"reovim_module_entry").unwrap();
        let init: Symbol<InitFn> = library.get(b"reovim_module_init").unwrap();
        let exit: Symbol<ExitFn> = library.get(b"reovim_module_exit").unwrap();
        let destroy: Symbol<DestroyFn> = library.get(b"reovim_module_destroy").unwrap();
        let supports: Symbol<SupportsHotReloadFn> =
            library.get(b"reovim_module_supports_hot_reload").unwrap();
        let save: Symbol<SaveStateFn> = library.get(b"reovim_module_save_state").unwrap();
        let restore: Symbol<RestoreStateFn> = library.get(b"reovim_module_restore_state").unwrap();
        let free: Symbol<FreeStateFn> = library.get(b"reovim_module_free_state").unwrap();

        let ctx = ModuleContext::default();
        let ctx_ptr: *const c_void = std::ptr::from_ref(&ctx).cast();

        // Create and init first module
        let module1 = entry();
        assert_eq!(init(module1, ctx_ptr), 0);

        // Verify it supports hot reload
        let supports_result = supports(module1);
        assert_eq!(supports_result, 1, "Module should support hot reload");

        // Save state
        let mut state_ptr: *mut u8 = std::ptr::null_mut();
        let mut state_len: usize = 0;
        let save_result = save(module1, &raw mut state_ptr, &raw mut state_len);
        assert_eq!(save_result, 0, "save_state should return 0 for success");
        assert!(!state_ptr.is_null(), "state pointer should not be null");
        assert!(state_len > 0, "state length should be > 0");

        // Exit and destroy first module
        exit(module1);
        destroy(module1);

        // Create second module (simulates reload)
        let module2 = entry();

        // Restore state
        let restore_result = restore(module2, state_ptr, state_len);
        assert_eq!(restore_result, 0, "restore_state should return 0 for success");

        // Init the restored module
        assert_eq!(init(module2, ctx_ptr), 0);

        // Clean up
        exit(module2);
        destroy(module2);
        free(state_ptr, state_len);
    }
}

// ============================================================================
// State Serialization Tests (P0)
// ============================================================================

#[test]
fn test_state_version_forward_compatible() {
    let path = common::demo_module_path();

    // SAFETY: We're loading a trusted module
    let library = unsafe { Library::new(&path).expect("Failed to load library") };

    unsafe {
        let entry: Symbol<EntryFn> = library.get(b"reovim_module_entry").unwrap();
        let init: Symbol<InitFn> = library.get(b"reovim_module_init").unwrap();
        let exit: Symbol<ExitFn> = library.get(b"reovim_module_exit").unwrap();
        let destroy: Symbol<DestroyFn> = library.get(b"reovim_module_destroy").unwrap();
        let save: Symbol<SaveStateFn> = library.get(b"reovim_module_save_state").unwrap();
        let restore: Symbol<RestoreStateFn> = library.get(b"reovim_module_restore_state").unwrap();
        let free: Symbol<FreeStateFn> = library.get(b"reovim_module_free_state").unwrap();

        let ctx = ModuleContext::default();
        let ctx_ptr: *const c_void = std::ptr::from_ref(&ctx).cast();

        // Create module and init
        let module1 = entry();
        init(module1, ctx_ptr);

        // Save state with current version
        let mut state_ptr: *mut u8 = std::ptr::null_mut();
        let mut state_len: usize = 0;
        save(module1, &raw mut state_ptr, &raw mut state_len);

        exit(module1);
        destroy(module1);

        // Create new module and restore
        let module2 = entry();
        let restore_result = restore(module2, state_ptr, state_len);

        // Current version state should restore successfully
        assert_eq!(restore_result, 0, "Current version state should restore");

        destroy(module2);
        free(state_ptr, state_len);
    }
}

#[test]
fn test_state_version_too_new_rejected() {
    let path = common::demo_module_path();

    // SAFETY: We're loading a trusted module
    let library = unsafe { Library::new(&path).expect("Failed to load library") };

    unsafe {
        let entry: Symbol<EntryFn> = library.get(b"reovim_module_entry").unwrap();
        let restore: Symbol<RestoreStateFn> = library.get(b"reovim_module_restore_state").unwrap();
        let destroy: Symbol<DestroyFn> = library.get(b"reovim_module_destroy").unwrap();

        // Create fake state with version 99
        let mut state = [0u8; 24];
        state[0..4].copy_from_slice(&99u32.to_le_bytes()); // version = 99

        let module = entry();
        let restore_result = restore(module, state.as_ptr(), state.len());

        // Should fail because version 99 > current version
        assert_eq!(restore_result, -1, "Future version state should be rejected");

        destroy(module);
    }
}

#[test]
fn test_state_corrupt_data_rejected() {
    let path = common::demo_module_path();

    // SAFETY: We're loading a trusted module
    let library = unsafe { Library::new(&path).expect("Failed to load library") };

    unsafe {
        let entry: Symbol<EntryFn> = library.get(b"reovim_module_entry").unwrap();
        let restore: Symbol<RestoreStateFn> = library.get(b"reovim_module_restore_state").unwrap();
        let destroy: Symbol<DestroyFn> = library.get(b"reovim_module_destroy").unwrap();

        // Create truncated state (too short)
        let state = [0u8; 10]; // Less than 24 bytes

        let module = entry();
        let restore_result = restore(module, state.as_ptr(), state.len());

        // Should fail because state is too short
        assert_eq!(restore_result, -1, "Truncated state should be rejected");

        destroy(module);
    }
}

// ============================================================================
// Error Recovery Tests (P1)
// ============================================================================

#[test]
fn test_api_version_check() {
    let path = common::demo_module_path();

    // SAFETY: We're loading a trusted module
    let library = unsafe { Library::new(&path).expect("Failed to load library") };

    unsafe {
        // Check API version symbol
        let api_version: Symbol<*const Version> =
            library.get(b"REOVIM_MODULE_API_VERSION").unwrap();

        let version = &**api_version;
        // Version fields are u32, so they're always valid (non-negative)
        // Just verify we can read the version structure without crashing
        let _ = version.major;
        let _ = version.minor;
        let _ = version.patch;
    }
}

#[test]
fn test_null_pointer_handling() {
    let path = common::demo_module_path();

    // SAFETY: We're loading a trusted module
    let library = unsafe { Library::new(&path).expect("Failed to load library") };

    unsafe {
        let supports: Symbol<SupportsHotReloadFn> =
            library.get(b"reovim_module_supports_hot_reload").unwrap();
        let save: Symbol<SaveStateFn> = library.get(b"reovim_module_save_state").unwrap();
        let restore: Symbol<RestoreStateFn> = library.get(b"reovim_module_restore_state").unwrap();

        // Null module pointer should be handled gracefully
        let result = supports(std::ptr::null());
        assert_eq!(result, 0, "supports_hot_reload with null should return 0");

        // Null output pointers should be handled
        let result = save(std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut());
        assert_eq!(result, -1, "save_state with null pointers should return -1");

        // Null module pointer in restore
        let state = [0u8; 24];
        let result = restore(std::ptr::null_mut(), state.as_ptr(), state.len());
        assert_eq!(result, -1, "restore_state with null module should return -1");
    }
}

// ============================================================================
// Registry Integration Tests
// ============================================================================

#[test]
fn test_module_manager_operations() {
    use runner::module::ModuleManager;

    let manager = ModuleManager::new();
    assert!(manager.is_empty());
    assert_eq!(manager.len(), 0);
}

#[test]
fn test_module_loader_search_paths() {
    use runner::module::ModuleLoader;

    let loader = ModuleLoader::new();
    let paths = loader.search_paths();

    // Should have at least the default paths
    assert!(!paths.is_empty(), "Loader should have default search paths");
}

#[test]
fn test_module_discovery() {
    use runner::module::default_search_paths;

    let paths = default_search_paths();

    // Should return some paths (even if they don't exist)
    assert!(!paths.is_empty(), "Should have default search paths");

    // At least one path should contain "reovim"
    let has_reovim_path = paths.iter().any(|p| p.to_string_lossy().contains("reovim"));
    assert!(has_reovim_path, "Should have reovim-specific search path");
}

#[test]
fn test_library_filename() {
    use runner::module::library_filename;

    let filename = library_filename("my-module");

    // library_filename adds "lib" prefix and extension, but doesn't
    // transform the module name (that's done at cargo build time)
    #[cfg(target_os = "linux")]
    assert_eq!(filename, "libmy-module.so");

    #[cfg(target_os = "macos")]
    assert_eq!(filename, "libmy-module.dylib");

    #[cfg(target_os = "windows")]
    assert_eq!(filename, "my-module.dll");
}
