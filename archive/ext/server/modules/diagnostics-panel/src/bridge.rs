//! Diagnostics panel extension state bridge.
//!
//! Serializes [`DiagnosticsState`] to JSON for gRPC transmission to clients.

use {
    reovim_driver_text_lsp::DiagnosticSeverity,
    reovim_driver_text_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
};

use crate::{KIND, state::DiagnosticsState};

/// Bridge for diagnostics panel state.
///
/// Reads [`DiagnosticsState`] from the client's `ExtensionMap` and serializes
/// it to JSON for TUI and Web extension rendering.
pub struct DiagnosticsPanelBridge;

impl ExtensionStateBridge for DiagnosticsPanelBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<DiagnosticsState>()?;

        if !state.active {
            return Some(serde_json::json!({
                "active": false,
            }));
        }

        let counts = state.severity_counts();

        let items: Vec<serde_json::Value> = state
            .items
            .iter()
            .map(|item| {
                let mut obj = serde_json::json!({
                    "file": item.file_path,
                    "line": item.line,
                    "col": item.col,
                    "severity": severity_str(item.severity),
                    "message": item.message,
                });
                if let Some(ref src) = item.source {
                    obj["source"] = serde_json::json!(src);
                }
                if let Some(buf_id) = item.buffer_id {
                    obj["bufferId"] = serde_json::json!(buf_id);
                }
                obj
            })
            .collect();

        Some(serde_json::json!({
            "active": true,
            "mode": state.mode.title(),
            "panelTitle": state.mode.title(),
            "items": items,
            "selected": state.selected,
            "scrollOffset": state.scroll_offset,
            "sortOrder": sort_order_str(state.sort_order),
            "totalCount": state.items.len(),
            "errorCount": counts.errors,
            "warningCount": counts.warnings,
            "infoCount": counts.info,
            "hintCount": counts.hints,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<DiagnosticsState>()
            .is_some_and(|state| state.active)
    }
}

/// Convert severity to a short string for panel JSON.
///
/// Uses abbreviated form ("info" not "information") for compact UI display.
/// The LSP diagnostic bridge uses the full LSP severity names instead.
const fn severity_str(s: DiagnosticSeverity) -> &'static str {
    match s {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Information => "info",
        DiagnosticSeverity::Hint => "hint",
    }
}

/// Convert sort order to a string for JSON.
const fn sort_order_str(s: crate::items::SortOrder) -> &'static str {
    match s {
        crate::items::SortOrder::BySeverity => "severity",
        crate::items::SortOrder::ByFile => "file",
        crate::items::SortOrder::ByLine => "line",
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
