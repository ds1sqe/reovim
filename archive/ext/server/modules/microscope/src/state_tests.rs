use std::path::{Path, PathBuf};

use super::*;

#[test]
fn state_create_defaults() {
    let state = MicroscopeState::create();
    assert!(!state.active);
    assert!(state.query.is_empty());
    assert_eq!(state.cursor, 0);
    assert_eq!(state.selected, 0);
    assert_eq!(state.scroll_offset, 0);
    assert!(state.picker_name.is_empty());
    assert!(state.picker_title.is_empty());
    assert_eq!(state.prompt, "> ");
    assert!(state.items.is_empty());
    assert_eq!(state.total_count, 0);
    assert_eq!(state.matched_count, 0);
    assert!(state.preview.is_none());
    assert!(state.full_items.is_empty());
}

#[test]
fn state_debug() {
    let state = MicroscopeState::create();
    let debug = format!("{state:?}");
    assert!(debug.contains("MicroscopeState"));
}

#[test]
fn refresh_engine_empty() {
    let mut state = MicroscopeState::create();
    state.refresh_engine();
    assert_eq!(state.total_count, 0);
    assert_eq!(state.matched_count, 0);
    assert!(state.items.is_empty());
    assert!(state.full_items.is_empty());
    assert_eq!(state.selected, 0);
}

#[test]
fn refresh_engine_with_items() {
    use reovim_driver_picker::{PickerData, push_items};

    let mut state = MicroscopeState::create();
    let injector = state.engine.injector();
    push_items(
        &injector,
        vec![
            PickerItem {
                display: "main.rs".to_owned(),
                detail: None,
                data: PickerData::Text("a".to_owned()),
                icon: None,
            },
            PickerItem {
                display: "lib.rs".to_owned(),
                detail: Some("src/lib.rs".to_owned()),
                data: PickerData::Text("b".to_owned()),
                icon: Some('f'),
            },
        ],
    );
    state.refresh_engine();
    assert_eq!(state.total_count, 2);
    assert_eq!(state.matched_count, 2);
    assert_eq!(state.items.len(), 2);
    assert_eq!(state.full_items.len(), 2);
}

#[test]
fn refresh_engine_clamps_selection() {
    use reovim_driver_picker::{PickerData, push_items};

    let mut state = MicroscopeState::create();
    state.selected = 10; // Out of bounds.
    let injector = state.engine.injector();
    push_items(
        &injector,
        vec![PickerItem {
            display: "only.rs".to_owned(),
            detail: None,
            data: PickerData::Text("x".to_owned()),
            icon: None,
        }],
    );
    state.refresh_engine();
    assert_eq!(state.selected, 0); // Clamped to max valid index.
}

#[test]
fn insert_char_refreshes_engine() {
    use reovim_driver_picker::{PickerData, push_items};

    let mut state = MicroscopeState::create();
    state.active = true;
    let injector = state.engine.injector();
    push_items(
        &injector,
        vec![
            PickerItem {
                display: "main.rs".to_owned(),
                detail: None,
                data: PickerData::Text("a".to_owned()),
                icon: None,
            },
            PickerItem {
                display: "lib.rs".to_owned(),
                detail: None,
                data: PickerData::Text("b".to_owned()),
                icon: None,
            },
        ],
    );
    // Initial refresh to populate.
    state.refresh_engine();
    assert_eq!(state.matched_count, 2);

    // Type "main" to filter.
    TextInputSink::insert_char(&mut state, 'm');
    TextInputSink::insert_char(&mut state, 'a');
    TextInputSink::insert_char(&mut state, 'i');
    TextInputSink::insert_char(&mut state, 'n');
    // After typing, engine should have filtered.
    assert_eq!(state.matched_count, 1);
    assert_eq!(state.items[0].display, "main.rs");
}

#[test]
fn text_input_sink_insert_char() {
    let mut state = MicroscopeState::create();
    state.active = true;

    TextInputSink::insert_char(&mut state, 'h');
    TextInputSink::insert_char(&mut state, 'i');
    assert_eq!(state.query, "hi");
    assert_eq!(state.cursor, 2);
}

#[test]
fn text_input_sink_unicode() {
    let mut state = MicroscopeState::create();
    state.active = true;

    TextInputSink::insert_char(&mut state, '日');
    assert_eq!(state.query, "日");
    // Cursor is character index (1), not byte offset.
    assert_eq!(state.cursor, 1);
}

#[test]
fn as_text_input_sink_active() {
    let mut state = MicroscopeState::create();
    state.active = true;
    assert!(SessionExtension::as_text_input_sink(&mut state).is_some());
}

#[test]
fn as_text_input_sink_inactive() {
    let mut state = MicroscopeState::create();
    assert!(SessionExtension::as_text_input_sink(&mut state).is_none());
}

#[test]
fn picker_item_snapshot_clone() {
    let snap = PickerItemSnapshot {
        display: "test.rs".to_owned(),
        detail: Some("src/test.rs".to_owned()),
        icon: Some('f'),
    };
    #[allow(clippy::redundant_clone)]
    let cloned = snap.clone();
    assert_eq!(cloned.display, "test.rs");
    assert_eq!(cloned.detail.as_deref(), Some("src/test.rs"));
    assert_eq!(cloned.icon, Some('f'));
}

