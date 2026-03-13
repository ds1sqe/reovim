use super::*;

fn entry(key: &'static str) -> DepEntry<&'static str> {
    DepEntry {
        key,
        required: vec![],
        optional: vec![],
        provides_caps: vec![],
        requires_caps: vec![],
    }
}

fn entry_with_deps(key: &'static str, deps: &[&'static str]) -> DepEntry<&'static str> {
    DepEntry {
        key,
        required: deps.to_vec(),
        optional: vec![],
        provides_caps: vec![],
        requires_caps: vec![],
    }
}

fn entry_with_opt(
    key: &'static str,
    deps: &[&'static str],
    opt: &[&'static str],
) -> DepEntry<&'static str> {
    DepEntry {
        key,
        required: deps.to_vec(),
        optional: opt.to_vec(),
        provides_caps: vec![],
        requires_caps: vec![],
    }
}

fn pos(order: &[&str], key: &str) -> usize {
    order.iter().position(|k| *k == key).unwrap()
}

// =========================================================================
// Basic cases
// =========================================================================

#[test]
fn test_empty_input() {
    let result = resolve_dependencies::<&str>(&[]).unwrap();
    assert!(result.order.is_empty());
    assert!(result.dependents.is_empty());
}

#[test]
fn test_single_entry() {
    let result = resolve_dependencies(&[entry("a")]).unwrap();
    assert_eq!(result.order, vec!["a"]);
}

#[test]
fn test_no_dependencies() {
    let result = resolve_dependencies(&[entry("a"), entry("b"), entry("c")]).unwrap();
    assert_eq!(result.order.len(), 3);
}

// =========================================================================
// Ordering
// =========================================================================

#[test]
fn test_simple_chain() {
    let entries = vec![
        entry("a"),
        entry_with_deps("b", &["a"]),
        entry_with_deps("c", &["b"]),
    ];
    let result = resolve_dependencies(&entries).unwrap();

    assert!(pos(&result.order, "a") < pos(&result.order, "b"));
    assert!(pos(&result.order, "b") < pos(&result.order, "c"));
}

#[test]
fn test_diamond_dependency() {
    let entries = vec![
        entry("d"),
        entry_with_deps("b", &["d"]),
        entry_with_deps("c", &["d"]),
        entry_with_deps("a", &["b", "c"]),
    ];
    let result = resolve_dependencies(&entries).unwrap();

    assert!(pos(&result.order, "d") < pos(&result.order, "b"));
    assert!(pos(&result.order, "d") < pos(&result.order, "c"));
    assert!(pos(&result.order, "b") < pos(&result.order, "a"));
    assert!(pos(&result.order, "c") < pos(&result.order, "a"));
}

#[test]
fn test_optional_ordering() {
    let entries = vec![entry("b"), entry_with_opt("a", &[], &["b"])];
    let result = resolve_dependencies(&entries).unwrap();

    assert!(pos(&result.order, "b") < pos(&result.order, "a"));
}

#[test]
fn test_disconnected_components() {
    let entries = vec![
        entry("a"),
        entry_with_deps("b", &["a"]),
        entry("c"),
        entry_with_deps("d", &["c"]),
    ];
    let result = resolve_dependencies(&entries).unwrap();

    assert!(pos(&result.order, "a") < pos(&result.order, "b"));
    assert!(pos(&result.order, "c") < pos(&result.order, "d"));
    assert_eq!(result.order.len(), 4);
}

#[test]
fn test_complex_diamond_with_optional() {
    let entries = vec![
        entry("base"),
        entry_with_deps("left", &["base"]),
        entry_with_opt("right", &["base"], &["left"]),
        entry_with_deps("top", &["left", "right"]),
    ];
    let result = resolve_dependencies(&entries).unwrap();

    assert!(pos(&result.order, "base") < pos(&result.order, "left"));
    assert!(pos(&result.order, "base") < pos(&result.order, "right"));
    // right has optional dep on left, so left should come before right
    assert!(pos(&result.order, "left") < pos(&result.order, "right"));
    assert!(pos(&result.order, "right") < pos(&result.order, "top"));
}

// =========================================================================
// Error cases
// =========================================================================

#[test]
fn test_self_referential() {
    let entries = vec![DepEntry {
        key: "a",
        required: vec!["a"],
        optional: vec![],
        provides_caps: vec![],
        requires_caps: vec![],
    }];
    let err = resolve_dependencies(&entries).unwrap_err();
    assert!(matches!(err, DepgraphError::SelfReferential("a")));
}

#[test]
fn test_self_referential_optional() {
    let entries = vec![DepEntry {
        key: "a",
        required: vec![],
        optional: vec!["a"],
        provides_caps: vec![],
        requires_caps: vec![],
    }];
    let err = resolve_dependencies(&entries).unwrap_err();
    assert!(matches!(err, DepgraphError::SelfReferential("a")));
}

#[test]
fn test_circular_two() {
    let entries = vec![entry_with_deps("a", &["b"]), entry_with_deps("b", &["a"])];
    let err = resolve_dependencies(&entries).unwrap_err();
    assert!(matches!(err, DepgraphError::Cycle(_)));
}

#[test]
fn test_circular_three() {
    let entries = vec![
        entry_with_deps("a", &["c"]),
        entry_with_deps("b", &["a"]),
        entry_with_deps("c", &["b"]),
    ];
    let err = resolve_dependencies(&entries).unwrap_err();
    match err {
        DepgraphError::Cycle(keys) => {
            assert_eq!(keys.len(), 3);
        }
        other => panic!("Expected Cycle, got {other:?}"),
    }
}

#[test]
fn test_missing_required() {
    let entries = vec![entry_with_deps("a", &["nonexistent"])];
    let err = resolve_dependencies(&entries).unwrap_err();
    match err {
        DepgraphError::Missing(pairs) => {
            assert_eq!(pairs.len(), 1);
            assert_eq!(pairs[0], ("a", "nonexistent"));
        }
        other => panic!("Expected Missing, got {other:?}"),
    }
}

#[test]
fn test_missing_multiple_required() {
    let entries = vec![
        entry_with_deps("a", &["x"]),
        entry_with_deps("b", &["y", "z"]),
    ];
    let err = resolve_dependencies(&entries).unwrap_err();
    match err {
        DepgraphError::Missing(pairs) => {
            assert_eq!(pairs.len(), 3);
        }
        other => panic!("Expected Missing, got {other:?}"),
    }
}

#[test]
fn test_missing_optional_ok() {
    let entries = vec![entry_with_opt("a", &[], &["nonexistent"])];
    let result = resolve_dependencies(&entries).unwrap();
    assert_eq!(result.order, vec!["a"]);
}

// =========================================================================
// Reverse dependency map
// =========================================================================

#[test]
fn test_dependents_map() {
    let entries = vec![
        entry("a"),
        entry_with_deps("b", &["a"]),
        entry_with_deps("c", &["a"]),
    ];
    let result = resolve_dependencies(&entries).unwrap();

    let a_deps = result.dependents.get("a").unwrap();
    assert!(a_deps.contains("b"));
    assert!(a_deps.contains("c"));
    assert_eq!(a_deps.len(), 2);
}

#[test]
fn test_dependents_includes_optional() {
    let entries = vec![entry("a"), entry_with_opt("b", &[], &["a"])];
    let result = resolve_dependencies(&entries).unwrap();

    let a_deps = result.dependents.get("a").unwrap();
    assert!(a_deps.contains("b"));
}

// =========================================================================
// Display and Error trait
// =========================================================================

#[test]
fn test_error_display_self_referential() {
    let err = DepgraphError::SelfReferential("foo");
    let msg = format!("{err}");
    assert!(msg.contains("foo"));
    assert!(msg.contains("depends on itself"));
}

#[test]
fn test_error_display_cycle() {
    let err = DepgraphError::Cycle(vec!["a", "b"]);
    let msg = format!("{err}");
    assert!(msg.contains("circular dependency"));
}

#[test]
fn test_error_display_missing() {
    let err = DepgraphError::Missing(vec![("a", "b"), ("c", "d")]);
    let msg = format!("{err}");
    assert!(msg.contains("missing dependencies"));
    assert!(msg.contains("requires"));
}

#[test]
fn test_error_is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(DepgraphError::SelfReferential("test"));
    assert!(!err.to_string().is_empty());
}

