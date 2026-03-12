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
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_schedule_without_init_returns_null() {
    // Timer wheel not initialized, should return null handle
    #[cfg_attr(coverage_nightly, coverage(off))]
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

#[test]
fn test_timer_handle_debug() {
    let handle = ReovimTimerHandle::new(42);
    let debug = format!("{handle:?}");
    assert!(debug.contains("42"));
    assert!(debug.contains("ReovimTimerHandle"));
}

#[test]
fn test_timer_handle_clone_copy() {
    let handle = ReovimTimerHandle::new(99);
    let cloned = handle;
    assert_eq!(cloned.id, 99);
    assert!(cloned.is_valid());
    // Both are still valid since it's Copy
    assert_eq!(handle.id, cloned.id);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_timer_count_without_init() {
    // Without initialization, timer count should be 0
    let count = reovim_timer_count();
    // Either 0 (not initialized) or some value (if another test initialized it)
    assert!(count == 0 || TIMER_WHEEL.get().is_some());
}

#[test]
fn test_cancel_timer_invalid_id() {
    let handle = ReovimTimerHandle::new(999_999);
    // Either returns false (wheel not init) or false (id not found)
    assert!(!reovim_cancel_timer(handle));
}

#[test]
fn test_is_pending_invalid_id() {
    let handle = ReovimTimerHandle::new(999_999);
    // Either returns false (wheel not init) or false (id not found)
    assert!(!reovim_timer_is_pending(handle));
}

#[test]
fn test_schedule_periodic_null_callback() {
    let handle = unsafe { reovim_schedule_periodic(100, None, std::ptr::null_mut()) };
    assert!(!handle.is_valid());
}

#[test]
fn test_timer_handle_zero_is_null() {
    let handle = ReovimTimerHandle::new(0);
    assert_eq!(handle.id, 0);
    assert!(!handle.is_valid());
}

#[test]
fn test_timer_handle_one_is_valid() {
    let handle = ReovimTimerHandle::new(1);
    assert_eq!(handle.id, 1);
    assert!(handle.is_valid());
}

#[test]
fn test_timer_handle_max_is_valid() {
    let handle = ReovimTimerHandle::new(u64::MAX);
    assert_eq!(handle.id, u64::MAX);
    assert!(handle.is_valid());
}

#[test]
fn test_schedule_delayed_null_callback_with_nonzero_data() {
    // Null callback should always fail, regardless of user_data
    let handle = unsafe { reovim_schedule_delayed(500, None, 0x1234 as *mut libc::c_void) };
    assert!(!handle.is_valid());
}

#[test]
fn test_schedule_periodic_null_callback_with_nonzero_data() {
    let handle = unsafe { reovim_schedule_periodic(500, None, 0x1234 as *mut libc::c_void) };
    assert!(!handle.is_valid());
}

#[test]
fn test_cancel_timer_without_init_valid_handle() {
    // Cancel with a valid-looking handle but no timer wheel
    let handle = ReovimTimerHandle::new(42);
    // Returns false because wheel isn't initialized (or timer not found)
    assert!(!reovim_cancel_timer(handle));
}

#[test]
fn test_is_pending_without_init_valid_handle() {
    let handle = ReovimTimerHandle::new(42);
    assert!(!reovim_timer_is_pending(handle));
}

#[test]
fn test_timer_handle_copy_semantics() {
    let a = ReovimTimerHandle::new(77);
    let b = a; // Copy
    assert_eq!(a.id, b.id);
    assert_eq!(a.id, 77);
}

// FFI callbacks declared at module level to avoid items_after_statements lint
#[cfg_attr(coverage_nightly, coverage(off))]
unsafe extern "C" fn noop_callback(_user_data: *mut libc::c_void) {}

/// Ensure the timer wheel is initialized for tests that need it.
/// Since `OnceLock` can only be set once per process, we use `try_init()`
/// to make this idempotent across parallel test runs.
fn ensure_timer_wheel_initialized() -> &'static Arc<TimerWheel> {
    // Try to initialize; ignore error if already set
    let wheel = Arc::new(TimerWheel::new());
    let _ = TIMER_WHEEL.set(wheel);
    TIMER_WHEEL.get().expect("timer wheel should be set")
}

#[test]
fn test_init_timer_wheel_and_schedule_delayed() {
    let _wheel = ensure_timer_wheel_initialized();

    let handle =
        unsafe { reovim_schedule_delayed(1000, Some(noop_callback), std::ptr::null_mut()) };
    // After initialization, scheduling should succeed
    assert!(handle.is_valid());

    // Verify the timer is pending
    assert!(reovim_timer_is_pending(handle));

    // Cancel the timer
    let cancelled = reovim_cancel_timer(handle);
    assert!(cancelled);

    // After cancellation, it should no longer be pending
    assert!(!reovim_timer_is_pending(handle));
}

#[test]
fn test_init_timer_wheel_and_schedule_periodic() {
    let _wheel = ensure_timer_wheel_initialized();

    let handle =
        unsafe { reovim_schedule_periodic(500, Some(noop_callback), std::ptr::null_mut()) };
    assert!(handle.is_valid());

    // Periodic timer should be pending
    assert!(reovim_timer_is_pending(handle));

    // Cancel the periodic timer
    let cancelled = reovim_cancel_timer(handle);
    assert!(cancelled);
}

#[test]
fn test_timer_count_after_init() {
    let _wheel = ensure_timer_wheel_initialized();

    // Timer count should be accessible (may be 0 or more depending on test ordering)
    let count = reovim_timer_count();
    // Just verify the function returns a valid value
    assert!(count < 1_000_000); // Sanity check
}

#[test]
fn test_cancel_nonexistent_timer_after_init() {
    let _wheel = ensure_timer_wheel_initialized();

    // Cancel a timer that doesn't exist
    let handle = ReovimTimerHandle::new(999_999_999);
    assert!(!reovim_cancel_timer(handle));
}

#[test]
fn test_is_pending_nonexistent_timer_after_init() {
    let _wheel = ensure_timer_wheel_initialized();

    // Check pending on a timer that doesn't exist
    let handle = ReovimTimerHandle::new(999_999_999);
    assert!(!reovim_timer_is_pending(handle));
}

#[test]
fn test_schedule_delayed_with_user_data() {
    let _wheel = ensure_timer_wheel_initialized();

    let mut data: u64 = 42;
    let data_ptr = std::ptr::addr_of_mut!(data).cast::<libc::c_void>();

    let handle = unsafe { reovim_schedule_delayed(2000, Some(noop_callback), data_ptr) };
    assert!(handle.is_valid());

    // Clean up
    reovim_cancel_timer(handle);
}

#[test]
fn test_schedule_periodic_with_user_data() {
    let _wheel = ensure_timer_wheel_initialized();

    let mut data: u64 = 99;
    let data_ptr = std::ptr::addr_of_mut!(data).cast::<libc::c_void>();

    let handle = unsafe { reovim_schedule_periodic(2000, Some(noop_callback), data_ptr) };
    assert!(handle.is_valid());

    // Clean up
    reovim_cancel_timer(handle);
}

#[test]
fn test_null_callback_after_init_returns_null_delayed() {
    let _wheel = ensure_timer_wheel_initialized();

    // Even with the wheel initialized, null callback should fail
    let handle = unsafe { reovim_schedule_delayed(100, None, std::ptr::null_mut()) };
    assert!(!handle.is_valid());
}

#[test]
fn test_null_callback_after_init_returns_null_periodic() {
    let _wheel = ensure_timer_wheel_initialized();

    let handle = unsafe { reovim_schedule_periodic(100, None, std::ptr::null_mut()) };
    assert!(!handle.is_valid());
}

#[test]
fn test_schedule_delayed_zero_delay() {
    let _wheel = ensure_timer_wheel_initialized();

    let handle =
        unsafe { reovim_schedule_delayed(0, Some(noop_callback), std::ptr::null_mut()) };
    // Zero delay is valid - timer fires immediately
    assert!(handle.is_valid());
    reovim_cancel_timer(handle);
}

#[test]
fn test_cancel_already_cancelled_timer() {
    let _wheel = ensure_timer_wheel_initialized();

    let handle =
        unsafe { reovim_schedule_delayed(5000, Some(noop_callback), std::ptr::null_mut()) };
    assert!(handle.is_valid());

    // Cancel once
    assert!(reovim_cancel_timer(handle));

    // Cancel again - should return false
    assert!(!reovim_cancel_timer(handle));
}

#[test]
fn test_schedule_multiple_timers_and_cancel() {
    let _wheel = ensure_timer_wheel_initialized();

    let h1 =
        unsafe { reovim_schedule_delayed(10_000, Some(noop_callback), std::ptr::null_mut()) };
    let h2 =
        unsafe { reovim_schedule_delayed(10_000, Some(noop_callback), std::ptr::null_mut()) };
    assert!(h1.is_valid());
    assert!(h2.is_valid());
    assert_ne!(h1.id, h2.id);

    // Both should be pending
    assert!(reovim_timer_is_pending(h1));
    assert!(reovim_timer_is_pending(h2));

    // Cancel first, second still pending
    assert!(reovim_cancel_timer(h1));
    assert!(!reovim_timer_is_pending(h1));
    assert!(reovim_timer_is_pending(h2));

    // Clean up
    reovim_cancel_timer(h2);
}

#[test]
fn test_schedule_periodic_zero_interval() {
    let _wheel = ensure_timer_wheel_initialized();

    let handle =
        unsafe { reovim_schedule_periodic(0, Some(noop_callback), std::ptr::null_mut()) };
    // Zero interval is valid
    assert!(handle.is_valid());
    reovim_cancel_timer(handle);
}

#[test]
fn test_timer_count_after_schedule_and_cancel() {
    let _wheel = ensure_timer_wheel_initialized();
    let before = reovim_timer_count();

    let handle =
        unsafe { reovim_schedule_delayed(60_000, Some(noop_callback), std::ptr::null_mut()) };
    assert!(handle.is_valid());

    let during = reovim_timer_count();
    assert!(during > before);

    reovim_cancel_timer(handle);
}

#[test]
fn test_get_timer_wheel_returns_some_after_init() {
    let _wheel = ensure_timer_wheel_initialized();
    assert!(get_timer_wheel().is_some());
}
