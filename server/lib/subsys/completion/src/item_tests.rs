use super::*;

fn make_item() -> CompletionItem {
    CompletionItem {
        label: "test_func".to_string(),
        insert_text: "test_func()".to_string(),
        kind: CompletionKind::Function,
        detail: Some("fn test_func()".to_string()),
        documentation: None,
        source_id: "test",
        is_snippet: false,
        sort_priority: 100,
    }
}

#[test]
fn item_debug() {
    let item = make_item();
    let debug = format!("{item:?}");
    assert!(debug.contains("test_func"));
    assert!(debug.contains("Function"));
}

#[test]
fn item_clone() {
    let item = make_item();
    #[allow(clippy::redundant_clone)]
    let cloned = item.clone();
    assert_eq!(cloned.label, "test_func");
    assert_eq!(cloned.insert_text, "test_func()");
    assert_eq!(cloned.kind, CompletionKind::Function);
    assert_eq!(cloned.detail.as_deref(), Some("fn test_func()"));
    assert!(cloned.documentation.is_none());
    assert_eq!(cloned.source_id, "test");
    assert!(!cloned.is_snippet);
    assert_eq!(cloned.sort_priority, 100);
}

#[test]
fn item_with_documentation() {
    let item = CompletionItem {
        documentation: Some("Detailed docs here".to_string()),
        ..make_item()
    };
    assert_eq!(item.documentation.as_deref(), Some("Detailed docs here"));
}

#[test]
fn item_snippet_flag() {
    let item = CompletionItem {
        is_snippet: true,
        kind: CompletionKind::Snippet,
        ..make_item()
    };
    assert!(item.is_snippet);
    assert_eq!(item.kind, CompletionKind::Snippet);
}

#[test]
fn kind_debug() {
    let debug = format!("{:?}", CompletionKind::Function);
    assert_eq!(debug, "Function");
}

#[test]
fn kind_clone_copy() {
    let kind = CompletionKind::Method;
    let copied = kind;
    #[allow(clippy::clone_on_copy)]
    let cloned = kind.clone();
    assert_eq!(copied, cloned);
}

#[test]
fn kind_eq() {
    assert_eq!(CompletionKind::Text, CompletionKind::Text);
    assert_ne!(CompletionKind::Text, CompletionKind::Function);
}

#[test]
fn kind_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(CompletionKind::Function);
    set.insert(CompletionKind::Method);
    set.insert(CompletionKind::Function); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn abbreviation_all_variants() {
    let cases = [
        (CompletionKind::Text, "txt"),
        (CompletionKind::Function, "fn"),
        (CompletionKind::Method, "met"),
        (CompletionKind::Variable, "var"),
        (CompletionKind::Field, "fld"),
        (CompletionKind::Keyword, "kw"),
        (CompletionKind::Snippet, "snp"),
        (CompletionKind::Module, "mod"),
        (CompletionKind::Class, "cls"),
        (CompletionKind::Interface, "ifc"),
        (CompletionKind::Property, "prp"),
        (CompletionKind::Constant, "con"),
        (CompletionKind::Enum, "enm"),
        (CompletionKind::EnumMember, "emb"),
        (CompletionKind::File, "fil"),
        (CompletionKind::Folder, "dir"),
        (CompletionKind::TypeParameter, "typ"),
    ];
    for (kind, expected) in cases {
        assert_eq!(kind.abbreviation(), expected, "{kind:?} abbreviation mismatch");
    }
    // Ensure we covered all 17 variants
    assert_eq!(cases.len(), 17);
}

#[test]
fn icon_all_variants_non_empty() {
    let all_kinds = [
        CompletionKind::Text,
        CompletionKind::Function,
        CompletionKind::Method,
        CompletionKind::Variable,
        CompletionKind::Field,
        CompletionKind::Keyword,
        CompletionKind::Snippet,
        CompletionKind::Module,
        CompletionKind::Class,
        CompletionKind::Interface,
        CompletionKind::Property,
        CompletionKind::Constant,
        CompletionKind::Enum,
        CompletionKind::EnumMember,
        CompletionKind::File,
        CompletionKind::Folder,
        CompletionKind::TypeParameter,
    ];
    for kind in all_kinds {
        let icon = kind.icon();
        assert!(!icon.is_empty(), "{kind:?} should have a non-empty icon");
        // Nerd Font icons should be exactly one Unicode char
        assert_eq!(icon.chars().count(), 1, "{kind:?} icon should be a single char");
    }
    assert_eq!(all_kinds.len(), 17);
}
