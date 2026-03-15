//! Illuminate extension bridge.
//!
//! Serializes `IlluminateState` to JSON for gRPC transmission to clients.
//! Implements tick-based cursor-hold detection using the cursor shadow pattern.

use {
    reovim_driver_session::{
        CursorSnapshot, ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
    reovim_kernel::api::v1::ServiceRegistry,
    serde_json::json,
};

use crate::state::{HighlightKind, IlluminateState};

/// Number of idle ticks before triggering highlight computation.
///
/// At ~100ms per tick, 3 ticks ≈ 300ms cursor hold delay.
const HOLD_TICKS: u32 = 3;

/// Bridge for illuminate state serialization and tick-based cursor-hold.
pub struct IlluminateBridge;

impl ExtensionStateBridge for IlluminateBridge {
    fn kind(&self) -> &'static str {
        "illuminate"
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<IlluminateState>()?;

        if !state.active || state.ranges.is_empty() {
            return Some(json!({
                "active": false,
                "sequence": state.sequence,
            }));
        }

        let ranges: Vec<serde_json::Value> = state
            .ranges
            .iter()
            .map(|r| {
                json!({
                    "startLine": r.start_line,
                    "startCol": r.start_col,
                    "endLine": r.end_line,
                    "endCol": r.end_col,
                    "kind": r.kind.as_str(),
                })
            })
            .collect();

        Some(json!({
            "active": true,
            "bufferId": state.buffer_id.as_usize(),
            "word": state.word,
            "ranges": ranges,
            "sequence": state.sequence,
        }))
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions
            .get::<IlluminateState>()
            .is_some_and(|s| s.active && !s.ranges.is_empty())
    }

    fn on_mode_changed(&self, _from: &str, to: &str, extensions: &mut ExtensionMap) {
        // Clear highlights when entering insert or command-line modes
        if (to.contains("insert") || to.contains("command"))
            && let Some(state) = extensions.get_mut::<IlluminateState>()
        {
            state.clear();
        }
    }

    fn tick(
        &self,
        client_extensions: &mut ExtensionMap,
        _shared_extensions: &mut ExtensionMap,
        _services: &ServiceRegistry,
    ) -> bool {
        // Read cursor position from snapshot (written by runner after each key event).
        let (cursor_line, cursor_col) = {
            let snap = client_extensions.get_or_insert::<CursorSnapshot>();
            (snap.line, snap.col)
        };

        let state = client_extensions.get_or_insert::<IlluminateState>();

        // Feed cursor position into shadow tracking.
        // This resets idle_ticks when the cursor moves to a new position.
        state.cursor_moved(cursor_line, cursor_col);

        // If highlights are already computed for this position, nothing to do
        if state.computed {
            return false;
        }

        let ticks = state.tick();

        if ticks < HOLD_TICKS {
            return false;
        }

        // Cursor has been idle for >= HOLD_TICKS.
        // Mark as computed so we don't re-trigger on next tick.
        // Actual highlight computation (LSP or word-match) will be added
        // when the full pipeline is wired. For now, the tick mechanism
        // is in place and tested.
        state.computed = true;

        // Return false because we haven't actually computed highlights yet.
        // This will return true once word-match/LSP integration is added.
        false
    }
}

/// Convert LSP `DocumentHighlightKind` to our `HighlightKind`.
#[must_use]
pub fn from_lsp_kind(
    kind: Option<reovim_driver_lsp::lsp_types::DocumentHighlightKind>,
) -> HighlightKind {
    use reovim_driver_lsp::lsp_types::DocumentHighlightKind;

    match kind {
        Some(k) if k == DocumentHighlightKind::READ => HighlightKind::Read,
        Some(k) if k == DocumentHighlightKind::WRITE => HighlightKind::Write,
        _ => HighlightKind::Text,
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
