//! Pending key bindings extension for generic bridge consumption.
//!
//! [`PendingBindings`] records the accumulated pending keys and available
//! continuations after a resolver returns `Pending`. This is populated
//! generically by the session layer (not by any specific module) and
//! consumed by bridges (e.g., `WhichKeyBridge`) to produce UI hints.
//!
//! # Architecture (#468)
//!
//! ```text
//! VimNormalResolver::pending_keys()       ← trait method (ModeKeyResolver)
//!   → resolve_key_for_client (state.rs)   ← populates PendingBindings
//!     → WhichKeyBridge::snapshot()        ← reads PendingBindings, produces JSON
//! ```
//!
//! This is **mechanism** — records WHAT happened, not HOW to display it.

use {
    reovim_driver_session::SessionExtension,
    reovim_kernel::api::v1::{CommandId, ModeId, ModuleId},
};

use crate::KeySequence;

/// Sentinel module used for uninitialized `PendingBindings`.
const PENDING_MODULE: ModuleId = ModuleId::new("__pending__");

/// Pending key sequence and available continuations.
///
/// Populated by `resolve_key_for_client` after a `Pending` result or
/// `ModeTransition::Push` (operator modes like DELETE, YANK, CHANGE).
/// Consumed by bridges (e.g., `WhichKeyBridge`) to produce UI hints.
///
/// Lives in driver-input because it uses input types (`KeySequence`, `CommandId`).
pub struct PendingBindings {
    /// The key that triggered the current mode (e.g., `d` for DELETE).
    ///
    /// Set when a `ModeTransition::Push` occurs. Preserved across `Pending`
    /// updates within the pushed mode so the display shows the full prefix
    /// (e.g., "d" then "di" as the user types).
    pub mode_prefix: KeySequence,
    /// The accumulated pending key sequence within the mode (e.g., `i` in `di`).
    pub pending_keys: KeySequence,
    /// The mode in which the pending occurred.
    pub mode: ModeId,
    /// Available continuations: each is a remaining key sequence + command.
    pub continuations: Vec<(KeySequence, CommandId)>,
}

impl SessionExtension for PendingBindings {
    fn create() -> Self {
        Self {
            mode_prefix: KeySequence::new(),
            pending_keys: KeySequence::new(),
            mode: ModeId::new(PENDING_MODULE, "none"),
            continuations: Vec::new(),
        }
    }
}

impl PendingBindings {
    /// Whether there are active pending bindings.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !self.continuations.is_empty()
    }

    /// Clear all pending state.
    pub fn clear(&mut self) {
        self.mode_prefix.clear();
        self.pending_keys.clear();
        self.continuations.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MODULE: ModuleId = ModuleId::new("test");

    fn test_mode() -> ModeId {
        ModeId::new(TEST_MODULE, "normal")
    }

    fn goto_top_cmd() -> CommandId {
        CommandId::new(TEST_MODULE, "goto-top")
    }

    fn goto_def_cmd() -> CommandId {
        CommandId::new(TEST_MODULE, "goto-definition")
    }

    #[test]
    fn test_pending_bindings_create_default() {
        let pb = PendingBindings::create();
        assert!(pb.mode_prefix.is_empty());
        assert!(pb.pending_keys.is_empty());
        assert!(pb.continuations.is_empty());
        assert!(!pb.is_active());
    }

    #[test]
    fn test_pending_bindings_is_active() {
        let mut pb = PendingBindings::create();
        assert!(!pb.is_active());

        pb.continuations.push((KeySequence::new(), goto_top_cmd()));
        assert!(pb.is_active());
    }

    #[test]
    fn test_pending_bindings_clear() {
        let mut pb = PendingBindings::create();
        pb.mode_prefix = KeySequence::from_keys(&[crate::KeyEvent::new(crate::KeyCode::Char('d'))]);
        pb.pending_keys =
            KeySequence::from_keys(&[crate::KeyEvent::new(crate::KeyCode::Char('g'))]);
        pb.mode = test_mode();
        pb.continuations.push((KeySequence::new(), goto_top_cmd()));

        assert!(pb.is_active());

        pb.clear();
        assert!(pb.mode_prefix.is_empty());
        assert!(pb.pending_keys.is_empty());
        assert!(pb.continuations.is_empty());
        assert!(!pb.is_active());
    }

    #[test]
    fn test_pending_bindings_mode_prefix() {
        let mut pb = PendingBindings::create();
        let d_key = crate::KeyEvent::new(crate::KeyCode::Char('d'));
        pb.mode_prefix = KeySequence::from_keys(&[d_key]);
        pb.continuations.push((KeySequence::new(), goto_top_cmd()));

        assert!(pb.is_active());
        assert!(!pb.mode_prefix.is_empty());
        assert!(pb.pending_keys.is_empty());
    }

    #[test]
    fn test_pending_bindings_mode_prefix_with_pending_keys() {
        let mut pb = PendingBindings::create();
        pb.mode_prefix = KeySequence::from_keys(&[crate::KeyEvent::new(crate::KeyCode::Char('d'))]);
        pb.pending_keys =
            KeySequence::from_keys(&[crate::KeyEvent::new(crate::KeyCode::Char('i'))]);
        pb.continuations.push((KeySequence::new(), goto_top_cmd()));

        assert!(pb.is_active());
        assert!(!pb.mode_prefix.is_empty());
        assert!(!pb.pending_keys.is_empty());
    }

    #[test]
    fn test_pending_bindings_multiple_continuations() {
        let mut pb = PendingBindings::create();
        pb.continuations.push((
            KeySequence::from_keys(&[crate::KeyEvent::new(crate::KeyCode::Char('g'))]),
            goto_top_cmd(),
        ));
        pb.continuations.push((
            KeySequence::from_keys(&[crate::KeyEvent::new(crate::KeyCode::Char('d'))]),
            goto_def_cmd(),
        ));

        assert!(pb.is_active());
        assert_eq!(pb.continuations.len(), 2);
    }
}
