use super::*;

/// Helper: create a token span.
fn span(start: u32, end: u32, cat: &str) -> TokenSpan {
    TokenSpan {
        start_byte: start,
        end_byte: end,
        category: cat.to_string(),
    }
}

// =========================================================================
// LayeredTokenCache - Basic tests (single layer)
// =========================================================================

#[test]
fn test_empty_cache() {
    let cache = LayeredTokenCache::new();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.layer_count(), 0);
}

#[test]
fn test_rebuild_line_offsets() {
    let mut cache = LayeredTokenCache::new();
    cache.rebuild_line_offsets("hello\nworld\n");
    assert_eq!(cache.line_offsets, vec![0, 6, 12]);
}

#[test]
fn test_rebuild_line_offsets_empty() {
    let mut cache = LayeredTokenCache::new();
    cache.rebuild_line_offsets("");
    assert_eq!(cache.line_offsets, vec![0]);
}

#[test]
fn test_rebuild_line_offsets_single_line() {
    let mut cache = LayeredTokenCache::new();
    cache.rebuild_line_offsets("hello world");
    assert_eq!(cache.line_offsets, vec![0]);
}

#[test]
fn test_byte_to_position_simple() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello\nworld";
    cache.rebuild_line_offsets(content);

    assert_eq!(cache.byte_to_position(0, content), Some((0, 0)));
    assert_eq!(cache.byte_to_position(3, content), Some((0, 3)));
    assert_eq!(cache.byte_to_position(6, content), Some((1, 0)));
    assert_eq!(cache.byte_to_position(10, content), Some((1, 4)));
}

#[test]
fn test_byte_to_position_past_end() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello";
    cache.rebuild_line_offsets(content);
    assert!(cache.byte_to_position(100, content).is_none());
}

#[test]
fn test_byte_to_position_at_boundary() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello";
    cache.rebuild_line_offsets(content);

    let pos = cache.byte_to_position(5, content);
    assert!(pos.is_some());
    let (line, col) = pos.unwrap();
    assert_eq!(line, 0);
    assert_eq!(col, 5);
}

#[test]
fn test_unicode_byte_to_position() {
    let mut cache = LayeredTokenCache::new();
    let content = "café";
    cache.rebuild_line_offsets(content);

    assert_eq!(cache.byte_to_position(0, content), Some((0, 0)));
    assert_eq!(cache.byte_to_position(1, content), Some((0, 1)));
    assert_eq!(cache.byte_to_position(2, content), Some((0, 2)));
    assert_eq!(cache.byte_to_position(3, content), Some((0, 3)));
}

#[test]
fn test_line_end_col() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello\nworld\n";
    cache.rebuild_line_offsets(content);

    assert_eq!(cache.line_end_col(0, content), 5);
    assert_eq!(cache.line_end_col(1, content), 5);
}

#[test]
fn test_line_end_col_no_trailing_newline() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello\nworld";
    cache.rebuild_line_offsets(content);
    assert_eq!(cache.line_end_col(1, content), 5);
}

#[test]
fn test_line_end_col_empty_trailing_line() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello\n";
    cache.rebuild_line_offsets(content);
    assert_eq!(cache.line_end_col(1, content), 0);
}

// =========================================================================
// LayeredTokenCache - apply_update (backward compat)
// =========================================================================

#[test]
fn test_apply_update_full_refresh() {
    let mut cache = LayeredTokenCache::new();
    let content = "fn main() {}";

    let tokens = vec![span(0, 2, "keyword"), span(3, 7, "function")];

    cache.apply_update(&tokens, 0, 0, true, content);

    assert_eq!(cache.len(), 2);
    assert_eq!(cache.layer_count(), 1);

    let line_tokens = cache.tokens_for_line(0);
    assert_eq!(line_tokens.len(), 2);
    assert_eq!(line_tokens[0].category, "keyword");
    assert_eq!(line_tokens[0].start_col, 0);
    assert_eq!(line_tokens[0].end_col, 2);
}

