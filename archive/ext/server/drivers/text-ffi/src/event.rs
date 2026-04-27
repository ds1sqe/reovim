//! Event subscription API for FFI modules.
//!
//! This module provides event subscription infrastructure for external modules.
//!
//! # Design
//!
//! The kernel's `EventBus` uses Rust's type system (`TypeId`) for event
//! routing. FFI modules can't reference Rust types, so this module provides
//! a string-based event type system:
//!
//! 1. `FfiEvent` is a concrete Rust event type that carries an event name string
//! 2. FFI modules subscribe to `FfiEvent` filtered by name
//! 3. When the engine emits `FfiEvent`, matching subscribers are called
//!
//! # v1 Limitations
//!
//! Event data is opaque (`NULL`) in v1. Future versions can add structured
//! event payloads via `#[repr(C)]` event structs or JSON serialization.

#![allow(unsafe_code)]

use std::{
    ffi::CStr,
    sync::atomic::{AtomicU64, Ordering},
};

use libc::{c_char, c_void};

use std::panic::AssertUnwindSafe;

#[allow(clippy::wildcard_imports)]
use crate::error::*;
use crate::runtime::{ffi_catch_unwind, ffi_catch_unwind_handle};

// ============================================================================
// Event callback types
// ============================================================================

/// C function pointer type for event callbacks.
///
/// # Parameters
///
/// - `user_data`: Opaque pointer passed during subscription
/// - `event_type`: Null-terminated event type name string
/// - `event_data`: Event-specific data (NULL in v1)
/// - `event_data_len`: Length of `event_data` (0 in v1)
pub type ReovimEventCallback = Option<
    unsafe extern "C" fn(
        user_data: *mut c_void,
        event_type: *const c_char,
        event_data: *const c_void,
        event_data_len: u32,
    ),
>;

/// Opaque handle for event subscriptions.
///
/// Zero means invalid/null handle. Non-zero values are valid handles
/// that can be passed to `reovim_unsubscribe_event()`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReovimSubscriptionHandle {
    /// Internal ID (0 = null/invalid).
    pub id: u64,
}

impl ReovimSubscriptionHandle {
    /// Create a null (invalid) handle.
    #[must_use]
    pub const fn null() -> Self {
        Self { id: 0 }
    }

    /// Check if the handle is null (invalid).
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.id == 0
    }
}

/// Next subscription ID counter.
static NEXT_SUB_ID: AtomicU64 = AtomicU64::new(1);

// ============================================================================
// Subscription storage
// ============================================================================

/// A stored event subscription.
struct FfiSubscription {
    /// Unique subscription ID.
    id: u64,
    /// Event type filter (exact match).
    event_type: String,
    /// C callback function.
    callback: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_void, u32),
    /// Opaque user data for the callback.
    user_data: *mut c_void,
}

// Safety: user_data is an opaque pointer owned by the external module.
unsafe impl Send for FfiSubscription {}
unsafe impl Sync for FfiSubscription {}

/// Storage for FFI event subscriptions.
///
/// Manages subscriptions registered by FFI modules. When events are
/// dispatched, matching subscriptions have their callbacks invoked.
#[derive(Default)]
pub struct FfiEventSubscriptionStore {
    subs: std::sync::Mutex<Vec<FfiSubscription>>,
}

impl FfiEventSubscriptionStore {
    /// Subscribe to an event type.
    ///
    /// Returns the subscription handle.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn subscribe(
        &self,
        event_type: String,
        callback: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_void, u32),
        user_data: *mut c_void,
    ) -> ReovimSubscriptionHandle {
        let id = NEXT_SUB_ID.fetch_add(1, Ordering::Relaxed);
        self.subs
            .lock()
            .expect("lock poisoned")
            .push(FfiSubscription {
                id,
                event_type,
                callback,
                user_data,
            });
        ReovimSubscriptionHandle { id }
    }

    /// Unsubscribe by handle.
    ///
    /// Returns true if the subscription was found and removed.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn unsubscribe(&self, handle: ReovimSubscriptionHandle) -> bool {
        let mut subs = self.subs.lock().expect("lock poisoned");
        let len_before = subs.len();
        subs.retain(|s| s.id != handle.id);
        subs.len() < len_before
    }

    /// Dispatch an event to all matching subscribers.
    ///
    /// Calls each subscriber whose `event_type` matches the given type name.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn dispatch(&self, event_type: &str) {
        let subs = self.subs.lock().expect("lock poisoned");
        let c_event_type =
            std::ffi::CString::new(event_type).unwrap_or_else(|_| std::ffi::CString::default());
        for sub in subs.iter() {
            if sub.event_type == event_type {
                // Safety: the callback and user_data are provided by the FFI module
                // and must remain valid for the subscription's lifetime.
                unsafe {
                    (sub.callback)(sub.user_data, c_event_type.as_ptr(), std::ptr::null(), 0);
                }
            }
        }
    }

    /// Number of active subscriptions.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn count(&self) -> usize {
        self.subs.lock().expect("lock poisoned").len()
    }
}

impl reovim_kernel::api::v1::Service for FfiEventSubscriptionStore {}

// ============================================================================
// FFI functions
// ============================================================================

/// Subscribe to an event type.
///
/// The callback is invoked whenever an event of the given type is dispatched
/// to FFI subscribers.
///
/// # Returns
///
/// A subscription handle. Returns a null handle on error.
///
/// # Safety
///
/// - `event_type` must be a valid null-terminated C string or null
/// - `callback` must be a valid function pointer
/// - `user_data` must remain valid for the lifetime of the subscription
/// - Must be called during module init (`InitGuard` active)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_subscribe_event(
    event_type: *const c_char,
    callback: ReovimEventCallback,
    user_data: *mut c_void,
    _priority: u32,
) -> ReovimSubscriptionHandle {
    ffi_catch_unwind_handle(AssertUnwindSafe(|| {
        if event_type.is_null() {
            return ReovimSubscriptionHandle::null();
        }

        let Some(callback) = callback else {
            return ReovimSubscriptionHandle::null();
        };

        let c_str = unsafe { CStr::from_ptr(event_type) };
        let Ok(type_str) = c_str.to_str() else {
            return ReovimSubscriptionHandle::null();
        };

        crate::runtime::with_services(|services| {
            services
                .get_or_create::<FfiEventSubscriptionStore>()
                .subscribe(type_str.to_string(), callback, user_data)
        })
        .unwrap_or(ReovimSubscriptionHandle::null())
    }))
}

/// Unsubscribe from an event.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NOT_FOUND` if the handle is not found
/// - `REOVIM_ERR_NO_INIT_CTX` if called outside init context
///
/// # Safety
///
/// - `handle` must be a valid handle returned by `reovim_subscribe_event`
/// - Must be called during module init (`InitGuard` active)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_unsubscribe_event(handle: ReovimSubscriptionHandle) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if handle.is_null() {
            return REOVIM_ERR_NOT_FOUND;
        }

        crate::runtime::with_services(|services| {
            let store = services.get_or_create::<FfiEventSubscriptionStore>();
            if store.unsubscribe(handle) {
                REOVIM_OK
            } else {
                REOVIM_ERR_NOT_FOUND
            }
        })
        .unwrap_or(REOVIM_ERR_NO_INIT_CTX)
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "event_tests.rs"]
mod tests;
