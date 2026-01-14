//! Tests for markdown language support
//!
//! These tests verify that the markdown and markdown_inline grammars
//! work correctly with the decoration queries.

use {reovim_plugin_treesitter::LanguageSupport, tree_sitter::StreamingIterator};

use crate::{MarkdownInlineLanguage, MarkdownLanguage};

// ============================================================================
// LanguageSupport trait implementation tests
// ============================================================================

#[test]
fn test_markdown_language_id() {
    let lang = MarkdownLanguage;
    assert_eq!(lang.language_id(), "markdown");
}

#[test]
fn test_markdown_file_extensions() {
    let lang = MarkdownLanguage;
    let exts = lang.file_extensions();
    assert!(exts.contains(&"md"));
    assert!(exts.contains(&"markdown"));
}

#[test]
fn test_markdown_inline_language_id() {
    let lang = MarkdownInlineLanguage;
    assert_eq!(lang.language_id(), "markdown_inline");
}

#[test]
fn test_markdown_inline_no_file_extensions() {
    let lang = MarkdownInlineLanguage;
    // Inline is injected, not detected by extension
    assert!(lang.file_extensions().is_empty());
}

// ============================================================================
// Query parsing tests
// ============================================================================

#[test]
fn test_markdown_highlights_query_parses() {
    let lang = MarkdownLanguage;
    let ts_lang = lang.tree_sitter_language();
    let query_src = lang.highlights_query();

    let query = tree_sitter::Query::new(&ts_lang, query_src);
    assert!(query.is_ok(), "Markdown highlights query should parse: {:?}", query.err());
}

#[test]
fn test_markdown_decorations_query_parses() {
    let lang = MarkdownLanguage;
    let ts_lang = lang.tree_sitter_language();
    let query_src = lang
        .decorations_query()
        .expect("Should have decorations query");

    let query = tree_sitter::Query::new(&ts_lang, query_src);
    assert!(query.is_ok(), "Markdown decorations query should parse");
}

#[test]
fn test_markdown_inline_highlights_query_parses() {
    let lang = MarkdownInlineLanguage;
    let ts_lang = lang.tree_sitter_language();
    let query_src = lang.highlights_query();

    let query = tree_sitter::Query::new(&ts_lang, query_src);
    assert!(query.is_ok(), "Markdown inline highlights query should parse");
}

#[test]
fn test_markdown_inline_decorations_query_parses() {
    let lang = MarkdownInlineLanguage;
    let ts_lang = lang.tree_sitter_language();
    let query_src = lang
        .decorations_query()
        .expect("Should have decorations query");

    let query = tree_sitter::Query::new(&ts_lang, query_src);
    assert!(query.is_ok(), "Markdown inline decorations query should parse");
}

// ============================================================================
// Decoration query capture tests
// ============================================================================

#[test]
fn test_heading_markers_captured() {
    let lang = MarkdownLanguage;
    let ts_lang = lang.tree_sitter_language();
    let query_src = lang.decorations_query().unwrap();
    let query = tree_sitter::Query::new(&ts_lang, query_src).unwrap();

    let source = "# Heading 1\n## Heading 2\n### Heading 3\n";
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).unwrap();
    let tree = parser.parse(source, None).unwrap();

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    // Count heading captures
    let mut heading_count = 0;
    while let Some(match_) = matches.next() {
        for capture in match_.captures {
            if query.capture_names()[capture.index as usize].contains("heading") {
                heading_count += 1;
            }
        }
    }

    // At least 3 heading lines and their markers
    assert!(heading_count >= 3, "Should capture heading elements, got {heading_count}");
}

#[test]
fn test_list_bullets_captured() {
    let lang = MarkdownLanguage;
    let ts_lang = lang.tree_sitter_language();
    let query_src = lang.decorations_query().unwrap();
    let query = tree_sitter::Query::new(&ts_lang, query_src).unwrap();

    let source = "- Item 1\n- Item 2\n+ Item 3\n";
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).unwrap();
    let tree = parser.parse(source, None).unwrap();

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut bullet_count = 0;
    while let Some(match_) = matches.next() {
        for capture in match_.captures {
            if query.capture_names()[capture.index as usize].contains("bullet") {
                bullet_count += 1;
            }
        }
    }

    assert_eq!(bullet_count, 3, "Should capture 3 list bullets, got {bullet_count}");
}

#[test]
fn test_code_blocks_captured() {
    let lang = MarkdownLanguage;
    let ts_lang = lang.tree_sitter_language();
    let query_src = lang.decorations_query().unwrap();
    let query = tree_sitter::Query::new(&ts_lang, query_src).unwrap();

    let source = "```rust\nfn main() {}\n```\n";
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).unwrap();
    let tree = parser.parse(source, None).unwrap();

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut code_count = 0;
    while let Some(match_) = matches.next() {
        for capture in match_.captures {
            if query.capture_names()[capture.index as usize].contains("code_block") {
                code_count += 1;
            }
        }
    }

    assert!(code_count > 0, "Should capture code block");
}

