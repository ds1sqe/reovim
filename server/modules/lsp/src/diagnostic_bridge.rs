//! Diagnostic extension state bridge.
//!
//! Reads diagnostics from `LspProviderRegistry` caches via `tick()`,
//! resolves URIs to buffer IDs via `DiagnosticPathIndex`, and
//! serializes the result for gRPC transmission to TUI clients.

use std::{collections::HashMap, sync::Mutex};

use {
    reovim_driver_lsp::LspProviderRegistry,
    reovim_driver_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
    reovim_kernel::api::v1::{Service, ServiceRegistry},
};

use crate::{
    KIND,
    diagnostic_state::{
        BufferDiagnosticEntry, DiagnosticItem, DiagnosticSeverity, DiagnosticSnapshot,
    },
};

// ============================================================================
// DiagnosticPathIndex
// ============================================================================

/// Service mapping URI strings to buffer IDs.
///
/// Populated by the lsp module when buffers are saved (`BufferSaved` event)
/// and when LSP servers are auto-started (`auto_start` with `DidOpen`).
/// Read by `DiagnosticBridge::tick()` to resolve `DiagnosticCache` URIs.
#[derive(Debug, Default)]
pub struct DiagnosticPathIndex {
    map: Mutex<HashMap<String, u64>>,
}

impl Service for DiagnosticPathIndex {}

impl DiagnosticPathIndex {
    /// Insert or update a URI-to-buffer-ID mapping.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn insert(&self, uri: String, buffer_id: u64) {
        self.map
            .lock()
            .expect("DiagnosticPathIndex lock poisoned")
            .insert(uri, buffer_id);
    }

    /// Look up a buffer ID by URI string.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn get(&self, uri: &str) -> Option<u64> {
        self.map
            .lock()
            .expect("DiagnosticPathIndex lock poisoned")
            .get(uri)
            .copied()
    }

    /// Get all entries (for testing).
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[cfg(test)]
    #[must_use]
    pub fn entries(&self) -> Vec<(String, u64)> {
        self.map
            .lock()
            .expect("DiagnosticPathIndex lock poisoned")
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }
}

// ============================================================================
// DiagnosticBridge
// ============================================================================

/// Bridge for diagnostic state (LSP `publishDiagnostics`).
///
/// Stateless unit struct — looks up `LspProviderRegistry` and
/// `DiagnosticPathIndex` from the live session `ServiceRegistry` at
/// tick time (#555). This avoids capturing `Arc` references during
/// `init()` which would point to a dead temporary registry.
pub struct DiagnosticBridge;

impl ExtensionStateBridge for DiagnosticBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Shared
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let snap = extensions.get::<DiagnosticSnapshot>()?;

        if snap.entries.is_empty() {
            return Some(serde_json::json!({ "active": false }));
        }

        let diagnostics: Vec<serde_json::Value> = snap
            .entries
            .iter()
            .map(|entry| {
                let items: Vec<serde_json::Value> = entry
                    .diagnostics
                    .iter()
                    .map(|d| {
                        serde_json::json!({
                            "startLine": d.start_line,
                            "startCol": d.start_col,
                            "endLine": d.end_line,
                            "endCol": d.end_col,
                            "severity": severity_str(d.severity),
                            "message": d.message,
                            "source": d.source,
                        })
                    })
                    .collect();

                serde_json::json!({
                    "bufferId": entry.buffer_id,
                    "items": items,
                })
            })
            .collect();

        Some(serde_json::json!({
            "active": true,
            "diagnostics": diagnostics,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<DiagnosticSnapshot>()
            .is_some_and(|s| !s.entries.is_empty())
    }

    fn tick(
        &self,
        _client_extensions: &mut ExtensionMap,
        shared_extensions: &mut ExtensionMap,
        services: &ServiceRegistry,
    ) -> bool {
        let Some(provider_registry) = services.get::<LspProviderRegistry>() else {
            return false;
        };
        let path_index = services.get::<DiagnosticPathIndex>();

        let mut entries = Vec::new();

        for key in provider_registry.keys() {
            if let Some(provider) = provider_registry.get(&key)
                && provider.is_active()
            {
                let all_diags = provider.diagnostics().get_all();
                for (uri_str, buffer_diags) in &all_diags {
                    let buffer_id = path_index.as_ref().and_then(|idx| idx.get(uri_str));
                    if let Some(buffer_id) = buffer_id {
                        let items: Vec<DiagnosticItem> = buffer_diags
                            .diagnostics
                            .iter()
                            .map(|d| DiagnosticItem {
                                start_line: d.range.start.line,
                                start_col: d.range.start.character,
                                end_line: d.range.end.line,
                                end_col: d.range.end.character,
                                severity: convert_severity(d.severity),
                                message: d.message.clone(),
                                source: d.source.clone(),
                            })
                            .collect();

                        if !items.is_empty() {
                            entries.push(BufferDiagnosticEntry {
                                buffer_id,
                                diagnostics: items,
                            });
                        }
                    }
                }
            }
        }

        // Sort by buffer_id for deterministic output.
        entries.sort_by_key(|e| e.buffer_id);

        let snap = shared_extensions.get_or_insert::<DiagnosticSnapshot>();
        let changed = !entries_eq(&snap.entries, &entries);
        if changed {
            snap.entries = entries;
        }
        changed
    }
}

/// Convert severity to JSON string.
const fn severity_str(s: DiagnosticSeverity) -> &'static str {
    match s {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Information => "information",
        DiagnosticSeverity::Hint => "hint",
    }
}

/// Convert LSP severity to our enum.
const fn convert_severity(s: Option<lsp_types::DiagnosticSeverity>) -> DiagnosticSeverity {
    match s {
        Some(lsp_types::DiagnosticSeverity::ERROR) => DiagnosticSeverity::Error,
        Some(lsp_types::DiagnosticSeverity::INFORMATION) => DiagnosticSeverity::Information,
        Some(lsp_types::DiagnosticSeverity::HINT) => DiagnosticSeverity::Hint,
        // WARNING and unknown/None all map to Warning.
        _ => DiagnosticSeverity::Warning,
    }
}

/// Compare two entry lists for equality (order-sensitive).
fn entries_eq(a: &[BufferDiagnosticEntry], b: &[BufferDiagnosticEntry]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(ea, eb)| ea.buffer_id == eb.buffer_id && ea.diagnostics == eb.diagnostics)
}

#[cfg(test)]
#[path = "diagnostic_bridge_tests.rs"]
mod tests;
