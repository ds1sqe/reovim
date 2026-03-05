//! Lock-free capability store for dynamic LSP registration.
//!
//! Wraps `ArcSwap<ServerCapabilities>` to support dynamic capability
//! registration and unregistration from the saturator task, while
//! consumers read lock-free via `load_full()`.
//!
//! Follows the same pattern as [`DiagnosticCache`](crate::DiagnosticCache).

use std::{collections::HashMap, sync::Arc};

use {
    lsp_types::{
        CodeActionProviderCapability, CompletionOptions, HoverProviderCapability, OneOf,
        Registration, ServerCapabilities, SignatureHelpOptions, Unregistration,
    },
    reovim_kernel::api::v1::ArcSwap,
    tracing::{debug, warn},
};

/// Lock-free store for `ServerCapabilities` that supports dynamic updates.
///
/// # Thread Safety
///
/// - **Saturator task**: Calls `apply_registration()` / `apply_unregistration()`
/// - **Consumer threads**: Call `load_full()` for lock-free reads
///
/// The `registrations` map tracks `id -> method` so that `client/unregisterCapability`
/// can look up which capability to clear by registration ID.
pub struct CapabilityStore {
    /// Atomic pointer to current capabilities.
    inner: ArcSwap<ServerCapabilities>,
    /// Registration ID -> method name, for unregister lookups.
    registrations: parking_lot::Mutex<HashMap<String, String>>,
}

impl std::fmt::Debug for CapabilityStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let regs = self.registrations.lock();
        f.debug_struct("CapabilityStore")
            .field("registration_count", &regs.len())
            .finish_non_exhaustive()
    }
}

impl CapabilityStore {
    /// Create a new store with initial capabilities from the `initialize` response.
    #[must_use]
    pub fn new(caps: ServerCapabilities) -> Self {
        Self {
            inner: ArcSwap::from_pointee(caps),
            registrations: parking_lot::Mutex::new(HashMap::new()),
        }
    }

    /// Replace all capabilities atomically.
    ///
    /// Used after `initialize()` to set the server-reported capabilities.
    pub fn store(&self, caps: ServerCapabilities) {
        self.inner.store(Arc::new(caps));
    }

    /// Load current capabilities (lock-free).
    ///
    /// Returns an `Arc` for cheap cloning and sharing.
    #[must_use]
    pub fn load_full(&self) -> Arc<ServerCapabilities> {
        self.inner.load_full()
    }

    /// Apply a single dynamic registration.
    ///
    /// Parses the registration method to determine which capability field to set.
    /// If `register_options` is present, attempts to deserialize it; on failure,
    /// falls back to a default/truthy value. Unknown methods are logged and skipped.
    ///
    /// Returns `true` if the capability was successfully applied.
    pub fn apply_registration(&self, reg: &Registration) -> bool {
        let old = self.inner.load();
        let mut caps = ServerCapabilities::clone(&old);

        let applied =
            apply_method_registration(&mut caps, &reg.method, reg.register_options.as_ref());

        if applied {
            // Single writer: only the saturator task calls apply_registration.
            self.inner.store(Arc::new(caps));
            // Same method registered twice: last one wins (standard LSP behavior).
            self.registrations
                .lock()
                .insert(reg.id.clone(), reg.method.clone());
            debug!(id = %reg.id, method = %reg.method, "Applied dynamic registration");
        }

        applied
    }

    /// Apply a single dynamic unregistration.
    ///
    /// Looks up the registration ID to find the method, then clears the
    /// corresponding capability field.
    ///
    /// Returns `true` if the capability was successfully removed.
    pub fn apply_unregistration(&self, unreg: &Unregistration) -> bool {
        let method = {
            let mut regs = self.registrations.lock();
            regs.remove(&unreg.id)
        };

        let Some(method) = method else {
            debug!(id = %unreg.id, "Unregister: unknown registration ID");
            return false;
        };

        let old = self.inner.load();
        let mut caps = ServerCapabilities::clone(&old);
        let cleared = clear_method_capability(&mut caps, &method);

        if cleared {
            self.inner.store(Arc::new(caps));
            debug!(id = %unreg.id, method = %method, "Removed dynamic registration");
        }

        cleared
    }

    /// Apply a batch of registrations.
    pub fn apply_registrations(&self, regs: &[Registration]) {
        for reg in regs {
            self.apply_registration(reg);
        }
    }

    /// Apply a batch of unregistrations.
    pub fn apply_unregistrations(&self, unregs: &[Unregistration]) {
        for unreg in unregs {
            self.apply_unregistration(unreg);
        }
    }
}

/// Set the capability field for a known LSP method.
///
/// If `register_options` is `Some`, attempts to deserialize it into the
/// concrete options type. On deserialization failure, falls back to a
/// default/truthy value and logs a warning.
///
/// Returns `false` for unknown methods.
fn apply_method_registration(
    caps: &mut ServerCapabilities,
    method: &str,
    register_options: Option<&serde_json::Value>,
) -> bool {
    match method {
        "textDocument/completion" => {
            caps.completion_provider = Some(
                register_options
                    .and_then(|opts| {
                        serde_json::from_value::<CompletionOptions>(opts.clone())
                            .inspect_err(|e| {
                                warn!(method, error = %e, "Failed to parse register_options, using default");
                            })
                            .ok()
                    })
                    .unwrap_or_default(),
            );
            true
        }
        "textDocument/hover" => {
            caps.hover_provider = Some(HoverProviderCapability::Simple(true));
            true
        }
        "textDocument/definition" => {
            caps.definition_provider = Some(OneOf::Left(true));
            true
        }
        "textDocument/references" => {
            caps.references_provider = Some(OneOf::Left(true));
            true
        }
        "textDocument/signatureHelp" => {
            caps.signature_help_provider = Some(
                register_options
                    .and_then(|opts| {
                        serde_json::from_value::<SignatureHelpOptions>(opts.clone())
                            .inspect_err(|e| {
                                warn!(method, error = %e, "Failed to parse register_options, using default");
                            })
                            .ok()
                    })
                    .unwrap_or_default(),
            );
            true
        }
        "textDocument/codeAction" => {
            caps.code_action_provider = Some(CodeActionProviderCapability::Simple(true));
            true
        }
        "textDocument/formatting" => {
            caps.document_formatting_provider = Some(OneOf::Left(true));
            true
        }
        "textDocument/documentSymbol" => {
            caps.document_symbol_provider = Some(OneOf::Left(true));
            true
        }
        _ => {
            warn!(method, "Unknown dynamic registration method, skipping");
            false
        }
    }
}

/// Clear the capability field for a known LSP method.
///
/// Returns `false` for unknown methods.
fn clear_method_capability(caps: &mut ServerCapabilities, method: &str) -> bool {
    match method {
        "textDocument/completion" => {
            caps.completion_provider = None;
            true
        }
        "textDocument/hover" => {
            caps.hover_provider = None;
            true
        }
        "textDocument/definition" => {
            caps.definition_provider = None;
            true
        }
        "textDocument/references" => {
            caps.references_provider = None;
            true
        }
        "textDocument/signatureHelp" => {
            caps.signature_help_provider = None;
            true
        }
        "textDocument/codeAction" => {
            caps.code_action_provider = None;
            true
        }
        "textDocument/formatting" => {
            caps.document_formatting_provider = None;
            true
        }
        "textDocument/documentSymbol" => {
            caps.document_symbol_provider = None;
            true
        }
        _ => {
            warn!(method, "Unknown method for unregistration, skipping");
            false
        }
    }
}

#[cfg(test)]
mod tests {
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
}
