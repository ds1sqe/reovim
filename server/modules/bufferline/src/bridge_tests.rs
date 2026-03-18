use {
    reovim_driver_lsp::{
        BufferDiagnosticEntry, DiagnosticItem, DiagnosticSeverity, DiagnosticSnapshot,
    },
    reovim_driver_session::{ExtensionMap, bridges::ExtensionScope},
};

use {super::*, crate::service::BufferEntrySnapshot};

// ============================================================================
// Trait method tests
// ============================================================================

#[test]
fn bridge_kind() {
    assert_eq!(BufferlineBridge.kind(), "bufferline");
}

#[test]
fn bridge_scope_shared() {
    assert_eq!(BufferlineBridge.scope(), ExtensionScope::Shared);
}

#[test]
fn bridge_is_active_false_when_empty() {
    let extensions = ExtensionMap::new();
    assert!(!BufferlineBridge.is_active(&extensions));
}

#[test]
fn bridge_is_active_true_when_has_entries() {
    let mut extensions = ExtensionMap::new();
    let snap = extensions.get_or_insert::<BufferlineSnapshot>();
    snap.entries.push(BufferEntry {
        id: 1,
        name: "test.rs".to_string(),
        path: None,
        modified: false,
        pinned: false,
        filetype: None,
        error_count: 0,
        warning_count: 0,
    });
    assert!(BufferlineBridge.is_active(&extensions));
}

// ============================================================================
// snapshot()
// ============================================================================

#[test]
fn snapshot_no_state_returns_none() {
    let extensions = ExtensionMap::new();
    assert!(BufferlineBridge.snapshot(&extensions).is_none());
}

#[test]
fn snapshot_empty_entries() {
    let mut extensions = ExtensionMap::new();
    let _ = extensions.get_or_insert::<BufferlineSnapshot>();
    let json = BufferlineBridge.snapshot(&extensions).unwrap();
    assert_eq!(json["active"], true);
    assert!(json["buffers"].as_array().unwrap().is_empty());
}

#[test]
fn snapshot_with_entries() {
    let mut extensions = ExtensionMap::new();
    let snap = extensions.get_or_insert::<BufferlineSnapshot>();
    snap.entries.push(BufferEntry {
        id: 1,
        name: String::from("main.rs"),
        path: Some(String::from("/src/main.rs")),
        modified: false,
        pinned: true,
        filetype: Some(String::from("rust")),
        error_count: 2,
        warning_count: 1,
    });

    let json = BufferlineBridge.snapshot(&extensions).unwrap();
    let buf = &json["buffers"][0];
    assert_eq!(buf["id"], 1);
    assert_eq!(buf["name"], "main.rs");
    assert_eq!(buf["path"], "/src/main.rs");
    assert_eq!(buf["modified"], false);
    assert_eq!(buf["pinned"], true);
    assert_eq!(buf["filetype"], "rust");
    assert_eq!(buf["errorCount"], 2);
    assert_eq!(buf["warningCount"], 1);
}

#[test]
fn snapshot_optional_fields_absent() {
    let mut extensions = ExtensionMap::new();
    let snap = extensions.get_or_insert::<BufferlineSnapshot>();
    snap.entries.push(BufferEntry {
        id: 1,
        name: String::from("[No Name]"),
        path: None,
        modified: false,
        pinned: false,
        filetype: None,
        error_count: 0,
        warning_count: 0,
    });

    let json = BufferlineBridge.snapshot(&extensions).unwrap();
    let buf = &json["buffers"][0];
    assert!(buf.get("path").is_none());
    assert!(buf.get("filetype").is_none());
}

// ============================================================================
// tick()
// ============================================================================

#[test]
fn tick_no_service_returns_false() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();
    assert!(!BufferlineBridge.tick(&mut client, &mut shared, &services));
}

#[test]
fn tick_populates_snapshot() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    let svc = services.get_or_create::<BufferListService>();
    svc.add(BufferEntrySnapshot {
        id: 1,
        name: String::from("main.rs"),
        path: Some(String::from("/src/main.rs")),
        modified: false,
        filetype: Some(String::from("rust")),
    });

    let changed = BufferlineBridge.tick(&mut client, &mut shared, &services);
    assert!(changed);

    let snap = shared.get::<BufferlineSnapshot>().unwrap();
    assert_eq!(snap.entries.len(), 1);
    assert_eq!(snap.entries[0].name, "main.rs");
}

#[test]
fn tick_unchanged_returns_false() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    let svc = services.get_or_create::<BufferListService>();
    svc.add(BufferEntrySnapshot {
        id: 1,
        name: String::from("a.rs"),
        path: None,
        modified: false,
        filetype: None,
    });

    assert!(BufferlineBridge.tick(&mut client, &mut shared, &services));
    assert!(!BufferlineBridge.tick(&mut client, &mut shared, &services));
}

