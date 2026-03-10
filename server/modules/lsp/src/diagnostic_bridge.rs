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

use crate::diagnostic_state::{
    BufferDiagnosticEntry, DiagnosticItem, DiagnosticSeverity, DiagnosticSnapshot,
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
        "diagnostics"
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
mod tests {
    use {
        lsp_types::{Position, Range},
        reovim_driver_lsp::{DiagnosticCache, LspKey, LspProvider, LspRequest},
    };

    use std::sync::Arc;

    use super::*;

    // ========================================================================
    // DiagnosticPathIndex tests
    // ========================================================================

    #[test]
    fn path_index_default() {
        let idx = DiagnosticPathIndex::default();
        assert!(idx.entries().is_empty());
    }

    #[test]
    fn path_index_debug() {
        let idx = DiagnosticPathIndex::default();
        let debug = format!("{idx:?}");
        assert!(debug.contains("DiagnosticPathIndex"));
    }

    #[test]
    fn path_index_insert_and_get() {
        let idx = DiagnosticPathIndex::default();
        idx.insert("file:///a.rs".to_owned(), 1);
        assert_eq!(idx.get("file:///a.rs"), Some(1));
    }

    #[test]
    fn path_index_get_missing() {
        let idx = DiagnosticPathIndex::default();
        assert!(idx.get("file:///missing.rs").is_none());
    }

    #[test]
    fn path_index_overwrite() {
        let idx = DiagnosticPathIndex::default();
        idx.insert("file:///a.rs".to_owned(), 1);
        idx.insert("file:///a.rs".to_owned(), 2);
        assert_eq!(idx.get("file:///a.rs"), Some(2));
    }

    #[test]
    fn path_index_entries() {
        let idx = DiagnosticPathIndex::default();
        idx.insert("file:///a.rs".to_owned(), 1);
        idx.insert("file:///b.rs".to_owned(), 2);
        let mut entries = idx.entries();
        entries.sort_by_key(|(_, id)| *id);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], ("file:///a.rs".to_owned(), 1));
        assert_eq!(entries[1], ("file:///b.rs".to_owned(), 2));
    }

    #[test]
    fn path_index_service_trait() {
        use reovim_kernel::api::v1::ServiceRegistry;

        let registry = ServiceRegistry::new();
        let idx = registry.get_or_create::<DiagnosticPathIndex>();
        idx.insert("file:///test.rs".to_owned(), 42);

        let idx2 = registry.get_or_create::<DiagnosticPathIndex>();
        assert_eq!(idx2.get("file:///test.rs"), Some(42));
    }

    // ========================================================================
    // DiagnosticBridge basic tests
    // ========================================================================

    fn make_bridge() -> (DiagnosticBridge, ServiceRegistry) {
        let services = ServiceRegistry::new();
        // Pre-populate services so tick() can find them.
        let _ = services.get_or_create::<LspProviderRegistry>();
        let _ = services.get_or_create::<DiagnosticPathIndex>();
        (DiagnosticBridge, services)
    }

    #[test]
    fn bridge_kind() {
        let bridge = DiagnosticBridge;
        assert_eq!(bridge.kind(), "diagnostics");
    }

    #[test]
    fn bridge_scope() {
        let bridge = DiagnosticBridge;
        assert_eq!(bridge.scope(), ExtensionScope::Shared);
    }

    // ========================================================================
    // snapshot tests
    // ========================================================================

    #[test]
    fn snapshot_no_state_returns_none() {
        let (bridge, _services) = make_bridge();
        let map = ExtensionMap::new();
        assert!(bridge.snapshot(&map).is_none());
    }

    #[test]
    fn snapshot_empty_entries() {
        let bridge = DiagnosticBridge;
        let mut map = ExtensionMap::new();
        map.get_or_insert::<DiagnosticSnapshot>();

        let snap = bridge.snapshot(&map).unwrap();
        assert_eq!(snap["active"], false);
    }

    #[test]
    fn snapshot_with_entries() {
        let bridge = DiagnosticBridge;
        let mut map = ExtensionMap::new();
        let snap = map.get_or_insert::<DiagnosticSnapshot>();
        snap.entries.push(BufferDiagnosticEntry {
            buffer_id: 1,
            diagnostics: vec![DiagnosticItem {
                start_line: 5,
                start_col: 0,
                end_line: 5,
                end_col: 10,
                severity: DiagnosticSeverity::Error,
                message: "type mismatch".to_owned(),
                source: Some("rust-analyzer".to_owned()),
            }],
        });

        let json = bridge.snapshot(&map).unwrap();
        assert_eq!(json["active"], true);
        let diags = json["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0]["bufferId"], 1);
        let items = diags[0]["items"].as_array().unwrap();
        assert_eq!(items[0]["startLine"], 5);
        assert_eq!(items[0]["severity"], "error");
        assert_eq!(items[0]["message"], "type mismatch");
        assert_eq!(items[0]["source"], "rust-analyzer");
    }

    #[test]
    fn snapshot_null_source() {
        let bridge = DiagnosticBridge;
        let mut map = ExtensionMap::new();
        let snap = map.get_or_insert::<DiagnosticSnapshot>();
        snap.entries.push(BufferDiagnosticEntry {
            buffer_id: 2,
            diagnostics: vec![DiagnosticItem {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 1,
                severity: DiagnosticSeverity::Hint,
                message: "hint".to_owned(),
                source: None,
            }],
        });

        let json = bridge.snapshot(&map).unwrap();
        let items = json["diagnostics"][0]["items"].as_array().unwrap();
        assert!(items[0]["source"].is_null());
    }

    // ========================================================================
    // is_active tests
    // ========================================================================

    #[test]
    fn is_active_no_state() {
        let bridge = DiagnosticBridge;
        let map = ExtensionMap::new();
        assert!(!bridge.is_active(&map));
    }

    #[test]
    fn is_active_empty() {
        let bridge = DiagnosticBridge;
        let mut map = ExtensionMap::new();
        map.get_or_insert::<DiagnosticSnapshot>();
        assert!(!bridge.is_active(&map));
    }

    #[test]
    fn is_active_with_entries() {
        let bridge = DiagnosticBridge;
        let mut map = ExtensionMap::new();
        let snap = map.get_or_insert::<DiagnosticSnapshot>();
        snap.entries.push(BufferDiagnosticEntry {
            buffer_id: 1,
            diagnostics: vec![DiagnosticItem {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 5,
                severity: DiagnosticSeverity::Error,
                message: "err".to_owned(),
                source: None,
            }],
        });
        assert!(bridge.is_active(&map));
    }

    // ========================================================================
    // tick tests
    // ========================================================================

    struct MockLspProvider {
        cache: DiagnosticCache,
        active: bool,
    }

    impl MockLspProvider {
        fn new(active: bool) -> Self {
            Self {
                cache: DiagnosticCache::new(),
                active,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(clippy::unnecessary_literal_bound)]
    impl LspProvider for MockLspProvider {
        fn send_request(&self, _request: LspRequest) -> bool {
            false
        }

        fn diagnostics(&self) -> &DiagnosticCache {
            &self.cache
        }

        fn is_active(&self) -> bool {
            self.active
        }

        fn capabilities(&self) -> Option<Arc<lsp_types::ServerCapabilities>> {
            None
        }

        fn root_path(&self) -> &std::path::Path {
            std::path::Path::new("/mock")
        }

        fn language_id(&self) -> &str {
            "rust"
        }

        fn server_info(&self) -> Option<&lsp_types::ServerInfo> {
            None
        }
    }

    fn make_lsp_diagnostic(
        message: &str,
        severity: lsp_types::DiagnosticSeverity,
        line: u32,
    ) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: Range::new(Position::new(line, 0), Position::new(line, 10)),
            severity: Some(severity),
            message: message.to_owned(),
            source: Some("test-lsp".to_owned()),
            ..Default::default()
        }
    }

    #[test]
    fn tick_no_providers() {
        let (bridge, services) = make_bridge();
        let mut client = ExtensionMap::new();
        let mut shared = ExtensionMap::new();
        assert!(!bridge.tick(&mut client, &mut shared, &services));
    }

    #[test]
    fn tick_inactive_provider_skipped() {
        let (bridge, services) = make_bridge();
        let provider = MockLspProvider::new(false);
        services
            .get_or_create::<LspProviderRegistry>()
            .register(LspKey::Language("rust".to_owned()), Arc::new(provider));

        let mut client = ExtensionMap::new();
        let mut shared = ExtensionMap::new();
        assert!(!bridge.tick(&mut client, &mut shared, &services));
    }

    #[test]
    fn tick_uri_not_in_index_skipped() {
        let (bridge, services) = make_bridge();
        let provider = MockLspProvider::new(true);
        let uri: lsp_types::Uri = "file:///unknown.rs".parse().unwrap();
        provider.diagnostics().store(
            &uri,
            None,
            vec![make_lsp_diagnostic(
                "err",
                lsp_types::DiagnosticSeverity::ERROR,
                0,
            )],
        );
        services
            .get_or_create::<LspProviderRegistry>()
            .register(LspKey::Language("rust".to_owned()), Arc::new(provider));

        let mut client = ExtensionMap::new();
        let mut shared = ExtensionMap::new();
        // URI not in path index, so no entries populated.
        assert!(!bridge.tick(&mut client, &mut shared, &services));
    }

    #[test]
    fn tick_populates_snapshot() {
        let (bridge, services) = make_bridge();

        // Register provider with diagnostics.
        let provider = MockLspProvider::new(true);
        let uri: lsp_types::Uri = "file:///test.rs".parse().unwrap();
        provider.diagnostics().store(
            &uri,
            None,
            vec![make_lsp_diagnostic(
                "type error",
                lsp_types::DiagnosticSeverity::ERROR,
                5,
            )],
        );
        services
            .get_or_create::<LspProviderRegistry>()
            .register(LspKey::Language("rust".to_owned()), Arc::new(provider));

        // Map URI to buffer ID.
        services
            .get_or_create::<DiagnosticPathIndex>()
            .insert("file:///test.rs".to_owned(), 42);

        let mut client = ExtensionMap::new();
        let mut shared = ExtensionMap::new();

        // First tick should populate and return true.
        assert!(bridge.tick(&mut client, &mut shared, &services));

        let snap = shared.get::<DiagnosticSnapshot>().unwrap();
        assert_eq!(snap.entries.len(), 1);
        assert_eq!(snap.entries[0].buffer_id, 42);
        assert_eq!(snap.entries[0].diagnostics.len(), 1);
        assert_eq!(snap.entries[0].diagnostics[0].start_line, 5);
        assert_eq!(snap.entries[0].diagnostics[0].severity, DiagnosticSeverity::Error);
        assert_eq!(snap.entries[0].diagnostics[0].message, "type error");
        assert_eq!(snap.entries[0].diagnostics[0].source.as_deref(), Some("test-lsp"));
    }

    #[test]
    fn tick_unchanged_returns_false() {
        let (bridge, services) = make_bridge();

        let provider = MockLspProvider::new(true);
        let uri: lsp_types::Uri = "file:///test.rs".parse().unwrap();
        provider.diagnostics().store(
            &uri,
            None,
            vec![make_lsp_diagnostic(
                "err",
                lsp_types::DiagnosticSeverity::ERROR,
                0,
            )],
        );
        services
            .get_or_create::<LspProviderRegistry>()
            .register(LspKey::Language("rust".to_owned()), Arc::new(provider));
        services
            .get_or_create::<DiagnosticPathIndex>()
            .insert("file:///test.rs".to_owned(), 1);

        let mut client = ExtensionMap::new();
        let mut shared = ExtensionMap::new();

        // First tick: changed.
        assert!(bridge.tick(&mut client, &mut shared, &services));
        // Second tick: same data, not changed.
        assert!(!bridge.tick(&mut client, &mut shared, &services));
    }

    #[test]
    fn tick_empty_diagnostics_skipped() {
        let (bridge, services) = make_bridge();

        let provider = MockLspProvider::new(true);
        let uri: lsp_types::Uri = "file:///test.rs".parse().unwrap();
        provider.diagnostics().store(&uri, None, vec![]);
        services
            .get_or_create::<LspProviderRegistry>()
            .register(LspKey::Language("rust".to_owned()), Arc::new(provider));
        services
            .get_or_create::<DiagnosticPathIndex>()
            .insert("file:///test.rs".to_owned(), 1);

        let mut client = ExtensionMap::new();
        let mut shared = ExtensionMap::new();

        // Empty diagnostics should not create entries.
        assert!(!bridge.tick(&mut client, &mut shared, &services));
    }

    // ========================================================================
    // severity conversion tests
    // ========================================================================

    #[test]
    fn severity_str_all() {
        assert_eq!(severity_str(DiagnosticSeverity::Error), "error");
        assert_eq!(severity_str(DiagnosticSeverity::Warning), "warning");
        assert_eq!(severity_str(DiagnosticSeverity::Information), "information");
        assert_eq!(severity_str(DiagnosticSeverity::Hint), "hint");
    }

    #[test]
    fn convert_severity_all() {
        assert_eq!(
            convert_severity(Some(lsp_types::DiagnosticSeverity::ERROR)),
            DiagnosticSeverity::Error
        );
        assert_eq!(
            convert_severity(Some(lsp_types::DiagnosticSeverity::WARNING)),
            DiagnosticSeverity::Warning
        );
        assert_eq!(
            convert_severity(Some(lsp_types::DiagnosticSeverity::INFORMATION)),
            DiagnosticSeverity::Information
        );
        assert_eq!(
            convert_severity(Some(lsp_types::DiagnosticSeverity::HINT)),
            DiagnosticSeverity::Hint
        );
    }

    #[test]
    fn convert_severity_none_defaults_to_warning() {
        assert_eq!(convert_severity(None), DiagnosticSeverity::Warning);
    }

    // ========================================================================
    // entries_eq tests
    // ========================================================================

    #[test]
    fn entries_eq_both_empty() {
        assert!(entries_eq(&[], &[]));
    }

    #[test]
    fn entries_eq_different_lengths() {
        let a = vec![BufferDiagnosticEntry {
            buffer_id: 1,
            diagnostics: vec![],
        }];
        assert!(!entries_eq(&a, &[]));
    }

    #[test]
    fn entries_eq_same_content() {
        let item = DiagnosticItem {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 5,
            severity: DiagnosticSeverity::Error,
            message: "err".to_owned(),
            source: None,
        };
        let a = vec![BufferDiagnosticEntry {
            buffer_id: 1,
            diagnostics: vec![item.clone()],
        }];
        let b = vec![BufferDiagnosticEntry {
            buffer_id: 1,
            diagnostics: vec![item],
        }];
        assert!(entries_eq(&a, &b));
    }

    #[test]
    fn entries_eq_different_buffer_id() {
        let a = vec![BufferDiagnosticEntry {
            buffer_id: 1,
            diagnostics: vec![],
        }];
        let b = vec![BufferDiagnosticEntry {
            buffer_id: 2,
            diagnostics: vec![],
        }];
        assert!(!entries_eq(&a, &b));
    }
}
