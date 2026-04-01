//! Illuminate extension bridge.
//!
//! Serializes `IlluminateState` to JSON for gRPC transmission to clients.
//! Implements tick-based cursor-hold detection using the cursor shadow pattern.

use {
    reovim_driver_session::{
        BufferReadAccess, CursorSnapshot, ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
    reovim_kernel::api::v1::{
        BufferId, BufferOps, CharKind, ServiceRegistry, WordType, char_kind, word_bounds,
    },
    serde_json::json,
};

use crate::state::{HighlightKind, HighlightRange, IlluminateState};

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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn tick(
        &self,
        client_extensions: &mut ExtensionMap,
        _shared_extensions: &mut ExtensionMap,
        services: &ServiceRegistry,
    ) -> bool {
        // Read cursor position from snapshot (written by runner after each key event).
        let (cursor_line, cursor_col, raw_buffer_id) = {
            let snap = client_extensions.get_or_insert::<CursorSnapshot>();
            (snap.line, snap.col, snap.buffer_id)
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

        // Cursor has been idle for >= HOLD_TICKS — compute word highlights.

        let Some(buffer_access) = services.get::<BufferReadAccess>() else {
            state.computed = true;
            return false;
        };

        #[allow(clippy::cast_possible_truncation)]
        let buffer_id = BufferId::from_raw(raw_buffer_id as usize);
        let Some(buffer_lock) = buffer_access.manager().get(buffer_id) else {
            state.computed = true;
            return false;
        };

        // Scope the buffer read lock so it's dropped before mutating state.
        let result = {
            let buffer = buffer_lock.read();
            extract_word_and_occurrences(&*buffer, cursor_line, cursor_col)
        };

        match result {
            Some((word, ranges)) if ranges.len() >= 2 => {
                // Optimization: if same word and buffer, skip re-notification
                if state.word == word && state.buffer_id == buffer_id && state.active {
                    state.computed = true;
                    return false;
                }
                state.set_highlights(buffer_id, word, ranges, cursor_line, cursor_col);
                true
            }
            _ => {
                // Not on a word, or only one occurrence — clear any existing highlights
                let was_active = state.active;
                state.clear();
                state.computed = true;
                was_active
            }
        }
    }
}

/// Extract the word under the cursor and find all whole-word occurrences.
///
/// Returns `None` if the cursor is not on a word character.
#[cfg_attr(coverage_nightly, coverage(off))]
fn extract_word_and_occurrences(
    buffer: &dyn BufferOps,
    cursor_line: u32,
    cursor_col: u32,
) -> Option<(String, Vec<HighlightRange>)> {
    let line_text = buffer.line(cursor_line as usize)?;
    let chars: Vec<char> = line_text.chars().collect();
    let col = cursor_col as usize;

    if col >= chars.len() || char_kind(chars[col]) != CharKind::Word {
        return None;
    }

    let (start, end) = word_bounds(&chars, col, WordType::Small);
    // word_bounds returns inclusive end
    let word: String = chars[start..=end].iter().collect();

    let ranges = find_word_occurrences(buffer, &word);
    Some((word, ranges))
}

/// Scan all buffer lines for whole-word matches of `word`.
///
/// A match is "whole word" if the characters immediately before and after
/// are not word characters (alphanumeric or underscore).
#[cfg_attr(coverage_nightly, coverage(off))]
fn find_word_occurrences(buffer: &dyn BufferOps, word: &str) -> Vec<HighlightRange> {
    let word_chars: Vec<char> = word.chars().collect();
    let word_len = word_chars.len();
    let mut ranges = Vec::new();

    for line_idx in 0..buffer.line_count() {
        let Some(line_text) = buffer.line(line_idx) else {
            continue;
        };
        let line_chars: Vec<char> = line_text.chars().collect();

        let mut col = 0;
        while col + word_len <= line_chars.len() {
            if line_chars[col..col + word_len] == word_chars[..] {
                let before_ok = col == 0 || char_kind(line_chars[col - 1]) != CharKind::Word;
                let after_ok = col + word_len >= line_chars.len()
                    || char_kind(line_chars[col + word_len]) != CharKind::Word;

                if before_ok && after_ok {
                    #[allow(clippy::cast_possible_truncation)]
                    ranges.push(HighlightRange {
                        start_line: line_idx as u32,
                        start_col: col as u32,
                        end_line: line_idx as u32,
                        end_col: (col + word_len) as u32,
                        kind: HighlightKind::Text,
                    });
                    col += word_len;
                    continue;
                }
            }
            col += 1;
        }
    }

    ranges
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
