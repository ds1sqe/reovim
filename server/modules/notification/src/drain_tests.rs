use {
    super::*,
    reovim_driver_session::{PendingEntry, PendingLevel, PendingOp, SessionExtension},
};

// ========================================================================
// Trait implementation
// ========================================================================

#[test]
fn drain_impl_implements_trait() {
    let drain = NotificationDrainImpl::new();
    let _: &dyn NotificationDrain = &drain;
}

#[test]
fn drain_impl_default() {
    let drain = <NotificationDrainImpl as Default>::default();
    let _: &dyn NotificationDrain = &drain;
}

// ========================================================================
// NotificationTokenMap service tests (#691)
// ========================================================================

#[test]
fn token_map_default_is_empty() {
    let tm = NotificationTokenMap::default();
    assert!(tm.lock().is_empty());
}

#[test]
fn token_map_debug() {
    let tm = NotificationTokenMap::default();
    let debug = format!("{tm:?}");
    assert!(debug.contains("NotificationTokenMap"));
}

#[test]
fn token_map_as_service() {
    use reovim_kernel::api::v1::ServiceRegistry;
    let registry = ServiceRegistry::new();
    let tm = registry.get_or_create::<NotificationTokenMap>();
    tm.lock().insert("tok".to_owned(), 42);
    let tm2 = registry.get::<NotificationTokenMap>().unwrap();
    assert_eq!(*tm2.lock().get("tok").unwrap(), 42);
}

// ========================================================================
// drain_entries() pure helper tests (#691)
// ========================================================================

#[test]
fn drain_push_creates_notification() {
    let mut state = NotificationState::create();
    let mut token_map = HashMap::new();
    let pending = vec![PendingEntry {
        source: None,
        op: PendingOp::Push {
            level: PendingLevel::Info,
            title: "hello".to_owned(),
        },
    }];

    let changed = drain_entries(pending, &mut state, &mut token_map);
    assert!(changed);
    assert_eq!(state.entries().len(), 1);
    assert_eq!(state.entries()[0].title, "hello");
    assert_eq!(state.entries()[0].level, NotificationLevel::Info);
    assert!(state.entries()[0].source.is_none());
}

#[test]
fn drain_push_with_source() {
    let mut state = NotificationState::create();
    let mut token_map = HashMap::new();
    let pending = vec![PendingEntry {
        source: Some("rust-analyzer".to_owned()),
        op: PendingOp::Push {
            level: PendingLevel::Success,
            title: "Ready".to_owned(),
        },
    }];

    let changed = drain_entries(pending, &mut state, &mut token_map);
    assert!(changed);
    assert_eq!(state.entries()[0].source.as_deref(), Some("rust-analyzer"));
    assert_eq!(state.entries()[0].level, NotificationLevel::Success);
}

#[test]
fn drain_push_all_levels() {
    let mut state = NotificationState::create();
    let mut token_map = HashMap::new();
    let pending = vec![
        PendingEntry {
            source: None,
            op: PendingOp::Push {
                level: PendingLevel::Info,
                title: "i".to_owned(),
            },
        },
        PendingEntry {
            source: None,
            op: PendingOp::Push {
                level: PendingLevel::Success,
                title: "s".to_owned(),
            },
        },
        PendingEntry {
            source: None,
            op: PendingOp::Push {
                level: PendingLevel::Warning,
                title: "w".to_owned(),
            },
        },
        PendingEntry {
            source: None,
            op: PendingOp::Push {
                level: PendingLevel::Error,
                title: "e".to_owned(),
            },
        },
    ];

    drain_entries(pending, &mut state, &mut token_map);
    assert_eq!(state.entries().len(), 4);
    assert_eq!(state.entries()[0].level, NotificationLevel::Info);
    assert_eq!(state.entries()[1].level, NotificationLevel::Success);
    assert_eq!(state.entries()[2].level, NotificationLevel::Warning);
    assert_eq!(state.entries()[3].level, NotificationLevel::Error);
}

