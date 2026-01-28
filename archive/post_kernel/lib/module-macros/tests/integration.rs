//! Integration tests for `declare_module!` macro
//!
//! These tests verify the macro generates correct FFI entry points
//! and that the generated code works correctly.
//!
//! # Test Coverage
//!
//! - Static API version symbol
//! - Probe metadata retrieval
//! - Entry point (module creation)
//! - Init trampoline (success, defer, failed, panic)
//! - Exit trampoline (success, error, panic)
//! - Destroy trampoline (null safety, panic safety)
//! - Full lifecycle sequence

// FFI testing requires unsafe code
#![allow(unsafe_code)]

use std::cell::Cell;

use {reovim_kernel::api::v1::*, reovim_module_macros::declare_module};

// ============================================================================
// Thread-Local Behavior Control
// ============================================================================
// These thread-locals allow tests to control module behavior without
// needing separate module definitions (which would cause symbol conflicts).

thread_local! {
    static INIT_BEHAVIOR: Cell<InitBehavior> = const { Cell::new(InitBehavior::Success) };
    static EXIT_BEHAVIOR: Cell<ExitBehavior> = const { Cell::new(ExitBehavior::Success) };
    static DROP_BEHAVIOR: Cell<DropBehavior> = const { Cell::new(DropBehavior::Normal) };
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InitBehavior {
    Success,
    Defer,
    Failed,
    Panic,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExitBehavior {
    Success,
    Error,
    Panic,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DropBehavior {
    Normal,
    Panic,
}

/// Reset all behaviors to default (success paths)
fn reset_behaviors() {
    INIT_BEHAVIOR.set(InitBehavior::Success);
    EXIT_BEHAVIOR.set(ExitBehavior::Success);
    DROP_BEHAVIOR.set(DropBehavior::Normal);
}

// ============================================================================
// Test Module Definition
// ============================================================================

struct TestModule {
    initialized: bool,
}

impl Module for TestModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("test-module")
    }

    fn name(&self) -> &'static str {
        "Test Module"
    }

    fn version(&self) -> Version {
        Version::new(1, 2, 3)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        match INIT_BEHAVIOR.get() {
            InitBehavior::Success => {
                self.initialized = true;
                ProbeResult::Success
            }
            InitBehavior::Defer => ProbeResult::Defer("resource not ready".into()),
            InitBehavior::Failed => {
                ProbeResult::Failed(ModuleError::InitFailed("initialization failed".into()))
            }
            InitBehavior::Panic => panic!("intentional panic in init for testing"),
        }
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        match EXIT_BEHAVIOR.get() {
            ExitBehavior::Success => {
                self.initialized = false;
                Ok(())
            }
            ExitBehavior::Error => Err(ModuleError::InitFailed("exit failed".into())),
            ExitBehavior::Panic => panic!("intentional panic in exit for testing"),
        }
    }
}

impl TestModule {
    const fn new() -> Self {
        Self { initialized: false }
    }
}

impl Drop for TestModule {
    fn drop(&mut self) {
        assert!(
            DROP_BEHAVIOR.get() != DropBehavior::Panic,
            "intentional panic in Drop for testing"
        );
    }
}

// Generate FFI entry points
declare_module!(TestModule);

// ============================================================================
// Static Symbol Tests
// ============================================================================

#[test]
fn test_static_api_version() {
    // Static symbol should match kernel's API version
    assert_eq!(REOVIM_MODULE_API_VERSION, API_VERSION);
}

// ============================================================================
// Probe Tests
// ============================================================================

#[test]
fn test_probe_returns_metadata() {
    let probe = reovim_module_probe();

    assert_eq!(probe.id_str(), "test-module");
    assert_eq!(probe.name_str(), "Test Module");
    assert_eq!(probe.version, Version::new(1, 2, 3));
    assert_eq!(probe.api_version, API_VERSION);
}

#[test]
fn test_probe_is_copy() {
    // ModuleProbe should be Copy (no heap allocation)
    let probe1 = reovim_module_probe();
    let probe2 = probe1; // Copy
    assert_eq!(probe1.id_str(), probe2.id_str());
}

// ============================================================================
// Entry Tests
// ============================================================================

#[test]
fn test_entry_creates_module() {
    reset_behaviors();

    // Safety: we own the pointer and will properly dispose of it
    let module_ptr = unsafe { reovim_module_entry() };
    assert!(!module_ptr.is_null());

    // Clean up properly
    unsafe { reovim_module_destroy(module_ptr) };
}

#[test]
fn test_multiple_modules_independent() {
    reset_behaviors();

    // Each call should create a new instance
    let ptr1 = unsafe { reovim_module_entry() };
    let ptr2 = unsafe { reovim_module_entry() };

    assert!(!ptr1.is_null());
    assert!(!ptr2.is_null());
    assert_ne!(ptr1, ptr2); // Different allocations

    // Cleanup
    unsafe {
        reovim_module_destroy(ptr1);
        reovim_module_destroy(ptr2);
    }
}

// ============================================================================
// Init Trampoline Tests
// ============================================================================