#[test]
fn test_emphasis_captured() {
    let lang = MarkdownInlineLanguage;
    let ts_lang = lang.tree_sitter_language();
    let query_src = lang.decorations_query().unwrap();
    let query = tree_sitter::Query::new(&ts_lang, query_src).unwrap();

    let source = "*italic* and **bold**";
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).unwrap();
    let tree = parser.parse(source, None).unwrap();

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut emphasis_count = 0;
    while let Some(match_) = matches.next() {
        for capture in match_.captures {
            let name = &query.capture_names()[capture.index as usize];
            if name.contains("emphasis") || name.contains("strong") {
                emphasis_count += 1;
            }
        }
    }

    assert!(emphasis_count >= 2, "Should capture emphasis and strong, got {emphasis_count}");
}

#[test]
fn test_links_captured() {
    let lang = MarkdownInlineLanguage;
    let ts_lang = lang.tree_sitter_language();
    let query_src = lang.decorations_query().unwrap();
    let query = tree_sitter::Query::new(&ts_lang, query_src).unwrap();

    let source = "[link text](https://example.com)";
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).unwrap();
    let tree = parser.parse(source, None).unwrap();

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut link_count = 0;
    while let Some(match_) = matches.next() {
        for capture in match_.captures {
            if query.capture_names()[capture.index as usize].contains("link") {
                link_count += 1;
            }
        }
    }

    assert!(link_count > 0, "Should capture inline link");
}

// ============================================================================
// Injection query tests
// ============================================================================

#[test]
fn test_markdown_injection_query_parses() {
    let lang = MarkdownLanguage;
    let ts_lang = lang.tree_sitter_language();
    let query_src = lang
        .injections_query()
        .expect("Should have injections query");

    let query = tree_sitter::Query::new(&ts_lang, query_src);
    assert!(query.is_ok(), "Markdown injection query should parse: {:?}", query.err());

    let query = query.unwrap();
    println!("Injection query captures: {:?}", query.capture_names());
    println!("Injection query pattern count: {}", query.pattern_count());

    // Verify required captures exist
    let names = query.capture_names();
    assert!(names.contains(&"injection.content"), "Should have injection.content capture");
}

#[test]
fn test_injection_detects_code_blocks() {
    let lang = MarkdownLanguage;
    let ts_lang = lang.tree_sitter_language();
    let query_src = lang
        .injections_query()
        .expect("Should have injections query");
    let query = tree_sitter::Query::new(&ts_lang, query_src).unwrap();

    let source = r#"# Title

```rust
fn main() {}
```

Some text.

```python
print("hello")
```
"#;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).unwrap();
    let tree = parser.parse(source, None).unwrap();

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut injections = Vec::new();
    while let Some(match_) = matches.next() {
        let mut lang_id = None;
        let mut content_range = None;

        // Check for #set! predicates
        for prop in query.property_settings(match_.pattern_index) {
            if &*prop.key == "injection.language"
                && let Some(value) = &prop.value
            {
                lang_id = Some(value.to_string());
            }
        }

        for capture in match_.captures {
            let name = &query.capture_names()[capture.index as usize];
            if name == &"injection.language" {
                lang_id = Some(
                    capture
                        .node
                        .utf8_text(source.as_bytes())
                        .unwrap_or("")
                        .to_string(),
                );
            }
            if name == &"injection.content" {
                content_range =
                    Some((capture.node.start_position().row, capture.node.end_position().row));
            }
        }

        if let (Some(lang), Some(range)) = (lang_id, content_range) {
            injections.push((lang, range));
        }
    }

    println!("Found {} injections:", injections.len());
    for (lang, (start, end)) in &injections {
        println!("  {}: lines {}-{}", lang, start, end);
    }

    // Should find exactly 2 injections: rust and python code blocks
    assert_eq!(injections.len(), 2, "Should have exactly 2 code block injections");
    assert!(injections.iter().any(|(l, _)| l == "rust"), "Should detect rust injection");
    assert!(injections.iter().any(|(l, _)| l == "python"), "Should detect python injection");
}

#[test]
fn test_no_inline_injections() {
    // Verify that we only have code block injections (no inline injections)
    // Inline injections were removed because they caused performance issues
    let lang = MarkdownLanguage;
    let ts_lang = lang.tree_sitter_language();
    let query_src = lang
        .injections_query()
        .expect("Should have injections query");
    let query = tree_sitter::Query::new(&ts_lang, query_src).unwrap();

    // Simulate a realistic markdown document
    let source = r#"# Heading 1

This is a paragraph with some text.

## Heading 2

Another paragraph here.

- List item 1
- List item 2
- List item 3

```rust
fn main() {}
```

More text at the end.
"#;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).unwrap();
    let tree = parser.parse(source, None).unwrap();

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut inline_count = 0;
    let mut code_block_count = 0;

    while let Some(match_) = matches.next() {
        // Check pattern type (inline injections use #set!)
        for prop in query.property_settings(match_.pattern_index) {
            if &*prop.key == "injection.language"
                && let Some(value) = &prop.value
                && value.as_ref() == "markdown_inline"
            {
                inline_count += 1;
            }
        }
        // Check captured language (code block injections use @injection.language)
        for capture in match_.captures {
            let name = &query.capture_names()[capture.index as usize];
            if name == &"injection.language" {
                code_block_count += 1;
            }
        }
    }

    println!("Inline injections: {}", inline_count);
    println!("Code block injections: {}", code_block_count);

    // No inline injections - they were removed for performance
    assert_eq!(inline_count, 0, "Should have no inline injections");
    // Should still have code block injections
    assert_eq!(code_block_count, 1, "Should have 1 code block injection (rust)");
}
