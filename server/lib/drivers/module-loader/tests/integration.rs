//! Integration tests for dynamic module loading.
//!
//! These tests exercise the full FFI load-probe-init-exit cycle using a real
//! `.so` module built from the `reovim-test-dynamic-module` fixture crate.
//!
//! # Prerequisites
//!
//! Build the test module before running:
//! ```sh
//! cargo build -p reovim-test-dynamic-module
//! ```
//!
//! The tests locate the `.so` in `target/debug/` automatically.

use std::path::PathBuf;

use {
    reovim_driver_module_loader::{
        loader::ModuleLoader,
        lockfile::{ModuleSource, ModulesLock},
        registry::ModuleRegistry,
    },
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ModuleState, ProbeResult, Version,
    },
};

/// Locate the test module `.so` in `target/debug/`.
fn test_module_path() -> Option<PathBuf> {
    // Walk up from CARGO_MANIFEST_DIR to find workspace root
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()? // drivers
        .parent()? // lib
        .parent()? // server
        .parent()?; // workspace root

    let so_path = workspace_root
        .join("target")
        .join("debug")
        .join("libreovim_test_dynamic_module.so");

    if so_path.exists() {
        Some(so_path)
    } else {
        None
    }
}

/// Skip test if `.so` not found.
macro_rules! require_so {
    () => {
        match test_module_path() {
            Some(path) => path,
            None => {
                eprintln!(
                    "SKIP: test module .so not found. Run: cargo build -p reovim-test-dynamic-module"
                );
                return;
            }
        }
    };
}

// ============================================================================
// ModuleLoader: load_dynamic
// ============================================================================

#[test]
fn test_load_dynamic_module() {
    let so_path = require_so!();

    let mut loader = ModuleLoader::new();

    #[allow(unsafe_code)]
    // SAFETY: The test module was built from workspace source with matching ABI.
    let id = unsafe { loader.load_dynamic(&so_path) }.expect("load_dynamic failed");

    assert_eq!(id.as_str(), "test-dynamic");
    assert_eq!(loader.len(), 1);

    let handle = loader.get(&id).expect("handle not found");
    assert_eq!(handle.name(), "Test Dynamic Module");
    assert_eq!(handle.version(), Version::new(1, 0, 0));
    assert!(handle.is_dynamic());
    assert!(!handle.is_static());
    assert!(handle.path().is_some());
}

#[test]
fn test_probe_metadata() {
    let so_path = require_so!();

    let mut loader = ModuleLoader::new();

    #[allow(unsafe_code)]
    let id = unsafe { loader.load_dynamic(&so_path) }.unwrap();

    let handle = loader.get(&id).unwrap();
    assert_eq!(handle.id().as_str(), "test-dynamic");
    assert_eq!(handle.name(), "Test Dynamic Module");
    assert_eq!(handle.version(), Version::new(1, 0, 0));
    assert!(handle.dependencies().is_empty());
    assert!(handle.optional_dependencies().is_empty());
}

#[test]
fn test_duplicate_load_rejected() {
    let so_path = require_so!();

    let mut loader = ModuleLoader::new();

    #[allow(unsafe_code)]
    unsafe {
        loader.load_dynamic(&so_path).unwrap();
    }

    // Second load of same module should fail
    #[allow(unsafe_code)]
    let result = unsafe { loader.load_dynamic(&so_path) };
    assert!(result.is_err());
}

#[test]
fn test_load_nonexistent_so() {
    let mut loader = ModuleLoader::new();

    #[allow(unsafe_code)]
    let result = unsafe { loader.load_dynamic(std::path::Path::new("/nonexistent/module.so")) };
    assert!(result.is_err());
}

// ============================================================================
// ModuleRegistry: full lifecycle
// ============================================================================