#[test]
fn picker_item_snapshot_debug() {
    let snap = PickerItemSnapshot {
        display: "x".to_owned(),
        detail: None,
        icon: None,
    };
    let debug = format!("{snap:?}");
    assert!(debug.contains("PickerItemSnapshot"));
}

/// A dynamic picker that returns items based on query.
struct DynamicTestPicker;

impl reovim_driver_picker::Picker for DynamicTestPicker {
    fn name(&self) -> &'static str {
        "dynamic-test"
    }

    fn title(&self) -> &'static str {
        "Dynamic Test"
    }

    fn items(
        &self,
        ctx: &reovim_driver_picker::PickerContext,
        _services: &reovim_kernel::api::v1::ServiceRegistry,
    ) -> Vec<PickerItem> {
        if ctx.query.is_empty() {
            vec![
                PickerItem {
                    display: "alpha".to_owned(),
                    detail: None,
                    data: reovim_driver_picker::PickerData::Text("a".to_owned()),
                    icon: None,
                },
                PickerItem {
                    display: "beta".to_owned(),
                    detail: None,
                    data: reovim_driver_picker::PickerData::Text("b".to_owned()),
                    icon: None,
                },
            ]
        } else {
            vec![PickerItem {
                display: format!("result-{}", ctx.query),
                detail: None,
                data: reovim_driver_picker::PickerData::Text("r".to_owned()),
                icon: None,
            }]
        }
    }

    fn on_select(&self, _item: &PickerItem) -> reovim_driver_picker::PickerAction {
        reovim_driver_picker::PickerAction::Close
    }

    fn is_static(&self) -> bool {
        false
    }
}

#[test]
fn refresh_engine_dynamic_picker() {
    let services = Arc::new(ServiceRegistry::new());
    let registry = Arc::new(PickerRegistry::new());
    registry.register(Arc::new(DynamicTestPicker));
    services.register(registry);

    let mut state = MicroscopeState::create();
    state.services = Some(Arc::clone(&services));
    state.picker_name = "dynamic-test".to_owned();

    // Initial refresh with empty query — should get 2 items from dynamic picker.
    state.refresh_engine();
    assert_eq!(state.total_count, 2);
    assert_eq!(state.matched_count, 2);
    assert_eq!(state.items.len(), 2);

    // Set a query — dynamic picker re-fetches with query context.
    state.query = "foo".to_owned();
    state.refresh_engine();
    assert_eq!(state.total_count, 1);
    assert_eq!(state.matched_count, 1);
    assert_eq!(state.items[0].display, "result-foo");
}

#[test]
fn refresh_engine_dynamic_picker_empty_items() {
    /// Dynamic picker that always returns empty.
    struct EmptyDynamicPicker;

    impl reovim_driver_picker::Picker for EmptyDynamicPicker {
        fn name(&self) -> &'static str {
            "empty-dynamic"
        }

        fn title(&self) -> &'static str {
            "Empty Dynamic"
        }

        fn items(
            &self,
            _ctx: &reovim_driver_picker::PickerContext,
            _services: &reovim_kernel::api::v1::ServiceRegistry,
        ) -> Vec<PickerItem> {
            vec![]
        }

        fn on_select(&self, _item: &PickerItem) -> reovim_driver_picker::PickerAction {
            reovim_driver_picker::PickerAction::Close
        }

        fn is_static(&self) -> bool {
            false
        }
    }

    let services = Arc::new(ServiceRegistry::new());
    let registry = Arc::new(PickerRegistry::new());
    registry.register(Arc::new(EmptyDynamicPicker));
    services.register(registry);

    let mut state = MicroscopeState::create();
    state.services = Some(Arc::clone(&services));
    state.picker_name = "empty-dynamic".to_owned();

    // Empty items path — engine restarts but no items pushed.
    state.refresh_engine();
    assert_eq!(state.total_count, 0);
    assert_eq!(state.matched_count, 0);
    assert!(state.items.is_empty());
}

/// Push many items to force the engine to require multiple ticks.
#[test]
fn refresh_engine_many_items_multi_tick() {
    use reovim_driver_picker::{PickerData, push_items};

    let mut state = MicroscopeState::create();
    let injector = state.engine.injector();

    // Push 50000 items — enough that tick(10) needs multiple passes.
    let items: Vec<PickerItem> = (0..50_000)
        .map(|i| PickerItem {
            display: format!("item_{i:05}"),
            detail: Some(format!("detail/path/to/item_{i:05}.rs")),
            data: PickerData::Text(format!("data_{i}")),
            icon: None,
        })
        .collect();
    push_items(&injector, items);

    state.query = "item_25".to_owned();
    state.refresh_engine();

    assert!(state.total_count > 0);
    assert!(state.matched_count > 0);
}

