//! Which-key commands

use std::{any::Any, sync::Arc};

use reovim_core::{
    command::{CommandResult, CommandTrait, ExecutionContext},
    declare_event_command,
    event_bus::{DynEvent, Event},
    keystroke::KeySequence,
};

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

/// Command to trigger which-key with a specific prefix
///
/// This is used as an inline command for keybindings like `g?`, `z?`, etc.
/// Each binding carries its own prefix so which-key knows what to filter.
#[derive(Debug, Clone)]
pub struct WhichKeyTrigger {
    pub prefix: KeySequence,
}

impl WhichKeyTrigger {
    #[must_use]
    pub const fn new(prefix: KeySequence) -> Self {
        Self { prefix }
    }

    #[must_use]
    pub fn all() -> Self {
        Self {
            prefix: KeySequence::default(),
        }
    }

    /// Create an Arc-wrapped command for use as inline command
    #[must_use]
    pub fn arc(prefix: KeySequence) -> Arc<dyn CommandTrait> {
        Arc::new(Self::new(prefix))
    }
}

impl CommandTrait for WhichKeyTrigger {
    fn name(&self) -> &'static str {
        "which_key_trigger"
    }

    fn description(&self) -> &'static str {
        "Show available keybindings"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        // Emit WhichKeyOpen event with this command's prefix
        CommandResult::EmitEvent(DynEvent::new(WhichKeyOpen::new(self.prefix.clone())))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

declare_event_command! {
    WhichKeyClose,
    id: "which_key_close",
    description: "Close which-key panel",
}

declare_event_command! {
    WhichKeyBackspace,
    id: "which_key_backspace",
    description: "Remove last key from filter",
}
