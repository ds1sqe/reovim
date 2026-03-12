use super::*;

#[test]
fn overlay_queries() {
    let cat = ExtensionCategory::Overlay;
    assert!(cat.is_overlay());
    assert!(!cat.is_inline());
}

#[test]
fn inline_queries() {
    let cat = ExtensionCategory::Inline;
    assert!(cat.is_inline());
    assert!(!cat.is_overlay());
}

#[test]
fn copy_semantics() {
    let cat = ExtensionCategory::Overlay;
    let copied = cat;
    assert_eq!(cat, copied);
}

#[test]
fn debug_format() {
    let cat = ExtensionCategory::Inline;
    let debug = format!("{cat:?}");
    assert!(debug.contains("Inline"));
}

#[test]
fn serde_roundtrip_overlay() {
    let cat = ExtensionCategory::Overlay;
    let json = serde_json::to_string(&cat).unwrap();
    let deserialized: ExtensionCategory = serde_json::from_str(&json).unwrap();
    assert_eq!(cat, deserialized);
}

#[test]
fn serde_roundtrip_inline() {
    let cat = ExtensionCategory::Inline;
    let json = serde_json::to_string(&cat).unwrap();
    let deserialized: ExtensionCategory = serde_json::from_str(&json).unwrap();
    assert_eq!(cat, deserialized);
}

#[test]
fn eq_different_variants() {
    assert_ne!(ExtensionCategory::Overlay, ExtensionCategory::Inline);
}