#[test]
fn drain_progress_begin_creates_notification() {
    let mut state = NotificationState::create();
    let mut token_map = HashMap::new();
    let pending = vec![PendingEntry {
        source: Some("rust-analyzer".to_owned()),
        op: PendingOp::ProgressBegin {
            token: "token-1".to_owned(),
            title: "Indexing".to_owned(),
            message: "crate foo".to_owned(),
            percentage: 10,
        },
    }];

    let changed = drain_entries(pending, &mut state, &mut token_map);
    assert!(changed);
    assert_eq!(state.entries().len(), 1);
    assert_eq!(state.entries()[0].title, "Indexing");
    assert_eq!(state.entries()[0].source.as_deref(), Some("rust-analyzer"));
    let progress = state.entries()[0].progress.as_ref().unwrap();
    assert_eq!(progress.percent, 10);
    assert_eq!(progress.detail, "crate foo");
    assert!(token_map.contains_key("token-1"));
}

#[test]
fn drain_progress_report_updates_existing() {
    let mut state = NotificationState::create();
    let mut token_map = HashMap::new();

    // Begin
    let pending = vec![PendingEntry {
        source: Some("lsp".to_owned()),
        op: PendingOp::ProgressBegin {
            token: "t1".to_owned(),
            title: "Indexing".to_owned(),
            message: String::new(),
            percentage: 0,
        },
    }];
    drain_entries(pending, &mut state, &mut token_map);

    // Report
    let pending = vec![PendingEntry {
        source: Some("lsp".to_owned()),
        op: PendingOp::ProgressReport {
            token: "t1".to_owned(),
            message: Some("50% done".to_owned()),
            percentage: Some(50),
        },
    }];
    let changed = drain_entries(pending, &mut state, &mut token_map);
    assert!(changed);
    let progress = state.entries()[0].progress.as_ref().unwrap();
    assert_eq!(progress.percent, 50);
    assert_eq!(progress.detail, "50% done");
}

#[test]
fn drain_progress_report_preserves_percent_when_none() {
    let mut state = NotificationState::create();
    let mut token_map = HashMap::new();

    // Begin with 30%
    drain_entries(
        vec![PendingEntry {
            source: None,
            op: PendingOp::ProgressBegin {
                token: "t1".to_owned(),
                title: "Build".to_owned(),
                message: String::new(),
                percentage: 30,
            },
        }],
        &mut state,
        &mut token_map,
    );

    // Report with no percentage update
    drain_entries(
        vec![PendingEntry {
            source: None,
            op: PendingOp::ProgressReport {
                token: "t1".to_owned(),
                message: Some("still going".to_owned()),
                percentage: None,
            },
        }],
        &mut state,
        &mut token_map,
    );

    let progress = state.entries()[0].progress.as_ref().unwrap();
    assert_eq!(progress.percent, 30); // Preserved from begin
    assert_eq!(progress.detail, "still going");
}

#[test]
fn drain_progress_end_dismisses() {
    let mut state = NotificationState::create();
    let mut token_map = HashMap::new();

    // Begin
    drain_entries(
        vec![PendingEntry {
            source: None,
            op: PendingOp::ProgressBegin {
                token: "t1".to_owned(),
                title: "Build".to_owned(),
                message: String::new(),
                percentage: 0,
            },
        }],
        &mut state,
        &mut token_map,
    );
    assert_eq!(state.entries().len(), 1);

    // End
    let changed = drain_entries(
        vec![PendingEntry {
            source: None,
            op: PendingOp::ProgressEnd {
                token: "t1".to_owned(),
                message: None,
            },
        }],
        &mut state,
        &mut token_map,
    );
    assert!(changed);
    assert!(state.entries().is_empty());
    assert!(!token_map.contains_key("t1"));
}

#[test]
fn drain_progress_report_unknown_token_noop() {
    let mut state = NotificationState::create();
    let mut token_map = HashMap::new();

    let changed = drain_entries(
        vec![PendingEntry {
            source: None,
            op: PendingOp::ProgressReport {
                token: "unknown".to_owned(),
                message: None,
                percentage: Some(50),
            },
        }],
        &mut state,
        &mut token_map,
    );
    assert!(!changed);
    assert!(state.entries().is_empty());
}

