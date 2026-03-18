use super::*;

#[test]
fn test_session_extension_create() {
    let state = NotificationState::create();
    assert!(!state.is_active());
    assert!(state.entries().is_empty());
}

#[test]
fn test_push_returns_sequential_ids() {
    let mut state = NotificationState::create();
    let id0 = state.push(NotificationLevel::Info, "first");
    let id1 = state.push(NotificationLevel::Info, "second");
    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
}

#[test]
fn test_push_stores_entry() {
    let mut state = NotificationState::create();
    state.push(NotificationLevel::Success, "saved");
    assert!(state.is_active());
    assert_eq!(state.entries().len(), 1);
    assert_eq!(state.entries()[0].title, "saved");
    assert_eq!(state.entries()[0].level, NotificationLevel::Success);
    assert!(state.entries()[0].body.is_empty());
    assert!(state.entries()[0].progress.is_none());
}

#[test]
fn test_push_with_body() {
    let mut state = NotificationState::create();
    let id = state.push_with_body(NotificationLevel::Error, "Error", "details here");
    assert_eq!(id, 0);
    assert_eq!(state.entries()[0].body, "details here");
    assert_eq!(state.entries()[0].level, NotificationLevel::Error);
}

#[test]
fn test_push_progress() {
    let mut state = NotificationState::create();
    let id = state.push_progress("Building", 35, "3/10 files");
    assert_eq!(id, 0);
    let entry = &state.entries()[0];
    assert_eq!(entry.level, NotificationLevel::Info);
    assert_eq!(entry.title, "Building");
    let progress = entry.progress.as_ref().unwrap();
    assert_eq!(progress.percent, 35);
    assert_eq!(progress.detail, "3/10 files");
}

#[test]
fn test_push_progress_clamps_percent() {
    let mut state = NotificationState::create();
    state.push_progress("test", 200, "over");
    assert_eq!(state.entries()[0].progress.as_ref().unwrap().percent, 100);
}

#[test]
fn test_update_progress_found() {
    let mut state = NotificationState::create();
    let id = state.push_progress("Building", 10, "starting");
    assert!(state.update_progress(id, 50, "halfway"));
    let progress = state.entries()[0].progress.as_ref().unwrap();
    assert_eq!(progress.percent, 50);
    assert_eq!(progress.detail, "halfway");
}

#[test]
fn test_update_progress_not_found() {
    let mut state = NotificationState::create();
    assert!(!state.update_progress(999, 50, "nope"));
}

#[test]
fn test_update_progress_clamps_percent() {
    let mut state = NotificationState::create();
    let id = state.push(NotificationLevel::Info, "test");
    assert!(state.update_progress(id, 150, "over"));
    assert_eq!(state.entries()[0].progress.as_ref().unwrap().percent, 100);
}

#[test]
fn test_dismiss_found() {
    let mut state = NotificationState::create();
    let id = state.push(NotificationLevel::Info, "gone");
    assert!(state.dismiss(id));
    assert!(!state.is_active());
}

#[test]
fn test_dismiss_not_found() {
    let mut state = NotificationState::create();
    assert!(!state.dismiss(999));
}

#[test]
fn test_dismiss_preserves_others() {
    let mut state = NotificationState::create();
    let id0 = state.push(NotificationLevel::Info, "first");
    let _id1 = state.push(NotificationLevel::Info, "second");
    state.dismiss(id0);
    assert_eq!(state.entries().len(), 1);
    assert_eq!(state.entries()[0].title, "second");
}

#[test]
fn test_is_active_empty() {
    let state = NotificationState::create();
    assert!(!state.is_active());
}

#[test]
fn test_is_active_after_push() {
    let mut state = NotificationState::create();
    state.push(NotificationLevel::Info, "test");
    assert!(state.is_active());
}

#[test]
fn test_max_entries_eviction() {
    let mut state = NotificationState::create();
    // Push 51 entries (max is 50)
    for i in 0..51 {
        state.push(NotificationLevel::Info, format!("msg-{i}"));
    }
    assert_eq!(state.entries().len(), 50);
    // Oldest (msg-0) should be evicted, first entry is msg-1
    assert_eq!(state.entries()[0].title, "msg-1");
    assert_eq!(state.entries()[49].title, "msg-50");
}

#[test]
fn test_notification_level_debug() {
    let level = NotificationLevel::Warning;
    assert_eq!(format!("{level:?}"), "Warning");
}

#[test]
fn test_notification_level_clone_copy() {
    let level = NotificationLevel::Error;
    let cloned = level;
    assert_eq!(level, cloned);
}

#[test]
fn test_notification_level_all_variants() {
    assert_ne!(NotificationLevel::Info, NotificationLevel::Success);
    assert_ne!(NotificationLevel::Success, NotificationLevel::Warning);
    assert_ne!(NotificationLevel::Warning, NotificationLevel::Error);
}