#[test]
fn test_registry_dynamic_lifecycle() {
    let so_path = require_so!();

    let registry = ModuleRegistry::new();

    // Load dynamic module
    #[allow(unsafe_code)]
    let id = unsafe { registry.load_dynamic(&so_path) }.expect("load failed");

    assert_eq!(id.as_str(), "test-dynamic");
    assert_eq!(registry.state(&id), Some(ModuleState::Loaded));

    // Use init_all so the module is tracked in init_order for shutdown
    let ctx = ModuleContext::default();
    registry.init_all(&ctx).expect("init_all failed");
    assert_eq!(registry.state(&id), Some(ModuleState::Running));

    // Shutdown (requires init_order populated by init_all)
    registry.shutdown();
    assert_eq!(registry.state(&id), Some(ModuleState::Loaded));
}

#[test]
fn test_registry_init_all_with_dynamic() {
    let so_path = require_so!();

    let registry = ModuleRegistry::new();

    #[allow(unsafe_code)]
    unsafe {
        registry.load_dynamic(&so_path).unwrap();
    }

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).expect("init_all failed");

    let id = ModuleId::new("test-dynamic");
    assert_eq!(registry.state(&id), Some(ModuleState::Running));
}

// ============================================================================
// Mixed static + dynamic
// ============================================================================

struct StaticModule;
impl Module for StaticModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("static-mod")
    }
    fn name(&self) -> &'static str {
        "Static Module"
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn init(&mut self, _: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }
    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[test]
fn test_mixed_static_dynamic() {
    let so_path = require_so!();

    let registry = ModuleRegistry::new();

    // Register static
    registry.register(StaticModule).unwrap();

    // Load dynamic
    #[allow(unsafe_code)]
    unsafe {
        registry.load_dynamic(&so_path).unwrap();
    }

    // Init all
    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    assert_eq!(registry.state(&ModuleId::new("static-mod")), Some(ModuleState::Running));
    assert_eq!(registry.state(&ModuleId::new("test-dynamic")), Some(ModuleState::Running));
    assert_eq!(registry.len(), 2);
}

// ============================================================================
// Lock file generation with dynamic module
// ============================================================================

#[test]
fn test_lock_file_with_dynamic_module() {
    let so_path = require_so!();

    let registry = ModuleRegistry::new();

    #[allow(unsafe_code)]
    unsafe {
        registry.load_dynamic(&so_path).unwrap();
    }

    let lock = ModulesLock::generate(&registry, "0.10.0-dev");
    assert_eq!(lock.modules.len(), 1);

    let entry = &lock.modules[0];
    assert_eq!(entry.id, "test-dynamic");
    // generate() currently uses String::new() for version (probe-based population pending)
    assert_eq!(entry.source, ModuleSource::External);
    assert!(entry.path.is_some());
    assert!(entry.sha256.is_some());
    assert!(entry.dependencies.is_empty());
}

// ============================================================================
// Checksum verification with real .so
// ============================================================================

#[test]
fn test_checksum_verification_real_so() {
    use reovim_driver_module_loader::lockfile::sha256_file;

    let so_path = require_so!();

    // Compute real checksum
    let hash = sha256_file(&so_path).expect("sha256 failed");
    assert!(!hash.is_empty());
    assert_eq!(hash.len(), 64); // SHA-256 hex = 64 chars
}

// ============================================================================
// Unload and re-load cycle
// ============================================================================

#[test]
fn test_unload_and_reload() {
    let so_path = require_so!();

    let registry = ModuleRegistry::new();

    // Load and init
    #[allow(unsafe_code)]
    let id = unsafe { registry.load_dynamic(&so_path) }.unwrap();

    let ctx = ModuleContext::default();
    registry.init_module(&id, &ctx).unwrap();
    assert_eq!(registry.state(&id), Some(ModuleState::Running));

    // Unload
    registry.unload(&id).unwrap();
    assert!(registry.state(&id).is_none());
    assert_eq!(registry.len(), 0);

    // Reload
    #[allow(unsafe_code)]
    let id2 = unsafe { registry.load_dynamic(&so_path) }.unwrap();
    assert_eq!(id2.as_str(), "test-dynamic");

    registry.init_module(&id2, &ctx).unwrap();
    assert_eq!(registry.state(&id2), Some(ModuleState::Running));
}

