use super::*;

// ========== OptionScope tests ==========

#[test]
fn test_option_scope_default() {
    assert_eq!(OptionScope::default(), OptionScope::Global);
}

#[test]
fn test_option_scope_display_names() {
    assert_eq!(OptionScope::Global.display_name(), "global");
    assert_eq!(OptionScope::Buffer.display_name(), "buffer");
    assert_eq!(OptionScope::Window.display_name(), "window");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_scope_display() {
    assert_eq!(format!("{}", OptionScope::Global), "global");
    assert_eq!(format!("{}", OptionScope::Buffer), "buffer");
    assert_eq!(format!("{}", OptionScope::Window), "window");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_scope_debug() {
    let debug = format!("{:?}", OptionScope::Global);
    assert_eq!(debug, "Global");
}

#[test]
fn test_option_scope_clone() {
    let scope = OptionScope::Buffer;
    let cloned = scope;
    assert_eq!(scope, cloned);
}

#[test]
fn test_option_scope_eq() {
    assert_eq!(OptionScope::Global, OptionScope::Global);
    assert_ne!(OptionScope::Global, OptionScope::Buffer);
    assert_ne!(OptionScope::Buffer, OptionScope::Window);
}

#[test]
fn test_option_scope_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(OptionScope::Global);
    set.insert(OptionScope::Buffer);
    set.insert(OptionScope::Window);
    set.insert(OptionScope::Global); // duplicate
    assert_eq!(set.len(), 3);
}

// ========== OptionScopeId tests ==========

#[test]
fn test_option_scope_id_default() {
    assert_eq!(OptionScopeId::default(), OptionScopeId::Global);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_scope_id_display_global() {
    assert_eq!(format!("{}", OptionScopeId::Global), "global");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_scope_id_display_buffer() {
    let id = BufferId::from_raw(42);
    let display = format!("{}", OptionScopeId::Buffer(id));
    assert!(display.contains("buffer"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_scope_id_display_window() {
    let id = WindowId::from_raw(7);
    let display = format!("{}", OptionScopeId::Window(id));
    assert!(display.contains("window"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_scope_id_debug() {
    let scope_id = OptionScopeId::Global;
    let debug = format!("{scope_id:?}");
    assert_eq!(debug, "Global");
}

#[test]
fn test_option_scope_id_eq() {
    let id = BufferId::from_raw(1);
    assert_eq!(OptionScopeId::Global, OptionScopeId::Global);
    assert_eq!(OptionScopeId::Buffer(id), OptionScopeId::Buffer(id));
    assert_ne!(OptionScopeId::Global, OptionScopeId::Buffer(id));
}

#[test]
fn test_option_scope_id_hash() {
    use std::collections::HashSet;
    let id1 = BufferId::from_raw(1);
    let id2 = BufferId::from_raw(2);
    let mut set = HashSet::new();
    set.insert(OptionScopeId::Global);
    set.insert(OptionScopeId::Buffer(id1));
    set.insert(OptionScopeId::Buffer(id2));
    set.insert(OptionScopeId::Global); // duplicate
    assert_eq!(set.len(), 3);
}

#[test]
fn test_option_scope_id_clone() {
    let id = BufferId::from_raw(5);
    let scope_id = OptionScopeId::Buffer(id);
    let cloned = scope_id;
    assert_eq!(scope_id, cloned);
}
