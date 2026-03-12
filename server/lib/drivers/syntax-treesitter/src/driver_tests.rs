use super::*;

#[test]
fn test_syntax_edit_conversion() {
    // Verify SyntaxEdit -> InputEdit conversion logic
    let edit = SyntaxEdit::insert(10, 2, 5, 15, 2, 10);

    let input_edit = InputEdit {
        start_byte: edit.start_byte,
        old_end_byte: edit.old_end_byte,
        new_end_byte: edit.new_end_byte,
        start_position: Point::new(edit.start_row as usize, edit.start_col as usize),
        old_end_position: Point::new(edit.old_end_row as usize, edit.old_end_col as usize),
        new_end_position: Point::new(edit.new_end_row as usize, edit.new_end_col as usize),
    };

    assert_eq!(input_edit.start_byte, 10);
    assert_eq!(input_edit.old_end_byte, 10);
    assert_eq!(input_edit.new_end_byte, 15);
}

// Integration tests with real languages are in the treesitter-rust module

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_driver_with_injections_has_manager() {
    // Driver with injections query should have an injection manager
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());

    // A minimal injections query (doesn't matter what it matches, just needs to compile)
    let injections_query =
        Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());

    let driver = TreeSitterDriver::with_queries(
        "rust",
        &language,
        highlight_query,
        None,
        Some(injections_query),
        None, // No indents query
    )
    .unwrap();

    assert!(
        driver.supports_injections(),
        "Driver with injections query should support injections"
    );
    assert!(driver.injection_manager().is_some(), "Driver should have an injection manager");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_driver_without_injections_no_manager() {
    // Basic driver without injections query should not have a manager
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

    assert!(!driver.supports_injections(), "Basic driver should not support injections");
    assert!(
        driver.injection_manager().is_none(),
        "Basic driver should not have an injection manager"
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_driver_with_queries_no_injections() {
    // Driver with folds but no injections query should not have a manager
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let folds_query = Arc::new(Query::new(&language, "(function_item) @fold").unwrap());
    let driver = TreeSitterDriver::with_queries(
        "rust",
        &language,
        highlight_query,
        Some(folds_query),
        None, // No injections query
        None, // No indents query
    )
    .unwrap();

    assert!(
        !driver.supports_injections(),
        "Driver without injections query should not support injections"
    );
    assert!(
        driver.injection_manager().is_none(),
        "Driver without injections query should not have an injection manager"
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_highlights_with_registered_injection_layer() {
    use crate::InjectionLayer;

    // Create a driver with injection support
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());

    // A minimal injections query that will match string literals as injection content
    let injections_query =
        Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());

    let mut driver = TreeSitterDriver::with_queries(
        "rust",
        &language,
        highlight_query.clone(),
        None,
        Some(injections_query),
        None, // No indents query
    )
    .unwrap();

    // Register a Rust injection layer
    {
        let manager = driver.injection_manager().unwrap();
        let mut manager_guard = manager.lock();
        let layer = InjectionLayer::new("rust", &language, highlight_query).unwrap();
        manager_guard.register_layer(layer);
    }

    // Parse code (the string content won't actually match Rust syntax properly,
    // but this tests the wiring)
    driver.parse("let x = \"hello\";");

    let highlights = driver.highlights(0..100);

    // Should have parent highlights (identifier 'x', etc.)
    assert!(!highlights.is_empty(), "Expected parent highlights at minimum");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_highlights_no_injection_layer_graceful_skip() {
    // Create a driver with injection support but no layers registered
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());

    // An injections query that matches something
    let injections_query =
        Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());

    let mut driver = TreeSitterDriver::with_queries(
        "rust",
        &language,
        highlight_query,
        None,
        Some(injections_query),
        None, // No indents query
    )
    .unwrap();

    // Don't register any injection layers

    // Parse code - should not panic even though injections are detected but no layer exists
    driver.parse("let x = \"hello\";");

    let highlights = driver.highlights(0..100);

    // Should have parent highlights only (no panic, graceful handling)
    assert!(
        !highlights.is_empty(),
        "Expected parent highlights (injection skipped gracefully)"
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_highlights_sorted_by_position() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

    driver.parse("let x = y; let z = w;");

    let highlights = driver.highlights(0..100);

    // Verify highlights are sorted by start_byte
    for i in 1..highlights.len() {
        assert!(
            highlights[i - 1].start_byte <= highlights[i].start_byte,
            "Highlights should be sorted by start_byte"
        );
    }
}

#[test]
fn test_driver_language() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
    assert_eq!(driver.language(), "rust");
}

#[test]
fn test_driver_not_parsed_initially() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
    assert!(!driver.is_parsed());
}