#[test]
fn drain_progress_end_unknown_token_noop() {
    let mut state = NotificationState::create();
    let mut token_map = HashMap::new();

    let changed = drain_entries(
        vec![PendingEntry {
            source: None,
            op: PendingOp::ProgressEnd {
                token: "unknown".to_owned(),
                message: None,
            },
        }],
        &mut state,
        &mut token_map,
    );
    assert!(!changed);
}

#[test]
fn drain_full_progress_lifecycle() {
    let mut state = NotificationState::create();
    let mut token_map = HashMap::new();

    // Begin
    drain_entries(
        vec![PendingEntry {
            source: Some("ra".to_owned()),
            op: PendingOp::ProgressBegin {
                token: "idx".to_owned(),
                title: "Indexing".to_owned(),
                message: "starting".to_owned(),
                percentage: 0,
            },
        }],
        &mut state,
        &mut token_map,
    );
    assert_eq!(state.entries().len(), 1);

    // Report 1
    drain_entries(
        vec![PendingEntry {
            source: Some("ra".to_owned()),
            op: PendingOp::ProgressReport {
                token: "idx".to_owned(),
                message: Some("3/10".to_owned()),
                percentage: Some(30),
            },
        }],
        &mut state,
        &mut token_map,
    );
    assert_eq!(state.entries()[0].progress.as_ref().unwrap().percent, 30);

    // Report 2
    drain_entries(
        vec![PendingEntry {
            source: Some("ra".to_owned()),
            op: PendingOp::ProgressReport {
                token: "idx".to_owned(),
                message: Some("9/10".to_owned()),
                percentage: Some(90),
            },
        }],
        &mut state,
        &mut token_map,
    );
    assert_eq!(state.entries()[0].progress.as_ref().unwrap().percent, 90);

    // End
    drain_entries(
        vec![PendingEntry {
            source: Some("ra".to_owned()),
            op: PendingOp::ProgressEnd {
                token: "idx".to_owned(),
                message: Some("Done".to_owned()),
            },
        }],
        &mut state,
        &mut token_map,
    );
    assert!(state.entries().is_empty());
    assert!(token_map.is_empty());
}

#[test]
fn drain_multiple_concurrent_tokens() {
    let mut state = NotificationState::create();
    let mut token_map = HashMap::new();

    // Begin two tokens
    drain_entries(
        vec![
            PendingEntry {
                source: Some("ra".to_owned()),
                op: PendingOp::ProgressBegin {
                    token: "idx".to_owned(),
                    title: "Indexing".to_owned(),
                    message: String::new(),
                    percentage: 0,
                },
            },
            PendingEntry {
                source: Some("ra".to_owned()),
                op: PendingOp::ProgressBegin {
                    token: "chk".to_owned(),
                    title: "cargo check".to_owned(),
                    message: String::new(),
                    percentage: 0,
                },
            },
        ],
        &mut state,
        &mut token_map,
    );
    assert_eq!(state.entries().len(), 2);
    assert_eq!(token_map.len(), 2);

    // End only one
    drain_entries(
        vec![PendingEntry {
            source: Some("ra".to_owned()),
            op: PendingOp::ProgressEnd {
                token: "idx".to_owned(),
                message: None,
            },
        }],
        &mut state,
        &mut token_map,
    );
    assert_eq!(state.entries().len(), 1);
    assert_eq!(state.entries()[0].title, "cargo check");
    assert_eq!(token_map.len(), 1);
    assert!(token_map.contains_key("chk"));
}

#[test]
fn drain_empty_returns_false() {
    let mut state = NotificationState::create();
    let mut token_map = HashMap::new();
    let changed = drain_entries(vec![], &mut state, &mut token_map);
    assert!(!changed);
}

#[test]
fn convert_level_all_variants() {
    assert_eq!(convert_level(PendingLevel::Info), NotificationLevel::Info);
    assert_eq!(convert_level(PendingLevel::Success), NotificationLevel::Success);
    assert_eq!(convert_level(PendingLevel::Warning), NotificationLevel::Warning);
    assert_eq!(convert_level(PendingLevel::Error), NotificationLevel::Error);
}
