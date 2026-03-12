use super::*;

#[test]
fn source_id() {
    let source = LspCompletionSource::new();
    assert_eq!(source.id(), "lsp");
}

#[test]
fn source_priority() {
    let source = LspCompletionSource::new();
    assert_eq!(source.priority(), 200);
}

#[test]
fn available_with_language_id() {
    let source = LspCompletionSource::new();
    let ctx = make_ctx("", "", Some("rust"));
    assert!(source.is_available(&ctx));
}

#[test]
fn not_available_without_language_id() {
    let source = LspCompletionSource::new();
    let ctx = make_ctx("", "", None);
    assert!(!source.is_available(&ctx));
}

#[test]
fn complete_empty_cache() {
    let source = LspCompletionSource::new();
    let ctx = make_ctx("let x =", "x", Some("rust"));
    let items = source.complete(&ctx);
    assert!(items.is_empty());
}

#[test]
fn complete_returns_cached_items() {
    let source = LspCompletionSource::new();
    source.update_cache(vec![
        make_item("println", CompletionKind::Function),
        make_item("print", CompletionKind::Function),
    ]);

    let ctx = make_ctx("let x = pr", "pr", Some("rust"));
    let items = source.complete(&ctx);
    assert_eq!(items.len(), 2);
}

#[test]
fn complete_filters_by_prefix() {
    let source = LspCompletionSource::new();
    source.update_cache(vec![
        make_item("println", CompletionKind::Function),
        make_item("print", CompletionKind::Function),
        make_item("format", CompletionKind::Function),
    ]);

    let ctx = make_ctx("let x = pri", "pri", Some("rust"));
    let items = source.complete(&ctx);
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| i.label.starts_with("pri")));
}

#[test]
fn complete_case_insensitive_filter() {
    let source = LspCompletionSource::new();
    source.update_cache(vec![
        make_item("String", CompletionKind::Class),
        make_item("str", CompletionKind::Keyword),
    ]);

    let ctx = make_ctx("let x: st", "st", Some("rust"));
    let items = source.complete(&ctx);
    assert_eq!(items.len(), 2);
}

#[test]
fn complete_empty_prefix_returns_all() {
    let source = LspCompletionSource::new();
    source.update_cache(vec![
        make_item("foo", CompletionKind::Function),
        make_item("bar", CompletionKind::Variable),
    ]);

    let ctx = make_ctx("let x = ", "", Some("rust"));
    let items = source.complete(&ctx);
    assert_eq!(items.len(), 2);
}

#[test]
fn clear_cache() {
    let source = LspCompletionSource::new();
    source.update_cache(vec![make_item("foo", CompletionKind::Text)]);
    assert!(!source.is_cache_empty());
    assert_eq!(source.cache_len(), 1);

    source.clear_cache();
    assert!(source.is_cache_empty());
    assert_eq!(source.cache_len(), 0);
}

#[test]
fn default_creates_empty() {
    let source = LspCompletionSource::default();
    assert!(source.is_cache_empty());
}

#[test]
fn debug_impl() {
    let source = LspCompletionSource::new();
    let debug = format!("{source:?}");
    assert!(debug.contains("LspCompletionSource"));
}

// ========================================================================
// map_lsp_item tests
// ========================================================================

#[test]
fn map_lsp_item_basic() {
    let lsp_item = lsp_types::CompletionItem {
        label: "my_func".to_owned(),
        kind: Some(lsp_types::CompletionItemKind::FUNCTION),
        detail: Some("fn() -> i32".to_owned()),
        insert_text: None,
        ..Default::default()
    };
    let item = map_lsp_item(&lsp_item);
    assert_eq!(item.label, "my_func");
    assert_eq!(item.insert_text, "my_func"); // Falls back to label.
    assert_eq!(item.kind, CompletionKind::Function);
    assert_eq!(item.detail.as_deref(), Some("fn() -> i32"));
    assert_eq!(item.source_id, "lsp");
    assert!(!item.is_snippet);
}

#[test]
fn map_lsp_item_with_insert_text() {
    let lsp_item = lsp_types::CompletionItem {
        label: "println!".to_owned(),
        insert_text: Some("println!(\"$1\")$0".to_owned()),
        insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
        kind: Some(lsp_types::CompletionItemKind::SNIPPET),
        ..Default::default()
    };
    let item = map_lsp_item(&lsp_item);
    assert_eq!(item.label, "println!");
    assert_eq!(item.insert_text, "println!(\"$1\")$0");
    assert!(item.is_snippet);
    assert_eq!(item.kind, CompletionKind::Snippet);
}

#[test]
fn map_lsp_item_with_string_documentation() {
    let lsp_item = lsp_types::CompletionItem {
        label: "test".to_owned(),
        documentation: Some(lsp_types::Documentation::String("A test function".to_owned())),
        ..Default::default()
    };
    let item = map_lsp_item(&lsp_item);
    assert_eq!(item.documentation.as_deref(), Some("A test function"));
}