#[test]
fn test_driver_parsed_after_parse() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

    driver.parse("fn main() {}");
    assert!(driver.is_parsed());
}

#[test]
fn test_driver_version_increments() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

    assert_eq!(driver.version(), 0);
    driver.parse("fn main() {}");
    assert_eq!(driver.version(), 1);
    driver.parse("fn foo() {}");
    assert_eq!(driver.version(), 2);
}

#[test]
fn test_driver_last_error_initially_none() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
    assert!(driver.last_error().is_none());
}

#[test]
fn test_driver_highlights_empty_before_parse() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
    let highlights = driver.highlights(0..100);
    assert!(highlights.is_empty());
}

#[test]
fn test_driver_update_increments_version() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

    driver.parse("fn main() {}");
    let v1 = driver.version();

    let edit = SyntaxEdit::insert(12, 0, 12, 20, 0, 20);
    driver.update("fn main() { let x; }", &edit);

    assert_eq!(driver.version(), v1 + 1);
}

#[test]
fn test_driver_injections_empty_without_query() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
    driver.parse("fn main() {}");

    assert!(driver.injections().is_empty());
}

#[test]
fn test_driver_folds_empty_without_query() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
    driver.parse("fn main() {\n    let x = 1;\n}");

    // No folds query, so folds should be empty
    assert!(driver.folds().is_empty());
}

#[test]
fn test_driver_indent_for_none_without_query() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
    driver.parse("fn main() {}");

    assert_eq!(driver.indent_for(0), None);
}

#[test]
fn test_driver_supports_folds() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let folds_query = Arc::new(Query::new(&language, "(function_item) @fold").unwrap());

    let driver = TreeSitterDriver::with_queries(
        "rust",
        &language,
        highlight_query.clone(),
        Some(folds_query),
        None,
        None,
    )
    .unwrap();

    assert!(driver.supports_folds());

    let driver2 = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
    assert!(!driver2.supports_folds());
}

#[test]
fn test_driver_supports_indents() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let indents_query = Arc::new(Query::new(&language, "(block) @indent").unwrap());

    let driver = TreeSitterDriver::with_queries(
        "rust",
        &language,
        highlight_query.clone(),
        None,
        None,
        Some(indents_query),
    )
    .unwrap();

    assert!(driver.supports_indents());

    let driver2 = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
    assert!(!driver2.supports_indents());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_driver_folds_with_query() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    // Query that captures blocks as fold regions
    let folds_query = Arc::new(Query::new(&language, "(block) @fold").unwrap());
    let mut driver = TreeSitterDriver::with_queries(
        "rust",
        &language,
        highlight_query,
        Some(folds_query),
        None,
        None,
    )
    .unwrap();

    driver.parse("fn main() {\n    let x = 1;\n}");

    let folds = driver.folds();
    // Should have at least the function body fold
    assert!(!folds.is_empty(), "Expected at least one fold for a multi-line function");
}

#[test]
fn test_driver_folds_empty_before_parse() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let folds_query = Arc::new(Query::new(&language, "(block) @fold").unwrap());
    let driver = TreeSitterDriver::with_queries(
        "rust",
        &language,
        highlight_query,
        Some(folds_query),
        None,
        None,
    )
    .unwrap();

    // Not parsed yet, should return empty
    assert!(driver.folds().is_empty());
}

#[test]
fn test_driver_indent_for_with_query() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let indents_query = Arc::new(Query::new(&language, "(block) @indent").unwrap());
    let mut driver = TreeSitterDriver::with_queries(
        "rust",
        &language,
        highlight_query,
        None,
        None,
        Some(indents_query),
    )
    .unwrap();

    driver.parse("fn main() {\n    let x = 1;\n}");

    // Line 1 is inside the function body (a block), should have some indent level
    let indent = driver.indent_for(1);
    assert!(indent.is_some());
}

#[test]
fn test_driver_indent_for_before_parse() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let indents_query = Arc::new(Query::new(&language, "(block) @indent").unwrap());
    let driver = TreeSitterDriver::with_queries(
        "rust",
        &language,
        highlight_query,
        None,
        None,
        Some(indents_query),
    )
    .unwrap();

    // Not parsed yet, should return None
    assert_eq!(driver.indent_for(0), None);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_driver_folds_sorted_by_start_line() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let folds_query = Arc::new(Query::new(&language, "(block) @fold").unwrap());
    let mut driver = TreeSitterDriver::with_queries(
        "rust",
        &language,
        highlight_query,
        Some(folds_query),
        None,
        None,
    )
    .unwrap();

    driver.parse("fn foo() {\n    x\n}\n\nfn bar() {\n    y\n}");

    let folds = driver.folds();
    // Verify folds are sorted by start_line
    for i in 1..folds.len() {
        assert!(
            folds[i - 1].start_line <= folds[i].start_line,
            "Folds should be sorted by start_line"
        );
    }
}

