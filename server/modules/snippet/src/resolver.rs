//! Snippet mode key resolver (#136).
//!
//! `SnippetResolver` is the `ModeKeyResolver` for the `snippet:navigating` mode.
//! It handles snippet-specific key bindings (Tab, S-Tab, Esc) and delegates
//! unhandled keys to the vim insert resolver via `inherits_from()`.
//!
//! When a placeholder is visually selected, printable character input replaces
//! the selection via `resolve_with_session` — the resolver deletes the selected
//! range and inserts the typed character directly through the session API.

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, Modifiers,
        ResolveContext, ResolveInput, ResolveResult,
    },
    reovim_driver_session::{ExtensionMap, SessionApiDyn},
    reovim_kernel::api::v1::ModeId,
    reovim_types_text::Edit,
};

use crate::{ids, state::SnippetSessionState};

/// Key resolver for snippet navigation mode.
///
/// When the snippet module activates `snippet:navigating` mode, this
/// resolver handles Tab/S-Tab for tab stop navigation and Esc for
/// cancellation. Printable characters replace any active placeholder
/// selection. Unhandled keys fall through to vim insert mode via
/// the `inherits_from()` chain in `ResolverRegistry`.
pub struct SnippetResolver {
    /// The mode ID for this resolver (owned to satisfy lifetime requirements).
    mode_id: ModeId,
    /// The parent mode ID for inheritance (owned).
    parent_mode_id: ModeId,
}

impl SnippetResolver {
    /// Create a snippet resolver with the given parent mode for inheritance.
    ///
    /// The parent mode is resolved at init time via `ModeInfoStore::find_by_name()`
    /// rather than hardcoding foreign module constants.
    #[must_use]
    pub const fn with_parent(parent_mode: ModeId) -> Self {
        Self {
            mode_id: ids::NAVIGATING_MODE,
            parent_mode_id: parent_mode,
        }
    }
}

impl ModeKeyResolver for SnippetResolver {
    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        // Fall through to vim insert for unhandled keys
        Some(&self.parent_mode_id)
    }

    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Query keymap for snippet-specific bindings
        let keys = KeySequence::from_keys(&[*key]);
        match input.keymap.query(self.mode_id(), &keys) {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                ResolveResult::Execute(cmd, ResolveContext::new())
            }
            KeyLookupState::PrefixOnly => ResolveResult::Pending,
            KeyLookupState::NotFound => ResolveResult::NotHandled,
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn resolve_with_session(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        session: &mut dyn SessionApiDyn,
        _shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // Check keymap first (Tab/S-Tab/Esc).
        let keymap_result = self.resolve_with_keymap(key, state, input);
        if !matches!(keymap_result, ResolveResult::NotHandled) {
            return keymap_result;
        }

        // If the key is a printable character (no Ctrl/Alt modifiers)
        // and there's an active placeholder selection, replace it.
        let is_printable = matches!(key.code, KeyCode::Char(_))
            && (key.modifiers == Modifiers::NONE || key.modifiers == Modifiers::SHIFT);

        if is_printable && let Some(sel) = session.active_selection().cloned() {
            // Get the buffer ID from the active window.
            let Some(window_id) = session.active_window() else {
                return ResolveResult::NotHandled;
            };
            let Some(buffer_id) = session.window_buffer(window_id) else {
                return ResolveResult::NotHandled;
            };

            // Read the placeholder text being replaced (for position updates).
            let placeholder_text = session
                .buffer_text_range(buffer_id, sel.start, sel.end)
                .unwrap_or_default();

            // Delete the selected placeholder text.
            session.delete_range(buffer_id, sel.start, sel.end);

            // Update snippet positions to account for the deletion.
            let delete_edit = Edit::delete(sel.start, &placeholder_text);
            let snippet_state = client_extensions.get_or_insert::<SnippetSessionState>();
            if let Some(active) = &mut snippet_state.active {
                active.update_positions(&delete_edit);
            }

            // Clear selection — the character itself is NOT inserted here.
            // By returning NotHandled, we let the parent resolver (vim insert)
            // handle the insertion. This correctly advances the cursor and
            // records buffer modifications. reconcile_typing() adjusts
            // snippet positions when the user tabs away.
            session.set_active_selection(None);
            session.record_selection_change(buffer_id);

            // Fall through to vim insert mode for the character insertion.
        }

        // Not handled — fall through to vim insert mode
        ResolveResult::NotHandled
    }
}

#[cfg(test)]
#[path = "resolver_tests.rs"]
mod tests;