#[test]
fn insert_char_at_middle() {
    let mut state = MicroscopeState::create();
    state.active = true;
    state.query = "ac".to_owned();
    state.cursor = 1; // Between 'a' and 'c'.

    TextInputSink::insert_char(&mut state, 'b');
    assert_eq!(state.query, "abc");
    assert_eq!(state.cursor, 2);
}

#[test]
fn insert_unicode_at_middle() {
    let mut state = MicroscopeState::create();
    state.active = true;
    state.query = "ac".to_owned();
    state.cursor = 1;

    TextInputSink::insert_char(&mut state, '日');
    assert_eq!(state.query, "a日c");
    assert_eq!(state.cursor, 2);
}

#[test]
fn insert_multiple_unicode() {
    let mut state = MicroscopeState::create();
    state.active = true;

    TextInputSink::insert_char(&mut state, '日');
    TextInputSink::insert_char(&mut state, '本');
    TextInputSink::insert_char(&mut state, '語');
    assert_eq!(state.query, "日本語");
    assert_eq!(state.cursor, 3);
}

// ========================================================================
// language_id_from_path tests
// ========================================================================

#[test]
fn language_id_rust() {
    assert_eq!(language_id_from_path(Path::new("main.rs")), Some("rust"));
}

#[test]
fn language_id_markdown() {
    assert_eq!(language_id_from_path(Path::new("README.md")), Some("markdown"));
    assert_eq!(language_id_from_path(Path::new("doc.markdown")), Some("markdown"));
}

#[test]
fn language_id_python() {
    assert_eq!(language_id_from_path(Path::new("app.py")), Some("python"));
    assert_eq!(language_id_from_path(Path::new("stubs.pyi")), Some("python"));
}

#[test]
fn language_id_go() {
    assert_eq!(language_id_from_path(Path::new("main.go")), Some("go"));
}

#[test]
fn language_id_c() {
    assert_eq!(language_id_from_path(Path::new("foo.c")), Some("c"));
    assert_eq!(language_id_from_path(Path::new("foo.h")), Some("c"));
}

#[test]
fn language_id_bash() {
    assert_eq!(language_id_from_path(Path::new("run.sh")), Some("bash"));
    assert_eq!(language_id_from_path(Path::new("script.bash")), Some("bash"));
}

#[test]
fn language_id_json() {
    assert_eq!(language_id_from_path(Path::new("config.json")), Some("json"));
}

#[test]
fn language_id_toml() {
    assert_eq!(language_id_from_path(Path::new("Cargo.toml")), Some("toml"));
}

#[test]
fn language_id_javascript() {
    assert_eq!(language_id_from_path(Path::new("app.js")), Some("javascript"));
    assert_eq!(language_id_from_path(Path::new("module.mjs")), Some("javascript"));
    assert_eq!(language_id_from_path(Path::new("require.cjs")), Some("javascript"));
}

#[test]
fn language_id_typescript() {
    assert_eq!(language_id_from_path(Path::new("app.ts")), Some("typescript"));
    assert_eq!(language_id_from_path(Path::new("module.mts")), Some("typescript"));
    assert_eq!(language_id_from_path(Path::new("require.cts")), Some("typescript"));
}

#[test]
fn language_id_unknown() {
    assert_eq!(language_id_from_path(Path::new("image.png")), None);
    assert_eq!(language_id_from_path(Path::new("noext")), None);
}

// ========================================================================
// apply_syntax_highlights tests
// ========================================================================

#[test]
fn apply_syntax_highlights_no_path() {
    let services = ServiceRegistry::new();
    let mut preview = PreviewContent {
        lines: vec!["fn main() {}".to_owned()],
        highlight_line: None,
        file_path: None,
        ..Default::default()
    };
    MicroscopeState::apply_syntax_highlights(&mut preview, &services);
    assert!(preview.highlights.is_empty());
}

#[test]
fn apply_syntax_highlights_unknown_extension() {
    let services = ServiceRegistry::new();
    let mut preview = PreviewContent {
        lines: vec!["data".to_owned()],
        highlight_line: None,
        file_path: Some(PathBuf::from("file.xyz")),
        ..Default::default()
    };
    MicroscopeState::apply_syntax_highlights(&mut preview, &services);
    assert!(preview.highlights.is_empty());
}

#[test]
fn apply_syntax_highlights_no_factory_store() {
    let services = ServiceRegistry::new();
    let mut preview = PreviewContent {
        lines: vec!["fn main() {}".to_owned()],
        highlight_line: None,
        file_path: Some(PathBuf::from("main.rs")),
        ..Default::default()
    };
    MicroscopeState::apply_syntax_highlights(&mut preview, &services);
    // No SyntaxFactoryStore registered, so no highlights
    assert!(preview.highlights.is_empty());
}

#[test]
fn apply_syntax_highlights_too_many_lines() {
    let services = ServiceRegistry::new();
    let mut preview = PreviewContent {
        lines: (0..600).map(|i| format!("line {i}")).collect(),
        highlight_line: None,
        file_path: Some(PathBuf::from("big.rs")),
        ..Default::default()
    };
    MicroscopeState::apply_syntax_highlights(&mut preview, &services);
    assert!(preview.highlights.is_empty());
}
