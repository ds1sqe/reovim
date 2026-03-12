use {
    super::*,
    std::sync::atomic::{AtomicI32, Ordering as AOrdering},
};

// ========================================================================
// Subscription handle tests
// ========================================================================

#[test]
fn test_subscription_handle_null() {
    let h = ReovimSubscriptionHandle::null();
    assert!(h.is_null());
    assert_eq!(h.id, 0);
}

#[test]
fn test_subscription_handle_non_null() {
    let h = ReovimSubscriptionHandle { id: 42 };
    assert!(!h.is_null());
}

#[test]
fn test_subscription_handle_clone_copy() {
    let h = ReovimSubscriptionHandle { id: 1 };
    let copied = h;
    assert_eq!(h, copied);
}

#[test]
fn test_subscription_handle_debug() {
    let h = ReovimSubscriptionHandle { id: 5 };
    let debug = format!("{h:?}");
    assert!(debug.contains('5'));
}

// ========================================================================
// FfiEventSubscriptionStore tests
// ========================================================================

#[test]
fn test_store_empty() {
    let store = FfiEventSubscriptionStore::default();
    assert_eq!(store.count(), 0);
}

#[test]
fn test_store_subscribe_and_count() {
    unsafe extern "C" fn noop(_: *mut c_void, _: *const c_char, _: *const c_void, _: u32) {}

    let store = FfiEventSubscriptionStore::default();
    let h1 = store.subscribe("buffer:changed".to_string(), noop, std::ptr::null_mut());
    assert!(!h1.is_null());
    assert_eq!(store.count(), 1);

    let h2 = store.subscribe("mode:changed".to_string(), noop, std::ptr::null_mut());
    assert!(!h2.is_null());
    assert_ne!(h1.id, h2.id);
    assert_eq!(store.count(), 2);
}

#[test]
fn test_store_unsubscribe() {
    unsafe extern "C" fn noop(_: *mut c_void, _: *const c_char, _: *const c_void, _: u32) {}

    let store = FfiEventSubscriptionStore::default();
    let h = store.subscribe("test".to_string(), noop, std::ptr::null_mut());
    assert_eq!(store.count(), 1);

    assert!(store.unsubscribe(h));
    assert_eq!(store.count(), 0);

    // Second unsubscribe should return false
    assert!(!store.unsubscribe(h));
}

#[test]
fn test_store_unsubscribe_null_handle() {
    let store = FfiEventSubscriptionStore::default();
    assert!(!store.unsubscribe(ReovimSubscriptionHandle::null()));
}

#[test]
fn test_store_dispatch_calls_matching() {
    static CALL_COUNT: AtomicI32 = AtomicI32::new(0);

    unsafe extern "C" fn counter(_: *mut c_void, _: *const c_char, _: *const c_void, _: u32) {
        CALL_COUNT.fetch_add(1, AOrdering::SeqCst);
    }

    CALL_COUNT.store(0, AOrdering::SeqCst);

    let store = FfiEventSubscriptionStore::default();
    store.subscribe("test:event".to_string(), counter, std::ptr::null_mut());
    store.subscribe("other:event".to_string(), counter, std::ptr::null_mut());

    store.dispatch("test:event");
    assert_eq!(CALL_COUNT.load(AOrdering::SeqCst), 1);

    store.dispatch("other:event");
    assert_eq!(CALL_COUNT.load(AOrdering::SeqCst), 2);

    // No match
    store.dispatch("unknown:event");
    assert_eq!(CALL_COUNT.load(AOrdering::SeqCst), 2);
}

#[test]
fn test_store_dispatch_with_user_data() {
    unsafe extern "C" fn increment(
        user_data: *mut c_void,
        _: *const c_char,
        _: *const c_void,
        _: u32,
    ) {
        let counter = unsafe { &*(user_data.cast::<AtomicI32>()) };
        counter.fetch_add(1, AOrdering::SeqCst);
    }

    let counter = AtomicI32::new(0);
    let store = FfiEventSubscriptionStore::default();
    store.subscribe(
        "test".to_string(),
        increment,
        std::ptr::from_ref(&counter).cast_mut().cast(),
    );

    store.dispatch("test");
    assert_eq!(counter.load(AOrdering::SeqCst), 1);

    store.dispatch("test");
    assert_eq!(counter.load(AOrdering::SeqCst), 2);
}

