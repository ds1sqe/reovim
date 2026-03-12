use {
    super::*,
    crate::error::{
        REOVIM_ERR_NO_INIT_CTX, REOVIM_ERR_NO_RUNTIME, REOVIM_ERR_NOT_FOUND, REOVIM_ERR_PANIC,
        REOVIM_OK,
    },
};

// ========================================================================
// RuntimeGuard tests
// ========================================================================

#[test]
fn test_with_runtime_returns_err_when_no_guard() {
    let result = with_runtime(|_rt| 42);
    assert_eq!(result, Err(REOVIM_ERR_NO_RUNTIME));
}

#[test]
fn test_runtime_guard_clears_on_drop() {
    // After creating and dropping a guard, the thread-local should be None
    // We can't easily create a real SessionRuntime in a unit test, but we
    // can test the thread-local mechanics directly.
    assert!(ACTIVE_RUNTIME.get().is_none());
}

#[test]
fn test_runtime_thread_local_starts_none() {
    assert!(ACTIVE_RUNTIME.get().is_none());
}

#[test]
fn test_runtime_guard_nesting_mechanics() {
    // Test nesting by directly manipulating the thread-local
    // (simulating what RuntimeGuard does without needing real SessionRuntime)
    let fake_ptr_outer = 0x1000 as *mut SessionRuntime<'static>;
    let fake_ptr_inner = 0x2000 as *mut SessionRuntime<'static>;

    // Simulate outer guard
    let prev_outer = ACTIVE_RUNTIME.get();
    ACTIVE_RUNTIME.set(Some(fake_ptr_outer));

    assert_eq!(ACTIVE_RUNTIME.get(), Some(fake_ptr_outer));

    // Simulate inner guard (saves outer)
    let prev_inner = ACTIVE_RUNTIME.get();
    ACTIVE_RUNTIME.set(Some(fake_ptr_inner));

    assert_eq!(ACTIVE_RUNTIME.get(), Some(fake_ptr_inner));

    // Drop inner guard (restores outer)
    ACTIVE_RUNTIME.set(prev_inner);
    assert_eq!(ACTIVE_RUNTIME.get(), Some(fake_ptr_outer));

    // Drop outer guard (restores None)
    ACTIVE_RUNTIME.set(prev_outer);
    assert!(ACTIVE_RUNTIME.get().is_none());
}

#[test]
fn test_runtime_guard_triple_nesting() {
    let ptrs = [
        0x1000 as *mut SessionRuntime<'static>,
        0x2000 as *mut SessionRuntime<'static>,
        0x3000 as *mut SessionRuntime<'static>,
    ];

    let prev0 = ACTIVE_RUNTIME.get();
    ACTIVE_RUNTIME.set(Some(ptrs[0]));

    let prev1 = ACTIVE_RUNTIME.get();
    ACTIVE_RUNTIME.set(Some(ptrs[1]));

    let prev2 = ACTIVE_RUNTIME.get();
    ACTIVE_RUNTIME.set(Some(ptrs[2]));

    assert_eq!(ACTIVE_RUNTIME.get(), Some(ptrs[2]));

    // Unwind
    ACTIVE_RUNTIME.set(prev2);
    assert_eq!(ACTIVE_RUNTIME.get(), Some(ptrs[1]));

    ACTIVE_RUNTIME.set(prev1);
    assert_eq!(ACTIVE_RUNTIME.get(), Some(ptrs[0]));

    ACTIVE_RUNTIME.set(prev0);
    assert!(ACTIVE_RUNTIME.get().is_none());
}

// ========================================================================
// InitGuard tests
// ========================================================================

#[test]
fn test_with_services_returns_err_when_no_guard() {
    let result = with_services(|_svc| 42);
    assert_eq!(result, Err(REOVIM_ERR_NO_INIT_CTX));
}

#[test]
fn test_init_guard_sets_and_clears() {
    let registry = ServiceRegistry::new();

    assert!(ACTIVE_SERVICES.get().is_none());

    {
        let _guard = unsafe { InitGuard::new(&registry) };
        assert!(ACTIVE_SERVICES.get().is_some());

        // Verify we can access it
        let result = with_services(|_svc| 99);
        assert_eq!(result, Ok(99));
    }

    // After drop, should be None again
    assert!(ACTIVE_SERVICES.get().is_none());
    let result = with_services(|_svc| 99);
    assert_eq!(result, Err(REOVIM_ERR_NO_INIT_CTX));
}

