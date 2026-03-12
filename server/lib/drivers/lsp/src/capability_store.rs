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
#[path = "capability_store_tests.rs"]
mod tests;
