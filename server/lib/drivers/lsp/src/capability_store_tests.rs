use super::*;

fn empty_caps() -> ServerCapabilities {
    ServerCapabilities::default()
}

fn make_registration(id: &str, method: &str) -> Registration {
    Registration {
        id: id.to_string(),
        method: method.to_string(),
        register_options: None,
    }
}

fn make_registration_with_options(
    id: &str,
    method: &str,
    options: serde_json::Value,
) -> Registration {
    Registration {
        id: id.to_string(),
        method: method.to_string(),
        register_options: Some(options),
    }
}

fn make_unregistration(id: &str, method: &str) -> Unregistration {
    Unregistration {
        id: id.to_string(),
        method: method.to_string(),
    }
}

// ========================================================================
// Construction and basic operations
// ========================================================================

#[test]
fn test_new_stores_initial_capabilities() {
    let mut caps = empty_caps();
    caps.hover_provider = Some(HoverProviderCapability::Simple(true));
    let store = CapabilityStore::new(caps);
    let loaded = store.load_full();
    assert!(loaded.hover_provider.is_some());
}

#[test]
fn test_load_full_returns_arc() {
    let store = CapabilityStore::new(empty_caps());
    let arc = store.load_full();
    // Verify it's a valid Arc by cloning
    let _clone = Arc::clone(&arc);
    assert!(arc.hover_provider.is_none());
}

#[test]
fn test_store_replaces_capabilities() {
    let store = CapabilityStore::new(empty_caps());
    assert!(store.load_full().hover_provider.is_none());

    let mut caps = empty_caps();
    caps.hover_provider = Some(HoverProviderCapability::Simple(true));
    store.store(caps);

    assert!(store.load_full().hover_provider.is_some());
}

#[test]
fn test_debug_impl() {
    let store = CapabilityStore::new(empty_caps());
    let debug = format!("{store:?}");
    assert!(debug.contains("CapabilityStore"));
    assert!(debug.contains("registration_count: 0"));
}

#[test]
fn test_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CapabilityStore>();
}

// ========================================================================
// Registration: all 8 capability types
// ========================================================================

#[test]
fn test_apply_registration_completion() {
    let store = CapabilityStore::new(empty_caps());
    let reg = make_registration("r1", "textDocument/completion");
    assert!(store.apply_registration(&reg));
    let caps = store.load_full();
    assert!(caps.completion_provider.is_some());
}

#[test]
fn test_apply_registration_hover() {
    let store = CapabilityStore::new(empty_caps());
    let reg = make_registration("r1", "textDocument/hover");
    assert!(store.apply_registration(&reg));
    let caps = store.load_full();
    assert!(caps.hover_provider.is_some());
}

#[test]
fn test_apply_registration_definition() {
    let store = CapabilityStore::new(empty_caps());
    let reg = make_registration("r1", "textDocument/definition");
    assert!(store.apply_registration(&reg));
    let caps = store.load_full();
    assert!(caps.definition_provider.is_some());
}

#[test]
fn test_apply_registration_references() {
    let store = CapabilityStore::new(empty_caps());
    let reg = make_registration("r1", "textDocument/references");
    assert!(store.apply_registration(&reg));
    let caps = store.load_full();
    assert!(caps.references_provider.is_some());
}

#[test]
fn test_apply_registration_signature_help() {
    let store = CapabilityStore::new(empty_caps());
    let reg = make_registration("r1", "textDocument/signatureHelp");
    assert!(store.apply_registration(&reg));
    let caps = store.load_full();
    assert!(caps.signature_help_provider.is_some());
}

#[test]
fn test_apply_registration_code_action() {
    let store = CapabilityStore::new(empty_caps());
    let reg = make_registration("r1", "textDocument/codeAction");
    assert!(store.apply_registration(&reg));
    let caps = store.load_full();
    assert!(caps.code_action_provider.is_some());
}

#[test]
fn test_apply_registration_formatting() {
    let store = CapabilityStore::new(empty_caps());
    let reg = make_registration("r1", "textDocument/formatting");
    assert!(store.apply_registration(&reg));
    let caps = store.load_full();
    assert!(caps.document_formatting_provider.is_some());
}

#[test]
fn test_apply_registration_document_symbol() {
    let store = CapabilityStore::new(empty_caps());
    let reg = make_registration("r1", "textDocument/documentSymbol");
    assert!(store.apply_registration(&reg));
    let caps = store.load_full();
    assert!(caps.document_symbol_provider.is_some());
}

// ========================================================================
// Registration with options
// ========================================================================

