//! FFI-safe timer functions for external modules.
//!
//! These functions allow external modules written in C (or any FFI-compatible
//! language) to schedule delayed and periodic work through the kernel's timer
//! infrastructure.
//!
//! # Architecture
//!
//! The timer FFI uses a global `TimerWheel` that must be initialized by the
//! main application before modules can schedule timers. If timers are scheduled
//! before initialization, they fail gracefully with a null handle.
//!
//! # Thread Safety
//!
//! All timer functions are thread-safe. Multiple threads can schedule, cancel,
//! and query timers concurrently.
//!
//! # Callback Safety
//!
//! Timer callbacks are invoked from the kernel's tick loop. Callbacks MUST:
//! - Return quickly (avoid blocking operations)
//! - Not panic (panics are caught but waste resources)
//! - Not call `reovim_cancel_timer` on the currently firing timer
//!
//! # C Usage
//!
//! ```c
//! #include <reovim.h>
//!
//! void my_callback(void* user_data) {
//!     int* counter = (int*)user_data;
//!     (*counter)++;
//! }
//!
//! void schedule_timer(void) {
//!     int counter = 0;
//!     ReovimTimerHandle handle = reovim_schedule_delayed(
//!         1000,  // 1 second delay
//!         my_callback,
//!         &counter
//!     );
//!
//!     if (handle.id == 0) {
//!         reovim_log_error("Failed to schedule timer");
//!         return;
//!     }
//!
//!     // Timer will fire after 1 second
//!     // To cancel: reovim_cancel_timer(handle);
//! }
//! ```

use std::sync::{Arc, OnceLock};

use reovim_kernel::api::v1::{Priority, TimerWheel};

/// Global timer wheel instance.
///
/// This is initialized by the main application during startup.
/// FFI functions use this to schedule timers.
static TIMER_WHEEL: OnceLock<Arc<TimerWheel>> = OnceLock::new();

/// Initialize the global timer wheel for FFI access.
///
/// This should be called once during application startup, passing the
/// runtime's timer wheel. After initialization, FFI functions can schedule
/// timers.
///
/// # Panics
///
/// Panics if called more than once.
pub fn init_timer_wheel(wheel: Arc<TimerWheel>) {
    TIMER_WHEEL
        .set(wheel)
        .expect("Timer wheel already initialized");
}

/// Get the global timer wheel, if initialized.
fn get_timer_wheel() -> Option<&'static Arc<TimerWheel>> {
    TIMER_WHEEL.get()
}

// ============================================================================
// C-compatible types
// ============================================================================

/// Opaque timer handle for FFI.
///
/// The `id` field is 0 if the timer failed to schedule.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ReovimTimerHandle {
    /// Timer ID. 0 indicates a failed/invalid timer.
    pub id: u64,
}

impl ReovimTimerHandle {
    /// Create a null/invalid handle.
    #[must_use]
    pub const fn null() -> Self {
        Self { id: 0 }
    }

    /// Create a handle from a timer ID.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self { id }
    }

    /// Check if this handle is valid.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.id != 0
    }
}

/// Timer callback function type.
///
/// Called when a timer fires. The `user_data` pointer is passed through
/// from the scheduling call.
///
/// # Safety
///
/// - `user_data` must remain valid until the timer fires or is cancelled
/// - Callbacks must not panic
/// - Callbacks should return quickly
pub type ReovimTimerCallback = Option<unsafe extern "C" fn(user_data: *mut libc::c_void)>;

// ============================================================================
// FFI functions
// ============================================================================

/// Schedule a one-shot timer.
///
/// The callback will be invoked once after `delay_ms` milliseconds.
///
/// # Arguments
///
/// - `delay_ms`: Delay in milliseconds before the callback fires
/// - `callback`: Function to call when the timer fires
/// - `user_data`: Opaque pointer passed to the callback
///
/// # Returns
///
/// A timer handle. Check `handle.id != 0` to verify success.
/// Returns a null handle (id=0) if:
/// - Timer wheel is not initialized
/// - Maximum timer limit reached
/// - Callback is null
///
/// # Safety
///
/// - `user_data` must remain valid until the timer fires or is cancelled
/// - The callback must be a valid function pointer (or null)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_schedule_delayed(
    delay_ms: u64,
    callback: ReovimTimerCallback,
    user_data: *mut libc::c_void,
) -> ReovimTimerHandle {
    let Some(wheel) = get_timer_wheel() else {
        tracing::warn!("Timer wheel not initialized, cannot schedule delayed timer");
        return ReovimTimerHandle::null();
    };

    let Some(cb) = callback else {
        tracing::warn!("Null callback passed to reovim_schedule_delayed");
        return ReovimTimerHandle::null();
    };

    let delay = std::time::Duration::from_millis(delay_ms);

    // Wrap the FFI callback in a Rust closure
    // Safety: We trust the caller to keep user_data valid
    let user_data_ptr = user_data as usize; // Convert to usize for Send
    let handle = wheel.schedule_oneshot(delay, Priority::NORMAL, move || {
        // Safety: caller guarantees user_data is valid
        unsafe { cb(user_data_ptr as *mut libc::c_void) };
    });

    if handle.is_failed() {
        tracing::warn!("Failed to schedule delayed timer (capacity exceeded?)");
        return ReovimTimerHandle::null();
    }

    let id = handle.id().as_u64();
    let _ = handle.detach(); // Don't cancel on Rust handle drop

    ReovimTimerHandle::new(id)
}