// =========================================================================
// Clone and genericity
// =========================================================================

#[test]
fn test_dep_entry_clone() {
    let e = entry_with_deps("a", &["b"]);
    let cloned = e.clone();
    assert_eq!(cloned.key, "a");
    assert_eq!(cloned.required, vec!["b"]);
}

#[test]
fn test_dependency_order_clone() {
    let result = resolve_dependencies(&[entry("a"), entry_with_deps("b", &["a"])]).unwrap();
    let cloned = result.clone();
    assert_eq!(cloned.order, result.order);
}

#[test]
fn test_with_custom_key_type() {
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct ModId(u32);

    let entries = vec![
        DepEntry {
            key: ModId(1),
            required: vec![],
            optional: vec![],
            provides_caps: vec![],
            requires_caps: vec![],
        },
        DepEntry {
            key: ModId(2),
            required: vec![ModId(1)],
            optional: vec![],
            provides_caps: vec![],
            requires_caps: vec![],
        },
    ];
    let result = resolve_dependencies(&entries).unwrap();
    assert_eq!(result.order, vec![ModId(1), ModId(2)]);
}

// =========================================================================
// Scale test
// =========================================================================

#[test]
fn test_large_graph() {
    // 100 entries in a linear chain: 0 -> 1 -> 2 -> ... -> 99
    let entries: Vec<DepEntry<usize>> = (0..100)
        .map(|i| {
            if i == 0 {
                DepEntry {
                    key: i,
                    required: vec![],
                    optional: vec![],
                    provides_caps: vec![],
                    requires_caps: vec![],
                }
            } else {
                DepEntry {
                    key: i,
                    required: vec![i - 1],
                    optional: vec![],
                    provides_caps: vec![],
                    requires_caps: vec![],
                }
            }
        })
        .collect();

    let result = resolve_dependencies(&entries).unwrap();
    assert_eq!(result.order.len(), 100);

    // Verify all ordering invariants
    for i in 1..100 {
        let prev_pos = result.order.iter().position(|&k| k == i - 1).unwrap();
        let curr_pos = result.order.iter().position(|&k| k == i).unwrap();
        assert!(prev_pos < curr_pos, "Entry {i} should come after {}", i - 1);
    }
}

