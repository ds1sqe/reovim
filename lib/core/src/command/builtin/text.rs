//! Text editing commands

use crate::buffer::TextOps;
use crate::command::traits::{CommandResult, CommandTrait, ExecutionContext};
use std::any::Any;

/// Insert a character at cursor position
#[derive(Debug, Clone)]
pub struct InsertCharCommand {
    pub char: char,
}

impl InsertCharCommand {
    #[must_use]
    pub const fn new(c: char) -> Self {
        Self { char: c }
    }
}

impl CommandTrait for InsertCharCommand {
    fn name(&self) -> &'static str {
        "insert_char"
    }

    fn description(&self) -> &'static str {
        "Insert a character at cursor position"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        ctx.buffer.insert_char(self.char);
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Insert a newline at cursor position (Enter key)
#[derive(Debug, Clone)]
pub struct InsertNewlineCommand;

impl CommandTrait for InsertNewlineCommand {
    fn name(&self) -> &'static str {
        "insert_newline"
    }

    fn description(&self) -> &'static str {
        "Insert a newline at cursor position"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        ctx.buffer.insert_newline();
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Delete character backward (backspace)
#[derive(Debug, Clone)]
pub struct DeleteCharBackwardCommand;

impl CommandTrait for DeleteCharBackwardCommand {
    fn name(&self) -> &'static str {
        "delete_char_backward"
    }

    fn description(&self) -> &'static str {
        "Delete character before cursor"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        ctx.buffer.delete_char_backward();
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Delete character forward (x in normal mode)
#[derive(Debug, Clone)]
pub struct DeleteCharForwardCommand;

impl CommandTrait for DeleteCharForwardCommand {
    fn name(&self) -> &'static str {
        "delete_char_forward"
    }

    fn description(&self) -> &'static str {
        "Delete character at cursor"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        ctx.buffer.delete_char_forward();
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Delete entire line
#[derive(Debug, Clone)]
pub struct DeleteLineCommand;

impl CommandTrait for DeleteLineCommand {
    fn name(&self) -> &'static str {
        "delete_line"
    }

    fn description(&self) -> &'static str {
        "Delete current line"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        ctx.buffer.delete_line();
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
