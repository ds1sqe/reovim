use super::*;

// ============================================================================
// ScopeKind
// ============================================================================

#[test]
fn scope_kind_is_definition_function() {
    assert!(ScopeKind::Function.is_definition());
}

#[test]
fn scope_kind_is_definition_class() {
    assert!(ScopeKind::Class.is_definition());
}

#[test]
fn scope_kind_is_definition_module() {
    assert!(ScopeKind::Module.is_definition());
}

#[test]
fn scope_kind_is_definition_namespace() {
    assert!(ScopeKind::Namespace.is_definition());
}

#[test]
fn scope_kind_is_not_definition_block() {
    assert!(!ScopeKind::Block.is_definition());
}

#[test]
fn scope_kind_is_not_definition_heading() {
    assert!(!ScopeKind::Heading.is_definition());
}

#[test]
fn scope_kind_as_str() {
    assert_eq!(ScopeKind::Function.as_str(), "fn");
    assert_eq!(ScopeKind::Class.as_str(), "class");
    assert_eq!(ScopeKind::Module.as_str(), "mod");
    assert_eq!(ScopeKind::Block.as_str(), "block");
    assert_eq!(ScopeKind::Heading.as_str(), "heading");
    assert_eq!(ScopeKind::Namespace.as_str(), "namespace");
}

#[test]
fn scope_kind_display() {
    assert_eq!(format!("{}", ScopeKind::Function), "fn");
    assert_eq!(format!("{}", ScopeKind::Class), "class");
}

#[test]
fn scope_kind_clone_eq() {
    let kind = ScopeKind::Function;
    #[allow(clippy::clone_on_copy)]
    let cloned = kind.clone();
    assert_eq!(kind, cloned);
}

#[test]
fn scope_kind_debug() {
    let debug = format!("{:?}", ScopeKind::Function);
    assert!(debug.contains("Function"));
}

#[test]
fn scope_kind_hash() {
    use std::hash::{Hash, Hasher};
    let mut hasher1 = std::collections::hash_map::DefaultHasher::new();
    let mut hasher2 = std::collections::hash_map::DefaultHasher::new();
    ScopeKind::Function.hash(&mut hasher1);
    ScopeKind::Function.hash(&mut hasher2);
    assert_eq!(hasher1.finish(), hasher2.finish());
}

// ============================================================================
// ScopeRange
// ============================================================================

#[test]
fn scope_range_new() {
    let range = ScopeRange::new(5, 20, ScopeKind::Function, "fn main", Some("main".into()));
    assert_eq!(range.start_line, 5);
    assert_eq!(range.end_line, 20);
    assert_eq!(range.kind, ScopeKind::Function);
    assert_eq!(range.display_text, "fn main");
    assert_eq!(range.name, Some("main".into()));
}

#[test]
fn scope_range_new_no_name() {
    let range = ScopeRange::new(0, 10, ScopeKind::Block, "{ ... }", None);
    assert!(range.name.is_none());
}

#[test]
fn scope_range_contains_line_inside() {
    let range = ScopeRange::new(5, 20, ScopeKind::Function, "fn main", None);
    assert!(range.contains_line(10));
}

#[test]
fn scope_range_contains_line_start() {
    let range = ScopeRange::new(5, 20, ScopeKind::Function, "fn main", None);
    assert!(range.contains_line(5));
}

#[test]
fn scope_range_contains_line_end() {
    let range = ScopeRange::new(5, 20, ScopeKind::Function, "fn main", None);
    assert!(range.contains_line(20));
}

#[test]
fn scope_range_contains_line_before() {
    let range = ScopeRange::new(5, 20, ScopeKind::Function, "fn main", None);
    assert!(!range.contains_line(4));
}

#[test]
fn scope_range_contains_line_after() {
    let range = ScopeRange::new(5, 20, ScopeKind::Function, "fn main", None);
    assert!(!range.contains_line(21));
}

#[test]
fn scope_range_line_count() {
    let range = ScopeRange::new(5, 20, ScopeKind::Function, "fn main", None);
    assert_eq!(range.line_count(), 16);
}

#[test]
fn scope_range_line_count_single_line() {
    let range = ScopeRange::new(5, 5, ScopeKind::Function, "fn foo()", None);
    assert_eq!(range.line_count(), 1);
}

#[test]
fn scope_range_is_multiline() {
    let range = ScopeRange::new(5, 20, ScopeKind::Function, "fn main", None);
    assert!(range.is_multiline());
}

#[test]
fn scope_range_is_not_multiline() {
    let range = ScopeRange::new(5, 5, ScopeKind::Function, "fn foo()", None);
    assert!(!range.is_multiline());
}

#[test]
fn scope_range_clone_eq() {
    let range = ScopeRange::new(5, 20, ScopeKind::Function, "fn main", Some("main".into()));
    let cloned = range.clone();
    assert_eq!(range, cloned);
}

#[test]
fn scope_range_debug() {
    let range = ScopeRange::new(5, 20, ScopeKind::Function, "fn main", None);
    let debug = format!("{range:?}");
    assert!(debug.contains("ScopeRange"));
}

// ============================================================================
// ContextHierarchy
// ============================================================================

