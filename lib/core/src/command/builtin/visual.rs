//! Visual mode commands

use {
    crate::{
        buffer::SelectionOps,
        command::traits::{CommandResult, CommandTrait, ExecutionContext},
    },
    std::any::Any,
};

/// Extend selection up
#[derive(Debug, Clone)]
pub struct VisualExtendUpCommand;

impl CommandTrait for VisualExtendUpCommand {
    fn name(&self) -> &'static str {
        "visual_extend_up"
    }

    fn description(&self) -> &'static str {
        "Extend selection up"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let count = ctx.count.unwrap_or(1);
        ctx.buffer.cur.y = ctx.buffer.cur.y.saturating_sub(count as u16);
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Extend selection down
#[derive(Debug, Clone)]
pub struct VisualExtendDownCommand;

impl CommandTrait for VisualExtendDownCommand {
    fn name(&self) -> &'static str {
        "visual_extend_down"
    }

    fn description(&self) -> &'static str {
        "Extend selection down"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let count = ctx.count.unwrap_or(1);
        let max_y = ctx.buffer.contents.len().saturating_sub(1) as u16;
        ctx.buffer.cur.y = (ctx.buffer.cur.y + count as u16).min(max_y);
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Extend selection left
#[derive(Debug, Clone)]
pub struct VisualExtendLeftCommand;

impl CommandTrait for VisualExtendLeftCommand {
    fn name(&self) -> &'static str {
        "visual_extend_left"
    }

    fn description(&self) -> &'static str {
        "Extend selection left"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let count = ctx.count.unwrap_or(1);
        ctx.buffer.cur.x = ctx.buffer.cur.x.saturating_sub(count as u16);
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Extend selection right
#[derive(Debug, Clone)]
pub struct VisualExtendRightCommand;

impl CommandTrait for VisualExtendRightCommand {
    fn name(&self) -> &'static str {
        "visual_extend_right"
    }

    fn description(&self) -> &'static str {
        "Extend selection right"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let count = ctx.count.unwrap_or(1);
        if let Some(line) = ctx.buffer.contents.get(ctx.buffer.cur.y as usize) {
            let max_x = line.inner.len().saturating_sub(1) as u16;
            ctx.buffer.cur.x = (ctx.buffer.cur.x + count as u16).min(max_x);
        }
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Delete selection
#[derive(Debug, Clone)]
pub struct VisualDeleteCommand;

impl CommandTrait for VisualDeleteCommand {
    fn name(&self) -> &'static str {
        "visual_delete"
    }

    fn description(&self) -> &'static str {
        "Delete selected text"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let text = ctx.buffer.delete_selection();
        CommandResult::ClipboardWrite {
            text,
            register: None,
        }
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Yank (copy) selection
#[derive(Debug, Clone)]
pub struct VisualYankCommand;

impl CommandTrait for VisualYankCommand {
    fn name(&self) -> &'static str {
        "visual_yank"
    }

    fn description(&self) -> &'static str {
        "Copy selected text"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let text = ctx.buffer.get_selected_text();
        ctx.buffer.clear_selection();
        CommandResult::ClipboardWrite {
            text,
            register: None,
        }
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