// ============================================================================
// #725 Phase 1 — on_all_loaded FFI trampoline
// ============================================================================

/// P1-T1: verify `declare_module!` emits the `reovim_module_on_all_loaded`
/// symbol. Catches regressions in the macro output without needing a
/// behavioral dispatch test.
#[test]
fn test_dynamic_module_exports_on_all_loaded_symbol() {
    let so_path = require_so!();

    // SAFETY: fixture built from workspace source with matching ABI.
    #[allow(unsafe_code)]
    let library = unsafe { libloading::Library::new(&so_path) }.unwrap();

    #[allow(unsafe_code)]
    let sym: Result<
        libloading::Symbol<
            unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_void),
        >,
        _,
    > = unsafe { library.get(b"reovim_module_on_all_loaded") };
    assert!(
        sym.is_ok(),
        "declare_module! must emit reovim_module_on_all_loaded symbol",
    );
}

/// P1-T2: verify `ModuleHandle::on_all_loaded` actually dispatches through
/// the FFI trampoline for a dynamic module. Uses the test fixture's
/// `test_module_on_all_loaded_flag` observable + `reset` helper.
#[test]
fn test_dynamic_on_all_loaded_fires_via_ffi() {
    let so_path = require_so!();

    // Reset the fixture's static flag via the dedicated symbol so parallel
    // test runs do not see leaked state from previous cases.
    #[allow(unsafe_code)]
    {
        let library = unsafe { libloading::Library::new(&so_path) }.unwrap();
        let reset: libloading::Symbol<unsafe extern "C" fn()> =
            unsafe { library.get(b"test_module_reset_on_all_loaded_flag") }.unwrap();
        unsafe { reset() };
        // Library drops here but the fixture crate stays loaded via refcount
        // because the next Library::new call below reopens the same path.
    }

    let mut loader = ModuleLoader::new();

    #[allow(unsafe_code)]
    let id = unsafe { loader.load_dynamic(&so_path) }.unwrap();

    let ctx = ModuleContext::default();
    // Init must run before on_all_loaded per the lifecycle contract.
    loader.get_mut(&id).unwrap().init(&ctx).unwrap();

    // Dispatch on_all_loaded through the handle — this is the FFI path
    // being tested.
    loader.get_mut(&id).unwrap().on_all_loaded(&ctx);

    // Read the fixture's static flag via a separate dlsym lookup. The
    // loader still holds the library alive, so this second `Library::new`
    // resolves through the OS's dlopen refcount and sees the same static.
    #[allow(unsafe_code)]
    let flag_library = unsafe { libloading::Library::new(&so_path) }.unwrap();
    #[allow(unsafe_code)]
    let flag_fn: libloading::Symbol<unsafe extern "C" fn() -> bool> =
        unsafe { flag_library.get(b"test_module_on_all_loaded_flag") }.unwrap();

    #[allow(unsafe_code)]
    let flag = unsafe { flag_fn() };
    assert!(
        flag,
        "on_all_loaded trampoline should have set the observable flag",
    );
}

// ============================================================================
// #725 Phase 3 — ModuleLoader::take() unit test (P3-T1)
// ============================================================================

/// Minimal static module for the `take()` unit test. Does not need to be a
/// separate crate — this is a pure in-test definition.
struct TakeTestModule;

impl Module for TakeTestModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("take-test")
    }
    fn name(&self) -> &'static str {
        "Take Test Module"
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }
    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[test]
fn test_loader_take_removes_and_returns_handle() {
    let mut loader = ModuleLoader::new();
    let id = loader
        .register_static(TakeTestModule)
        .expect("register_static should succeed");

    // Present before take.
    assert!(loader.get(&id).is_some());
    assert_eq!(loader.len(), 1);

    // Take transfers ownership — returns Some, then None on repeat.
    let taken = loader.take(&id);
    assert!(taken.is_some(), "take should return the handle");
    assert!(loader.get(&id).is_none());
    assert_eq!(loader.len(), 0);

    assert!(
        loader.take(&id).is_none(),
        "second take should return None",
    );
}
