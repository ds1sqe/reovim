//! Vim command-line mode key resolver.
//!
//! In command-line mode (`:`, `/`, `?`), typed characters accumulate in
//! the command-line buffer. Enter executes, Escape cancels.

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, Modifiers,
        ResolveContext, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::ModeId,
    reovim_module_cmdline::CmdlineState,
};

use crate::modes::VimMode;

/// Vim command-line mode key resolver.
///
/// Handles input for `:` (Ex commands), `/` (forward search), and `?` (backward search).
/// Characters are accumulated in the command-line buffer until Enter or Escape.
///
/// # Behavior
///
/// - Printable characters → `InsertChar` (goes to cmdline buffer)
/// - Enter → `NotHandled` (keybinding executes command/search)
/// - Escape → `NotHandled` (keybinding cancels)
/// - Backspace → `NotHandled` (keybinding deletes char)
pub struct VimCommandLineResolver {
    mode_id: ModeId,
}

impl VimCommandLineResolver {
    /// Create a new command-line mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: VimMode::COMMANDLINE_ID,
        }
    }

    /// Check if a key should insert a character into the command-line buffer.
    const fn is_insertable(key: &KeyEvent) -> Option<char> {
        // Only consider keys without control/alt modifiers for insertion
        if key.modifiers.contains(Modifiers::CTRL) || key.modifiers.contains(Modifiers::ALT) {
            return None;
        }

        match key.code {
            KeyCode::Char(c) => Some(c),
            // Space is insertable
            // Tab could be for completion (future)
            // Enter is NOT insertable - it executes
            _ => None,
        }
    }
}

impl Default for VimCommandLineResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for VimCommandLineResolver {
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Check for insertable character first
        // Route to CmdlineState extension (#482 - Generic Input Target)
        if let Some(c) = Self::is_insertable(key) {
            return ResolveResult::insert_char_to::<CmdlineState>(c);
        }

        // For non-insertable keys (Escape, Enter, Backspace, etc.), look up in keymap
        let mut keys = KeySequence::new();
        keys.push(*key);
        let lookup_state = input.keymap.query(input.mode, &keys);

        match lookup_state {
            KeyLookupState::ExactWithLonger { exact, .. } | KeyLookupState::ExactOnly(exact) => {
                // Execute the command
                ResolveResult::Execute(exact, ResolveContext::default())
            }
            KeyLookupState::PrefixOnly => {
                // Wait for more keys
                ResolveResult::Pending
            }
            KeyLookupState::NotFound => {
                // No binding found
                ResolveResult::NotHandled
            }
        }
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        // Command-line mode doesn't inherit from other modes
        None
    }

    fn reset(&mut self) {
        // No state to reset
    }
}
