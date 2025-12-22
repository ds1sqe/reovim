//! Fold commands
//!
//! Commands emit fold events via the event bus.

use std::any::Any;

use reovim_core::{
    command::traits::{CommandResult, CommandTrait, ExecutionContext},
    event_bus::DynEvent,
};

use crate::events::{
    FoldCloseAllEvent, FoldCloseEvent, FoldOpenAllEvent, FoldOpenEvent, FoldToggleEvent,
};

/// Toggle fold at cursor line (za)
#[derive(Debug, Clone)]
pub struct FoldToggleCommand;

impl CommandTrait for FoldToggleCommand {
    fn name(&self) -> &'static str {
        "fold_toggle"
    }

    fn description(&self) -> &'static str {
        "Toggle fold at cursor line"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let line = u32::from(ctx.buffer.cur.y);
        let event = FoldToggleEvent {
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

/// Open fold at cursor line (zo)
#[derive(Debug, Clone)]
pub struct FoldOpenCommand;

impl CommandTrait for FoldOpenCommand {
    fn name(&self) -> &'static str {
        "fold_open"
    }

    fn description(&self) -> &'static str {
        "Open fold at cursor line"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let line = u32::from(ctx.buffer.cur.y);
        let event = FoldOpenEvent {
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

/// Close fold at cursor line (zc)
#[derive(Debug, Clone)]
pub struct FoldCloseCommand;

impl CommandTrait for FoldCloseCommand {
    fn name(&self) -> &'static str {
        "fold_close"
    }

    fn description(&self) -> &'static str {
        "Close fold at cursor line"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let line = u32::from(ctx.buffer.cur.y);
        let event = FoldCloseEvent {
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

/// Open all folds in buffer (zR)
#[derive(Debug, Clone)]
pub struct FoldOpenAllCommand;

impl CommandTrait for FoldOpenAllCommand {
    fn name(&self) -> &'static str {
        "fold_open_all"
    }

    fn description(&self) -> &'static str {
        "Open all folds in buffer"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let event = FoldOpenAllEvent {
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

/// Close all folds in buffer (zM)
#[derive(Debug, Clone)]
pub struct FoldCloseAllCommand;

impl CommandTrait for FoldCloseAllCommand {
    fn name(&self) -> &'static str {
        "fold_close_all"
    }

    fn description(&self) -> &'static str {
        "Close all folds in buffer"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let event = FoldCloseAllEvent {
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