#[test]
fn test_apply_registration_with_valid_options() {
    let store = CapabilityStore::new(empty_caps());
    let options = serde_json::json!({
        "triggerCharacters": [".", ":"],
        "resolveProvider": true
    });
    let reg = make_registration_with_options("r1", "textDocument/completion", options);
    assert!(store.apply_registration(&reg));

    let caps = store.load_full();
    let comp = caps.completion_provider.as_ref().unwrap();
    assert_eq!(comp.resolve_provider, Some(true));
    let triggers = comp.trigger_characters.as_ref().unwrap();
    assert_eq!(triggers.len(), 2);
}

#[test]
fn test_apply_registration_with_malformed_options() {
    let store = CapabilityStore::new(empty_caps());
    // Invalid type: triggerCharacters should be Vec<String>, not a number
    let options = serde_json::json!({
        "triggerCharacters": 42
    });
    let reg = make_registration_with_options("r1", "textDocument/completion", options);
    // Should still succeed, falling back to default CompletionOptions
    assert!(store.apply_registration(&reg));

    let caps = store.load_full();
    // Default CompletionOptions has no trigger characters
    let comp = caps.completion_provider.as_ref().unwrap();
    assert!(comp.trigger_characters.is_none());
}

#[test]
fn test_apply_registration_with_null_options() {
    let store = CapabilityStore::new(empty_caps());
    let reg = make_registration("r1", "textDocument/completion");
    assert!(store.apply_registration(&reg));

    let caps = store.load_full();
    // Should use default CompletionOptions
    assert!(caps.completion_provider.is_some());
}

#[test]
fn test_apply_registration_signature_help_with_options() {
    let store = CapabilityStore::new(empty_caps());
    let options = serde_json::json!({
        "triggerCharacters": ["(", ","]
    });
    let reg = make_registration_with_options("r1", "textDocument/signatureHelp", options);
    assert!(store.apply_registration(&reg));

    let caps = store.load_full();
    let sig = caps.signature_help_provider.as_ref().unwrap();
    let triggers = sig.trigger_characters.as_ref().unwrap();
    assert_eq!(triggers.len(), 2);
}

#[test]
fn test_apply_registration_signature_help_malformed_options() {
    let store = CapabilityStore::new(empty_caps());
    let options = serde_json::json!({"triggerCharacters": 999});
    let reg = make_registration_with_options("r1", "textDocument/signatureHelp", options);
    assert!(store.apply_registration(&reg));

    let caps = store.load_full();
    // Falls back to default
    assert!(caps.signature_help_provider.is_some());
}

// ========================================================================
// Unknown method
// ========================================================================

#[test]
fn test_apply_registration_unknown_method() {
    let store = CapabilityStore::new(empty_caps());
    let reg = make_registration("r1", "textDocument/unknownCapability");
    assert!(!store.apply_registration(&reg));
}

// ========================================================================
// Unregistration
// ========================================================================

#[test]
fn test_apply_unregistration_removes_capability() {
    let store = CapabilityStore::new(empty_caps());
    let reg = make_registration("r1", "textDocument/hover");
    store.apply_registration(&reg);
    assert!(store.load_full().hover_provider.is_some());

    let unreg = make_unregistration("r1", "textDocument/hover");
    assert!(store.apply_unregistration(&unreg));
    assert!(store.load_full().hover_provider.is_none());
}

#[test]
fn test_apply_unregistration_unknown_id() {
    let store = CapabilityStore::new(empty_caps());
    let unreg = make_unregistration("nonexistent", "textDocument/hover");
    assert!(!store.apply_unregistration(&unreg));
}

// ========================================================================
// Batch operations
// ========================================================================

#[test]
fn test_apply_registrations_batch() {
    let store = CapabilityStore::new(empty_caps());
    let regs = vec![
        make_registration("r1", "textDocument/hover"),
        make_registration("r2", "textDocument/definition"),
        make_registration("r3", "textDocument/references"),
    ];
    store.apply_registrations(&regs);

    let caps = store.load_full();
    assert!(caps.hover_provider.is_some());
    assert!(caps.definition_provider.is_some());
    assert!(caps.references_provider.is_some());
}

#[test]
fn test_apply_unregistrations_batch() {
    let store = CapabilityStore::new(empty_caps());
    // Register first
    let regs = vec![
        make_registration("r1", "textDocument/hover"),
        make_registration("r2", "textDocument/definition"),
    ];
    store.apply_registrations(&regs);

    // Unregister both
    let unregs = vec![
        make_unregistration("r1", "textDocument/hover"),
        make_unregistration("r2", "textDocument/definition"),
    ];
    store.apply_unregistrations(&unregs);

    let caps = store.load_full();
    assert!(caps.hover_provider.is_none());
    assert!(caps.definition_provider.is_none());
}

