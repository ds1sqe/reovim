
use super::*;


#[test]
fn new_queue_is_empty() {
    let queue = PendingNotificationQueue::new();
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
}

#[test]
fn default_queue_is_empty() {
    let queue = PendingNotificationQueue::default();
    assert!(queue.is_empty());
}

#[test]
fn push_and_drain() {
    let queue = PendingNotificationQueue::new();
    queue.push(PendingLevel::Info, "hello");
    queue.push(PendingLevel::Success, "world");

    assert_eq!(queue.len(), 2);
    assert!(!queue.is_empty());

    let items = queue.drain();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].level, PendingLevel::Info);
    assert_eq!(items[0].title, "hello");
    assert_eq!(items[1].level, PendingLevel::Success);
    assert_eq!(items[1].title, "world");

    // Queue is empty after drain.
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
}

#[test]
fn drain_empty_returns_empty() {
    let queue = PendingNotificationQueue::new();
    let items = queue.drain();
    assert!(items.is_empty());
}

#[test]
fn drain_clears_queue() {
    let queue = PendingNotificationQueue::new();
    queue.push(PendingLevel::Warning, "first");
    let _ = queue.drain();
    queue.push(PendingLevel::Error, "second");
    let items = queue.drain();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "second");
}

#[test]
fn concurrent_push() {
    use std::sync::Arc;

    let queue = Arc::new(PendingNotificationQueue::new());
    let mut handles = Vec::new();

    for i in 0..10 {
        let q = Arc::clone(&queue);
        handles.push(std::thread::spawn(move || {
            q.push(PendingLevel::Info, format!("msg-{i}"));
        }));
    }

    for h in handles {
        h.join().expect("Thread panicked");
    }

    assert_eq!(queue.len(), 10);
    let items = queue.drain();
    assert_eq!(items.len(), 10);
    assert!(queue.is_empty());
}

#[test]
fn all_levels() {
    let queue = PendingNotificationQueue::new();
    queue.push(PendingLevel::Info, "i");
    queue.push(PendingLevel::Success, "s");
    queue.push(PendingLevel::Warning, "w");
    queue.push(PendingLevel::Error, "e");

    let items = queue.drain();
    assert_eq!(items[0].level, PendingLevel::Info);
    assert_eq!(items[1].level, PendingLevel::Success);
    assert_eq!(items[2].level, PendingLevel::Warning);
    assert_eq!(items[3].level, PendingLevel::Error);
}

#[test]
fn debug_impls() {
    let queue = PendingNotificationQueue::new();
    let debug = format!("{queue:?}");
    assert!(debug.contains("PendingNotificationQueue"));

    let level = PendingLevel::Warning;
    let debug = format!("{level:?}");
    assert_eq!(debug, "Warning");

    let notif = PendingNotification {
        level: PendingLevel::Error,
        title: "test".to_owned(),
    };
    let debug = format!("{notif:?}");
    assert!(debug.contains("PendingNotification"));
}

#[test]
fn level_clone_copy_eq() {
    let level = PendingLevel::Success;
    let copied = level;
    #[allow(clippy::clone_on_copy)]
    let cloned = level.clone();
    assert_eq!(level, copied);
    assert_eq!(level, cloned);
    assert_ne!(PendingLevel::Info, PendingLevel::Error);
}

#[test]
fn notification_clone_eq() {
    let notif = PendingNotification {
        level: PendingLevel::Info,
        title: "test".to_owned(),
    };
    let cloned = notif.clone();
    assert_eq!(notif, cloned);
}

#[test]
fn new_and_default_equivalent() {
    let new = PendingNotificationQueue::new();
    let default = PendingNotificationQueue::default();
    assert!(new.is_empty());
    assert!(default.is_empty());
    assert_eq!(new.len(), default.len());
}

#[test]
fn len_tracks_pushes() {
    let queue = PendingNotificationQueue::new();
    assert_eq!(queue.len(), 0);
    queue.push(PendingLevel::Info, "one");
    assert_eq!(queue.len(), 1);
    queue.push(PendingLevel::Info, "two");
    assert_eq!(queue.len(), 2);
    queue.push(PendingLevel::Info, "three");
    assert_eq!(queue.len(), 3);
}

#[test]
fn service_impl() {
    use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

    let registry = ServiceRegistry::new();
    let queue = Arc::new(PendingNotificationQueue::new());
    registry.register(queue);

    let retrieved = registry.get::<PendingNotificationQueue>();
    assert!(retrieved.is_some());
}