// =========================================================================
// Capability-based resolution (#618)
// =========================================================================

fn entry_with_caps(
    key: &'static str,
    provides: &[&'static str],
    requires: &[&'static str],
) -> DepEntry<&'static str> {
    DepEntry {
        key,
        required: vec![],
        optional: vec![],
        provides_caps: provides.to_vec(),
        requires_caps: requires.to_vec(),
    }
}

#[test]
fn test_capability_single_provider() {
    let entries = vec![
        entry_with_caps("provider", &["buffer-manager"], &[]),
        entry_with_caps("consumer", &[], &["buffer-manager"]),
    ];
    let result = resolve_dependencies(&entries).unwrap();
    assert_eq!(result.order, vec!["provider", "consumer"]);
    // Consumer depends on provider via capability
    assert!(result.dependents["provider"].contains("consumer"));
}

#[test]
fn test_capability_multiple_providers() {
    // First registered provider wins
    let entries = vec![
        entry_with_caps("provider-a", &["syntax"], &[]),
        entry_with_caps("provider-b", &["syntax"], &[]),
        entry_with_caps("consumer", &[], &["syntax"]),
    ];
    let result = resolve_dependencies(&entries).unwrap();
    // consumer must come after provider-a (first provider)
    let pos_a = pos(&result.order, "provider-a");
    let pos_c = pos(&result.order, "consumer");
    assert!(pos_a < pos_c, "consumer must come after first provider");
}

#[test]
fn test_capability_unsatisfied() {
    let entries = vec![entry_with_caps("consumer", &[], &["missing-cap"])];
    let result = resolve_dependencies(&entries);
    assert!(result.is_err());
    match result.unwrap_err() {
        DepgraphError::UnsatisfiedCapability(pairs) => {
            assert_eq!(pairs.len(), 1);
            assert_eq!(pairs[0].0, "consumer");
            assert_eq!(pairs[0].1, "missing-cap");
        }
        other => panic!("Expected UnsatisfiedCapability, got {other:?}"),
    }
}

#[test]
fn test_capability_self_provide_and_require() {
    // A module that provides and requires the same cap should not create a self-edge
    let entries = vec![entry_with_caps("self-sufficient", &["cap-x"], &["cap-x"])];
    let result = resolve_dependencies(&entries).unwrap();
    assert_eq!(result.order, vec!["self-sufficient"]);
}

#[test]
fn test_capability_combined_with_explicit_deps() {
    // Explicit deps + capability deps coexist
    let entries = vec![
        DepEntry {
            key: "base",
            required: vec![],
            optional: vec![],
            provides_caps: vec!["undo"],
            requires_caps: vec![],
        },
        DepEntry {
            key: "middle",
            required: vec!["base"],
            optional: vec![],
            provides_caps: vec!["search"],
            requires_caps: vec![],
        },
        DepEntry {
            key: "top",
            required: vec![],
            optional: vec![],
            provides_caps: vec![],
            requires_caps: vec!["undo", "search"],
        },
    ];
    let result = resolve_dependencies(&entries).unwrap();
    let pos_base = pos(&result.order, "base");
    let pos_mid = pos(&result.order, "middle");
    let pos_top = pos(&result.order, "top");
    assert!(pos_base < pos_mid, "base before middle (explicit dep)");
    assert!(pos_base < pos_top, "base before top (cap: undo)");
    assert!(pos_mid < pos_top, "middle before top (cap: search)");
}

#[test]
fn test_capability_error_display() {
    let err: DepgraphError<&str> =
        DepgraphError::UnsatisfiedCapability(vec![("mod-a", "cap-x".into())]);
    let msg = format!("{err}");
    assert!(msg.contains("unsatisfied capabilities"));
    assert!(msg.contains("mod-a"));
    assert!(msg.contains("cap-x"));
}

#[test]
fn test_capability_no_caps_no_change() {
    // Entries with empty caps behave exactly like before
    let entries = vec![entry_with_deps("a", &[]), entry_with_deps("b", &["a"])];
    let result = resolve_dependencies(&entries).unwrap();
    assert_eq!(result.order, vec!["a", "b"]);
}