fn sample_hierarchy() -> ContextHierarchy {
    ContextHierarchy::new(
        42,
        10,
        5,
        vec![
            ScopeRange::new(0, 100, ScopeKind::Module, "mod utils", Some("utils".into())),
            ScopeRange::new(2, 50, ScopeKind::Class, "impl Foo", Some("Foo".into())),
            ScopeRange::new(5, 20, ScopeKind::Function, "fn bar", Some("bar".into())),
        ],
    )
}

#[test]
fn context_hierarchy_new() {
    let ctx = sample_hierarchy();
    assert_eq!(ctx.buffer_id, 42);
    assert_eq!(ctx.line, 10);
    assert_eq!(ctx.col, 5);
    assert_eq!(ctx.items.len(), 3);
}

#[test]
fn context_hierarchy_empty() {
    let ctx = ContextHierarchy::empty();
    assert!(ctx.is_empty());
    assert_eq!(ctx.len(), 0);
    assert_eq!(ctx.buffer_id, 0);
    assert_eq!(ctx.line, 0);
    assert_eq!(ctx.col, 0);
}

#[test]
fn context_hierarchy_default() {
    let ctx = ContextHierarchy::default();
    assert!(ctx.is_empty());
}

#[test]
fn context_hierarchy_is_empty_false() {
    let ctx = sample_hierarchy();
    assert!(!ctx.is_empty());
}

#[test]
fn context_hierarchy_len() {
    let ctx = sample_hierarchy();
    assert_eq!(ctx.len(), 3);
}

#[test]
fn context_hierarchy_len_empty() {
    let ctx = ContextHierarchy::empty();
    assert_eq!(ctx.len(), 0);
}

#[test]
fn context_hierarchy_len_single() {
    let ctx = ContextHierarchy::new(
        0,
        5,
        0,
        vec![ScopeRange::new(0, 10, ScopeKind::Function, "fn main", None)],
    );
    assert_eq!(ctx.len(), 1);
}

#[test]
fn context_hierarchy_current_scope_empty() {
    let ctx = ContextHierarchy::empty();
    assert!(ctx.current_scope().is_none());
}

#[test]
fn context_hierarchy_current_scope_single() {
    let ctx = ContextHierarchy::new(
        0,
        5,
        0,
        vec![ScopeRange::new(
            0,
            10,
            ScopeKind::Function,
            "fn main",
            Some("main".into()),
        )],
    );
    let current = ctx.current_scope().expect("should have current scope");
    assert_eq!(current.display_text, "fn main");
}

#[test]
fn context_hierarchy_current_scope_multiple() {
    let ctx = sample_hierarchy();
    let current = ctx.current_scope().expect("should have current scope");
    assert_eq!(current.display_text, "fn bar");
}

#[test]
fn context_hierarchy_to_breadcrumb_empty() {
    let ctx = ContextHierarchy::empty();
    assert_eq!(ctx.to_breadcrumb(" > "), "");
}

#[test]
fn context_hierarchy_to_breadcrumb_single() {
    let ctx = ContextHierarchy::new(
        0,
        5,
        0,
        vec![ScopeRange::new(0, 10, ScopeKind::Function, "fn main", None)],
    );
    assert_eq!(ctx.to_breadcrumb(" > "), "fn main");
}

#[test]
fn context_hierarchy_to_breadcrumb_multiple() {
    let ctx = sample_hierarchy();
    assert_eq!(ctx.to_breadcrumb(" > "), "mod utils > impl Foo > fn bar");
}

#[test]
fn context_hierarchy_to_breadcrumb_custom_separator() {
    let ctx = sample_hierarchy();
    assert_eq!(ctx.to_breadcrumb(" / "), "mod utils / impl Foo / fn bar");
}

#[test]
fn context_hierarchy_to_breadcrumb_max_under_limit() {
    let ctx = sample_hierarchy();
    assert_eq!(ctx.to_breadcrumb_max(" > ", 5), "mod utils > impl Foo > fn bar");
}

#[test]
fn context_hierarchy_to_breadcrumb_max_at_limit() {
    let ctx = sample_hierarchy();
    assert_eq!(ctx.to_breadcrumb_max(" > ", 3), "mod utils > impl Foo > fn bar");
}

#[test]
fn context_hierarchy_to_breadcrumb_max_over_limit() {
    let ctx = sample_hierarchy();
    assert_eq!(ctx.to_breadcrumb_max(" > ", 2), "... > impl Foo > fn bar");
}

#[test]
fn context_hierarchy_to_breadcrumb_max_one() {
    let ctx = sample_hierarchy();
    assert_eq!(ctx.to_breadcrumb_max(" > ", 1), "... > fn bar");
}

#[test]
fn context_hierarchy_to_breadcrumb_max_zero() {
    let ctx = sample_hierarchy();
    // max=0: skip all items, only "..." remains
    assert_eq!(ctx.to_breadcrumb_max(" > ", 0), "...");
}

#[test]
fn context_hierarchy_to_breadcrumb_max_empty() {
    let ctx = ContextHierarchy::empty();
    assert_eq!(ctx.to_breadcrumb_max(" > ", 3), "");
}

#[test]
fn context_hierarchy_clone() {
    let ctx = sample_hierarchy();
    let cloned = ctx.clone();
    assert_eq!(ctx.items.len(), cloned.items.len());
    assert_eq!(ctx.buffer_id, cloned.buffer_id);
}

#[test]
fn context_hierarchy_debug() {
    let ctx = sample_hierarchy();
    let debug = format!("{ctx:?}");
    assert!(debug.contains("ContextHierarchy"));
}
