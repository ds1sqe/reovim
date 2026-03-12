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
    reovim_kernel::api::v1::{ModeId, ModuleId},
};

use crate::{BindingInfo, KeySequence};

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
    /// Available continuations: each is a remaining key sequence + binding metadata.
    pub continuations: Vec<(KeySequence, BindingInfo)>,
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

