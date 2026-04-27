//! Initial mode provider for cross-personality support (#623).
//!
//! Personality modules (vim, emacs) register their initial mode during
//! `init()`. Bootstrap reads it after all modules have initialized to
//! determine which mode to start in.
//!
//! This follows the established `ServiceRegistry` pattern used by
//! `LookupPolicyStore`, `KeybindingStore`, `ModeBridgeStore`, etc.

use reovim_kernel::api::v1::{ModeId, Service};

/// Provider for the initial editor mode.
///
/// Personality modules call [`set`](Self::set) during `init()` to declare
/// their initial mode. Bootstrap calls [`get`](Self::get) after all modules
/// have loaded to determine the startup mode.
///
/// If multiple personality modules are loaded (e.g., both vim and emacs),
/// the last writer wins and a warning is logged.
pub struct InitialModeProvider {
    mode: parking_lot::RwLock<Option<ModeId>>,
}

impl InitialModeProvider {
    /// Create a new empty provider.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode: parking_lot::RwLock::new(None),
        }
    }

    /// Set the initial mode. Called by personality modules during `init()`.
    ///
    /// If a mode was already set (another personality module registered first),
    /// the previous value is overwritten. The caller (personality module) is
    /// responsible for logging if needed.
    ///
    /// Returns the previous mode if one was already set.
    pub fn set(&self, mode: ModeId) -> Option<ModeId> {
        self.mode.write().replace(mode)
    }

    /// Get the registered initial mode, if any.
    #[must_use]
    pub fn get(&self) -> Option<ModeId> {
        self.mode.read().clone()
    }
}

impl Default for InitialModeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for InitialModeProvider {}

#[cfg(test)]
#[path = "initial_mode_tests.rs"]
mod tests;
