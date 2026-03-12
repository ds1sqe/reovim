use {super::*, std::sync::atomic::AtomicBool};

use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

// ========== SubscriptionId tests ==========

#[test]
fn test_subscription_id_unique() {
    let id1 = SubscriptionId::new();
    let id2 = SubscriptionId::new();
    assert_ne!(id1, id2);
}

#[test]
fn test_subscription_id_display() {
    let id = SubscriptionId::new();
    let display = format!("{id}");
    assert!(display.starts_with("sub-"));
}

#[test]
fn test_subscription_id_as_u64() {
    let id = SubscriptionId::new();
    assert!(id.as_u64() > 0);
}

// ========== Subscription tests ==========

#[test]
fn test_subscription_new() {
    let called = Arc::new(AtomicBool::new(false));
    let called2 = called.clone();

    let sub = Subscription::new::<i32>(SubscriptionId::new(), move || {
        called2.store(true, Ordering::SeqCst);
    });

    assert!(sub.is_active());
    assert!(!called.load(Ordering::SeqCst));
}

#[test]
fn test_subscription_id() {
    let id = SubscriptionId::new();
    let sub = Subscription::new::<i32>(id, || {});
    assert_eq!(sub.id(), id);
}

#[test]
fn test_subscription_type_info() {
    use std::any::TypeId;

    let sub = Subscription::new::<String>(SubscriptionId::new(), || {});
    assert_eq!(sub.type_id(), TypeId::of::<String>());
    assert!(sub.type_name().contains("String"));
}

#[test]
fn test_subscription_cancel() {
    let called = Arc::new(AtomicBool::new(false));
    let called2 = called.clone();

    let sub = Subscription::new::<i32>(SubscriptionId::new(), move || {
        called2.store(true, Ordering::SeqCst);
    });

    assert!(sub.is_active());
    sub.cancel();
    assert!(!sub.is_active());
    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn test_subscription_cancel_idempotent() {
    let call_count = Arc::new(AtomicU64::new(0));
    let call_count2 = call_count.clone();

    let sub = Subscription::new::<i32>(SubscriptionId::new(), move || {
        call_count2.fetch_add(1, Ordering::SeqCst);
    });

    sub.cancel();
    sub.cancel();
    sub.cancel();

    // Should only be called once
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_subscription_drop_calls_unsubscribe() {
    let called = Arc::new(AtomicBool::new(false));
    let called2 = called.clone();

    {
        let _sub = Subscription::new::<i32>(SubscriptionId::new(), move || {
            called2.store(true, Ordering::SeqCst);
        });
        assert!(!called.load(Ordering::SeqCst));
    } // Dropped here

    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn test_subscription_cancel_then_drop() {
    let call_count = Arc::new(AtomicU64::new(0));
    let call_count2 = call_count.clone();

    {
        let sub = Subscription::new::<i32>(SubscriptionId::new(), move || {
            call_count2.fetch_add(1, Ordering::SeqCst);
        });

        sub.cancel(); // Called once here
    } // Drop should not call again

    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_subscription_debug() {
    let sub = Subscription::new::<String>(SubscriptionId::new(), || {});
    let debug_str = format!("{sub:?}");
    assert!(debug_str.contains("Subscription"));
    assert!(debug_str.contains("String"));
    assert!(debug_str.contains("active: true"));
}

#[test]
fn test_subscription_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Subscription>();
}

#[test]
fn test_subscription_move_between_threads() {
    let called = Arc::new(AtomicBool::new(false));
    let called2 = called.clone();

    let sub = Subscription::new::<i32>(SubscriptionId::new(), move || {
        called2.store(true, Ordering::SeqCst);
    });

    std::thread::spawn(move || {
        drop(sub);
    })
    .join()
    .unwrap();

    assert!(called.load(Ordering::SeqCst));
}
