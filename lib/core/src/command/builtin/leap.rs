//! Leap motion commands

use std::any::Any;

use crate::command::traits::{CommandResult, CommandTrait, DeferredAction, ExecutionContext, LeapAction};
use crate::leap::LeapDirection;
use crate::modd::ModeState;

/// Enter leap mode (forward search)
#[derive(Debug, Clone)]
pub struct LeapForwardCommand;

impl CommandTrait for LeapForwardCommand {
    fn name(&self) -> &'static str {
        "leap_forward"
    }

    fn description(&self) -> &'static str {
        "Leap forward to a two-character sequence"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Leap(LeapAction::Start {
            direction: LeapDirection::Forward,
            operator: None,
            count: None,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enter leap mode (backward search)
#[derive(Debug, Clone)]
pub struct LeapBackwardCommand;

impl CommandTrait for LeapBackwardCommand {
    fn name(&self) -> &'static str {
        "leap_backward"
    }

    fn description(&self) -> &'static str {
        "Leap backward to a two-character sequence"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Leap(LeapAction::Start {
            direction: LeapDirection::Backward,
            operator: None,
            count: None,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Cancel leap mode and return to normal
#[derive(Debug, Clone)]
pub struct LeapCancelCommand;

impl CommandTrait for LeapCancelCommand {
    fn name(&self) -> &'static str {
        "leap_cancel"
    }

    fn description(&self) -> &'static str {
        "Cancel leap mode"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::ModeChange(ModeState::normal())
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
