use super::*;

#[test]
fn test_watch_id() {
    let id = WatchId::new(42);
    assert_eq!(id.as_u64(), 42);
    assert_eq!(format!("{id}"), "WatchId(42)");
}

#[test]
fn test_watch_id_equality() {
    let id1 = WatchId::new(1);
    let id2 = WatchId::new(1);
    let id3 = WatchId::new(2);
    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[test]
fn test_watch_handle() {
    let handle = WatchHandle::new(WatchId::new(1), PathBuf::from("/test"));
    assert_eq!(handle.id().as_u64(), 1);
    assert_eq!(handle.path(), &PathBuf::from("/test"));
}

#[test]
fn test_watch_event_created() {
    let event = WatchEvent::Created(PathBuf::from("/new"));
    assert_eq!(event.path(), Some(&PathBuf::from("/new")));
    assert!(event.is_created());
    assert!(!event.is_modified());
    assert!(!event.is_error());
    assert_eq!(format!("{event}"), "created: /new");
}

#[test]
fn test_watch_event_modified() {
    let event = WatchEvent::Modified(PathBuf::from("/file"));
    assert!(event.is_modified());
    assert_eq!(format!("{event}"), "modified: /file");
}

#[test]
fn test_watch_event_deleted() {
    let event = WatchEvent::Deleted(PathBuf::from("/old"));
    assert!(event.is_deleted());
    assert_eq!(format!("{event}"), "deleted: /old");
}

#[test]
fn test_watch_event_renamed() {
    let event = WatchEvent::Renamed {
        from: PathBuf::from("/old"),
        to: PathBuf::from("/new"),
    };
    assert_eq!(event.path(), Some(&PathBuf::from("/new")));
    assert!(event.is_renamed());
    assert_eq!(format!("{event}"), "renamed: /old -> /new");
}

#[test]
fn test_watch_event_metadata_changed() {
    let event = WatchEvent::MetadataChanged(PathBuf::from("/file"));
    assert!(event.is_metadata_changed());
    assert_eq!(format!("{event}"), "metadata changed: /file");
}

#[test]
fn test_watch_event_error_with_path() {
    let event = WatchEvent::Error {
        path: Some(PathBuf::from("/bad")),
        message: "failed".to_string(),
    };
    assert!(event.is_error());
    assert_eq!(event.path(), Some(&PathBuf::from("/bad")));
    assert_eq!(format!("{event}"), "error at /bad: failed");
}

#[test]
fn test_watch_event_error_without_path() {
    let event = WatchEvent::Error {
        path: None,
        message: "unknown error".to_string(),
    };
    assert!(event.is_error());
    assert_eq!(event.path(), None);
    assert_eq!(format!("{event}"), "error: unknown error");
}

#[test]
fn test_watch_id_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(WatchId::new(1));
    set.insert(WatchId::new(2));
    set.insert(WatchId::new(1)); // Duplicate

    assert_eq!(set.len(), 2);
}

#[test]
fn test_watch_id_clone() {
    let id = WatchId::new(42);
    let cloned = id;
    assert_eq!(id, cloned);
}

#[test]
fn test_watch_id_debug() {
    let id = WatchId::new(99);
    let debug = format!("{id:?}");
    assert!(debug.contains("99"));
}

#[test]
fn test_watch_handle_debug() {
    let handle = WatchHandle::new(WatchId::new(5), PathBuf::from("/watched"));
    let debug = format!("{handle:?}");
    assert!(debug.contains("WatchHandle"));
}

#[test]
fn test_watch_event_created_not_other_types() {
    let event = WatchEvent::Created(PathBuf::from("/new"));
    assert!(event.is_created());
    assert!(!event.is_modified());
    assert!(!event.is_deleted());
    assert!(!event.is_renamed());
    assert!(!event.is_metadata_changed());
    assert!(!event.is_error());
}

#[test]
fn test_watch_event_modified_not_other_types() {
    let event = WatchEvent::Modified(PathBuf::from("/file"));
    assert!(!event.is_created());
    assert!(event.is_modified());
    assert!(!event.is_deleted());
    assert!(!event.is_renamed());
    assert!(!event.is_metadata_changed());
    assert!(!event.is_error());
}

#[test]
fn test_watch_event_deleted_not_other_types() {
    let event = WatchEvent::Deleted(PathBuf::from("/old"));
    assert!(!event.is_created());
    assert!(!event.is_modified());
    assert!(event.is_deleted());
    assert!(!event.is_renamed());
    assert!(!event.is_metadata_changed());
    assert!(!event.is_error());
}

#[test]
fn test_watch_event_renamed_not_other_types() {
    let event = WatchEvent::Renamed {
        from: PathBuf::from("/old"),
        to: PathBuf::from("/new"),
    };
    assert!(!event.is_created());
    assert!(!event.is_modified());
    assert!(!event.is_deleted());
    assert!(event.is_renamed());
    assert!(!event.is_metadata_changed());
    assert!(!event.is_error());
}

#[test]
fn test_watch_event_metadata_changed_not_other_types() {
    let event = WatchEvent::MetadataChanged(PathBuf::from("/file"));
    assert!(!event.is_created());
    assert!(!event.is_modified());
    assert!(!event.is_deleted());
    assert!(!event.is_renamed());
    assert!(event.is_metadata_changed());
    assert!(!event.is_error());
}

#[test]
fn test_watch_event_clone() {
    let event = WatchEvent::Created(PathBuf::from("/test"));
    let cloned = event.clone();
    assert_eq!(event, cloned);
}

#[test]
fn test_watch_event_equality() {
    let a = WatchEvent::Modified(PathBuf::from("/file"));
    let b = WatchEvent::Modified(PathBuf::from("/file"));
    let c = WatchEvent::Modified(PathBuf::from("/other"));

    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn test_watch_event_path_for_metadata_changed() {
    let event = WatchEvent::MetadataChanged(PathBuf::from("/changed"));
    assert_eq!(event.path(), Some(&PathBuf::from("/changed")));
}

#[test]
fn test_watch_event_renamed_from_path() {
    let event = WatchEvent::Renamed {
        from: PathBuf::from("/old"),
        to: PathBuf::from("/new"),
    };
    // path() returns the 'to' path for renamed events
    assert_eq!(event.path(), Some(&PathBuf::from("/new")));
}

#[test]
fn test_watch_id_zero() {
    let id = WatchId::new(0);
    assert_eq!(id.as_u64(), 0);
    assert_eq!(format!("{id}"), "WatchId(0)");
}

#[test]
fn test_watch_id_max() {
    let id = WatchId::new(u64::MAX);
    assert_eq!(id.as_u64(), u64::MAX);
}
