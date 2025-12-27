//! Which-key event definitions
//!
//! This module defines events used by the which-key plugin that need to be
//! dispatched from core (e.g., from `CommandHandler` when '?' is pressed).

use crate::{event_bus::Event, keystroke::KeySequence};

/// Event to open the which-key panel with a prefix filter
///
/// When dispatched, the which-key plugin will show keybindings that start
/// with the given prefix. For example, if prefix is `[g]`, it will show
/// all bindings starting with `g` (like `gg`, `ge`, `gd`, etc.).
#[derive(Debug, Clone)]
pub struct WhichKeyOpen {
    /// The prefix keys to filter bindings (e.g., [g] for g? trigger)
    pub prefix: KeySequence,
}

impl WhichKeyOpen {
    /// Create a new `WhichKeyOpen` event with the given prefix
    #[must_use]
    pub const fn new(prefix: KeySequence) -> Self {
        Self { prefix }
    }

    /// Create a `WhichKeyOpen` event with no prefix (show all bindings)
    #[must_use]
    pub fn all() -> Self {
        Self {
            prefix: KeySequence::default(),
        }
    }
}

impl Event for WhichKeyOpen {
    fn priority(&self) -> u32 {
        100
    }
}
