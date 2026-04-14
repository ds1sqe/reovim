use super::*;

#[test]
fn test_bridge_kind() {
    assert_eq!(NotificationBridge.kind(), "notification");
}

#[test]
fn test_bridge_scope() {
    assert_eq!(NotificationBridge.scope(), ExtensionScope::Client);
}

#[test]
fn test_snapshot_empty_map() {
    let map = ExtensionMap::new();
    assert!(NotificationBridge.snapshot(&map).is_none());
}

#[test]
fn test_snapshot_inactive_state() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<NotificationState>();

    let snap = NotificationBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], false);
    assert!(snap["entries"].as_array().unwrap().is_empty());
}

#[test]
fn test_snapshot_with_entries() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<NotificationState>();
    state.push(NotificationLevel::Success, "File saved");
    state.push(NotificationLevel::Error, "Build failed");

    let snap = NotificationBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], true);

    let entries = snap["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["title"], "File saved");
    assert_eq!(entries[0]["level"], "success");
    assert_eq!(entries[0]["id"], 0);
    assert_eq!(entries[1]["title"], "Build failed");
    assert_eq!(entries[1]["level"], "error");
    assert_eq!(entries[1]["id"], 1);
}

#[test]
fn test_snapshot_with_body() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<NotificationState>();
    state.push_with_body(NotificationLevel::Warning, "Warn", "details");

    let snap = NotificationBridge.snapshot(&map).unwrap();
    let entry = &snap["entries"][0];
    assert_eq!(entry["body"], "details");
}

#[test]
fn test_snapshot_with_progress() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<NotificationState>();
    state.push_progress("Building", 35, "3/10 files");

    let snap = NotificationBridge.snapshot(&map).unwrap();
    let entry = &snap["entries"][0];
    assert_eq!(entry["level"], "info");
    let progress = &entry["progress"];
    assert_eq!(progress["percent"], 35);
    assert_eq!(progress["detail"], "3/10 files");
}

#[test]
fn test_snapshot_without_progress() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<NotificationState>();
    state.push(NotificationLevel::Info, "no progress");

    let snap = NotificationBridge.snapshot(&map).unwrap();
    let entry = &snap["entries"][0];
    // progress field should not be present
    assert!(entry.get("progress").is_none());
}

#[test]
fn test_is_active_empty_map() {
    let map = ExtensionMap::new();
    assert!(!NotificationBridge.is_active(&map));
}

#[test]
fn test_is_active_no_entries() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<NotificationState>();
    assert!(!NotificationBridge.is_active(&map));
}

#[test]
fn test_is_active_with_entries() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<NotificationState>();
    state.push(NotificationLevel::Info, "test");
    assert!(NotificationBridge.is_active(&map));
}

#[test]
fn test_is_active_after_dismiss() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<NotificationState>();
    let id = state.push(NotificationLevel::Info, "test");
    assert!(NotificationBridge.is_active(&map));

    let state = map.get_or_insert::<NotificationState>();
    state.dismiss(id);
    assert!(!NotificationBridge.is_active(&map));
}

#[test]
fn test_snapshot_mixed_progress_and_no_progress() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<NotificationState>();
    state.push(NotificationLevel::Info, "plain");
    state.push_progress("Building", 42, "4/10");

    let snap = NotificationBridge.snapshot(&map).unwrap();
    let entries = snap["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);

    // First entry has no progress
    assert!(entries[0].get("progress").is_none());

    // Second entry has progress
    let progress = &entries[1]["progress"];
    assert_eq!(progress["percent"], 42);
    assert_eq!(progress["detail"], "4/10");
}

#[test]
fn test_level_str_all_variants() {
    assert_eq!(level_str(NotificationLevel::Info), "info");
    assert_eq!(level_str(NotificationLevel::Success), "success");
    assert_eq!(level_str(NotificationLevel::Warning), "warning");
    assert_eq!(level_str(NotificationLevel::Error), "error");
}

// ========================================================================
// Source field serialization tests (#691)
// ========================================================================

#[test]
fn test_snapshot_with_source() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<NotificationState>();
    state.push_with_source(Some("rust-analyzer".to_string()), NotificationLevel::Info, "Indexing");

    let snap = NotificationBridge.snapshot(&map).unwrap();
    let entry = &snap["entries"][0];
    assert_eq!(entry["source"], "rust-analyzer");
}

