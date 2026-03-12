use {
    super::*,
    std::{thread, time::Duration},
};

// ========== ScopeId tests ==========

#[test]
fn test_scope_id_unique() {
    let id1 = ScopeId::new();
    let id2 = ScopeId::new();
    assert_ne!(id1, id2);
}

#[test]
fn test_scope_id_display() {
    let id = ScopeId::new();
    let display = format!("{id}");
    assert!(display.starts_with("scope-"));
}

#[test]
fn test_scope_id_as_u64() {
    let id = ScopeId::new();
    assert!(id.as_u64() > 0);
}

// ========== EventScope basic tests ==========

#[test]
fn test_event_scope_new() {
    let scope = EventScope::new();
    assert_eq!(scope.in_flight(), 0);
    assert!(scope.is_complete());
}

#[test]
fn test_event_scope_increment_decrement() {
    let scope = EventScope::new();

    scope.increment();
    assert_eq!(scope.in_flight(), 1);
    assert!(!scope.is_complete());

    scope.increment();
    assert_eq!(scope.in_flight(), 2);

    scope.decrement();
    assert_eq!(scope.in_flight(), 1);

    scope.decrement();
    assert_eq!(scope.in_flight(), 0);
    assert!(scope.is_complete());
}

#[test]
fn test_event_scope_clone_shares_state() {
    let scope1 = EventScope::new();
    let scope2 = scope1.clone();

    scope1.increment();
    assert_eq!(scope2.in_flight(), 1);

    scope2.increment();
    assert_eq!(scope1.in_flight(), 2);

    scope1.decrement();
    scope1.decrement();
    assert!(scope2.is_complete());
}

#[test]
fn test_event_scope_unique_ids() {
    let scope1 = EventScope::new();
    let scope2 = EventScope::new();
    assert_ne!(scope1.id(), scope2.id());
}

#[test]
fn test_event_scope_clone_same_id() {
    let scope1 = EventScope::new();
    let scope2 = scope1.clone();
    assert_eq!(scope1.id(), scope2.id());
}

// ========== EventScope wait tests ==========

#[test]
fn test_event_scope_wait_immediate() {
    let scope = EventScope::new();
    scope.wait();
    assert!(scope.is_complete());
}

#[test]
fn test_event_scope_wait_timeout_immediate() {
    let scope = EventScope::new();
    let completed = scope.wait_timeout(Duration::from_millis(100));
    assert!(completed);
}

#[test]
fn test_event_scope_wait_timeout_expires() {
    let scope = EventScope::new();
    scope.increment();

    let completed = scope.wait_timeout(Duration::from_millis(10));
    assert!(!completed);
    assert_eq!(scope.in_flight(), 1);
}

#[test]
fn test_event_scope_wait_timeout_completes() {
    let scope = EventScope::new();
    let scope2 = scope.clone();

    scope.increment();

    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        scope2.decrement();
    });

    let completed = scope.wait_timeout(Duration::from_secs(1));
    assert!(completed);
    handle.join().unwrap();
}

#[test]
fn test_event_scope_wait_with_thread() {
    let scope = EventScope::new();
    let scope2 = scope.clone();

    scope.increment();

    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(5));
        scope2.decrement();
    });

    scope.wait();
    assert!(scope.is_complete());
    handle.join().unwrap();
}

// ========== EventScope concurrent tests ==========

#[test]
fn test_event_scope_concurrent_increments() {
    let scope = EventScope::new();
    let mut handles = vec![];

    for _ in 0..10 {
        let s = scope.clone();
        handles.push(thread::spawn(move || {
            s.increment();
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(scope.in_flight(), 10);
}

#[test]
fn test_event_scope_concurrent_increment_decrement() {
    let scope = EventScope::new();
    let mut handles = vec![];

    // First, increment 10 times
    for _ in 0..10 {
        scope.increment();
    }

    // Then decrement from multiple threads
    for _ in 0..10 {
        let s = scope.clone();
        handles.push(thread::spawn(move || {
            s.decrement();
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert!(scope.is_complete());
}

#[test]
fn test_event_scope_multiple_waiters() {
    let scope = EventScope::new();
    scope.increment();

    let mut handles = vec![];

    // Start multiple waiters
    for _ in 0..3 {
        let s = scope.clone();
        handles.push(thread::spawn(move || {
            let completed = s.wait_timeout(Duration::from_secs(1));
            assert!(completed);
        }));
    }

    // Small delay then decrement
    thread::sleep(Duration::from_millis(10));
    scope.decrement();

    for h in handles {
        h.join().unwrap();
    }
}

// ========== Debug and Default tests ==========

#[test]
fn test_event_scope_debug() {
    let scope = EventScope::new();
    scope.increment();
    let debug_str = format!("{scope:?}");
    assert!(debug_str.contains("EventScope"));
    assert!(debug_str.contains("in_flight: 1"));
}

#[test]
fn test_event_scope_default() {
    let scope = EventScope::default();
    assert!(scope.is_complete());
}

#[test]
fn test_event_scope_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EventScope>();
}

#[test]
fn test_wait_with_default_timeout_immediate() {
    let scope = EventScope::new();
    assert!(scope.wait_with_default_timeout());
}

#[test]
fn test_wait_timeout_zero_with_in_flight() {
    let scope = EventScope::new();
    scope.increment();

    let completed = scope.wait_timeout(Duration::ZERO);
    assert!(!completed);
    assert_eq!(scope.in_flight(), 1);
}