#[test]
fn test_driver_update_preserves_highlighting() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

    driver.parse("let x = 1;");
    let h1 = driver.highlights(0..100);
    assert!(!h1.is_empty());

    // Update: add more code
    let edit = SyntaxEdit::insert(10, 0, 10, 22, 0, 22);
    driver.update("let x = 1; let y = 2;", &edit);

    let h2 = driver.highlights(0..100);
    // Should have more highlights after adding more identifiers
    assert!(h2.len() >= h1.len());
}

#[test]
fn test_driver_last_error_cleared_after_parse() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

    driver.parse("fn main() {}");
    assert!(driver.last_error().is_none());
}

#[test]
fn test_driver_injections_empty_before_parse() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let injections_query =
        Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());
    let driver = TreeSitterDriver::with_queries(
        "rust",
        &language,
        highlight_query,
        None,
        Some(injections_query),
        None,
    )
    .unwrap();

    // Not parsed yet, should return empty
    assert!(driver.injections().is_empty());
}

#[test]
fn test_with_tree_returns_none_before_parse() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

    let result = driver.with_tree(|_tree, _content| 42);
    assert!(result.is_none(), "with_tree should return None before parse");
}

#[test]
fn test_with_tree_returns_some_after_parse() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

    driver.parse("fn main() {}");

    let result = driver.with_tree(|tree, content| {
        assert!(!content.is_empty());
        assert_eq!(content, "fn main() {}");
        tree.root_node().kind().to_string()
    });
    assert_eq!(result, Some("source_file".to_string()));
}

#[test]
fn test_with_tree_can_run_query() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

    driver.parse("let x = 1;");

    let query = Query::new(&language, "(identifier) @name").unwrap();
    let count = driver.with_tree(|tree, content| {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
        let mut count = 0;
        while let Some(_m) = matches.next() {
            count += 1;
        }
        count
    });
    assert_eq!(count, Some(1), "Should find one identifier 'x'");
}

// ========================================================================
// Builder Tests
// ========================================================================

#[test]
fn test_builder_basic() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());

    let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .build()
        .unwrap();

    assert_eq!(driver.language(), "rust");
    assert!(!driver.supports_decorations());
    assert!(!driver.supports_folds());
    assert!(!driver.supports_injections());
    assert!(!driver.supports_indents());
}

#[test]
fn test_builder_with_folds() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let folds_query = Arc::new(Query::new(&language, "(function_item) @fold").unwrap());

    let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .folds_query(folds_query)
        .build()
        .unwrap();

    assert!(driver.supports_folds());
}

#[test]
fn test_builder_with_injections() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let injections_query =
        Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());

    let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .injections_query(injections_query)
        .build()
        .unwrap();

    assert!(driver.supports_injections());
}

#[test]
fn test_builder_with_decoration() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let deco_query = Arc::new(Query::new(&language, "(function_item) @fn_block").unwrap());

    let rules = vec![DecorationRule {
        capture_name: "fn_block".into(),
        category: HighlightCategory::new("test.background"),
    }];

    let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .decoration(deco_query, rules)
        .build()
        .unwrap();

    assert!(driver.supports_decorations());
}

#[test]
fn test_builder_all_options() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let folds_query = Arc::new(Query::new(&language, "(function_item) @fold").unwrap());
    let injections_query =
        Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());
    let indents_query = Arc::new(Query::new(&language, "(block) @indent").unwrap());
    let deco_query = Arc::new(Query::new(&language, "(function_item) @fn_block").unwrap());

    let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .folds_query(folds_query)
        .injections_query(injections_query)
        .indents_query(indents_query)
        .decoration(deco_query, Vec::new())
        .build()
        .unwrap();

    assert!(driver.supports_folds());
    assert!(driver.supports_injections());
    assert!(driver.supports_indents());
    assert!(driver.supports_decorations());
}

// ========================================================================
// Decorations Tests
// ========================================================================

#[test]
fn test_decorations_empty_without_query() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

    driver.parse("fn main() {}");
    let decorations = driver.decorations(0..100);
    assert!(decorations.is_empty(), "Driver without decoration query should return empty");
}

#[test]
fn test_decorations_empty_before_parse() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let deco_query = Arc::new(Query::new(&language, "(function_item) @fn_block").unwrap());

    let rules = vec![DecorationRule {
        capture_name: "fn_block".into(),
        category: HighlightCategory::new("test"),
    }];

    let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .decoration(deco_query, rules)
        .build()
        .unwrap();

    // Not parsed yet
    let decorations = driver.decorations(0..100);
    assert!(decorations.is_empty(), "Should return empty before parse");
}