#[test]
fn test_init_trampoline_success() {
    reset_behaviors();
    INIT_BEHAVIOR.set(InitBehavior::Success);

    let module_ptr = unsafe { reovim_module_entry() };
    let ctx = ModuleContext::default();

    // Safety: module_ptr is valid from entry(), ctx is valid stack reference
    let result = unsafe {
        reovim_module_init(module_ptr, std::ptr::from_ref(&ctx).cast::<std::ffi::c_void>())
    };

    // Success should return 0
    assert_eq!(result, 0, "init success should return 0");

    // Cleanup
    unsafe { reovim_module_destroy(module_ptr) };
}

#[test]
fn test_init_trampoline_defer() {
    reset_behaviors();
    INIT_BEHAVIOR.set(InitBehavior::Defer);

    let module_ptr = unsafe { reovim_module_entry() };
    let ctx = ModuleContext::default();

    let result = unsafe {
        reovim_module_init(module_ptr, std::ptr::from_ref(&ctx).cast::<std::ffi::c_void>())
    };

    // Defer should return 1
    assert_eq!(result, 1, "init defer should return 1");

    // Cleanup
    unsafe { reovim_module_destroy(module_ptr) };
}

#[test]
fn test_init_trampoline_failed() {
    reset_behaviors();
    INIT_BEHAVIOR.set(InitBehavior::Failed);

    let module_ptr = unsafe { reovim_module_entry() };
    let ctx = ModuleContext::default();

    let result = unsafe {
        reovim_module_init(module_ptr, std::ptr::from_ref(&ctx).cast::<std::ffi::c_void>())
    };

    // Failed should return -1
    assert_eq!(result, -1, "init failed should return -1");

    // Cleanup
    unsafe { reovim_module_destroy(module_ptr) };
}

#[test]
fn test_init_trampoline_panic_returns_negative_two() {
    reset_behaviors();
    INIT_BEHAVIOR.set(InitBehavior::Panic);

    let module_ptr = unsafe { reovim_module_entry() };
    let ctx = ModuleContext::default();

    // Panic should be caught and return -2
    let result = unsafe {
        reovim_module_init(module_ptr, std::ptr::from_ref(&ctx).cast::<std::ffi::c_void>())
    };

    assert_eq!(result, -2, "init panic should return -2 (caught by catch_unwind)");

    // Reset before cleanup to avoid panic in Drop
    DROP_BEHAVIOR.set(DropBehavior::Normal);
    unsafe { reovim_module_destroy(module_ptr) };
}

// ============================================================================
// Exit Trampoline Tests
// ============================================================================

#[test]
fn test_exit_trampoline_success() {
    reset_behaviors();

    let module_ptr = unsafe { reovim_module_entry() };

    // Exit should return success (0)
    let result = unsafe { reovim_module_exit(module_ptr) };
    assert_eq!(result, 0, "exit success should return 0");

    // Clean up
    unsafe { reovim_module_destroy(module_ptr) };
}

#[test]
fn test_exit_trampoline_error() {
    reset_behaviors();
    EXIT_BEHAVIOR.set(ExitBehavior::Error);

    let module_ptr = unsafe { reovim_module_entry() };

    // Error should return -1
    let result = unsafe { reovim_module_exit(module_ptr) };
    assert_eq!(result, -1, "exit error should return -1");

    // Clean up
    unsafe { reovim_module_destroy(module_ptr) };
}

#[test]
fn test_exit_trampoline_panic_returns_negative_two() {
    reset_behaviors();
    EXIT_BEHAVIOR.set(ExitBehavior::Panic);

    let module_ptr = unsafe { reovim_module_entry() };

    // Panic should be caught and return -2
    let result = unsafe { reovim_module_exit(module_ptr) };
    assert_eq!(result, -2, "exit panic should return -2 (caught by catch_unwind)");

    // Reset before cleanup
    DROP_BEHAVIOR.set(DropBehavior::Normal);
    unsafe { reovim_module_destroy(module_ptr) };
}

// ============================================================================
// Destroy Trampoline Tests
// ============================================================================

#[test]
fn test_destroy_null_safe() {
    // destroy() should handle null safely
    unsafe {
        reovim_module_destroy(std::ptr::null_mut());
    }
    // Should not crash
}

#[test]
fn test_destroy_panic_safe() {
    reset_behaviors();
    DROP_BEHAVIOR.set(DropBehavior::Panic);

    let module_ptr = unsafe { reovim_module_entry() };

    // Panic in Drop should be caught silently
    // This should NOT crash or propagate the panic
    unsafe { reovim_module_destroy(module_ptr) };

    // If we reach here, the panic was successfully caught
    // Reset for other tests
    DROP_BEHAVIOR.set(DropBehavior::Normal);
}

// ============================================================================
// Full Lifecycle Tests
// ============================================================================

