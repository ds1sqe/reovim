//! Microscope extension state bridge.
//!
//! Serializes [`MicroscopeState`] to JSON for gRPC transmission to clients.
//! Both TUI (Rust) and Web (TypeScript) extensions consume the same JSON.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::{KIND, state::MicroscopeState};

/// Bridge for microscope picker state.
///
/// Reads [`MicroscopeState`] from the client's `ExtensionMap` and serializes
/// it to JSON with fields matching what TUI and Web extensions expect.
pub struct MicroscopeBridge;

impl ExtensionStateBridge for MicroscopeBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<MicroscopeState>()?;

        if !state.active {
            return Some(serde_json::json!({
                "active": false,
            }));
        }

        let items: Vec<serde_json::Value> = state
            .items
            .iter()
            .map(|item| {
                let mut obj = serde_json::json!({
                    "display": item.display,
                });
                if let Some(ref detail) = item.detail {
                    obj["detail"] = serde_json::json!(detail);
                }
                if let Some(icon) = item.icon {
                    obj["icon"] = serde_json::json!(icon.to_string());
                }
                obj
            })
            .collect();

        let mut json = serde_json::json!({
            "active": true,
            "query": state.query,
            "cursor": state.cursor,
            "selected": state.selected,
            "scrollOffset": state.scroll_offset,
            "pickerName": state.picker_name,
            "pickerTitle": state.picker_title,
            "prompt": state.prompt,
            "items": items,
            "totalCount": state.total_count,
            "matchedCount": state.matched_count,
        });

        if let Some(ref preview) = state.preview {
            let mut preview_json = serde_json::json!({
                "lines": preview.lines,
            });
            if let Some(hl) = preview.highlight_line {
                preview_json["highlightLine"] = serde_json::json!(hl);
            }
            if let Some(ref path) = preview.file_path {
                preview_json["filePath"] = serde_json::json!(path.to_string_lossy());
            }
            if !preview.highlights.is_empty() {
                let highlights: Vec<serde_json::Value> = preview
                    .highlights
                    .iter()
                    .map(|h| {
                        serde_json::json!({
                            "line": h.line,
                            "colStart": h.col_start,
                            "colEnd": h.col_end,
                            "category": h.category,
                        })
                    })
                    .collect();
                preview_json["highlights"] = serde_json::json!(highlights);
            }
            json["preview"] = preview_json;
        }

        Some(json)
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<MicroscopeState>()
            .is_some_and(|s| s.active)
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