#[test]
fn test_progress_debug_clone_eq() {
    let p = Progress {
        percent: 50,
        detail: "half".to_string(),
    };
    let p2 = p.clone();
    assert_eq!(p, p2);
    assert_eq!(format!("{p:?}"), "Progress { percent: 50, detail: \"half\" }");
}

#[test]
fn test_notification_debug_clone() {
    let n = Notification {
        id: 1,
        level: NotificationLevel::Info,
        title: "test".to_string(),
        body: String::new(),
        progress: None,
        source: None,
    };
    let n2 = n.clone();
    assert_eq!(n2.id, 1);
    assert!(format!("{n:?}").contains("test"));
}

#[test]
fn test_notification_debug_clone_with_source() {
    let n = Notification {
        id: 1,
        level: NotificationLevel::Info,
        title: "test".to_string(),
        body: String::new(),
        progress: None,
        source: Some("rust-analyzer".to_string()),
    };
    let n2 = n.clone();
    assert_eq!(n2.source.as_deref(), Some("rust-analyzer"));
    assert_eq!(n2.id, 1);
    assert!(format!("{n:?}").contains("rust-analyzer"));
}

#[test]
fn test_state_debug() {
    let state = NotificationState::create();
    assert!(format!("{state:?}").contains("NotificationState"));
}

#[test]
fn test_push_progress_exact_100() {
    let mut state = NotificationState::create();
    state.push_progress("test", 100, "done");
    assert_eq!(state.entries()[0].progress.as_ref().unwrap().percent, 100);
}

#[test]
fn test_update_progress_exact_100() {
    let mut state = NotificationState::create();
    let id = state.push_progress("test", 0, "starting");
    assert!(state.update_progress(id, 100, "done"));
    assert_eq!(state.entries()[0].progress.as_ref().unwrap().percent, 100);
}

#[test]
fn test_max_entries_boundary_no_eviction() {
    let mut state = NotificationState::create();
    // Push exactly 50 (max_entries) — no eviction should occur
    for i in 0..50 {
        state.push(NotificationLevel::Info, format!("msg-{i}"));
    }
    assert_eq!(state.entries().len(), 50);
    assert_eq!(state.entries()[0].title, "msg-0"); // oldest preserved
    assert_eq!(state.entries()[49].title, "msg-49");
}

// ========================================================================
// Source field tests (#691)
// ========================================================================

#[test]
fn test_push_has_no_source() {
    let mut state = NotificationState::create();
    state.push(NotificationLevel::Info, "test");
    assert!(state.entries()[0].source.is_none());
}

#[test]
fn test_push_with_body_has_no_source() {
    let mut state = NotificationState::create();
    state.push_with_body(NotificationLevel::Info, "test", "body");
    assert!(state.entries()[0].source.is_none());
}

#[test]
fn test_push_progress_has_no_source() {
    let mut state = NotificationState::create();
    state.push_progress("test", 50, "detail");
    assert!(state.entries()[0].source.is_none());
}

#[test]
fn test_push_with_source_some() {
    let mut state = NotificationState::create();
    let id = state.push_with_source(
        Some("rust-analyzer".to_string()),
        NotificationLevel::Success,
        "Server ready",
    );
    assert_eq!(id, 0);
    let entry = &state.entries()[0];
    assert_eq!(entry.source.as_deref(), Some("rust-analyzer"));
    assert_eq!(entry.title, "Server ready");
    assert_eq!(entry.level, NotificationLevel::Success);
}

#[test]
fn test_push_with_source_none() {
    let mut state = NotificationState::create();
    let id = state.push_with_source(None, NotificationLevel::Info, "no source");
    assert_eq!(id, 0);
    assert!(state.entries()[0].source.is_none());
}

#[test]
fn test_push_progress_with_source() {
    let mut state = NotificationState::create();
    let id = state.push_progress_with_source(
        Some("rust-analyzer".to_string()),
        "Indexing",
        42,
        "3/10 crates",
    );
    assert_eq!(id, 0);
    let entry = &state.entries()[0];
    assert_eq!(entry.source.as_deref(), Some("rust-analyzer"));
    assert_eq!(entry.title, "Indexing");
    let progress = entry.progress.as_ref().unwrap();
    assert_eq!(progress.percent, 42);
    assert_eq!(progress.detail, "3/10 crates");
}

#[test]
fn test_push_progress_with_source_clamps() {
    let mut state = NotificationState::create();
    state.push_progress_with_source(Some("lsp".to_string()), "test", 200, "over");
    assert_eq!(state.entries()[0].progress.as_ref().unwrap().percent, 100);
}

#[test]
fn test_push_progress_with_source_none() {
    let mut state = NotificationState::create();
    state.push_progress_with_source(None, "test", 50, "detail");
    assert!(state.entries()[0].source.is_none());
}