#[test]
fn test_full_lifecycle_sequence() {
    reset_behaviors();

    // 1. Entry: Create module instance
    let module_ptr = unsafe { reovim_module_entry() };
    assert!(!module_ptr.is_null(), "entry should return non-null pointer");

    // 2. Init: Initialize the module
    let ctx = ModuleContext::default();
    let init_result = unsafe {
        reovim_module_init(module_ptr, std::ptr::from_ref(&ctx).cast::<std::ffi::c_void>())
    };
    assert_eq!(init_result, 0, "init should succeed");

    // 3. Exit: Cleanup module state
    let exit_result = unsafe { reovim_module_exit(module_ptr) };
    assert_eq!(exit_result, 0, "exit should succeed");

    // 4. Destroy: Free memory
    unsafe { reovim_module_destroy(module_ptr) };

    // If we reach here, the full lifecycle completed successfully
}

#[test]
fn test_lifecycle_with_defer_retry() {
    reset_behaviors();

    let module_ptr = unsafe { reovim_module_entry() };
    let ctx = ModuleContext::default();

    // First init: Defer
    INIT_BEHAVIOR.set(InitBehavior::Defer);
    let result1 = unsafe {
        reovim_module_init(module_ptr, std::ptr::from_ref(&ctx).cast::<std::ffi::c_void>())
    };
    assert_eq!(result1, 1, "first init should defer");

    // Second init: Still defer
    let result2 = unsafe {
        reovim_module_init(module_ptr, std::ptr::from_ref(&ctx).cast::<std::ffi::c_void>())
    };
    assert_eq!(result2, 1, "second init should still defer");

    // Third init: Success (resource now available)
    INIT_BEHAVIOR.set(InitBehavior::Success);
    let result3 = unsafe {
        reovim_module_init(module_ptr, std::ptr::from_ref(&ctx).cast::<std::ffi::c_void>())
    };
    assert_eq!(result3, 0, "third init should succeed");

    // Normal cleanup
    let exit_result = unsafe { reovim_module_exit(module_ptr) };
    assert_eq!(exit_result, 0);
    unsafe { reovim_module_destroy(module_ptr) };
}

// ============================================================================
// Compile-time Trait Verification
// ============================================================================

#[test]
fn test_module_trait_bounds() {
    const fn assert_send_sync<T: Send + Sync + 'static>() {}
    assert_send_sync::<TestModule>();
}

#[test]
fn test_module_object_safety() {
    // These function signatures must compile
    fn accepts_module_ref(_: &dyn Module) {}
    fn accepts_boxed_module(_: Box<dyn Module>) {}

    let module = TestModule::new();
    accepts_module_ref(&module);
    accepts_boxed_module(Box::new(module));
}

// ============================================================================
// FFI Type Safety
// ============================================================================

#[test]
fn test_thin_pointer_size() {
    // Verify we're using thin pointers, not fat pointers
    // Thin pointer: 8 bytes on 64-bit
    // Fat pointer: 16 bytes on 64-bit (data + vtable)
    assert_eq!(std::mem::size_of::<*mut std::ffi::c_void>(), std::mem::size_of::<usize>());
}

#[test]
fn test_probe_struct_ffi_safe() {
    // Verify ModuleProbe has expected size for FFI
    // id: 64 + name: 128 + version: 12 + api_version: 12 + rustc_version: 64
    // + required_deps_count: 1 + required_deps: 512 + optional_deps_count: 1
    // + optional_deps: 512 + padding: 2 = 1308
    assert_eq!(std::mem::size_of::<ModuleProbe>(), 1308);
}

// ============================================================================
// Return Code Documentation Verification
// ============================================================================

/// Verify return codes match documentation:
/// - 0: Success
/// - 1: Defer (init only)
/// - -1: Failed/Error
/// - -2: Panic occurred
#[test]
fn test_return_code_documentation() {
    reset_behaviors();
    let ctx = ModuleContext::default();

    // Test all init return codes
    let cases = [
        (InitBehavior::Success, 0, "Success"),
        (InitBehavior::Defer, 1, "Defer"),
        (InitBehavior::Failed, -1, "Failed"),
        (InitBehavior::Panic, -2, "Panic"),
    ];

    for (behavior, expected_code, name) in cases {
        INIT_BEHAVIOR.set(behavior);
        DROP_BEHAVIOR.set(DropBehavior::Normal); // Ensure clean Drop

        let module_ptr = unsafe { reovim_module_entry() };
        let result = unsafe {
            reovim_module_init(module_ptr, std::ptr::from_ref(&ctx).cast::<std::ffi::c_void>())
        };

        assert_eq!(
            result, expected_code,
            "init {name} should return {expected_code}, got {result}"
        );

        unsafe { reovim_module_destroy(module_ptr) };
    }

    // Test all exit return codes
    let exit_cases = [
        (ExitBehavior::Success, 0, "Success"),
        (ExitBehavior::Error, -1, "Error"),
        (ExitBehavior::Panic, -2, "Panic"),
    ];

    for (behavior, expected_code, name) in exit_cases {
        EXIT_BEHAVIOR.set(behavior);
        DROP_BEHAVIOR.set(DropBehavior::Normal);

        let module_ptr = unsafe { reovim_module_entry() };
        let result = unsafe { reovim_module_exit(module_ptr) };

        assert_eq!(
            result, expected_code,
            "exit {name} should return {expected_code}, got {result}"
        );

        unsafe { reovim_module_destroy(module_ptr) };
    }
}
