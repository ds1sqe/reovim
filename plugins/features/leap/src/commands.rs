//! Leap command implementations
//!
//! These commands were previously in lib/core/src/command/builtin/leap.rs
//! and are now part of the leap plugin.

use std::any::Any;

use reovim_core::command::{CommandResult, CommandTrait, ExecutionContext, id::CommandId};

use super::{handlers::leap_start_result, state::LeapDirection};

/// Command ID for leap forward
pub const LEAP_FORWARD: CommandId = CommandId::new("leap_forward");
/// Command ID for leap backward
pub const LEAP_BACKWARD: CommandId = CommandId::new("leap_backward");
/// Command ID for leap cancel
pub const LEAP_CANCEL: CommandId = CommandId::new("leap_cancel");

/// Leap forward command - starts leap mode searching forward
#[derive(Debug, Clone)]
pub struct LeapForwardCommand;

impl CommandTrait for LeapForwardCommand {
    fn name(&self) -> &'static str {
        "leap_forward"
    }

    fn description(&self) -> &'static str {
        "Start leap mode searching forward"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        leap_start_result(LeapDirection::Forward, None, None)
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Leap backward command - starts leap mode searching backward
#[derive(Debug, Clone)]
pub struct LeapBackwardCommand;

impl CommandTrait for LeapBackwardCommand {
    fn name(&self) -> &'static str {
        "leap_backward"
    }

    fn description(&self) -> &'static str {
        "Start leap mode searching backward"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        leap_start_result(LeapDirection::Backward, None, None)
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Leap cancel command - exits leap mode without jumping
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
        // This command is handled by the runtime's leap input handler
        // which checks for Escape and other cancel keys
        CommandResult::Success
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
