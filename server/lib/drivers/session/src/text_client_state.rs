//! Per-client text-domain state bundle.
//!
//! [`TextClientState`] groups the 4 per-client text-domain state types into
//! a single [`SessionExtension`]. This is the migration target: when the
//! server switches to `DomainDriver` (sub-plan 05), per-client text state
//! will be owned by `TextDomainDriver` and accessed through
//! `ExtensionMap::get::<TextClientState>()`.
//!
//! In the current architecture, these fields are borrowed individually by
//! `SessionRuntime` from `server::EditingState`. This type exists as a
//! parallel bundle that does NOT change `EditingState` or `SessionRuntime`.

use {
    reovim_domain_text::{HistoryRing, RegisterBank},
    reovim_subsys_session::SessionExtension,
};

use crate::{Jumplist, MarkBank};

/// Per-client text-domain state stored as a `SessionExtension`.
///
/// Bundles register storage, clipboard history, local marks, and jump list
/// into one extension type for `ExtensionMap` access.
///
/// # Fields
///
/// - `registers`: Named registers (unnamed, a-z/A-Z) for yank/paste
/// - `clipboard_history`: Numbered registers 0-9 (clipboard ring)
/// - `local_marks`: Buffer-local marks (a-z) and special marks
/// - `jumplist`: Cursor position history for Ctrl-O / Ctrl-I
pub struct TextClientState {
    /// Per-client register storage (unnamed, named a-z/A-Z).
    pub registers: RegisterBank,
    /// Per-client clipboard history ring (numbered registers 0-9).
    pub clipboard_history: HistoryRing,
    /// Per-client local marks (a-z) and special marks.
    pub local_marks: MarkBank,
    /// Per-client jump list for Ctrl-O / Ctrl-I navigation.
    pub jumplist: Jumplist,
}

impl TextClientState {
    /// Create a new text client state with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registers: RegisterBank::new(),
            clipboard_history: HistoryRing::new(),
            local_marks: MarkBank::new(),
            jumplist: Jumplist::new(),
        }
    }

    /// Borrow the register bank.
    #[must_use]
    pub const fn registers(&self) -> &RegisterBank {
        &self.registers
    }

    /// Mutably borrow the register bank.
    pub const fn registers_mut(&mut self) -> &mut RegisterBank {
        &mut self.registers
    }

    /// Borrow the clipboard history ring.
    #[must_use]
    pub const fn clipboard_history(&self) -> &HistoryRing {
        &self.clipboard_history
    }

    /// Mutably borrow the clipboard history ring.
    pub const fn clipboard_history_mut(&mut self) -> &mut HistoryRing {
        &mut self.clipboard_history
    }

    /// Borrow the local mark bank.
    #[must_use]
    pub const fn local_marks(&self) -> &MarkBank {
        &self.local_marks
    }

    /// Mutably borrow the local mark bank.
    pub const fn local_marks_mut(&mut self) -> &mut MarkBank {
        &mut self.local_marks
    }

    /// Borrow the jump list.
    #[must_use]
    pub const fn jumplist(&self) -> &Jumplist {
        &self.jumplist
    }

    /// Mutably borrow the jump list.
    pub const fn jumplist_mut(&mut self) -> &mut Jumplist {
        &mut self.jumplist
    }
}

impl Default for TextClientState {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionExtension for TextClientState {
    fn create() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "text_client_state_tests.rs"]
mod tests;
