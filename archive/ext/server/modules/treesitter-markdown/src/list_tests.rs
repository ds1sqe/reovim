use super::*;

#[cfg_attr(coverage_nightly, coverage(off))]
fn parse_md(content: &str) -> (Tree, Arc<Query>) {
    let language: reovim_driver_syntax_treesitter::Language = tree_sitter_md::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(content, None).unwrap();
    let query = Arc::new(
        Query::new(
            &language,
            "(list_marker_minus) @marker (list_marker_plus) @marker (list_marker_star) @marker",
        )
        .unwrap(),
    );
    (tree, query)
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_list_depth_0_bullet() {
    let content = "- item\n";
    let (tree, query) = parse_md(content);
    let provider = ListDecorationProvider::new(query);
    let annotations = provider.decorations(&tree, content, 0..content.len());

    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].category.as_str(), "markup.list.bullet.0");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_list_depth_1_circle() {
    let content = "- outer\n  - inner\n";
    let (tree, query) = parse_md(content);
    let provider = ListDecorationProvider::new(query);
    let annotations = provider.decorations(&tree, content, 0..content.len());

    assert_eq!(annotations.len(), 2);
    assert_eq!(annotations[0].category.as_str(), "markup.list.bullet.0");
    assert_eq!(annotations[1].category.as_str(), "markup.list.bullet.1");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_list_depth_2_square() {
    let content = "- a\n  - b\n    - c\n";
    let (tree, query) = parse_md(content);
    let provider = ListDecorationProvider::new(query);
    let annotations = provider.decorations(&tree, content, 0..content.len());

    assert_eq!(annotations.len(), 3);
    assert_eq!(annotations[2].category.as_str(), "markup.list.bullet.2");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_list_depth_3_plus() {
    let content = "- a\n  - b\n    - c\n      - d\n";
    let (tree, query) = parse_md(content);
    let provider = ListDecorationProvider::new(query);
    let annotations = provider.decorations(&tree, content, 0..content.len());

    assert_eq!(annotations.len(), 4);
    assert_eq!(annotations[3].category.as_str(), "markup.list.bullet.3");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_list_mixed_markers_nested() {
    let content = "* outer\n  + middle\n    - inner\n";
    let (tree, query) = parse_md(content);
    let provider = ListDecorationProvider::new(query);
    let annotations = provider.decorations(&tree, content, 0..content.len());

    assert_eq!(annotations.len(), 3);
    // Depth 0, 1, 2 regardless of marker character
    assert_eq!(annotations[0].category.as_str(), "markup.list.bullet.0");
    assert_eq!(annotations[1].category.as_str(), "markup.list.bullet.1");
    assert_eq!(annotations[2].category.as_str(), "markup.list.bullet.2");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_list_provider_caching() {
    let content = "- item\n";
    let (tree, query) = parse_md(content);
    let provider = ListDecorationProvider::new(query);

    let a1 = provider.decorations(&tree, content, 0..content.len());
    let a2 = provider.decorations(&tree, content, 0..content.len());

    assert_eq!(a1.len(), a2.len());
    assert_eq!(a1[0].category.as_str(), a2[0].category.as_str());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_list_provider_skips_out_of_range() {
    let content = "- item\n\n- second\n";
    let (tree, query) = parse_md(content);
    let provider = ListDecorationProvider::new(query);

    // Only query first line
    let annotations = provider.decorations(&tree, content, 0..7);
    assert_eq!(annotations.len(), 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_nesting_depth_flat() {
    let content = "- a\n- b\n";
    let (tree, query) = parse_md(content);
    let provider = ListDecorationProvider::new(query);
    let annotations = provider.decorations(&tree, content, 0..content.len());

    // Both at depth 0
    assert_eq!(annotations.len(), 2);
    for ann in &annotations {
        assert_eq!(ann.category.as_str(), "markup.list.bullet.0");
    }
}
