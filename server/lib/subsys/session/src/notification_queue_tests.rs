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
    assert!(items[0].source.is_none());
    assert_eq!(
        items[0].op,
        PendingOp::Push {
            level: PendingLevel::Info,
            title: "hello".to_owned(),
        }
    );
    assert_eq!(
        items[1].op,
        PendingOp::Push {
            level: PendingLevel::Success,
            title: "world".to_owned(),
        }
    );

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
    assert_eq!(
        items[0].op,
        PendingOp::Push {
            level: PendingLevel::Error,
            title: "second".to_owned(),
        }
    );
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
    let levels: Vec<_> = items
        .iter()
        .map(|e| match &e.op {
            PendingOp::Push { level, .. } => *level,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(
        levels,
        [
            PendingLevel::Info,
            PendingLevel::Success,
            PendingLevel::Warning,
            PendingLevel::Error,
        ]
    );
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

// ========================================================================
// PendingOp and PendingEntry tests (#691)
// ========================================================================

#[test]
fn push_op_with_source() {
    let queue = PendingNotificationQueue::new();
    queue.push_op(
        Some("rust-analyzer".to_owned()),
        PendingOp::Push {
            level: PendingLevel::Info,
            title: "Server ready".to_owned(),
        },
    );

    let items = queue.drain();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].source.as_deref(), Some("rust-analyzer"));
    assert_eq!(
        items[0].op,
        PendingOp::Push {
            level: PendingLevel::Info,
            title: "Server ready".to_owned(),
        }
    );
}

#[test]
fn push_op_progress_begin() {
    let queue = PendingNotificationQueue::new();
    queue.push_op(
        Some("rust-analyzer".to_owned()),
        PendingOp::ProgressBegin {
            token: "token-1".to_owned(),
            title: "Indexing".to_owned(),
            message: "crate foo".to_owned(),
            percentage: 0,
        },
    );

    let items = queue.drain();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].source.as_deref(), Some("rust-analyzer"));
    assert_eq!(
        items[0].op,
        PendingOp::ProgressBegin {
            token: "token-1".to_owned(),
            title: "Indexing".to_owned(),
            message: "crate foo".to_owned(),
            percentage: 0,
        }
    );
}

#[test]
fn push_op_progress_report() {
    let queue = PendingNotificationQueue::new();
    queue.push_op(
        Some("rust-analyzer".to_owned()),
        PendingOp::ProgressReport {
            token: "token-1".to_owned(),
            message: Some("3/10 crates".to_owned()),
            percentage: Some(30),
        },
    );

    let items = queue.drain();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].op,
        PendingOp::ProgressReport {
            token: "token-1".to_owned(),
            message: Some("3/10 crates".to_owned()),
            percentage: Some(30),
        }
    );
}

#[test]
fn push_op_progress_end() {
    let queue = PendingNotificationQueue::new();
    queue.push_op(
        None,
        PendingOp::ProgressEnd {
            token: "token-1".to_owned(),
            message: Some("Done".to_owned()),
        },
    );

    let items = queue.drain();
    assert_eq!(items.len(), 1);
    assert!(items[0].source.is_none());
    assert_eq!(
        items[0].op,
        PendingOp::ProgressEnd {
            token: "token-1".to_owned(),
            message: Some("Done".to_owned()),
        }
    );
}

#[test]
fn mixed_ops_drain_in_order() {
    let queue = PendingNotificationQueue::new();
    queue.push(PendingLevel::Info, "plain");
    queue.push_op(
        Some("lsp".to_owned()),
        PendingOp::ProgressBegin {
            token: "t1".to_owned(),
            title: "Indexing".to_owned(),
            message: String::new(),
            percentage: 0,
        },
    );
    queue.push_op(
        Some("lsp".to_owned()),
        PendingOp::ProgressReport {
            token: "t1".to_owned(),
            message: None,
            percentage: Some(50),
        },
    );
    queue.push_op(
        Some("lsp".to_owned()),
        PendingOp::ProgressEnd {
            token: "t1".to_owned(),
            message: None,
        },
    );

    let items = queue.drain();
    assert_eq!(items.len(), 4);
    assert!(matches!(items[0].op, PendingOp::Push { .. }));
    assert!(matches!(items[1].op, PendingOp::ProgressBegin { .. }));
    assert!(matches!(items[2].op, PendingOp::ProgressReport { .. }));
    assert!(matches!(items[3].op, PendingOp::ProgressEnd { .. }));
}

#[test]
fn concurrent_push_op() {
    use std::sync::Arc;

    let queue = Arc::new(PendingNotificationQueue::new());
    let mut handles = Vec::new();

    for i in 0..5 {
        let q = Arc::clone(&queue);
        handles.push(std::thread::spawn(move || {
            q.push_op(
                Some(format!("source-{i}")),
                PendingOp::Push {
                    level: PendingLevel::Info,
                    title: format!("msg-{i}"),
                },
            );
        }));
    }

    for h in handles {
        h.join().expect("Thread panicked");
    }

    assert_eq!(queue.len(), 5);
    let items = queue.drain();
    assert_eq!(items.len(), 5);
    assert!(items.iter().all(|e| e.source.is_some()));
}

#[test]
fn pending_op_debug_clone_eq() {
    let op = PendingOp::ProgressBegin {
        token: "t".to_owned(),
        title: "x".to_owned(),
        message: String::new(),
        percentage: 42,
    };
    let cloned = op.clone();
    assert_eq!(op, cloned);

    let debug = format!("{op:?}");
    assert!(debug.contains("ProgressBegin"));
}

#[test]
fn pending_entry_debug_clone_eq() {
    let entry = PendingEntry {
        source: Some("test".to_owned()),
        op: PendingOp::Push {
            level: PendingLevel::Success,
            title: "ok".to_owned(),
        },
    };
    let cloned = entry.clone();
    assert_eq!(entry, cloned);

    let debug = format!("{entry:?}");
    assert!(debug.contains("PendingEntry"));
    assert!(debug.contains("test"));
}

#[test]
fn push_convenience_has_no_source() {
    let queue = PendingNotificationQueue::new();
    queue.push(PendingLevel::Warning, "test");

    let items = queue.drain();
    assert_eq!(items.len(), 1);
    assert!(items[0].source.is_none());
    assert!(matches!(items[0].op, PendingOp::Push { .. }));
}
