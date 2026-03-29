//! Window mode key resolver.
//!
//! Window mode (`<C-w>` prefix) handles window navigation and manipulation.
//! Each key in window mode executes a window command and automatically
//! returns to normal mode.
//!
//! # Architecture (Epic #438)
//!
//! Window mode is a simple modal resolver:
//! - Enter via `<C-w>` from normal mode
//! - Each key executes a window command (split, focus, resize, close)
//! - Automatically return to normal mode after command execution
//!
//! # Keybindings
//!
//! - `h/j/k/l` or arrows - Focus navigation
//! - `w/W/p` - Focus cycling
//! - `s/v/n` - Split operations
//! - `c/q/o` - Close operations
//! - `+/-/>/</=` - Resize operations
//! - `<Esc>` - Cancel and return to normal mode

use std::collections::HashMap;

use {
    reovim_driver_input::{
        KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, ModeTransition,
        PopResult, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::ModeId,
};

use crate::modes::VimMode;

/// Vim window mode key resolver.
///
/// Window mode is entered via `<C-w>` from normal mode. Each key executes
/// a window command and the mode automatically returns to normal.
///
/// This resolver is simple compared to operator resolvers:
/// - No count accumulation (window commands don't use counts in window mode)
/// - No multi-key sequences beyond the initial key
/// - Automatic return to normal mode after each command
///
/// # Example
///
/// ```text
/// // Enter window mode (from normal mode via <C-w>)
/// // Press 'h' - executes focus-left, returns to normal
/// // Press 's' - executes split-horizontal, returns to normal
/// ```
pub struct VimWindowResolver {
    /// Mode ID for window mode.
    mode_id: ModeId,
}

impl VimWindowResolver {
    /// Create a new window mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: VimMode::WINDOW_ID,
        }
    }
}

impl Default for VimWindowResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for VimWindowResolver {
    /// Window mode key resolution.
    ///
    /// Each key in window mode is looked up in the keymap. If a binding is found,
    /// the command is executed and we transition back to normal mode. If not found,
    /// the key is ignored (`NotHandled`).
    ///
    /// Unlike operator modes, window mode doesn't accumulate counts or wait for
    /// motions - each key is a complete command.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Query keymap for this key in window mode
        let keys = KeySequence::from_keys(&[*key]);
        let lookup_state = input.keymap.query(input.mode, &keys);

        match lookup_state {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd, .. } => {
                // Found a binding - execute command and return to normal mode
                // Use ModeTransition::Pop with PopResult::ExecuteCommand to both
                // execute the command and return to normal mode (parent mode).

                // Check if this is the cancel command (Escape)
                if cmd == crate::ids::CANCEL_TO_NORMAL {
                    // Just pop back to normal without executing anything extra
                    ResolveResult::ModeTransition(ModeTransition::Pop {
                        result: Some(PopResult::Cancelled),
                    })
                } else {
                    // Window command - execute and pop back to normal mode
                    ResolveResult::ModeTransition(ModeTransition::Pop {
                        result: Some(PopResult::ExecuteCommand {
                            command: cmd,
                            args: HashMap::new(),
                        }),
                    })
                }
            }
            KeyLookupState::PrefixOnly => {
                // Waiting for more keys (window mode currently has no multi-key sequences)
                ResolveResult::Pending
            }
            KeyLookupState::NotFound => {
                // No binding - key not handled
                // Vim ignores unbound keys in window mode without exiting
                ResolveResult::NotHandled
            }
        }
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        // Window mode doesn't inherit from normal mode
        // (keys have different meanings)
        None
    }

    fn reset(&mut self) {
        // No state to reset in window mode
    }
}