#[test]
fn test_apply_update_incremental() {
    let mut cache = LayeredTokenCache::new();
    let content = "let x = 1;\nlet y = 2;";

    let tokens1 = vec![span(0, 3, "keyword")];
    cache.apply_update(&tokens1, 0, 0, true, content);

    let tokens2 = vec![span(11, 14, "keyword")];
    cache.apply_update(&tokens2, 1, 1, false, content);

    assert_eq!(cache.tokens_for_line(0).len(), 1);
    assert_eq!(cache.tokens_for_line(1).len(), 1);
}

#[test]
fn test_apply_update_incremental_replaces_range() {
    let mut cache = LayeredTokenCache::new();
    let content = "line0\nline1\nline2";

    let tokens = vec![span(0, 5, "a"), span(6, 11, "b"), span(12, 17, "c")];
    cache.apply_update(&tokens, 0, 2, true, content);
    assert_eq!(cache.len(), 3);

    let new_tokens = vec![span(6, 11, "replaced")];
    cache.apply_update(&new_tokens, 1, 1, false, content);

    let line0 = cache.tokens_for_line(0);
    assert_eq!(line0.len(), 1);
    assert_eq!(line0[0].category, "a");

    let line1 = cache.tokens_for_line(1);
    assert_eq!(line1.len(), 1);
    assert_eq!(line1[0].category, "replaced");

    let line2 = cache.tokens_for_line(2);
    assert_eq!(line2.len(), 1);
    assert_eq!(line2[0].category, "c");
}

#[test]
fn test_multi_line_tokens() {
    let mut cache = LayeredTokenCache::new();
    let content = "fn main() {\n    println!(\"Hello\");\n}";

    let tokens = vec![
        span(0, 2, "keyword"),
        span(3, 7, "function"),
        span(16, 24, "function.builtin"),
        span(25, 32, "string"),
    ];

    cache.apply_update(&tokens, 0, 2, true, content);

    let line0 = cache.tokens_for_line(0);
    assert_eq!(line0.len(), 2);
    assert_eq!(line0[0].category, "keyword");
    assert_eq!(line0[1].category, "function");

    let line1 = cache.tokens_for_line(1);
    assert_eq!(line1.len(), 2);
    assert_eq!(line1[0].category, "function.builtin");
    assert_eq!(line1[1].category, "string");

    assert!(cache.tokens_for_line(2).is_empty());
}

#[test]
fn test_multi_line_token_span() {
    let mut cache = LayeredTokenCache::new();
    let content = "line1\nline2\nline3";

    let tokens = vec![TokenSpan {
        start_byte: 0,
        end_byte: 8,
        category: "comment".to_string(),
    }];

    cache.apply_update(&tokens, 0, 2, true, content);
    let tokens = cache.tokens_for_line(0);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].category, "comment");
    assert_eq!(tokens[0].end_col, 5);
}

#[test]
fn test_token_category_hierarchical_lookup() {
    let mut cache = LayeredTokenCache::new();
    let content = "fn main() {}";

    let tokens = vec![
        span(0, 2, "keyword.function"),
        span(3, 7, "function.definition"),
    ];

    cache.apply_update(&tokens, 0, 0, true, content);

    let line_tokens = cache.tokens_for_line(0);
    assert_eq!(line_tokens.len(), 2);
    assert_eq!(line_tokens[0].category, "keyword.function");
    assert_eq!(line_tokens[1].category, "function.definition");
    assert_eq!(line_tokens[0].start_col, 0);
    assert_eq!(line_tokens[0].end_col, 2);
    assert_eq!(line_tokens[1].start_col, 3);
    assert_eq!(line_tokens[1].end_col, 7);
}

#[test]
fn test_cache_clear() {
    let mut cache = LayeredTokenCache::new();
    let content = "fn main() {}";
    cache.apply_update(&[span(0, 2, "keyword")], 0, 0, true, content);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.layer_count(), 1);

    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.layer_count(), 0);
}

