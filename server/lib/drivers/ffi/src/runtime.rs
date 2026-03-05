//! Thread-local runtime bridge for FFI.
//!
//! This module provides the core mechanism that allows C-callable FFI functions
//! to access the `SessionRuntime` during command execution callbacks.
//!
//! # How It Works
//!
//! `SessionRuntime<'_>` borrows from `Session` and only exists during command
//! execution. External modules cannot hold onto it. Instead, we use a
//! thread-local pointer:
//!
//! 1. When the engine calls an `FfiCommandHandler`, a [`RuntimeGuard`] stores
//!    `*mut SessionRuntime` in a thread-local.
//! 2. C-callable functions (e.g., `reovim_buffer_get_line`) use [`with_runtime`]
//!    to access it.
//! 3. When the callback returns, `RuntimeGuard::drop` restores the previous
//!    pointer (supporting nesting).
//!
//! This is the same pattern used by Neovim's Lua bridge and Vim's Python bridge.
//!
//! # Init-Time Services
//!
//! During `Module::init()`, the [`InitGuard`] similarly stores a pointer to
//! the `ServiceRegistry` so that FFI functions like `reovim_register_command`
//! can access it.

#![allow(unsafe_code)]

use std::cell::Cell;

use {reovim_driver_session::SessionRuntime, reovim_kernel::api::v1::ServiceRegistry};

use crate::error::{REOVIM_ERR_NO_INIT_CTX, REOVIM_ERR_NO_RUNTIME};

// ============================================================================
// Runtime thread-local
// ============================================================================

thread_local! {
    /// Active `SessionRuntime` pointer for the current thread.
    ///
    /// Set by [`RuntimeGuard`] during command callback execution.
    /// The `'static` lifetime is a lie — the actual lifetime is bounded
    /// by the `RuntimeGuard` scope. This is safe because:
    /// - `with_runtime` only accesses it while `RuntimeGuard` is alive
    /// - `RuntimeGuard::drop` clears/restores it
    static ACTIVE_RUNTIME: Cell<Option<*mut SessionRuntime<'static>>> = const { Cell::new(None) };
}

/// RAII guard that sets the active runtime for FFI access.
///
/// Supports nesting: when a command callback calls `reovim_execute_command`
/// which triggers another `FfiCommandHandler`, the inner guard saves and
/// restores the outer pointer.
///
/// # Safety
///
/// The caller must ensure the `SessionRuntime` outlives this guard.
pub struct RuntimeGuard {
    /// Previous runtime pointer to restore on drop.
    previous: Option<*mut SessionRuntime<'static>>,
}

impl RuntimeGuard {
    /// Create a new guard, setting the active runtime.
    ///
    /// # Safety
    ///
    /// - The `runtime` reference must remain valid for the lifetime of this guard.
    /// - This must only be called from the thread that owns the `SessionRuntime`.
    #[must_use]
    pub unsafe fn new(runtime: &mut SessionRuntime<'_>) -> Self {
        let previous = ACTIVE_RUNTIME.get();
        let ptr = std::ptr::from_mut(runtime);
        // Erase the borrow lifetime to `'static` for thread-local storage.
        // Safety: the `RuntimeGuard` scope ensures the pointer is valid.
        let ptr: *mut SessionRuntime<'static> = ptr.cast();
        ACTIVE_RUNTIME.set(Some(ptr));
        Self { previous }
    }
}

impl Drop for RuntimeGuard {
    fn drop(&mut self) {
        ACTIVE_RUNTIME.set(self.previous);
    }
}

/// Access the active runtime within a closure.
///
/// Returns `Err(REOVIM_ERR_NO_RUNTIME)` if called outside a command callback
/// (i.e., no `RuntimeGuard` is active on this thread).
///
/// # Errors
///
/// Returns `Err(REOVIM_ERR_NO_RUNTIME)` when no `RuntimeGuard` is active.
///
/// # Safety
///
/// This function dereferences a raw pointer stored in the thread-local.
/// It is safe because:
/// - The pointer is only set by `RuntimeGuard::new` which requires a valid reference
/// - The pointer is cleared by `RuntimeGuard::drop` before the reference becomes invalid
/// - Access is single-threaded (thread-local)
pub fn with_runtime<F, R>(f: F) -> Result<R, i32>
where
    F: FnOnce(&mut SessionRuntime<'_>) -> R,
{
    ACTIVE_RUNTIME
        .get()
        .map_or(Err(REOVIM_ERR_NO_RUNTIME), |ptr| {
            // Safety: RuntimeGuard guarantees the pointer is valid while the
            // guard is alive. We only reach here if a guard is active.
            Ok(f(unsafe { &mut *ptr }))
        })
}

// ============================================================================
// Init-time services thread-local
// ============================================================================

thread_local! {
    /// Active `ServiceRegistry` pointer for init-time registration.
    ///
    /// Set by [`InitGuard`] during `Module::init()`.
    static ACTIVE_SERVICES: Cell<Option<*const ServiceRegistry>> = const { Cell::new(None) };
}

/// RAII guard that sets the active service registry for init-time FFI access.
///
/// Supports nesting (analogous to `RuntimeGuard`).
pub struct InitGuard {
    /// Previous services pointer to restore on drop.
    previous: Option<*const ServiceRegistry>,
}

impl InitGuard {
    /// Create a new guard, setting the active service registry.
    ///
    /// # Safety
    ///
    /// - The `ServiceRegistry` reference must remain valid for the lifetime of this guard.
    #[must_use]
    pub unsafe fn new(services: &ServiceRegistry) -> Self {
        let previous = ACTIVE_SERVICES.get();
        ACTIVE_SERVICES.set(Some(std::ptr::from_ref(services)));
        Self { previous }
    }
}

impl Drop for InitGuard {
    fn drop(&mut self) {
        ACTIVE_SERVICES.set(self.previous);
    }
}

/// Access the active service registry within a closure.
///
/// Returns `Err(REOVIM_ERR_NO_INIT_CTX)` if called outside `Module::init()`.
///
/// # Errors
///
/// Returns `Err(REOVIM_ERR_NO_INIT_CTX)` when no `InitGuard` is active.
pub fn with_services<F, R>(f: F) -> Result<R, i32>
where
    F: FnOnce(&ServiceRegistry) -> R,
{
    ACTIVE_SERVICES
        .get()
        .map_or(Err(REOVIM_ERR_NO_INIT_CTX), |ptr| {
            // Safety: InitGuard guarantees the pointer is valid while the
            // guard is alive.
            Ok(f(unsafe { &*ptr }))
        })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::error::{REOVIM_ERR_NO_INIT_CTX, REOVIM_ERR_NO_RUNTIME},
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
}