#[test]
fn test_store_dispatch_after_unsubscribe() {
    static CALL_COUNT2: AtomicI32 = AtomicI32::new(0);

    unsafe extern "C" fn counter(_: *mut c_void, _: *const c_char, _: *const c_void, _: u32) {
        CALL_COUNT2.fetch_add(1, AOrdering::SeqCst);
    }

    CALL_COUNT2.store(0, AOrdering::SeqCst);

    let store = FfiEventSubscriptionStore::default();
    let h = store.subscribe("test".to_string(), counter, std::ptr::null_mut());

    store.dispatch("test");
    assert_eq!(CALL_COUNT2.load(AOrdering::SeqCst), 1);

    store.unsubscribe(h);
    store.dispatch("test");
    assert_eq!(CALL_COUNT2.load(AOrdering::SeqCst), 1); // no change
}

// ========================================================================
// FFI function tests
// ========================================================================

#[test]
fn test_subscribe_event_null_type() {
    let h = unsafe { reovim_subscribe_event(std::ptr::null(), None, std::ptr::null_mut(), 0) };
    assert!(h.is_null());
}

#[test]
fn test_subscribe_event_null_callback() {
    let event_type = std::ffi::CString::new("test").unwrap();
    let h =
        unsafe { reovim_subscribe_event(event_type.as_ptr(), None, std::ptr::null_mut(), 0) };
    assert!(h.is_null());
}

#[test]
fn test_subscribe_event_no_init_ctx() {
    unsafe extern "C" fn noop(_: *mut c_void, _: *const c_char, _: *const c_void, _: u32) {}
    let event_type = std::ffi::CString::new("test").unwrap();
    let h = unsafe {
        reovim_subscribe_event(event_type.as_ptr(), Some(noop), std::ptr::null_mut(), 0)
    };
    assert!(h.is_null());
}

#[test]
fn test_subscribe_event_invalid_utf8() {
    unsafe extern "C" fn noop(_: *mut c_void, _: *const c_char, _: *const c_void, _: u32) {}
    let invalid_bytes: &[u8] = &[0x80, 0x00];
    let h = unsafe {
        reovim_subscribe_event(
            invalid_bytes.as_ptr().cast(),
            Some(noop),
            std::ptr::null_mut(),
            0,
        )
    };
    assert!(h.is_null());
}

#[test]
fn test_subscribe_event_with_init_guard() {
    use reovim_kernel::api::v1::ServiceRegistry;

    unsafe extern "C" fn noop(_: *mut c_void, _: *const c_char, _: *const c_void, _: u32) {}

    let registry = ServiceRegistry::new();
    let _guard = unsafe { crate::runtime::InitGuard::new(&registry) };

    let event_type = std::ffi::CString::new("buffer:changed").unwrap();
    let h = unsafe {
        reovim_subscribe_event(event_type.as_ptr(), Some(noop), std::ptr::null_mut(), 100)
    };
    assert!(!h.is_null());

    let store = registry
        .get::<FfiEventSubscriptionStore>()
        .expect("store should exist");
    assert_eq!(store.count(), 1);
}

#[test]
fn test_unsubscribe_event_null_handle() {
    let result = unsafe { reovim_unsubscribe_event(ReovimSubscriptionHandle::null()) };
    assert_eq!(result, REOVIM_ERR_NOT_FOUND);
}

#[test]
fn test_unsubscribe_event_no_init_ctx() {
    let result = unsafe { reovim_unsubscribe_event(ReovimSubscriptionHandle { id: 1 }) };
    assert_eq!(result, REOVIM_ERR_NO_INIT_CTX);
}

#[test]
fn test_subscribe_and_unsubscribe_roundtrip() {
    use reovim_kernel::api::v1::ServiceRegistry;

    unsafe extern "C" fn noop(_: *mut c_void, _: *const c_char, _: *const c_void, _: u32) {}

    let registry = ServiceRegistry::new();
    let _guard = unsafe { crate::runtime::InitGuard::new(&registry) };

    let event_type = std::ffi::CString::new("test:event").unwrap();
    let h = unsafe {
        reovim_subscribe_event(event_type.as_ptr(), Some(noop), std::ptr::null_mut(), 0)
    };
    assert!(!h.is_null());

    let result = unsafe { reovim_unsubscribe_event(h) };
    assert_eq!(result, REOVIM_OK);

    // Already unsubscribed
    let result = unsafe { reovim_unsubscribe_event(h) };
    assert_eq!(result, REOVIM_ERR_NOT_FOUND);
}