#[test]
fn test_byte_span_to_cached_returns_none() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello";
    cache.rebuild_line_offsets(content);

    let invalid_span = TokenSpan {
        start_byte: 100,
        end_byte: 200,
        category: "error".to_string(),
    };

    cache.apply_update(&[invalid_span], 0, 0, true, content);
    assert!(cache.tokens_for_line(0).is_empty());
}

// =========================================================================
// LayeredTokenCache - Multi-layer tests
// =========================================================================

#[test]
fn test_multi_layer_merge() {
    let mut cache = LayeredTokenCache::new();
    let content = "let x = 42;";

    // Syntax layer (priority 0)
    cache.apply_layer_update("syntax", 0, &[span(0, 3, "keyword")], 0, 0, true, content);

    // LSP semantic layer (priority 10)
    cache.apply_layer_update(
        "lsp.semantic",
        10,
        &[span(4, 5, "variable.local")],
        0,
        0,
        true,
        content,
    );

    assert_eq!(cache.layer_count(), 2);
    assert_eq!(cache.len(), 2);

    let tokens = cache.tokens_for_line(0);
    assert_eq!(tokens.len(), 2);
    // Lower priority (syntax) first
    assert_eq!(tokens[0].category, "keyword");
    // Higher priority (lsp.semantic) second
    assert_eq!(tokens[1].category, "variable.local");
}

#[test]
fn test_multi_layer_priority_order() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello";

    // Insert high priority first
    cache.apply_layer_update("high", 100, &[span(0, 5, "from_high")], 0, 0, true, content);
    // Insert low priority second
    cache.apply_layer_update("low", 0, &[span(0, 5, "from_low")], 0, 0, true, content);

    let tokens = cache.tokens_for_line(0);
    assert_eq!(tokens.len(), 2);
    // Low priority comes first regardless of insertion order
    assert_eq!(tokens[0].category, "from_low");
    assert_eq!(tokens[1].category, "from_high");
}

#[test]
fn test_same_priority_alphabetical_order() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello";

    // Both layers have same priority, ordered alphabetically
    cache.apply_layer_update("zeta", 0, &[span(0, 5, "from_zeta")], 0, 0, true, content);
    cache.apply_layer_update("alpha", 0, &[span(0, 5, "from_alpha")], 0, 0, true, content);

    let tokens = cache.tokens_for_line(0);
    assert_eq!(tokens.len(), 2);
    // Alphabetical: alpha before zeta
    assert_eq!(tokens[0].category, "from_alpha");
    assert_eq!(tokens[1].category, "from_zeta");
}

#[test]
fn test_full_refresh_clears_only_layer() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello world";

    // Two layers
    cache.apply_layer_update("syntax", 0, &[span(0, 5, "keyword")], 0, 0, true, content);
    cache.apply_layer_update("lsp", 10, &[span(6, 11, "variable")], 0, 0, true, content);
    assert_eq!(cache.len(), 2);

    // Full refresh of syntax layer only
    cache.apply_layer_update("syntax", 0, &[span(0, 5, "new_keyword")], 0, 0, true, content);

    assert_eq!(cache.len(), 2);
    let tokens = cache.tokens_for_line(0);
    assert_eq!(tokens.len(), 2);
    // Syntax layer was refreshed
    assert_eq!(tokens[0].category, "new_keyword");
    // LSP layer was NOT touched
    assert_eq!(tokens[1].category, "variable");
}

