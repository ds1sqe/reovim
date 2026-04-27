use super::*;

// ========================================================================
// DecorationRule Tests
// ========================================================================

#[test]
fn test_decoration_rule_creation() {
    let rule = DecorationRule {
        capture_name: "heading.1.marker".into(),
        category: HighlightCategory::new("markup.heading.1"),
    };

    assert_eq!(rule.capture_name.as_ref(), "heading.1.marker");
    assert_eq!(rule.category.as_str(), "markup.heading.1");
}

#[test]
fn test_decoration_rule_clone() {
    let rule = DecorationRule {
        capture_name: "code_block".into(),
        category: HighlightCategory::new("markup.raw.block"),
    };
    #[allow(clippy::redundant_clone)]
    let cloned = rule.clone();
    assert_eq!(cloned.capture_name.as_ref(), "code_block");
    assert_eq!(cloned.category.as_str(), "markup.raw.block");
}

#[test]
fn test_decoration_rule_debug() {
    let rule = DecorationRule {
        capture_name: "test".into(),
        category: HighlightCategory::new("test"),
    };
    let debug = format!("{rule:?}");
    assert!(debug.contains("DecorationRule"));
    assert!(debug.contains("test"));
}

// ========================================================================
// DecorationCapture Tests
// ========================================================================

#[test]
fn test_decoration_capture_creation() {
    let cap = DecorationCapture {
        name: "heading.1.marker".into(),
        start_byte: 0,
        end_byte: 2,
    };

    assert_eq!(cap.name.as_ref(), "heading.1.marker");
    assert_eq!(cap.start_byte, 0);
    assert_eq!(cap.end_byte, 2);
}

#[test]
fn test_decoration_capture_byte_range() {
    let cap = DecorationCapture {
        name: "test".into(),
        start_byte: 10,
        end_byte: 20,
    };
    assert_eq!(cap.byte_range(), 10..20);
}

#[test]
fn test_decoration_capture_len() {
    let cap = DecorationCapture {
        name: "test".into(),
        start_byte: 5,
        end_byte: 15,
    };
    assert_eq!(cap.len(), 10);
}

#[test]
fn test_decoration_capture_is_empty() {
    let empty = DecorationCapture {
        name: "test".into(),
        start_byte: 5,
        end_byte: 5,
    };
    assert!(empty.is_empty());

    let non_empty = DecorationCapture {
        name: "test".into(),
        start_byte: 5,
        end_byte: 10,
    };
    assert!(!non_empty.is_empty());
}

#[test]
fn test_decoration_capture_clone() {
    let cap = DecorationCapture {
        name: "test".into(),
        start_byte: 0,
        end_byte: 5,
    };
    let cloned = cap.clone();
    assert_eq!(cap, cloned);
}

#[test]
fn test_decoration_capture_equality() {
    let a = DecorationCapture {
        name: "test".into(),
        start_byte: 0,
        end_byte: 5,
    };
    let b = DecorationCapture {
        name: "test".into(),
        start_byte: 0,
        end_byte: 5,
    };
    let c = DecorationCapture {
        name: "other".into(),
        start_byte: 0,
        end_byte: 5,
    };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn test_decoration_capture_debug() {
    let cap = DecorationCapture {
        name: "heading".into(),
        start_byte: 0,
        end_byte: 2,
    };
    let debug = format!("{cap:?}");
    assert!(debug.contains("DecorationCapture"));
    assert!(debug.contains("heading"));
}

// ========================================================================
// apply_rules Tests
// ========================================================================

#[test]
fn test_apply_rules_basic_match() {
    let rules = vec![DecorationRule {
        capture_name: "code_block".into(),
        category: HighlightCategory::new("markup.raw.block"),
    }];

    let captures = vec![DecorationCapture {
        name: "code_block".into(),
        start_byte: 10,
        end_byte: 50,
    }];

    let annotations = apply_rules(&captures, &rules);
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].start_byte, 10);
    assert_eq!(annotations[0].end_byte, 50);
    assert_eq!(annotations[0].category.as_str(), "markup.raw.block");
}