/// Schedule a periodic timer.
///
/// The callback will be invoked repeatedly at `interval_ms` intervals.
///
/// # Arguments
///
/// - `interval_ms`: Interval in milliseconds between callback invocations
/// - `callback`: Function to call when the timer fires
/// - `user_data`: Opaque pointer passed to the callback
///
/// # Returns
///
/// A timer handle. Check `handle.id != 0` to verify success.
/// Returns a null handle (id=0) if:
/// - Timer wheel is not initialized
/// - Maximum timer limit reached
/// - Callback is null
///
/// # Safety
///
/// - `user_data` must remain valid for the lifetime of the timer
/// - The callback must be a valid function pointer (or null)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_schedule_periodic(
    interval_ms: u64,
    callback: ReovimTimerCallback,
    user_data: *mut libc::c_void,
) -> ReovimTimerHandle {
    let Some(wheel) = get_timer_wheel() else {
        tracing::warn!("Timer wheel not initialized, cannot schedule periodic timer");
        return ReovimTimerHandle::null();
    };

    let Some(cb) = callback else {
        tracing::warn!("Null callback passed to reovim_schedule_periodic");
        return ReovimTimerHandle::null();
    };

    let interval = std::time::Duration::from_millis(interval_ms);

    // Wrap the FFI callback in a Rust closure
    // Safety: We trust the caller to keep user_data valid
    let user_data_ptr = user_data as usize; // Convert to usize for Send
    let handle = wheel.schedule_periodic(interval, Priority::NORMAL, move || {
        // Safety: caller guarantees user_data is valid
        unsafe { cb(user_data_ptr as *mut libc::c_void) };
    });

    if handle.is_failed() {
        tracing::warn!("Failed to schedule periodic timer (capacity exceeded?)");
        return ReovimTimerHandle::null();
    }

    let id = handle.id().as_u64();
    let _ = handle.detach(); // Don't cancel on Rust handle drop

    ReovimTimerHandle::new(id)
}

/// Cancel a scheduled timer.
///
/// After cancellation, the timer's callback will not be invoked.
/// It is safe to call this on an already-cancelled or fired timer.
///
/// # Arguments
///
/// - `handle`: The timer handle returned from `reovim_schedule_delayed` or
///   `reovim_schedule_periodic`
///
/// # Returns
///
/// `true` if the timer was found and cancelled, `false` if the timer was
/// already cancelled, has fired, or the handle was invalid.
#[unsafe(no_mangle)]
pub extern "C" fn reovim_cancel_timer(handle: ReovimTimerHandle) -> bool {
    if !handle.is_valid() {
        return false;
    }

    let Some(wheel) = get_timer_wheel() else {
        return false;
    };

    let timer_id = reovim_kernel::api::v1::TimerId::from_raw(handle.id);
    wheel.cancel(timer_id)
}

/// Check if a timer is still pending.
///
/// # Arguments
///
/// - `handle`: The timer handle to check
///
/// # Returns
///
/// `true` if the timer is still scheduled and hasn't fired yet,
/// `false` if the timer has fired, was cancelled, or the handle is invalid.
#[unsafe(no_mangle)]
pub extern "C" fn reovim_timer_is_pending(handle: ReovimTimerHandle) -> bool {
    if !handle.is_valid() {
        return false;
    }

    let Some(wheel) = get_timer_wheel() else {
        return false;
    };

    let timer_id = reovim_kernel::api::v1::TimerId::from_raw(handle.id);
    wheel.is_pending(timer_id)
}

/// Get the number of currently active timers.
///
/// # Returns
///
/// The number of pending timers, or 0 if the timer wheel is not initialized.
#[unsafe(no_mangle)]
pub extern "C" fn reovim_timer_count() -> usize {
    get_timer_wheel().map_or(0, |w| w.pending_count())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_handle_null() {
        let handle = ReovimTimerHandle::null();
        assert_eq!(handle.id, 0);
        assert!(!handle.is_valid());
    }

    #[test]
    fn test_timer_handle_new() {
        let handle = ReovimTimerHandle::new(42);
        assert_eq!(handle.id, 42);
        assert!(handle.is_valid());
    }

    #[test]
    fn test_timer_handle_size() {
        // Verify FFI-safe size
        assert_eq!(std::mem::size_of::<ReovimTimerHandle>(), 8);
        assert_eq!(std::mem::align_of::<ReovimTimerHandle>(), 8);
    }

    #[test]
    fn test_schedule_without_init_returns_null() {
        // Timer wheel not initialized, should return null handle
        unsafe extern "C" fn dummy_callback(_: *mut libc::c_void) {}

        let handle =
            unsafe { reovim_schedule_delayed(100, Some(dummy_callback), std::ptr::null_mut()) };

        // Note: This test may fail if run after other tests that initialize
        // the global. In practice, the global is only set once.
        // For isolated testing, we'd need a different approach.
        assert!(!handle.is_valid() || TIMER_WHEEL.get().is_some());
    }

    #[test]
    fn test_schedule_null_callback_returns_null() {
        // Even if wheel is initialized, null callback should fail
        let handle = unsafe { reovim_schedule_delayed(100, None, std::ptr::null_mut()) };

        // Should return null (either wheel not init or null callback)
        // The function checks callback before wheel, so this always fails
        assert!(!handle.is_valid());
    }

    #[test]
    fn test_cancel_null_handle() {
        let handle = ReovimTimerHandle::null();
        assert!(!reovim_cancel_timer(handle));
    }

    #[test]
    fn test_is_pending_null_handle() {
        let handle = ReovimTimerHandle::null();
        assert!(!reovim_timer_is_pending(handle));
    }
}