#[test]
fn test_incremental_update_specific_layer() {
    let mut cache = LayeredTokenCache::new();
    let content = "line0\nline1\nline2";

    // Syntax layer on all lines
    let tokens = vec![span(0, 5, "a"), span(6, 11, "b"), span(12, 17, "c")];
    cache.apply_layer_update("syntax", 0, &tokens, 0, 2, true, content);

    // LSP layer on line 1
    cache.apply_layer_update("lsp", 10, &[span(6, 11, "lsp_b")], 0, 2, true, content);

    // Incremental update to syntax layer line 1 only
    cache.apply_layer_update("syntax", 0, &[span(6, 11, "replaced")], 1, 1, false, content);

    // Syntax line 0 preserved
    let line0 = cache.tokens_for_line(0);
    assert_eq!(line0.len(), 1);
    assert_eq!(line0[0].category, "a");

    // Syntax line 1 replaced, LSP line 1 preserved
    let line1 = cache.tokens_for_line(1);
    assert_eq!(line1.len(), 2);
    assert_eq!(line1[0].category, "replaced"); // syntax (priority 0)
    assert_eq!(line1[1].category, "lsp_b"); // lsp (priority 10)

    // Syntax line 2 preserved
    let line2 = cache.tokens_for_line(2);
    assert_eq!(line2.len(), 1);
    assert_eq!(line2[0].category, "c");
}

#[test]
fn test_multi_layer_tokens_for_line() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello world";

    // Syntax highlight
    cache.apply_layer_update("syntax", 0, &[span(0, 5, "keyword")], 0, 0, true, content);

    // Decoration layer
    cache.apply_layer_update("decorations", 20, &[span(5, 5, "hint")], 0, 0, true, content);

    // tokens_for_line returns all
    assert_eq!(cache.tokens_for_line(0).len(), 2);
}

#[test]
fn test_token_span_with_category() {
    let mut cache = LayeredTokenCache::new();
    let content = "```rust\ncode\n```";

    let conceal_span = TokenSpan {
        start_byte: 0,
        end_byte: 7,
        category: "markup.raw".to_string(),
    };

    cache.apply_layer_update("syntax", 0, &[conceal_span], 0, 0, true, content);

    let tokens = cache.tokens_for_line(0);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].category, "markup.raw");
}

#[test]
fn test_token_span_with_search_category() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello";

    let bg_span = TokenSpan {
        start_byte: 0,
        end_byte: 5,
        category: "search.match".to_string(),
    };

    cache.apply_layer_update("search", 50, &[bg_span], 0, 0, true, content);

    let tokens = cache.tokens_for_line(0);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].category, "search.match");
}

#[test]
fn test_multi_line_token_splits_to_all_lines() {
    let mut cache = LayeredTokenCache::new();
    let content = "line1\nline2\nline3\nline4";

    // Token spanning lines 0-3 (bytes 0..22)
    let bg_span = TokenSpan {
        start_byte: 0,
        end_byte: 22,
        category: "markup.raw.block".to_string(),
    };

    cache.apply_layer_update("decoration", 5, &[bg_span], 0, 3, true, content);

    // All 4 lines should have a token
    for line in 0..4 {
        let tokens = cache.tokens_for_line(line);
        assert!(!tokens.is_empty(), "Line {line} should have a token");
        assert_eq!(tokens[0].category, "markup.raw.block");
    }

    // Line 0: start_col=0, end_col=5
    let t0 = &cache.tokens_for_line(0)[0];
    assert_eq!((t0.start_col, t0.end_col), (0, 5));

    // Line 1: start_col=0, end_col=5
    let t1 = &cache.tokens_for_line(1)[0];
    assert_eq!((t1.start_col, t1.end_col), (0, 5));

    // Line 3 (last): start_col=0, end_col=4 ("line4" is 4 chars, no trailing newline)
    let t3 = &cache.tokens_for_line(3)[0];
    assert_eq!((t3.start_col, t3.end_col), (0, 4));
}

#[test]
fn test_layer_priority_update() {
    let mut cache = LayeredTokenCache::new();
    let content = "hello";

    // Create layer with priority 0
    cache.apply_layer_update("lsp", 0, &[span(0, 5, "var")], 0, 0, true, content);

    // Update same layer with new priority
    cache.apply_layer_update("lsp", 10, &[span(0, 5, "var")], 0, 0, true, content);

    // Layer count should still be 1
    assert_eq!(cache.layer_count(), 1);
}

#[test]
fn test_tokens_for_line_no_tokens() {
    let cache = LayeredTokenCache::new();
    assert!(cache.tokens_for_line(0).is_empty());
}

// =========================================================================
// AnnotationCacheManager tests
// =========================================================================