// ========================================================================
// Roundtrip and edge cases
// ========================================================================

#[test]
fn test_register_then_unregister_roundtrip() {
    let store = CapabilityStore::new(empty_caps());

    // Register completion
    let reg = make_registration("comp-1", "textDocument/completion");
    assert!(store.apply_registration(&reg));
    assert!(store.load_full().completion_provider.is_some());

    // Unregister by same ID
    let unreg = make_unregistration("comp-1", "textDocument/completion");
    assert!(store.apply_unregistration(&unreg));
    assert!(store.load_full().completion_provider.is_none());

    // Re-register with new ID
    let reg2 = make_registration("comp-2", "textDocument/completion");
    assert!(store.apply_registration(&reg2));
    assert!(store.load_full().completion_provider.is_some());
}

#[test]
fn test_multiple_registrations_same_method() {
    let store = CapabilityStore::new(empty_caps());
    let options1 = serde_json::json!({"triggerCharacters": ["."]});
    let options2 =
        serde_json::json!({"triggerCharacters": [".", ":"], "resolveProvider": true});

    let reg1 = make_registration_with_options("r1", "textDocument/completion", options1);
    let reg2 = make_registration_with_options("r2", "textDocument/completion", options2);

    store.apply_registration(&reg1);
    store.apply_registration(&reg2);

    // Last one wins
    let caps = store.load_full();
    let comp = caps.completion_provider.as_ref().unwrap();
    assert_eq!(comp.resolve_provider, Some(true));
    let triggers = comp.trigger_characters.as_ref().unwrap();
    assert_eq!(triggers.len(), 2);
}

#[test]
fn test_unregister_does_not_affect_other_registrations() {
    let store = CapabilityStore::new(empty_caps());
    let regs = vec![
        make_registration("r1", "textDocument/hover"),
        make_registration("r2", "textDocument/definition"),
    ];
    store.apply_registrations(&regs);

    // Unregister only hover
    let unreg = make_unregistration("r1", "textDocument/hover");
    store.apply_unregistration(&unreg);

    let caps = store.load_full();
    assert!(caps.hover_provider.is_none());
    assert!(caps.definition_provider.is_some()); // Unaffected
}

// ========================================================================
// clear_method_capability edge cases
// ========================================================================

#[test]
fn test_clear_unknown_method() {
    let mut caps = empty_caps();
    assert!(!clear_method_capability(&mut caps, "textDocument/unknown"));
}

#[test]
fn test_clear_all_known_methods() {
    let mut caps = empty_caps();
    caps.completion_provider = Some(CompletionOptions::default());
    caps.hover_provider = Some(HoverProviderCapability::Simple(true));
    caps.definition_provider = Some(OneOf::Left(true));
    caps.references_provider = Some(OneOf::Left(true));
    caps.signature_help_provider = Some(SignatureHelpOptions::default());
    caps.code_action_provider = Some(CodeActionProviderCapability::Simple(true));
    caps.document_formatting_provider = Some(OneOf::Left(true));
    caps.document_symbol_provider = Some(OneOf::Left(true));

    for method in [
        "textDocument/completion",
        "textDocument/hover",
        "textDocument/definition",
        "textDocument/references",
        "textDocument/signatureHelp",
        "textDocument/codeAction",
        "textDocument/formatting",
        "textDocument/documentSymbol",
    ] {
        assert!(clear_method_capability(&mut caps, method));
    }

    assert!(caps.completion_provider.is_none());
    assert!(caps.hover_provider.is_none());
    assert!(caps.definition_provider.is_none());
    assert!(caps.references_provider.is_none());
    assert!(caps.signature_help_provider.is_none());
    assert!(caps.code_action_provider.is_none());
    assert!(caps.document_formatting_provider.is_none());
    assert!(caps.document_symbol_provider.is_none());
}

// ========================================================================
// apply_method_registration edge cases
// ========================================================================

#[test]
fn test_apply_unknown_method() {
    let mut caps = empty_caps();
    assert!(!apply_method_registration(&mut caps, "textDocument/rename", None));
}

#[test]
fn test_apply_all_methods_without_options() {
    for method in [
        "textDocument/completion",
        "textDocument/hover",
        "textDocument/definition",
        "textDocument/references",
        "textDocument/signatureHelp",
        "textDocument/codeAction",
        "textDocument/formatting",
        "textDocument/documentSymbol",
    ] {
        let mut caps = empty_caps();
        assert!(apply_method_registration(&mut caps, method, None), "Failed for {method}");
    }
}