#[test]
fn tick_changed_entries_returns_true() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    let svc = services.get_or_create::<BufferListService>();
    svc.add(BufferEntrySnapshot {
        id: 1,
        name: String::from("a.rs"),
        path: None,
        modified: false,
        filetype: None,
    });

    BufferlineBridge.tick(&mut client, &mut shared, &services);

    svc.add(BufferEntrySnapshot {
        id: 2,
        name: String::from("b.rs"),
        path: None,
        modified: false,
        filetype: None,
    });

    assert!(BufferlineBridge.tick(&mut client, &mut shared, &services));
}

#[test]
fn tick_merges_diagnostics() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    let svc = services.get_or_create::<BufferListService>();
    svc.add(BufferEntrySnapshot {
        id: 1,
        name: String::from("main.rs"),
        path: None,
        modified: false,
        filetype: None,
    });

    // Add diagnostic snapshot.
    let diag = shared.get_or_insert::<DiagnosticSnapshot>();
    diag.entries.push(BufferDiagnosticEntry {
        buffer_id: 1,
        diagnostics: vec![
            DiagnosticItem {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 5,
                severity: DiagnosticSeverity::Error,
                message: String::from("err1"),
                source: None,
            },
            DiagnosticItem {
                start_line: 1,
                start_col: 0,
                end_line: 1,
                end_col: 3,
                severity: DiagnosticSeverity::Error,
                message: String::from("err2"),
                source: None,
            },
            DiagnosticItem {
                start_line: 2,
                start_col: 0,
                end_line: 2,
                end_col: 3,
                severity: DiagnosticSeverity::Warning,
                message: String::from("warn1"),
                source: None,
            },
        ],
    });

    BufferlineBridge.tick(&mut client, &mut shared, &services);

    let snap = shared.get::<BufferlineSnapshot>().unwrap();
    assert_eq!(snap.entries[0].error_count, 2);
    assert_eq!(snap.entries[0].warning_count, 1);
}

#[test]
fn tick_no_diagnostics_zero_counts() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    let svc = services.get_or_create::<BufferListService>();
    svc.add(BufferEntrySnapshot {
        id: 1,
        name: String::from("a.rs"),
        path: None,
        modified: false,
        filetype: None,
    });

    BufferlineBridge.tick(&mut client, &mut shared, &services);

    let snap = shared.get::<BufferlineSnapshot>().unwrap();
    assert_eq!(snap.entries[0].error_count, 0);
    assert_eq!(snap.entries[0].warning_count, 0);
}

#[test]
fn tick_sets_pinned_flags() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    let svc = services.get_or_create::<BufferListService>();
    svc.add(BufferEntrySnapshot {
        id: 1,
        name: String::from("a.rs"),
        path: None,
        modified: false,
        filetype: None,
    });
    svc.add(BufferEntrySnapshot {
        id: 2,
        name: String::from("b.rs"),
        path: None,
        modified: false,
        filetype: None,
    });

    let state = shared.get_or_insert::<BufferlineState>();
    state.pin(2);

    BufferlineBridge.tick(&mut client, &mut shared, &services);

    let snap = shared.get::<BufferlineSnapshot>().unwrap();
    // Pinned buffer should come first.
    assert_eq!(snap.entries[0].id, 2);
    assert!(snap.entries[0].pinned);
    assert_eq!(snap.entries[1].id, 1);
    assert!(!snap.entries[1].pinned);
}

#[test]
fn tick_cleans_stale_pins() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    let svc = services.get_or_create::<BufferListService>();
    svc.add(BufferEntrySnapshot {
        id: 1,
        name: String::from("a.rs"),
        path: None,
        modified: false,
        filetype: None,
    });

    // Pin a buffer that doesn't exist in the service.
    let state = shared.get_or_insert::<BufferlineState>();
    state.pin(1);
    state.pin(99);

    BufferlineBridge.tick(&mut client, &mut shared, &services);

    let state = shared.get::<BufferlineState>().unwrap();
    assert!(state.is_pinned(1));
    assert!(!state.is_pinned(99));
}

// ============================================================================
// diagnostic_counts()
// ============================================================================

#[test]
fn diagnostic_counts_no_snapshot() {
    let extensions = ExtensionMap::new();
    assert!(diagnostic_counts(&extensions).is_empty());
}

#[test]
fn diagnostic_counts_info_hint_ignored() {
    let mut extensions = ExtensionMap::new();
    let diag = extensions.get_or_insert::<DiagnosticSnapshot>();
    diag.entries.push(BufferDiagnosticEntry {
        buffer_id: 1,
        diagnostics: vec![
            DiagnosticItem {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 1,
                severity: DiagnosticSeverity::Information,
                message: String::from("info"),
                source: None,
            },
            DiagnosticItem {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 1,
                severity: DiagnosticSeverity::Hint,
                message: String::from("hint"),
                source: None,
            },
        ],
    });

    let counts = diagnostic_counts(&extensions);
    assert_eq!(counts.len(), 1);
    assert_eq!(counts[0], (1, 0, 0));
}
