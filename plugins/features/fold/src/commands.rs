//! Fold commands
//!
//! Unified command-event types that extract data from `ExecutionContext`.

use std::any::Any;

use reovim_core::{
    command::traits::{CommandResult, CommandTrait, ExecutionContext},
    event_bus::{DynEvent, Event},
    folding::FoldRange,
};

/// Toggle fold at cursor line (za)
#[derive(Debug, Clone)]
pub struct FoldToggle {
    /// Buffer ID where the fold was toggled
    pub buffer_id: usize,
    /// Line number where the fold starts
    pub line: u32,
    /// Whether the fold is now collapsed (true) or expanded (false)
    pub is_collapsed: bool,
}

impl FoldToggle {
    /// Create a default instance for registration
    #[must_use]
    pub const fn default_instance() -> Self {
        Self {
            buffer_id: 0,
            line: 0,
            is_collapsed: false,
        }
    }
}

impl CommandTrait for FoldToggle {
    fn name(&self) -> &'static str {
        "fold_toggle"
    }

    fn description(&self) -> &'static str {
        "Toggle fold at cursor line"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let line = u32::from(ctx.buffer.cur.y);
        let event = Self {
            buffer_id: ctx.buffer_id,
            line,
            is_collapsed: false, // Will be determined by subscriber
        };
        CommandResult::EmitEvent(DynEvent::new(event))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Event for FoldToggle {
    fn priority(&self) -> u32 {
        100
    }
}

/// Open fold at cursor line (zo)
#[derive(Debug, Clone)]
pub struct FoldOpen {
    /// Buffer ID where the fold was opened
    pub buffer_id: usize,
    /// Line number where the fold starts
    pub line: u32,
}

impl FoldOpen {
    /// Create a default instance for registration
    #[must_use]
    pub const fn default_instance() -> Self {
        Self {
            buffer_id: 0,
            line: 0,
        }
    }
}

impl CommandTrait for FoldOpen {
    fn name(&self) -> &'static str {
        "fold_open"
    }

    fn description(&self) -> &'static str {
        "Open fold at cursor line"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let line = u32::from(ctx.buffer.cur.y);
        let event = Self {
            buffer_id: ctx.buffer_id,
            line,
        };
        CommandResult::EmitEvent(DynEvent::new(event))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Event for FoldOpen {
    fn priority(&self) -> u32 {
        100
    }
}

/// Close fold at cursor line (zc)
#[derive(Debug, Clone)]
pub struct FoldClose {
    /// Buffer ID where the fold was closed
    pub buffer_id: usize,
    /// Line number where the fold starts
    pub line: u32,
}

impl FoldClose {
    /// Create a default instance for registration
    #[must_use]
    pub const fn default_instance() -> Self {
        Self {
            buffer_id: 0,
            line: 0,
        }
    }
}

impl CommandTrait for FoldClose {
    fn name(&self) -> &'static str {
        "fold_close"
    }

    fn description(&self) -> &'static str {
        "Close fold at cursor line"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let line = u32::from(ctx.buffer.cur.y);
        let event = Self {
            buffer_id: ctx.buffer_id,
            line,
        };
        CommandResult::EmitEvent(DynEvent::new(event))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Event for FoldClose {
    fn priority(&self) -> u32 {
        100
    }
}

/// Open all folds in buffer (zR)
#[derive(Debug, Clone)]
pub struct FoldOpenAll {
    /// Buffer ID where all folds were opened
    pub buffer_id: usize,
}

impl FoldOpenAll {
    /// Create a default instance for registration
    #[must_use]
    pub const fn default_instance() -> Self {
        Self { buffer_id: 0 }
    }
}

impl CommandTrait for FoldOpenAll {
    fn name(&self) -> &'static str {
        "fold_open_all"
    }

    fn description(&self) -> &'static str {
        "Open all folds in buffer"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let event = Self {
            buffer_id: ctx.buffer_id,
        };
        CommandResult::EmitEvent(DynEvent::new(event))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Event for FoldOpenAll {
    fn priority(&self) -> u32 {
        100
    }
}

/// Close all folds in buffer (zM)
#[derive(Debug, Clone)]
pub struct FoldCloseAll {
    /// Buffer ID where all folds were closed
    pub buffer_id: usize,
}

impl FoldCloseAll {
    /// Create a default instance for registration
    #[must_use]
    pub const fn default_instance() -> Self {
        Self { buffer_id: 0 }
    }
}

impl CommandTrait for FoldCloseAll {
    fn name(&self) -> &'static str {
        "fold_close_all"
    }

    fn description(&self) -> &'static str {
        "Close all folds in buffer"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let event = Self {
            buffer_id: ctx.buffer_id,
        };
        CommandResult::EmitEvent(DynEvent::new(event))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Event for FoldCloseAll {
    fn priority(&self) -> u32 {
        100
    }
}

/// Event emitted when fold ranges are updated (e.g., after parsing)
///
/// This is an event-only type (not triggered by a command).
#[derive(Debug, Clone)]
pub struct FoldRangesUpdated {
    /// Buffer ID where ranges were updated
    pub buffer_id: usize,
    /// The new fold ranges
    pub ranges: Vec<FoldRange>,
}

impl Event for FoldRangesUpdated {
    fn priority(&self) -> u32 {
        50 // Higher priority for range updates
    }
}
