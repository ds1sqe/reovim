//! Text editing commands

use {
    crate::{
        buffer::TextOps,
        command::traits::{
            CommandResult, CommandTrait, DeferredAction, ExecutionContext, OperatorMotionAction,
        },
    },
    std::any::Any,
};

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

    fn is_text_modifying(&self) -> bool {
        true
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

    fn is_text_modifying(&self) -> bool {
        true
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

    fn is_text_modifying(&self) -> bool {
        true
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

    fn is_text_modifying(&self) -> bool {
        true
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
        let deleted = ctx.buffer.delete_line();
        if deleted.is_empty() {
            CommandResult::NeedsRender
        } else {
            use crate::register::YankType;
            CommandResult::ClipboardWrite {
                text: deleted,
                register: None,
                mode_change: None,
                yank_type: Some(YankType::Linewise),
                yank_range: None, // Delete doesn't need animation
            }
        }
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_text_modifying(&self) -> bool {
        true
    }
}

/// Yank (copy) current line
#[derive(Debug, Clone)]
pub struct YankLineCommand;

impl CommandTrait for YankLineCommand {
    fn name(&self) -> &'static str {
        "yank_line"
    }

    fn description(&self) -> &'static str {
        "Yank (copy) current line"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        use crate::{register::YankType, screen::Position};

        let y = ctx.buffer.cur.y;
        let y_usize = y as usize;
        let text = ctx
            .buffer
            .contents
            .get(y_usize)
            .map(|line| line.inner.clone() + "\n")
            .unwrap_or_default();

        // Calculate range for yank animation (entire line)
        let line_len = ctx
            .buffer
            .contents
            .get(y_usize)
            .map_or(0, |line| line.inner.len());
        let yank_range = Some((
            Position { x: 0, y },
            Position {
                x: line_len as u16,
                y,
            },
        ));

        CommandResult::ClipboardWrite {
            text,
            register: None,
            mode_change: None,
            yank_type: Some(YankType::Linewise),
            yank_range,
        }
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Yank (copy) from cursor to end of line
#[derive(Debug, Clone)]
pub struct YankToEndCommand;

impl CommandTrait for YankToEndCommand {
    fn name(&self) -> &'static str {
        "yank_to_end"
    }

    fn description(&self) -> &'static str {
        "Yank from cursor to end of line"
    }

    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        use crate::{register::YankType, screen::Position};

        let y_u16 = ctx.buffer.cur.y;
        let x_u16 = ctx.buffer.cur.x;
        let y = y_u16 as usize;
        let x = x_u16 as usize;
        let text = ctx
            .buffer
            .contents
            .get(y)
            .map(|line| {
                if x < line.inner.len() {
                    line.inner[x..].to_string()
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();

        // Calculate range for yank animation (cursor to end of line)
        // For Y command, animate from cursor to end of line
        let line_len = ctx
            .buffer
            .contents
            .get(y)
            .map_or(0, |line| line.inner.len());

        let yank_range = if !text.is_empty() && x < line_len {
            // Start animation from cursor position + 1 to avoid highlighting extra left char
            // The yanked text is still correct (line.inner[x..]), but the animation
            // visual feedback is adjusted based on how the rendering system interprets positions
            Some((
                Position {
                    x: x_u16.saturating_add(1),
                    y: y_u16,
                },
                Position {
                    x: line_len as u16,
                    y: y_u16,
                },
            ))
        } else {
            None
        };

        CommandResult::ClipboardWrite {
            text,
            register: None,
            mode_change: None,
            yank_type: Some(YankType::Characterwise),
            yank_range,
        }
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Change entire line (cc) - clears line content and enters insert mode
#[derive(Debug, Clone)]
pub struct ChangeLineCommand;

impl CommandTrait for ChangeLineCommand {
    fn name(&self) -> &'static str {
        "change_line"
    }

    fn description(&self) -> &'static str {
        "Change entire line"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::OperatorMotion(
            OperatorMotionAction::ChangeLine,
        ))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_text_modifying(&self) -> bool {
        true
    }
}