#[test]
fn map_lsp_item_with_markup_documentation() {
    let lsp_item = lsp_types::CompletionItem {
        label: "test".to_owned(),
        documentation: Some(lsp_types::Documentation::MarkupContent(
            lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::Markdown,
                value: "# Test\nA test.".to_owned(),
            },
        )),
        ..Default::default()
    };
    let item = map_lsp_item(&lsp_item);
    assert_eq!(item.documentation.as_deref(), Some("# Test\nA test."));
}

#[test]
fn map_lsp_item_no_kind() {
    let lsp_item = lsp_types::CompletionItem {
        label: "something".to_owned(),
        kind: None,
        ..Default::default()
    };
    let item = map_lsp_item(&lsp_item);
    assert_eq!(item.kind, CompletionKind::Text);
}

#[test]
fn map_lsp_item_no_documentation() {
    let lsp_item = lsp_types::CompletionItem {
        label: "test".to_owned(),
        documentation: None,
        ..Default::default()
    };
    let item = map_lsp_item(&lsp_item);
    assert!(item.documentation.is_none());
}

#[test]
fn map_lsp_item_plain_text_format() {
    let lsp_item = lsp_types::CompletionItem {
        label: "test".to_owned(),
        insert_text_format: Some(lsp_types::InsertTextFormat::PLAIN_TEXT),
        ..Default::default()
    };
    let item = map_lsp_item(&lsp_item);
    assert!(!item.is_snippet);
}

// ========================================================================
// map_lsp_kind tests
// ========================================================================

#[test]
fn map_all_known_kinds() {
    let mappings = [
        (lsp_types::CompletionItemKind::FUNCTION, CompletionKind::Function),
        (lsp_types::CompletionItemKind::METHOD, CompletionKind::Method),
        (lsp_types::CompletionItemKind::VARIABLE, CompletionKind::Variable),
        (lsp_types::CompletionItemKind::FIELD, CompletionKind::Field),
        (lsp_types::CompletionItemKind::KEYWORD, CompletionKind::Keyword),
        (lsp_types::CompletionItemKind::SNIPPET, CompletionKind::Snippet),
        (lsp_types::CompletionItemKind::MODULE, CompletionKind::Module),
        (lsp_types::CompletionItemKind::CLASS, CompletionKind::Class),
        (lsp_types::CompletionItemKind::STRUCT, CompletionKind::Class),
        (lsp_types::CompletionItemKind::INTERFACE, CompletionKind::Interface),
        (lsp_types::CompletionItemKind::PROPERTY, CompletionKind::Property),
        (lsp_types::CompletionItemKind::CONSTANT, CompletionKind::Constant),
        (lsp_types::CompletionItemKind::ENUM, CompletionKind::Enum),
        (lsp_types::CompletionItemKind::ENUM_MEMBER, CompletionKind::EnumMember),
        (lsp_types::CompletionItemKind::FILE, CompletionKind::File),
        (lsp_types::CompletionItemKind::FOLDER, CompletionKind::Folder),
        (lsp_types::CompletionItemKind::TYPE_PARAMETER, CompletionKind::TypeParameter),
    ];

    for (lsp_kind, expected) in &mappings {
        assert_eq!(map_lsp_kind(*lsp_kind), *expected, "Failed for {lsp_kind:?}");
    }
}

#[test]
fn map_unknown_kind_to_text() {
    // CompletionItemKind::TEXT maps to the default case.
    assert_eq!(map_lsp_kind(lsp_types::CompletionItemKind::TEXT), CompletionKind::Text);
}

#[test]
fn map_other_kinds_to_text() {
    // Other kinds not explicitly mapped should default to Text.
    assert_eq!(map_lsp_kind(lsp_types::CompletionItemKind::COLOR), CompletionKind::Text);
    assert_eq!(map_lsp_kind(lsp_types::CompletionItemKind::REFERENCE), CompletionKind::Text);
    assert_eq!(map_lsp_kind(lsp_types::CompletionItemKind::UNIT), CompletionKind::Text);
    assert_eq!(map_lsp_kind(lsp_types::CompletionItemKind::VALUE), CompletionKind::Text);
    assert_eq!(map_lsp_kind(lsp_types::CompletionItemKind::EVENT), CompletionKind::Text);
    assert_eq!(map_lsp_kind(lsp_types::CompletionItemKind::OPERATOR), CompletionKind::Text);
}

// ========================================================================
// Test helpers
// ========================================================================

fn make_ctx(content: &str, prefix: &str, language_id: Option<&str>) -> CompletionContext {
    CompletionContext {
        content: content.to_owned(),
        cursor_offset: content.len(),
        line: 0,
        col: prefix.len(),
        prefix: prefix.to_owned(),
        buffer_id: 1,
        file_path: None,
        language_id: language_id.map(String::from),
    }
}

fn make_item(label: &str, kind: CompletionKind) -> CompletionItem {
    CompletionItem {
        label: label.to_owned(),
        insert_text: label.to_owned(),
        kind,
        detail: None,
        documentation: None,
        source_id: "lsp",
        is_snippet: false,
        sort_priority: 200,
    }
}
