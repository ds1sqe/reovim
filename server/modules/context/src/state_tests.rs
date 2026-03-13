use reovim_driver_syntax::{ContextHierarchy, ScopeKind, ScopeRange};

use super::*;

fn sample_hierarchy() -> ContextHierarchy {
    ContextHierarchy::new(
        1,
        10,
        5,
        vec![
            ScopeRange::new(0, 100, ScopeKind::Module, "mod utils", Some("utils".into())),
            ScopeRange::new(5, 20, ScopeKind::Function, "fn bar", Some("bar".into())),
        ],
    )
}

#[test]
fn test_options_default() {
    let opts = ContextOptions::default();
    assert!(opts.breadcrumb);
    assert_eq!(opts.separator, " > ");
    assert_eq!(opts.max_items, 4);
}

#[test]
fn test_options_debug() {
    let opts = ContextOptions::default();
    let debug = format!("{opts:?}");
    assert!(debug.contains("breadcrumb"));
}

#[test]
fn test_options_clone() {
    let opts = ContextOptions::default();
    let cloned = opts.clone();
    assert_eq!(cloned.breadcrumb, opts.breadcrumb);
    assert_eq!(cloned.separator, opts.separator);
    assert_eq!(cloned.max_items, opts.max_items);
}

#[test]
fn test_state_default() {
    let state = ContextSessionState::default();
    assert!(state.hierarchy().is_none());
}

#[test]
fn test_state_debug() {
    let state = ContextSessionState::default();
    let debug = format!("{state:?}");
    assert!(debug.contains("ContextSessionState"));
}

#[test]
fn test_state_set_get_hierarchy() {
    let mut state = ContextSessionState::default();
    state.set_hierarchy(sample_hierarchy());
    assert!(state.hierarchy().is_some());
    assert_eq!(state.hierarchy().unwrap().len(), 2);
}

#[test]
fn test_state_clear() {
    let mut state = ContextSessionState::default();
    state.set_hierarchy(sample_hierarchy());
    assert!(state.hierarchy().is_some());
    state.clear();
    assert!(state.hierarchy().is_none());
}

#[test]
fn test_breadcrumb_text_with_hierarchy() {
    let mut state = ContextSessionState::default();
    state.set_hierarchy(sample_hierarchy());
    let text = state.breadcrumb_text();
    assert_eq!(text, Some("mod utils > fn bar".to_string()));
}

#[test]
fn test_breadcrumb_text_empty_hierarchy() {
    let mut state = ContextSessionState::default();
    state.set_hierarchy(ContextHierarchy::empty());
    assert!(state.breadcrumb_text().is_none());
}

#[test]
fn test_breadcrumb_text_no_hierarchy() {
    let state = ContextSessionState::default();
    assert!(state.breadcrumb_text().is_none());
}

#[test]
fn test_breadcrumb_text_disabled() {
    let mut state = ContextSessionState::default();
    state.options.breadcrumb = false;
    state.set_hierarchy(sample_hierarchy());
    assert!(state.breadcrumb_text().is_none());
}

#[test]
fn test_breadcrumb_text_custom_separator() {
    let mut state = ContextSessionState::default();
    state.options.separator = " / ".to_string();
    state.set_hierarchy(sample_hierarchy());
    assert_eq!(state.breadcrumb_text(), Some("mod utils / fn bar".to_string()));
}

#[test]
fn test_breadcrumb_text_max_items() {
    let mut state = ContextSessionState::default();
    state.options.max_items = 1;
    state.set_hierarchy(sample_hierarchy());
    assert_eq!(state.breadcrumb_text(), Some("... > fn bar".to_string()));
}