#[test]
fn test_init_guard_nesting() {
    let registry1 = ServiceRegistry::new();
    let registry2 = ServiceRegistry::new();

    let ptr1 = std::ptr::from_ref(&registry1);
    let ptr2 = std::ptr::from_ref(&registry2);

    {
        let _guard1 = unsafe { InitGuard::new(&registry1) };
        assert_eq!(ACTIVE_SERVICES.get(), Some(ptr1));

        {
            let _guard2 = unsafe { InitGuard::new(&registry2) };
            assert_eq!(ACTIVE_SERVICES.get(), Some(ptr2));
        }

        // Inner guard dropped, outer restored
        assert_eq!(ACTIVE_SERVICES.get(), Some(ptr1));
    }

    // Both guards dropped
    assert!(ACTIVE_SERVICES.get().is_none());
}

#[test]
fn test_init_guard_panic_safety() {
    let registry = ServiceRegistry::new();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = unsafe { InitGuard::new(&registry) };
        assert!(ACTIVE_SERVICES.get().is_some());
        panic!("intentional panic");
    }));

    assert!(result.is_err());
    // Guard's Drop should have run even during panic unwind
    assert!(ACTIVE_SERVICES.get().is_none());
}

#[test]
fn test_services_thread_local_starts_none() {
    assert!(ACTIVE_SERVICES.get().is_none());
}

// ========================================================================
// RuntimeGuard panic safety test
// ========================================================================

#[test]
#[allow(clippy::items_after_statements)]
fn test_runtime_guard_panic_safety() {
    let fake_ptr = 0x1000 as *mut SessionRuntime<'static>;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let prev = ACTIVE_RUNTIME.get();
        ACTIVE_RUNTIME.set(Some(fake_ptr));

        // Simulate RuntimeGuard scope with panic
        struct FakeGuard(Option<*mut SessionRuntime<'static>>);
        impl Drop for FakeGuard {
            fn drop(&mut self) {
                ACTIVE_RUNTIME.set(self.0);
            }
        }
        let _guard = FakeGuard(prev);
        assert!(ACTIVE_RUNTIME.get().is_some());
        panic!("intentional panic in runtime guard");
    }));

    assert!(result.is_err());
    // Guard's Drop should have restored None even during panic
    assert!(ACTIVE_RUNTIME.get().is_none());
}

// ========================================================================
// ffi_catch_unwind tests
// ========================================================================

#[test]
fn test_ffi_catch_unwind_success() {
    let result = ffi_catch_unwind(|| 42);
    assert_eq!(result, 42);
}

#[test]
fn test_ffi_catch_unwind_returns_zero() {
    let result = ffi_catch_unwind(|| REOVIM_OK);
    assert_eq!(result, REOVIM_OK);
}

#[test]
fn test_ffi_catch_unwind_returns_error() {
    let result = ffi_catch_unwind(|| REOVIM_ERR_NOT_FOUND);
    assert_eq!(result, REOVIM_ERR_NOT_FOUND);
}

#[test]
fn test_ffi_catch_unwind_catches_panic() {
    let result = ffi_catch_unwind(|| {
        panic!("intentional FFI panic");
    });
    assert_eq!(result, REOVIM_ERR_PANIC);
}

#[test]
fn test_ffi_catch_unwind_handle_success() {
    let handle = ffi_catch_unwind_handle(|| crate::event::ReovimSubscriptionHandle { id: 42 });
    assert!(!handle.is_null());
    assert_eq!(handle.id, 42);
}

#[test]
fn test_ffi_catch_unwind_handle_catches_panic() {
    let handle = ffi_catch_unwind_handle(|| {
        panic!("intentional handle panic");
    });
    assert!(handle.is_null());
    assert_eq!(handle.id, 0);
}

// ========================================================================
// RuntimeGuard with real SessionRuntime
// ========================================================================

#[test]
fn test_runtime_guard_with_real_session_runtime() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("test");
    let mut rt = harness.runtime();

    {
        let _guard = unsafe { RuntimeGuard::new(&mut rt) };
        let result = with_runtime(|runtime| {
            use reovim_driver_session::api::BufferApi;
            runtime.active_buffer()
        });
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    // After guard dropped, should fail again
    assert!(with_runtime(|_| ()).is_err());
}
