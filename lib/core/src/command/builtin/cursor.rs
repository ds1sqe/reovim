//! Cursor movement commands

use {
    crate::{
        buffer::{CursorOps, calculate_motion_with_desired_col},
        command::traits::{CommandResult, CommandTrait, ExecutionContext},
        motion::Motion,
    },
    std::any::Any,
};

/// Move cursor up
#[derive(Debug, Clone)]
pub struct CursorUpCommand;

impl CommandTrait for CursorUpCommand {
    fn name(&self) -> &'static str {
        "cursor_up"
    }

    fn description(&self) -> &'static str {
        "Move cursor up"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let count = ctx.count.unwrap_or(1);
        let (new_pos, new_desired_col) = calculate_motion_with_desired_col(
            &ctx.buffer.contents,
            ctx.buffer.cur,
            ctx.buffer.desired_col,
            Motion::Up,
            count,
        );
        ctx.buffer.cur = new_pos;
        ctx.buffer.desired_col = new_desired_col;
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Move cursor down
#[derive(Debug, Clone)]
pub struct CursorDownCommand;

impl CommandTrait for CursorDownCommand {
    fn name(&self) -> &'static str {
        "cursor_down"
    }

    fn description(&self) -> &'static str {
        "Move cursor down"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let count = ctx.count.unwrap_or(1);
        let (new_pos, new_desired_col) = calculate_motion_with_desired_col(
            &ctx.buffer.contents,
            ctx.buffer.cur,
            ctx.buffer.desired_col,
            Motion::Down,
            count,
        );
        ctx.buffer.cur = new_pos;
        ctx.buffer.desired_col = new_desired_col;
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Move cursor left
#[derive(Debug, Clone)]
pub struct CursorLeftCommand;

impl CommandTrait for CursorLeftCommand {
    fn name(&self) -> &'static str {
        "cursor_left"
    }

    fn description(&self) -> &'static str {
        "Move cursor left"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let count = ctx.count.unwrap_or(1);
        ctx.buffer.cur.x = ctx.buffer.cur.x.saturating_sub(count as u16);
        ctx.buffer.clear_desired_col(); // Clear on horizontal movement
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Move cursor right
#[derive(Debug, Clone)]
pub struct CursorRightCommand;

impl CommandTrait for CursorRightCommand {
    fn name(&self) -> &'static str {
        "cursor_right"
    }

    fn description(&self) -> &'static str {
        "Move cursor right"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let count = ctx.count.unwrap_or(1);
        if let Some(line) = ctx.buffer.contents.get(ctx.buffer.cur.y as usize) {
            let max_x = line.inner.len().saturating_sub(1) as u16;
            ctx.buffer.cur.x = (ctx.buffer.cur.x + count as u16).min(max_x);
        }
        ctx.buffer.clear_desired_col(); // Clear on horizontal movement
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Move cursor to line start
#[derive(Debug, Clone)]
pub struct CursorLineStartCommand;

impl CommandTrait for CursorLineStartCommand {
    fn name(&self) -> &'static str {
        "cursor_line_start"
    }

    fn description(&self) -> &'static str {
        "Move cursor to start of line"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        ctx.buffer.cur.x = 0;
        ctx.buffer.clear_desired_col(); // Clear on horizontal movement
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Move cursor to line end
#[derive(Debug, Clone)]
pub struct CursorLineEndCommand;

impl CommandTrait for CursorLineEndCommand {
    fn name(&self) -> &'static str {
        "cursor_line_end"
    }

    fn description(&self) -> &'static str {
        "Move cursor to end of line"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        if let Some(line) = ctx.buffer.contents.get(ctx.buffer.cur.y as usize) {
            ctx.buffer.cur.x = line.inner.len().saturating_sub(1) as u16;
        }
        ctx.buffer.clear_desired_col(); // Clear on horizontal movement
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Move cursor to next word
#[derive(Debug, Clone)]
pub struct CursorWordForwardCommand;

impl CommandTrait for CursorWordForwardCommand {
    fn name(&self) -> &'static str {
        "cursor_word_forward"
    }

    fn description(&self) -> &'static str {
        "Move cursor to next word"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        ctx.buffer.word_forward();
        ctx.buffer.clear_desired_col(); // Clear on horizontal movement
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Move cursor to previous word
#[derive(Debug, Clone)]
pub struct CursorWordBackwardCommand;

impl CommandTrait for CursorWordBackwardCommand {
    fn name(&self) -> &'static str {
        "cursor_word_backward"
    }

    fn description(&self) -> &'static str {
        "Move cursor to previous word"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        ctx.buffer.word_backward();
        ctx.buffer.clear_desired_col(); // Clear on horizontal movement
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Go to first line (or line N with count)
#[derive(Debug, Clone)]
pub struct GotoFirstLineCommand;

impl CommandTrait for GotoFirstLineCommand {
    fn name(&self) -> &'static str {
        "goto_first_line"
    }

    fn description(&self) -> &'static str {
        "Go to first line (or line N with count)"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        if let Some(line_num) = ctx.count {
            let target_line = line_num.saturating_sub(1);
            let max_y = ctx.buffer.contents.len().saturating_sub(1);
            ctx.buffer.cur.y = target_line.min(max_y) as u16;
        } else {
            ctx.buffer.cur.y = 0;
        }
        ctx.buffer.cur.x = 0;
        ctx.buffer.clear_desired_col();
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_jump(&self) -> bool {
        true
    }
}

/// Go to last line (or line N with count)
#[derive(Debug, Clone)]
pub struct GotoLastLineCommand;

impl CommandTrait for GotoLastLineCommand {
    fn name(&self) -> &'static str {
        "goto_last_line"
    }

    fn description(&self) -> &'static str {
        "Go to last line (or line N with count)"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        if let Some(line_num) = ctx.count {
            let target_line = line_num.saturating_sub(1);
            let max_y = ctx.buffer.contents.len().saturating_sub(1);
            ctx.buffer.cur.y = target_line.min(max_y) as u16;
        } else {
            let max_y = ctx.buffer.contents.len().saturating_sub(1) as u16;
            ctx.buffer.cur.y = max_y;
        }
        ctx.buffer.cur.x = 0;
        ctx.buffer.clear_desired_col();
        CommandResult::NeedsRender
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_jump(&self) -> bool {
        true
    }
}
