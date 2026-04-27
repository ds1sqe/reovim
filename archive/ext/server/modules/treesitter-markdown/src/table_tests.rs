use super::*;

#[cfg_attr(coverage_nightly, coverage(off))]
fn parse_md(content: &str) -> Tree {
    let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .expect("failed to set language");
    parser.parse(content, None).expect("failed to parse")
}

// ── table_semantic_annotations tests ──

#[cfg_attr(coverage_nightly, coverage(off))]
fn find_table_node(tree: &Tree) -> Option<Node<'_>> {
    fn walk(node: Node<'_>) -> Option<Node<'_>> {
        if node.kind() == "pipe_table" {
            return Some(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = walk(child) {
                return Some(found);
            }
        }
        None
    }
    walk(tree.root_node())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_table_basic_3_rows() {
    let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
    let tree = parse_md(content);
    let table_node = find_table_node(&tree).expect("no pipe_table node");
    let annotations = table_semantic_annotations(table_node, content);

    // Should have table extent annotation
    let table_extent = annotations
        .iter()
        .filter(|a| a.category.as_str() == "markup.table")
        .count();
    assert_eq!(table_extent, 1, "Expected 1 table extent annotation");

    // Should have delimiter annotation
    let delimiters = annotations
        .iter()
        .filter(|a| a.category.as_str() == "markup.table.delimiter")
        .count();
    assert_eq!(delimiters, 1, "Expected 1 delimiter annotation");

    // Should have pipe annotations for header and data rows
    let pipes = annotations
        .iter()
        .filter(|a| a.category.as_str() == "markup.table.pipe")
        .count();
    assert!(pipes >= 4, "Expected at least 4 pipe annotations (header + data), got {pipes}");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_table_pipe_positions() {
    let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
    let tree = parse_md(content);
    let table_node = find_table_node(&tree).expect("no pipe_table node");
    let annotations = table_semantic_annotations(table_node, content);

    let pipe_annotations: Vec<_> = annotations
        .iter()
        .filter(|a| a.category.as_str() == "markup.table.pipe")
        .collect();

    // Each pipe annotation should be exactly 1 byte
    for pa in &pipe_annotations {
        assert_eq!(pa.end_byte - pa.start_byte, 1, "Pipe annotation should be 1 byte");
        assert_eq!(
            content.as_bytes()[pa.start_byte],
            b'|',
            "Pipe annotation should point to | character"
        );
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_table_delimiter_row() {
    let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
    let tree = parse_md(content);
    let table_node = find_table_node(&tree).expect("no pipe_table node");
    let annotations = table_semantic_annotations(table_node, content);

    let delim = annotations
        .iter()
        .find(|a| a.category.as_str() == "markup.table.delimiter")
        .expect("Expected delimiter annotation");

    let delim_text = &content[delim.start_byte..delim.end_byte];
    assert!(delim_text.contains("---"), "Delimiter row should contain dashes");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_table_extent() {
    let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
    let tree = parse_md(content);
    let table_node = find_table_node(&tree).expect("no pipe_table node");
    let annotations = table_semantic_annotations(table_node, content);

    let extent = annotations
        .iter()
        .find(|a| a.category.as_str() == "markup.table")
        .expect("Expected table extent annotation");

    assert_eq!(extent.start_byte, table_node.start_byte());
    assert_eq!(extent.end_byte, table_node.end_byte());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_table_empty_cells() {
    let content = "| A |  |\n|---|---|\n|  | B |\n";
    let tree = parse_md(content);
    let table_node = find_table_node(&tree).expect("no pipe_table node");
    let annotations = table_semantic_annotations(table_node, content);

    // Should still get pipe annotations even with empty cells
    let pipes = annotations
        .iter()
        .filter(|a| a.category.as_str() == "markup.table.pipe")
        .count();
    assert!(pipes >= 4, "Expected pipe annotations even with empty cells, got {pipes}");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_table_single_column() {
    let content = "| A |\n|---|\n| 1 |\n";
    let tree = parse_md(content);
    let table_node = find_table_node(&tree).expect("no pipe_table node");
    let annotations = table_semantic_annotations(table_node, content);

    // Should have table extent + delimiter + pipe annotations
    assert!(!annotations.is_empty());
    assert!(
        annotations
            .iter()
            .any(|a| a.category.as_str() == "markup.table")
    );
    assert!(
        annotations
            .iter()
            .any(|a| a.category.as_str() == "markup.table.delimiter")
    );
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_table_many_columns() {
    let content = "| A | B | C | D | E |\n|---|---|---|---|---|\n| 1 | 2 | 3 | 4 | 5 |\n";
    let tree = parse_md(content);
    let table_node = find_table_node(&tree).expect("no pipe_table node");
    let annotations = table_semantic_annotations(table_node, content);

    // 5 columns = 6 pipes per row, 2 rows (header + data) = 12 pipes
    let pipes = annotations
        .iter()
        .filter(|a| a.category.as_str() == "markup.table.pipe")
        .count();
    assert!(
        pipes >= 12,
        "Expected at least 12 pipe annotations for 5-column table, got {pipes}"
    );
}

// ── Provider tests ──

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn provider_returns_annotations_for_table() {
    let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
    let table_query = Arc::new(Query::new(&language, "(pipe_table) @table").expect("table query"));
    let provider = TableDecorationProvider::new(table_query);

    let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
    let tree = parse_md(content);

    let result = provider.decorations(&tree, content, 0..content.len());
    assert!(!result.is_empty(), "Provider should return annotations");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn provider_caches_results() {
    let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
    let table_query = Arc::new(Query::new(&language, "(pipe_table) @table").expect("table query"));
    let provider = TableDecorationProvider::new(table_query);

    let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
    let tree = parse_md(content);

    let result1 = provider.decorations(&tree, content, 0..content.len());
    let result2 = provider.decorations(&tree, content, 0..content.len());

    // Both calls should return the same annotations
    assert_eq!(result1.len(), result2.len());

    // Cache should have exactly one entry
    assert_eq!(provider.cache.read().unwrap().len(), 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn provider_invalidates_cache_on_content_change() {
    let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
    let table_query = Arc::new(Query::new(&language, "(pipe_table) @table").expect("table query"));
    let provider = TableDecorationProvider::new(table_query);

    let content1 = "| A | B |\n|---|---|\n| 1 | 2 |\n";
    let tree1 = parse_md(content1);
    let _ = provider.decorations(&tree1, content1, 0..content1.len());

    // Different content with same structure
    let content2 = "| X | Y |\n|---|---|\n| 3 | 4 |\n";
    let tree2 = parse_md(content2);
    let result2 = provider.decorations(&tree2, content2, 0..content2.len());

    assert!(!result2.is_empty());

    // Cache should still have one entry (old one evicted by range match)
    assert_eq!(provider.cache.read().unwrap().len(), 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn provider_skips_tables_outside_range() {
    let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
    let table_query = Arc::new(Query::new(&language, "(pipe_table) @table").expect("table query"));
    let provider = TableDecorationProvider::new(table_query);

    let content = "Some text.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nMore text.\n";
    let tree = parse_md(content);

    // Query only the first line (before the table)
    let result = provider.decorations(&tree, content, 0..11);
    assert!(result.is_empty(), "Should return no annotations for range before table");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn no_table_returns_empty() {
    let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
    let table_query = Arc::new(Query::new(&language, "(pipe_table) @table").expect("table query"));
    let provider = TableDecorationProvider::new(table_query);

    let content = "# Just a heading\n\nSome text.\n";
    let tree = parse_md(content);

    let result = provider.decorations(&tree, content, 0..content.len());
    assert!(result.is_empty());
}

// ── hash_content tests ──

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn hash_same_content_same_hash() {
    assert_eq!(hash_content("hello"), hash_content("hello"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn hash_different_content_different_hash() {
    assert_ne!(hash_content("hello"), hash_content("world"));
}
