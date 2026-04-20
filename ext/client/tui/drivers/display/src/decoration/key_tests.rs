use super::*;

#[test]
fn test_decoration_source_key_new() {
    let key = DecorationSourceKey::new("pair.rainbow");
    assert_eq!(key.as_str(), "pair.rainbow");
}

#[test]
fn test_decoration_source_key_from_static() {
    let key: DecorationSourceKey = "search.match".into();
    assert_eq!(key.as_str(), "search.match");
}

#[test]
fn test_decoration_source_key_from_string() {
    let key: DecorationSourceKey = String::from("visual.selection").into();
    assert_eq!(key.as_str(), "visual.selection");
}

#[test]
fn test_decoration_source_key_equality() {
    let key1 = DecorationSourceKey::new("pair.rainbow");
    let key2 = DecorationSourceKey::new("pair.rainbow");
    let key3 = DecorationSourceKey::new("other");
    assert_eq!(key1, key2);
    assert_ne!(key1, key3);
}

#[test]
fn test_decoration_source_key_display() {
    let key = DecorationSourceKey::new("pair.rainbow");
    assert_eq!(key.to_string(), "pair.rainbow");
}

#[test]
fn test_decoration_provider_key_service_name() {
    assert_eq!(DecorationProviderKey::service_name(), "DecorationProvider");
}

#[test]
fn test_decoration_provider_key_equality() {
    assert_eq!(DecorationProviderKey::Syntax, DecorationProviderKey::Syntax);
    assert_ne!(DecorationProviderKey::Syntax, DecorationProviderKey::Diagnostic);
}

#[test]
fn test_decoration_source_key_service_name() {
    assert_eq!(DecorationSourceKey::service_name(), "BufferDecorationSource");
}

#[test]
fn test_decoration_source_key_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(DecorationSourceKey::new("a"));
    set.insert(DecorationSourceKey::new("b"));
    set.insert(DecorationSourceKey::new("a")); // duplicate
    assert_eq!(set.len(), 2);
}
