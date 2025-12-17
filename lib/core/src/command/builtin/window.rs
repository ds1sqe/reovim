//! Window management commands

use crate::command::traits::{
    CommandResult, CommandTrait, DeferredAction, ExecutionContext, WindowAction,
};
use crate::screen::NavigateDirection;
use std::any::Any;

/// Focus window to the left (C-h)
#[derive(Debug, Clone)]
pub struct WindowFocusLeftCommand;

impl CommandTrait for WindowFocusLeftCommand {
    fn name(&self) -> &'static str {
        "window_focus_left"
    }

    fn description(&self) -> &'static str {
        "Focus window to the left"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::FocusDirection {
            direction: NavigateDirection::Left,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Focus window below (C-j)
#[derive(Debug, Clone)]
pub struct WindowFocusDownCommand;

impl CommandTrait for WindowFocusDownCommand {
    fn name(&self) -> &'static str {
        "window_focus_down"
    }

    fn description(&self) -> &'static str {
        "Focus window below"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::FocusDirection {
            direction: NavigateDirection::Down,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Focus window above (C-k)
#[derive(Debug, Clone)]
pub struct WindowFocusUpCommand;

impl CommandTrait for WindowFocusUpCommand {
    fn name(&self) -> &'static str {
        "window_focus_up"
    }

    fn description(&self) -> &'static str {
        "Focus window above"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::FocusDirection {
            direction: NavigateDirection::Up,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Focus window to the right (C-l)
#[derive(Debug, Clone)]
pub struct WindowFocusRightCommand;

impl CommandTrait for WindowFocusRightCommand {
    fn name(&self) -> &'static str {
        "window_focus_right"
    }

    fn description(&self) -> &'static str {
        "Focus window to the right"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::FocusDirection {
            direction: NavigateDirection::Right,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Move window to the left (C-S-H)
#[derive(Debug, Clone)]
pub struct WindowMoveLeftCommand;

impl CommandTrait for WindowMoveLeftCommand {
    fn name(&self) -> &'static str {
        "window_move_left"
    }

    fn description(&self) -> &'static str {
        "Move window to the left"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::MoveDirection {
            direction: NavigateDirection::Left,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Move window down (C-S-J)
#[derive(Debug, Clone)]
pub struct WindowMoveDownCommand;

impl CommandTrait for WindowMoveDownCommand {
    fn name(&self) -> &'static str {
        "window_move_down"
    }

    fn description(&self) -> &'static str {
        "Move window down"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::MoveDirection {
            direction: NavigateDirection::Down,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Move window up (C-S-K)
#[derive(Debug, Clone)]
pub struct WindowMoveUpCommand;

impl CommandTrait for WindowMoveUpCommand {
    fn name(&self) -> &'static str {
        "window_move_up"
    }

    fn description(&self) -> &'static str {
        "Move window up"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::MoveDirection {
            direction: NavigateDirection::Up,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Move window to the right (C-S-L)
#[derive(Debug, Clone)]
pub struct WindowMoveRightCommand;

impl CommandTrait for WindowMoveRightCommand {
    fn name(&self) -> &'static str {
        "window_move_right"
    }

    fn description(&self) -> &'static str {
        "Move window to the right"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::MoveDirection {
            direction: NavigateDirection::Right,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Split window horizontally (:sp)
#[derive(Debug, Clone)]
pub struct WindowSplitHorizontalCommand;

impl CommandTrait for WindowSplitHorizontalCommand {
    fn name(&self) -> &'static str {
        "window_split_horizontal"
    }

    fn description(&self) -> &'static str {
        "Split window horizontally"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::SplitHorizontal {
            filename: None,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Split window vertically (:vs)
#[derive(Debug, Clone)]
pub struct WindowSplitVerticalCommand;

impl CommandTrait for WindowSplitVerticalCommand {
    fn name(&self) -> &'static str {
        "window_split_vertical"
    }

    fn description(&self) -> &'static str {
        "Split window vertically"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::SplitVertical {
            filename: None,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Close current window (:close)
#[derive(Debug, Clone)]
pub struct WindowCloseCommand;

impl CommandTrait for WindowCloseCommand {
    fn name(&self) -> &'static str {
        "window_close"
    }

    fn description(&self) -> &'static str {
        "Close current window"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::Close { force: false }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Close all other windows (:only)
#[derive(Debug, Clone)]
pub struct WindowOnlyCommand;

impl CommandTrait for WindowOnlyCommand {
    fn name(&self) -> &'static str {
        "window_only"
    }

    fn description(&self) -> &'static str {
        "Close all other windows"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::CloseOthers))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Equalize window sizes
#[derive(Debug, Clone)]
pub struct WindowEqualizeCommand;

impl CommandTrait for WindowEqualizeCommand {
    fn name(&self) -> &'static str {
        "window_equalize"
    }

    fn description(&self) -> &'static str {
        "Equalize window sizes"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Window(WindowAction::Equalize))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
