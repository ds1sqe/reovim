use crate::CompletionKind;

use super::*;

fn make_item(label: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        insert_text: label.to_string(),
        kind: CompletionKind::Text,
        detail: None,
        documentation: None,
        source_id: "test",
        is_snippet: false,
        sort_priority: 100,
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn settle(engine: &mut CompletionEngine) {
    for _ in 0..100 {
        let status = engine.tick(10);
        if !status.running {
            break;
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn inject_and_settle(engine: &mut CompletionEngine, items: Vec<CompletionItem>) {
    let injector = engine.injector();
    push_items(&injector, items);
    settle(engine);
}

#[test]
fn empty_engine() {
    let engine = CompletionEngine::new();
    assert_eq!(engine.total_count(), 0);
    assert_eq!(engine.matched_count(), 0);
    assert!(engine.is_pattern_empty());
    assert!(engine.matched_items(10).is_empty());
}

#[test]
fn default_engine() {
    let engine = CompletionEngine::default();
    assert_eq!(engine.total_count(), 0);
}

#[test]
fn inject_and_count() {
    let mut engine = CompletionEngine::new();
    let items = vec![make_item("alpha"), make_item("beta"), make_item("gamma")];
    inject_and_settle(&mut engine, items);

    assert_eq!(engine.total_count(), 3);
    assert_eq!(engine.matched_count(), 3);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn pattern_filtering() {
    let mut engine = CompletionEngine::new();
    let items = vec![
        make_item("function_name"),
        make_item("variable_name"),
        make_item("constant_value"),
        make_item("func_helper"),
    ];
    inject_and_settle(&mut engine, items);

    engine.set_pattern("func");
    settle(&mut engine);

    assert!(!engine.is_pattern_empty());
    let matched = engine.matched_items(10);
    assert!(matched.len() >= 2);
    for item in &matched {
        assert!(
            item.label.contains("func") || item.label.contains("fun"),
            "Expected item to match 'func', got: {}",
            item.label
        );
    }
}

#[test]
fn matched_items_respects_max() {
    let mut engine = CompletionEngine::new();
    let items: Vec<CompletionItem> = (0..20).map(|i| make_item(&format!("item_{i}"))).collect();
    inject_and_settle(&mut engine, items);

    let matched = engine.matched_items(5);
    assert!(matched.len() <= 5);
}

#[test]
fn restart_clears_items() {
    let mut engine = CompletionEngine::new();
    inject_and_settle(&mut engine, vec![make_item("hello")]);
    assert_eq!(engine.total_count(), 1);

    engine.restart();
    settle(&mut engine);
    assert_eq!(engine.total_count(), 0);
}

#[test]
fn push_item_helper() {
    let mut engine = CompletionEngine::new();
    let injector = engine.injector();
    push_item(&injector, make_item("single"));
    settle(&mut engine);
    assert_eq!(engine.total_count(), 1);
}

#[test]
fn push_items_helper() {
    let mut engine = CompletionEngine::new();
    let injector = engine.injector();
    push_items(&injector, vec![make_item("a"), make_item("b")]);
    settle(&mut engine);
    assert_eq!(engine.total_count(), 2);
}

#[test]
fn tick_status_fields() {
    let mut engine = CompletionEngine::new();
    let status = engine.tick(0);
    let _ = status.running;
    let _ = status.changed;
    let debug = format!("{status:?}");
    assert!(debug.contains("TickStatus"));
}

#[test]
fn tick_status_copy() {
    let mut engine = CompletionEngine::new();
    let status = engine.tick(0);
    let copied = status;
    assert_eq!(copied.running, status.running);
    assert_eq!(copied.changed, status.changed);
}

#[test]
fn engine_item_clone() {
    let item = make_item("test");
    let engine_item = EngineItem {
        match_text: Utf32String::from(item.label.as_str()),
        item,
    };
    #[allow(clippy::redundant_clone)]
    let cloned = engine_item.clone();
    assert_eq!(cloned.item.label, "test");
}

#[test]
fn score_ordering() {
    let mut engine = CompletionEngine::new();
    let items = vec![
        make_item("xyzzy"),
        make_item("main_func"),
        make_item("lib_func"),
        make_item("mod_func"),
    ];
    inject_and_settle(&mut engine, items);

    engine.set_pattern("main");
    settle(&mut engine);

    let matched = engine.matched_items(10);
    assert!(!matched.is_empty());
    assert_eq!(matched[0].label, "main_func");
}

#[test]
fn unicode_items() {
    let mut engine = CompletionEngine::new();
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
    let mut engine = CompletionEngine::new();
    let items = vec![
        make_item("FooBar"),
        make_item("foobar"),
        make_item("FOOBAR"),
    ];
    inject_and_settle(&mut engine, items);

    engine.set_pattern("foo");
    settle(&mut engine);
    assert_eq!(engine.matched_count(), 3);
}
