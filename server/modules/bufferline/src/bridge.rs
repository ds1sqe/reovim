//! Bufferline extension state bridge.
//!
//! Reads `BufferListService`, `DiagnosticSnapshot`, and `BufferlineState`
//! during `tick()` to build `BufferlineSnapshot`. Serializes to JSON for
//! gRPC transmission to clients.

use std::collections::{HashMap, HashSet};

use {
    reovim_driver_lsp::{DiagnosticSeverity, DiagnosticSnapshot},
    reovim_driver_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
    reovim_kernel::api::v1::ServiceRegistry,
};

use crate::{
    KIND,
    service::BufferListService,
    state::{BufferEntry, BufferlineSnapshot, BufferlineState},
};

/// Bridge for bufferline state.
///
/// Stateless unit struct — reads `BufferListService` from the live session
/// `ServiceRegistry` at tick time.
pub struct BufferlineBridge;

impl ExtensionStateBridge for BufferlineBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Shared
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let snap = extensions.get::<BufferlineSnapshot>()?;

        let buffers: Vec<serde_json::Value> = snap
            .entries
            .iter()
            .map(|entry| {
                let mut obj = serde_json::json!({
                    "id": entry.id,
                    "name": entry.name,
                    "modified": entry.modified,
                    "pinned": entry.pinned,
                    "errorCount": entry.error_count,
                    "warningCount": entry.warning_count,
                });
                if let Some(ref path) = entry.path {
                    obj["path"] = serde_json::json!(path);
                }
                if let Some(ref ft) = entry.filetype {
                    obj["filetype"] = serde_json::json!(ft);
                }
                obj
            })
            .collect();

        Some(serde_json::json!({
            "active": true,
            "buffers": buffers,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<BufferlineSnapshot>()
            .is_some_and(|snap| !snap.entries.is_empty())
    }

    fn tick(
        &self,
        _client_extensions: &mut ExtensionMap,
        shared_extensions: &mut ExtensionMap,
        services: &ServiceRegistry,
    ) -> bool {
        let Some(buffer_service) = services.get::<BufferListService>() else {
            return false;
        };

        let raw_entries = buffer_service.snapshot();

        // Read diagnostic counts per buffer (build O(1) lookup map).
        let diag_counts = diagnostic_counts(shared_extensions);
        let diag_map: HashMap<u64, (u32, u32)> = diag_counts
            .into_iter()
            .map(|(id, e, w)| (id, (e, w)))
            .collect();

        // Read pin state.
        let pin_state = shared_extensions.get_or_insert::<BufferlineState>();
        let live_ids: Vec<u64> = raw_entries.iter().map(|e| e.id).collect();
        pin_state.clean_stale(&live_ids);
        let pinned_ids = &pin_state.pinned;
        let pinned_set: HashSet<u64> = pinned_ids.iter().copied().collect();
        let pinned_indices: HashMap<u64, usize> = pinned_ids
            .iter()
            .copied()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect();

        // Build entries: pinned first (in pin order), then unpinned (by buffer ID).
        let mut pinned_entries = Vec::new();
        let mut unpinned_entries = Vec::new();

        for raw in &raw_entries {
            let is_pinned = pinned_set.contains(&raw.id);
            let (error_count, warning_count) = diag_map.get(&raw.id).copied().unwrap_or((0, 0));

            let entry = BufferEntry {
                id: raw.id,
                name: raw.name.clone(),
                path: raw.path.clone(),
                modified: raw.modified,
                pinned: is_pinned,
                filetype: raw.filetype.clone(),
                error_count,
                warning_count,
            };

            if is_pinned {
                pinned_entries.push(entry);
            } else {
                unpinned_entries.push(entry);
            }
        }

        // Sort pinned entries by their position in the pin list.
        pinned_entries.sort_by_key(|e| pinned_indices.get(&e.id).copied().unwrap_or(usize::MAX));
        // Unpinned entries stay in buffer ID order (from the service).
        let mut entries = pinned_entries;
        entries.append(&mut unpinned_entries);

        // Compare with current snapshot.
        let snap = shared_extensions.get_or_insert::<BufferlineSnapshot>();
        let changed = snap.entries != entries;
        if changed {
            snap.entries = entries;
        }
        changed
    }
}

/// Extract per-buffer diagnostic counts from `DiagnosticSnapshot`.
///
/// Returns `(buffer_id, error_count, warning_count)` tuples.
fn diagnostic_counts(extensions: &ExtensionMap) -> Vec<(u64, u32, u32)> {
    let Some(diag_snap) = extensions.get::<DiagnosticSnapshot>() else {
        return Vec::new();
    };

    diag_snap
        .entries
        .iter()
        .map(|entry| {
            let mut errors = 0u32;
            let mut warnings = 0u32;
            for item in &entry.diagnostics {
                match item.severity {
                    DiagnosticSeverity::Error => errors += 1,
                    DiagnosticSeverity::Warning => warnings += 1,
                    DiagnosticSeverity::Information | DiagnosticSeverity::Hint => {}
                }
            }
            (entry.buffer_id, errors, warnings)
        })
        .collect()
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