#[test]
fn test_manager_basic() {
    let mut manager = AnnotationCacheManager::new();

    let cache = manager.get_or_create(1);
    assert!(cache.is_empty());

    let cache2 = manager.get_or_create(1);
    assert!(cache2.is_empty());

    let _cache3 = manager.get_or_create(2);

    assert!(manager.get(1).is_some());
    assert!(manager.get(2).is_some());
}

#[test]
fn test_manager_eviction() {
    let mut manager = AnnotationCacheManager::with_max_buffers(2);

    manager.get_or_create(1);
    manager.get_or_create(2);
    manager.get_or_create(3);

    let count = [1_u64, 2, 3]
        .iter()
        .filter(|&&id| manager.get(id).is_some())
        .count();
    assert_eq!(count, 2);
    assert!(manager.get(3).is_some());
}

#[test]
fn test_manager_eviction_with_max_one() {
    let mut manager = AnnotationCacheManager::with_max_buffers(1);

    manager.get_or_create(1);
    assert!(manager.get(1).is_some());

    manager.get_or_create(2);
    assert!(manager.get(1).is_none());
    assert!(manager.get(2).is_some());
}

#[test]
fn test_manager_tokens_for_line_empty() {
    let manager = AnnotationCacheManager::new();
    assert!(manager.tokens_for_line(999, 0).is_empty());
}

#[test]
fn test_manager_remove() {
    let mut manager = AnnotationCacheManager::new();
    manager.get_or_create(1);
    assert!(manager.get(1).is_some());

    manager.remove(1);
    assert!(manager.get(1).is_none());
}

#[test]
fn test_manager_clear() {
    let mut manager = AnnotationCacheManager::new();
    manager.get_or_create(1);
    manager.get_or_create(2);
    manager.get_or_create(3);

    manager.clear();
    assert!(manager.get(1).is_none());
    assert!(manager.get(2).is_none());
    assert!(manager.get(3).is_none());
}

#[test]
fn test_manager_apply_token_update() {
    let mut manager = AnnotationCacheManager::new();
    let content = "fn main() {}";
    let tokens = vec![span(0, 2, "keyword")];

    manager.apply_token_update(1, &tokens, 0, 0, true, content, "syntax", 0);

    let line_tokens = manager.tokens_for_line(1, 0);
    assert_eq!(line_tokens.len(), 1);
    assert_eq!(line_tokens[0].category, "keyword");
}

#[test]
fn test_manager_empty_layer_defaults_to_syntax() {
    let mut manager = AnnotationCacheManager::new();
    let content = "hello";

    // Empty layer name should default to "syntax"
    manager.apply_token_update(1, &[span(0, 5, "keyword")], 0, 0, true, content, "", 0);

    let cache = manager.get(1).unwrap();
    assert_eq!(cache.layer_count(), 1);

    let tokens = cache.tokens_for_line(0);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].category, "keyword");
}

#[test]
fn test_manager_eviction_at_limit() {
    let mut manager = AnnotationCacheManager::with_max_buffers(2);
    manager.get_or_create(1);
    manager.get_or_create(2);

    manager.get_or_create(3);

    let count = [1u64, 2, 3]
        .iter()
        .filter(|&&id| manager.get(id).is_some())
        .count();
    assert_eq!(count, 2);
}

#[test]
fn test_manager_default() {
    let manager = AnnotationCacheManager::default();
    assert!(manager.get(999).is_none());
}

#[test]
fn test_manager_with_max_buffers() {
    let manager = AnnotationCacheManager::with_max_buffers(5);
    assert!(manager.get(0).is_none());
}

#[test]
fn test_manager_get_or_create_existing_at_limit() {
    let mut manager = AnnotationCacheManager::with_max_buffers(2);
    manager.get_or_create(1);
    manager.get_or_create(2);

    // At limit. Accessing existing buffer should NOT trigger eviction.
    manager.get_or_create(1);
    assert!(manager.get(1).is_some());
    assert!(manager.get(2).is_some());
}