#[test]
fn test_decorations_returns_annotations_after_parse() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    // Capture function_item nodes as background decorations
    let deco_query = Arc::new(Query::new(&language, "(function_item) @fn_block").unwrap());

    let rules = vec![DecorationRule {
        capture_name: "fn_block".into(),
        category: HighlightCategory::new("test.fn_bg"),
    }];

    let mut driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .decoration(deco_query, rules)
        .build()
        .unwrap();

    driver.parse("fn main() {}");
    let decorations = driver.decorations(0..100);

    assert_eq!(decorations.len(), 1, "Should have one decoration for fn main");
    assert_eq!(decorations[0].category.as_str(), "test.fn_bg");
    assert_eq!(decorations[0].start_byte, 0);
}

#[test]
fn test_decorations_skips_unmatched_captures() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    // Query captures identifiers, but rule only matches "fn_block"
    let deco_query = Arc::new(Query::new(&language, "(identifier) @ident").unwrap());

    let rules = vec![DecorationRule {
        capture_name: "fn_block".into(), // won't match "ident"
        category: HighlightCategory::new("test"),
    }];

    let mut driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .decoration(deco_query, rules)
        .build()
        .unwrap();

    driver.parse("let x = 1;");
    let decorations = driver.decorations(0..100);

    assert!(decorations.is_empty(), "No rules match 'ident' captures, should be empty");
}

#[test]
fn test_decorations_respects_byte_range() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let deco_query = Arc::new(Query::new(&language, "(identifier) @ident").unwrap());

    let rules = vec![DecorationRule {
        capture_name: "ident".into(),
        category: HighlightCategory::new("test"),
    }];

    let mut driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .decoration(deco_query, rules)
        .build()
        .unwrap();

    // "fn main() {}\nfn other() {}" — two functions, identifiers at different positions
    driver.parse("fn main() {}\nfn other() {}");

    // Query only the first line (bytes 0..13)
    let decorations = driver.decorations(0..13);
    // "main" is at bytes 3..7
    assert!(!decorations.is_empty(), "Should find identifier in first line");
    for d in &decorations {
        assert!(d.start_byte < 13, "Decoration at byte {} should be in first line", d.start_byte);
    }
}

#[test]
fn test_decorations_alongside_highlights() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let deco_query = Arc::new(Query::new(&language, "(function_item) @fn_block").unwrap());

    let rules = vec![DecorationRule {
        capture_name: "fn_block".into(),
        category: HighlightCategory::new("test.bg"),
    }];

    let mut driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .decoration(deco_query, rules)
        .build()
        .unwrap();

    driver.parse("fn main() {}");

    // Both should work independently
    let highlights = driver.highlights(0..100);
    let decorations = driver.decorations(0..100);

    assert!(!highlights.is_empty(), "Should have highlights");
    assert!(!decorations.is_empty(), "Should have decorations");

    // Highlights have syntax categories
    for h in &highlights {
        assert!(!h.category.as_str().is_empty());
    }
    // Decorations have the configured category
    for d in &decorations {
        assert_eq!(d.category.as_str(), "test.bg");
    }
}

#[test]
fn test_decorations_conceal_category() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let deco_query = Arc::new(Query::new(&language, "(let_declaration \"let\" @kw)").unwrap());

    let rules = vec![DecorationRule {
        capture_name: "kw".into(),
        category: HighlightCategory::new("keyword.conceal"),
    }];

    let mut driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .decoration(deco_query, rules)
        .build()
        .unwrap();

    driver.parse("let x = 1;");
    let decorations = driver.decorations(0..100);

    assert_eq!(decorations.len(), 1);
    assert_eq!(decorations[0].category.as_str(), "keyword.conceal");
    assert_eq!(decorations[0].start_byte, 0);
    assert_eq!(decorations[0].end_byte, 3); // "let" is 3 bytes
}

// ========================================================================
// Coverage: production code branch tests
// ========================================================================

#[test]
fn test_set_injection_layer_store_no_manager() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());

    // Build WITHOUT injections_query → no injection_manager
    let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .build()
        .unwrap();

    assert!(!driver.supports_injections());

    // Calling set_injection_layer_store should do nothing (false branch of if-let)
    let store = Arc::new(crate::InjectionLayerStore::new());
    driver.set_injection_layer_store(store);
}

#[test]
fn test_set_injection_layer_store_with_manager() {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
    let injections_query =
        Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());

    let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
        .injections_query(injections_query)
        .build()
        .unwrap();

    assert!(driver.supports_injections());

    // Calling set_injection_layer_store should set the store (true branch of if-let)
    let store = Arc::new(crate::InjectionLayerStore::new());
    driver.set_injection_layer_store(store);
}