#[test]
fn test_snapshot_without_source() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<NotificationState>();
    state.push(NotificationLevel::Info, "no source");

    let snap = NotificationBridge.snapshot(&map).unwrap();
    let entry = &snap["entries"][0];
    assert!(entry.get("source").is_none());
}

#[test]
fn test_snapshot_mixed_sources() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<NotificationState>();
    state.push_with_source(Some("rust-analyzer".to_string()), NotificationLevel::Success, "Ready");
    state.push(NotificationLevel::Info, "generic");

    let snap = NotificationBridge.snapshot(&map).unwrap();
    let entries = snap["entries"].as_array().unwrap();
    assert_eq!(entries[0]["source"], "rust-analyzer");
    assert!(entries[1].get("source").is_none());
}

#[test]
fn test_snapshot_progress_with_source() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<NotificationState>();
    state.push_progress_with_source(
        Some("rust-analyzer".to_string()),
        "Indexing",
        42,
        "3/10 crates",
    );

    let snap = NotificationBridge.snapshot(&map).unwrap();
    let entry = &snap["entries"][0];
    assert_eq!(entry["source"], "rust-analyzer");
    assert_eq!(entry["progress"]["percent"], 42);
}

// ========================================================================
// Bridge tick() tests (#691 — periodic notification drain)
// ========================================================================

use {
    reovim_driver_text_session::{PendingLevel, PendingNotificationQueue, PendingOp},
    reovim_kernel::api::v1::ServiceRegistry,
};

#[test]
fn test_tick_no_queue_returns_false() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();
    assert!(!NotificationBridge.tick(&mut client, &mut shared, &services));
}

#[test]
fn test_tick_empty_queue_returns_false() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();
    let _ = services.get_or_create::<PendingNotificationQueue>();
    assert!(!NotificationBridge.tick(&mut client, &mut shared, &services));
}

#[test]
fn test_tick_drains_push_notification() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();
    let queue = services.get_or_create::<PendingNotificationQueue>();
    queue.push(PendingLevel::Info, "Hello from tick");

    let changed = NotificationBridge.tick(&mut client, &mut shared, &services);
    assert!(changed);

    let state = client.get::<NotificationState>().unwrap();
    assert_eq!(state.entries().len(), 1);
    assert_eq!(state.entries()[0].title, "Hello from tick");
}

#[test]
fn test_tick_drains_sourced_notification() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();
    let queue = services.get_or_create::<PendingNotificationQueue>();
    queue.push_op(
        Some("rust-analyzer".to_owned()),
        PendingOp::Push {
            level: PendingLevel::Success,
            title: "Ready".to_owned(),
        },
    );

    NotificationBridge.tick(&mut client, &mut shared, &services);

    let state = client.get::<NotificationState>().unwrap();
    assert_eq!(state.entries()[0].source.as_deref(), Some("rust-analyzer"));
}

#[test]
fn test_tick_progress_lifecycle() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();
    let queue = services.get_or_create::<PendingNotificationQueue>();

    // Begin
    queue.push_op(
        Some("ra".to_owned()),
        PendingOp::ProgressBegin {
            token: "t1".to_owned(),
            title: "Indexing".to_owned(),
            message: String::new(),
            percentage: 0,
        },
    );
    assert!(NotificationBridge.tick(&mut client, &mut shared, &services));
    assert_eq!(client.get::<NotificationState>().unwrap().entries().len(), 1);

    // Report
    queue.push_op(
        Some("ra".to_owned()),
        PendingOp::ProgressReport {
            token: "t1".to_owned(),
            message: Some("50%".to_owned()),
            percentage: Some(50),
        },
    );
    assert!(NotificationBridge.tick(&mut client, &mut shared, &services));
    let progress = client.get::<NotificationState>().unwrap().entries()[0]
        .progress
        .as_ref()
        .unwrap();
    assert_eq!(progress.percent, 50);

    // End
    queue.push_op(
        Some("ra".to_owned()),
        PendingOp::ProgressEnd {
            token: "t1".to_owned(),
            message: None,
        },
    );
    assert!(NotificationBridge.tick(&mut client, &mut shared, &services));
    assert!(
        client
            .get::<NotificationState>()
            .unwrap()
            .entries()
            .is_empty()
    );
}

#[test]
fn test_tick_second_call_empty_returns_false() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();
    let queue = services.get_or_create::<PendingNotificationQueue>();
    queue.push(PendingLevel::Info, "once");

    assert!(NotificationBridge.tick(&mut client, &mut shared, &services));
    assert!(!NotificationBridge.tick(&mut client, &mut shared, &services));
}