#[test]
fn test_apply_rules_heading_category() {
    let rules = vec![DecorationRule {
        capture_name: "heading.1.marker".into(),
        category: HighlightCategory::new("markup.heading.1"),
    }];

    let captures = vec![DecorationCapture {
        name: "heading.1.marker".into(),
        start_byte: 0,
        end_byte: 2,
    }];

    let annotations = apply_rules(&captures, &rules);
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].category.as_str(), "markup.heading.1");
    assert_eq!(annotations[0].start_byte, 0);
    assert_eq!(annotations[0].end_byte, 2);
}

#[test]
fn test_apply_rules_skips_unmatched_captures() {
    let rules = vec![DecorationRule {
        capture_name: "code_block".into(),
        category: HighlightCategory::new("markup.raw.block"),
    }];

    let captures = vec![
        DecorationCapture {
            name: "code_block".into(),
            start_byte: 10,
            end_byte: 50,
        },
        DecorationCapture {
            name: "unknown_capture".into(),
            start_byte: 60,
            end_byte: 70,
        },
    ];

    let annotations = apply_rules(&captures, &rules);
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].start_byte, 10);
}

#[test]
fn test_apply_rules_empty_captures() {
    let rules = vec![DecorationRule {
        capture_name: "code_block".into(),
        category: HighlightCategory::new("markup.raw.block"),
    }];

    let annotations = apply_rules(&[], &rules);
    assert!(annotations.is_empty());
}

#[test]
fn test_apply_rules_empty_rules() {
    let captures = vec![DecorationCapture {
        name: "code_block".into(),
        start_byte: 10,
        end_byte: 50,
    }];

    let annotations = apply_rules(&captures, &[]);
    assert!(annotations.is_empty());
}

#[test]
fn test_apply_rules_both_empty() {
    let annotations = apply_rules(&[], &[]);
    assert!(annotations.is_empty());
}

#[test]
fn test_apply_rules_multiple_rules_multiple_captures() {
    let rules = vec![
        DecorationRule {
            capture_name: "heading.1.marker".into(),
            category: HighlightCategory::new("markup.heading.1"),
        },
        DecorationRule {
            capture_name: "code_block".into(),
            category: HighlightCategory::new("markup.raw.block"),
        },
        DecorationRule {
            capture_name: "list.bullet".into(),
            category: HighlightCategory::new("markup.list"),
        },
    ];

    let captures = vec![
        DecorationCapture {
            name: "heading.1.marker".into(),
            start_byte: 0,
            end_byte: 2,
        },
        DecorationCapture {
            name: "code_block".into(),
            start_byte: 20,
            end_byte: 60,
        },
        DecorationCapture {
            name: "list.bullet".into(),
            start_byte: 70,
            end_byte: 72,
        },
    ];

    let annotations = apply_rules(&captures, &rules);
    assert_eq!(annotations.len(), 3);

    assert_eq!(annotations[0].category.as_str(), "markup.heading.1");
    assert_eq!(annotations[1].category.as_str(), "markup.raw.block");
    assert_eq!(annotations[2].category.as_str(), "markup.list");
}

#[test]
fn test_apply_rules_first_matching_rule_wins() {
    let rules = vec![
        DecorationRule {
            capture_name: "test".into(),
            category: HighlightCategory::new("first"),
        },
        DecorationRule {
            capture_name: "test".into(),
            category: HighlightCategory::new("second"),
        },
    ];

    let captures = vec![DecorationCapture {
        name: "test".into(),
        start_byte: 0,
        end_byte: 10,
    }];

    let annotations = apply_rules(&captures, &rules);
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].category.as_str(), "first");
}

#[test]
fn test_apply_rules_preserves_capture_order() {
    let rules = vec![
        DecorationRule {
            capture_name: "a".into(),
            category: HighlightCategory::new("a"),
        },
        DecorationRule {
            capture_name: "b".into(),
            category: HighlightCategory::new("b"),
        },
    ];

    let captures = vec![
        DecorationCapture {
            name: "b".into(),
            start_byte: 0,
            end_byte: 5,
        },
        DecorationCapture {
            name: "a".into(),
            start_byte: 10,
            end_byte: 15,
        },
    ];

    let annotations = apply_rules(&captures, &rules);
    assert_eq!(annotations.len(), 2);
    // Order preserved from captures, not rules
    assert_eq!(annotations[0].category.as_str(), "b");
    assert_eq!(annotations[1].category.as_str(), "a");
}
