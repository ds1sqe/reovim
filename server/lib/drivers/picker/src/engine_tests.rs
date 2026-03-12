use crate::PickerData;

use super::*;

fn make_item(display: &str) -> PickerItem {
    PickerItem {
        display: display.to_owned(),
        detail: None,
        data: PickerData::Text(display.to_owned()),
        icon: None,
    }
}

fn inject_and_settle(engine: &mut PickerEngine, items: Vec<PickerItem>) {
    let injector = engine.injector();
    push_items(&injector, items);
    // Tick until stable.
    for _ in 0..100 {
        let status = engine.tick(10);
        if !status.running {
            break;
        }
    }
}

#[test]
fn empty_engine() {
    let engine = PickerEngine::new();
    assert_eq!(engine.total_count(), 0);
    assert_eq!(engine.matched_count(), 0);
    assert!(engine.is_pattern_empty());
    assert!(engine.matched_items(10).is_empty());
}

#[test]
fn default_engine() {
    let engine = PickerEngine::default();
    assert_eq!(engine.total_count(), 0);
}

#[test]
fn inject_and_count() {
    let mut engine = PickerEngine::new();
    let items = vec![make_item("alpha"), make_item("beta"), make_item("gamma")];
    inject_and_settle(&mut engine, items);

    assert_eq!(engine.total_count(), 3);
    // Empty pattern matches everything.
    assert_eq!(engine.matched_count(), 3);
}

#[test]
fn pattern_filtering() {
    let mut engine = PickerEngine::new();
    let items = vec![
        make_item("src/main.rs"),
        make_item("src/lib.rs"),
        make_item("Cargo.toml"),
        make_item("README.md"),
    ];
    inject_and_settle(&mut engine, items);

    engine.set_pattern("rs");
    // Tick to process the new pattern.
    for _ in 0..100 {
        let status = engine.tick(10);
        if !status.running {
            break;
        }
    }

    assert!(!engine.is_pattern_empty());
    let matched = engine.matched_items(10);
    // Should match "src/main.rs" and "src/lib.rs".
    assert!(matched.len() >= 2);
    for item in &matched {
        assert!(
            item.display.contains("rs"),
            "Expected item to contain 'rs', got: {}",
            item.display
        );
    }
}

#[test]
fn matched_items_respects_max() {
    let mut engine = PickerEngine::new();
    let items: Vec<PickerItem> = (0..20).map(|i| make_item(&format!("item_{i}"))).collect();
    inject_and_settle(&mut engine, items);

    let matched = engine.matched_items(5);
    assert!(matched.len() <= 5);
}

#[test]
fn restart_clears_items() {
    let mut engine = PickerEngine::new();
    inject_and_settle(&mut engine, vec![make_item("hello")]);
    assert_eq!(engine.total_count(), 1);

    engine.restart();
    // After restart and a tick, items should be cleared.
    for _ in 0..100 {
        let status = engine.tick(10);
        if !status.running {
            break;
        }
    }
    assert_eq!(engine.total_count(), 0);
}

#[test]
fn push_item_helper() {
    let mut engine = PickerEngine::new();
    let injector = engine.injector();
    push_item(&injector, make_item("single"));
    for _ in 0..100 {
        let status = engine.tick(10);
        if !status.running {
            break;
        }
    }
    assert_eq!(engine.total_count(), 1);
}

#[test]
fn push_items_helper() {
    let mut engine = PickerEngine::new();
    let injector = engine.injector();
    push_items(&injector, vec![make_item("a"), make_item("b")]);
    for _ in 0..100 {
        let status = engine.tick(10);
        if !status.running {
            break;
        }
    }
    assert_eq!(engine.total_count(), 2);
}

#[test]
fn tick_status_fields() {
    let mut engine = PickerEngine::new();
    let status = engine.tick(0);
    // On empty engine, should not be running.
    let _ = status.running;
    let _ = status.changed;
    let debug = format!("{status:?}");
    assert!(debug.contains("TickStatus"));
}

#[test]
fn tick_status_copy() {
    let mut engine = PickerEngine::new();
    let status = engine.tick(0);
    let copied = status;
    assert_eq!(copied.running, status.running);
    assert_eq!(copied.changed, status.changed);
}

#[test]
fn engine_item_clone() {
    let item = make_item("test");
    let engine_item = EngineItem {
        match_text: Utf32String::from(item.display.as_str()),
        item,
    };
    #[allow(clippy::redundant_clone)]
    let cloned = engine_item.clone();
    assert_eq!(cloned.item.display, "test");
}

#[test]
fn score_ordering() {
    let mut engine = PickerEngine::new();
    let items = vec![
        make_item("xyzzy"),
        make_item("main.rs"),
        make_item("lib.rs"),
        make_item("mod.rs"),
    ];
    inject_and_settle(&mut engine, items);

    engine.set_pattern("main");
    for _ in 0..100 {
        let status = engine.tick(10);
        if !status.running {
            break;
        }
    }

    let matched = engine.matched_items(10);
    // "main.rs" should be the top match.
    assert!(!matched.is_empty());
    assert_eq!(matched[0].display, "main.rs");
}

#[test]
fn unicode_items() {
    let mut engine = PickerEngine::new();
    let items = vec![
        make_item("日本語テスト"),
        make_item("中文测试"),
        make_item("한국어"),
    ];
    inject_and_settle(&mut engine, items);
    assert_eq!(engine.total_count(), 3);
    assert_eq!(engine.matched_count(), 3);
}

#[test]
fn case_smart_matching() {
    let mut engine = PickerEngine::new();
    let items = vec![
        make_item("FooBar"),
        make_item("foobar"),
        make_item("FOOBAR"),
    ];
    inject_and_settle(&mut engine, items);

    // Lowercase query = case-insensitive (smart case).
    engine.set_pattern("foo");
    for _ in 0..100 {
        let status = engine.tick(10);
        if !status.running {
            break;
        }
    }
    assert_eq!(engine.matched_count(), 3);
}
