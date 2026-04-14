use {
    reovim_driver_text_syntax::{ContextHierarchy, ScopeKind, ScopeRange},
    reovim_module_context::ContextSessionState,
};

use super::*;

fn sample_hierarchy() -> ContextHierarchy {
    ContextHierarchy::new(
        1,
        20, // cursor at line 20
        5,
        vec![
            ScopeRange::new(0, 100, ScopeKind::Module, "mod utils", Some("utils".into())),
            ScopeRange::new(5, 50, ScopeKind::Class, "impl Foo", Some("Foo".into())),
            ScopeRange::new(10, 30, ScopeKind::Function, "fn bar", Some("bar".into())),
        ],
    )
}

#[test]
fn test_options_default() {
    let opts = StickyContextOptions::default();
    assert!(opts.enabled);
    assert_eq!(opts.max_count, 3);
    assert!(opts.separator);
}

#[test]
fn test_options_debug() {
    let opts = StickyContextOptions::default();
    let debug = format!("{opts:?}");
    assert!(debug.contains("enabled"));
}

#[test]
fn test_options_clone() {
    let opts = StickyContextOptions::default();
    let cloned = opts.clone();
    assert_eq!(cloned.enabled, opts.enabled);
    assert_eq!(cloned.max_count, opts.max_count);
}

#[test]
fn test_header_row_new() {
    let row = HeaderRow::new(42, "fn main", "fn");
    assert_eq!(row.line, 42);
    assert_eq!(row.text, "fn main");
    assert_eq!(row.kind, "fn");
}

#[test]
fn test_header_row_debug() {
    let row = HeaderRow::new(0, "test", "fn");
    let debug = format!("{row:?}");
    assert!(debug.contains("HeaderRow"));
}

#[test]
fn test_header_row_eq() {
    let a = HeaderRow::new(1, "fn foo", "fn");
    let b = HeaderRow::new(1, "fn foo", "fn");
    assert_eq!(a, b);
}

#[test]
fn test_header_row_clone() {
    let row = HeaderRow::new(1, "fn foo", "fn");
    let cloned = row.clone();
    assert_eq!(row, cloned);
}

#[test]
fn test_state_default() {
    let state = StickyContextState::default();
    assert!(state.options.enabled);
}

#[test]
fn test_state_debug() {
    let state = StickyContextState::default();
    let debug = format!("{state:?}");
    assert!(debug.contains("StickyContextState"));
}

#[test]
fn test_header_rows_disabled() {
    let mut state = StickyContextState::default();
    state.options.enabled = false;
    let mut ctx = ContextSessionState::default();
    ctx.set_hierarchy(sample_hierarchy());
    assert!(state.header_rows(Some(&ctx), 15).is_empty());
}

#[test]
fn test_header_rows_no_context() {
    let state = StickyContextState::default();
    assert!(state.header_rows(None, 15).is_empty());
}

#[test]
fn test_header_rows_no_hierarchy() {
    let state = StickyContextState::default();
    let ctx = ContextSessionState::default();
    assert!(state.header_rows(Some(&ctx), 15).is_empty());
}

#[test]
fn test_header_rows_empty_hierarchy() {
    let state = StickyContextState::default();
    let mut ctx = ContextSessionState::default();
    ctx.set_hierarchy(ContextHierarchy::empty());
    assert!(state.header_rows(Some(&ctx), 15).is_empty());
}

#[test]
fn test_header_rows_viewport_at_top() {
    let state = StickyContextState::default();
    let mut ctx = ContextSessionState::default();
    ctx.set_hierarchy(sample_hierarchy());
    // viewport_top = 0, no scope starts above line 0
    let rows = state.header_rows(Some(&ctx), 0);
    assert!(rows.is_empty());
}

#[test]
fn test_header_rows_scrolled_past_module() {
    let state = StickyContextState::default();
    let mut ctx = ContextSessionState::default();
    ctx.set_hierarchy(sample_hierarchy());
    // viewport_top = 3, only mod utils (line 0) is above
    let rows = state.header_rows(Some(&ctx), 3);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text, "mod utils");
    assert_eq!(rows[0].line, 0);
}

#[test]
fn test_header_rows_scrolled_past_two() {
    let state = StickyContextState::default();
    let mut ctx = ContextSessionState::default();
    ctx.set_hierarchy(sample_hierarchy());
    // viewport_top = 8, mod utils (0) and impl Foo (5) are above
    let rows = state.header_rows(Some(&ctx), 8);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].text, "mod utils");
    assert_eq!(rows[1].text, "impl Foo");
}

#[test]
fn test_header_rows_scrolled_past_all() {
    let state = StickyContextState::default();
    let mut ctx = ContextSessionState::default();
    ctx.set_hierarchy(sample_hierarchy());
    // viewport_top = 15, all three scopes start above
    let rows = state.header_rows(Some(&ctx), 15);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].text, "mod utils");
    assert_eq!(rows[1].text, "impl Foo");
    assert_eq!(rows[2].text, "fn bar");
}

#[test]
fn test_header_rows_max_count() {
    let mut state = StickyContextState::default();
    state.options.max_count = 2;
    let mut ctx = ContextSessionState::default();
    ctx.set_hierarchy(sample_hierarchy());
    // All 3 above viewport, but max is 2 — keep innermost 2
    let rows = state.header_rows(Some(&ctx), 15);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].text, "impl Foo");
    assert_eq!(rows[1].text, "fn bar");
}

#[test]
fn test_header_rows_max_count_one() {
    let mut state = StickyContextState::default();
    state.options.max_count = 1;
    let mut ctx = ContextSessionState::default();
    ctx.set_hierarchy(sample_hierarchy());
    let rows = state.header_rows(Some(&ctx), 15);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text, "fn bar");
}

#[test]
fn test_header_rows_kind_field() {
    let state = StickyContextState::default();
    let mut ctx = ContextSessionState::default();
    ctx.set_hierarchy(sample_hierarchy());
    let rows = state.header_rows(Some(&ctx), 15);
    assert_eq!(rows[0].kind, "mod");
    assert_eq!(rows[1].kind, "class");
    assert_eq!(rows[2].kind, "fn");
}
