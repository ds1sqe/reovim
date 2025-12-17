//! Mode switching commands

use crate::buffer::Line;
use crate::command::traits::{CommandResult, CommandTrait, ExecutionContext};
use crate::modd::ModeState;
use std::any::Any;

/// Enter normal mode
#[derive(Debug, Clone)]
pub struct EnterNormalModeCommand;

impl CommandTrait for EnterNormalModeCommand {
    fn name(&self) -> &'static str {
        "enter_normal_mode"
    }

    fn description(&self) -> &'static str {
        "Enter normal mode"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        // Move cursor back one position (vim behavior when exiting insert mode)
        if ctx.buffer.cur.x > 0 {
            ctx.buffer.cur.x -= 1;
        }
        CommandResult::ModeChange(ModeState::normal())
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enter insert mode
#[derive(Debug, Clone)]
pub struct EnterInsertModeCommand;

impl CommandTrait for EnterInsertModeCommand {
    fn name(&self) -> &'static str {
        "enter_insert_mode"
    }

    fn description(&self) -> &'static str {
        "Enter insert mode"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::ModeChange(ModeState::insert())
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enter insert mode after cursor (a)
#[derive(Debug, Clone)]
pub struct EnterInsertModeAfterCommand;

impl CommandTrait for EnterInsertModeAfterCommand {
    fn name(&self) -> &'static str {
        "enter_insert_mode_after"
    }

    fn description(&self) -> &'static str {
        "Enter insert mode after cursor"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        // Move cursor right before entering insert mode
        if let Some(line) = ctx.buffer.contents.get(ctx.buffer.cur.y as usize)
            && (ctx.buffer.cur.x as usize) < line.inner.len()
        {
            ctx.buffer.cur.x += 1;
        }
        CommandResult::ModeChange(ModeState::insert())
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enter insert mode at end of line (A)
#[derive(Debug, Clone)]
pub struct EnterInsertModeEolCommand;

impl CommandTrait for EnterInsertModeEolCommand {
    fn name(&self) -> &'static str {
        "enter_insert_mode_eol"
    }

    fn description(&self) -> &'static str {
        "Enter insert mode at end of line"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        // Move cursor to end of line, then enter insert mode
        if let Some(line) = ctx.buffer.contents.get(ctx.buffer.cur.y as usize) {
            ctx.buffer.cur.x = line.inner.len() as u16;
        }
        CommandResult::ModeChange(ModeState::insert())
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Open line below (o)
#[derive(Debug, Clone)]
pub struct OpenLineBelowCommand;

impl CommandTrait for OpenLineBelowCommand {
    fn name(&self) -> &'static str {
        "open_line_below"
    }

    fn description(&self) -> &'static str {
        "Open new line below and enter insert mode"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        // Insert new line below current line and enter insert mode
        if ctx.buffer.contents.is_empty() {
            ctx.buffer.contents.push(Line::from(""));
        } else {
            let insert_at = (ctx.buffer.cur.y as usize + 1).min(ctx.buffer.contents.len());
            ctx.buffer.contents.insert(insert_at, Line::from(""));
            ctx.buffer.cur.y += 1;
        }
        ctx.buffer.cur.x = 0;
        CommandResult::ModeChange(ModeState::insert())
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Open line above (O)
#[derive(Debug, Clone)]
pub struct OpenLineAboveCommand;

impl CommandTrait for OpenLineAboveCommand {
    fn name(&self) -> &'static str {
        "open_line_above"
    }

    fn description(&self) -> &'static str {
        "Open new line above and enter insert mode"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        // Insert new line above current line and enter insert mode
        if ctx.buffer.contents.is_empty() {
            ctx.buffer.contents.push(Line::from(""));
        } else {
            let insert_at = ctx.buffer.cur.y as usize;
            ctx.buffer.contents.insert(insert_at, Line::from(""));
        }
        ctx.buffer.cur.x = 0;
        CommandResult::ModeChange(ModeState::insert())
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enter visual mode
#[derive(Debug, Clone)]
pub struct EnterVisualModeCommand;

impl CommandTrait for EnterVisualModeCommand {
    fn name(&self) -> &'static str {
        "enter_visual_mode"
    }

    fn description(&self) -> &'static str {
        "Enter visual mode"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::ModeChange(ModeState::visual())
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enter visual block mode (Ctrl-V)
#[derive(Debug, Clone)]
pub struct EnterVisualBlockModeCommand;

impl CommandTrait for EnterVisualBlockModeCommand {
    fn name(&self) -> &'static str {
        "enter_visual_block_mode"
    }

    fn description(&self) -> &'static str {
        "Enter visual block mode"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::ModeChange(ModeState::visual_block())
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enter command mode
#[derive(Debug, Clone)]
pub struct EnterCommandModeCommand;

impl CommandTrait for EnterCommandModeCommand {
    fn name(&self) -> &'static str {
        "enter_command_mode"
    }

    fn description(&self) -> &'static str {
        "Enter command mode"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::ModeChange(ModeState::command())
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
