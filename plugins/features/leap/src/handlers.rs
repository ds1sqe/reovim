//! Deferred action handlers for Leap
//!
//! These handlers implement the `DeferredActionHandler` trait, allowing
//! Leap actions to be executed through the new plugin architecture instead
//! of the hardcoded `DeferredAction::Leap` variant.

use reovim_core::{
    command::{CommandResult, DeferredActionHandler},
    modd::{ModeState, OperatorType, SubMode},
    runtime::{RuntimeContext, RuntimeContextExt},
    ui_component::ComponentId,
};

use super::{
    events::LeapStartEvent,
    state::{LeapDirection, LeapState},
};

/// Handler for starting leap mode
///
/// This replaces `DeferredAction::Leap(LeapAction::Start { ... })`.
/// It activates leap mode with the given direction and optional operator context.
#[derive(Debug, Clone)]
pub struct LeapStartHandler {
    /// Search direction
    pub direction: LeapDirection,
    /// Pending operator (if from operator-pending mode)
    pub operator: Option<OperatorType>,
    /// Operator count
    pub count: Option<usize>,
}

impl LeapStartHandler {
    /// Create a new leap start handler
    #[must_use]
    pub const fn new(
        direction: LeapDirection,
        operator: Option<OperatorType>,
        count: Option<usize>,
    ) -> Self {
        Self {
            direction,
            operator,
            count,
        }
    }

    /// Create a forward leap handler
    #[must_use]
    pub const fn forward() -> Self {
        Self::new(LeapDirection::Forward, None, None)
    }

    /// Create a backward leap handler
    #[must_use]
    pub const fn backward() -> Self {
        Self::new(LeapDirection::Backward, None, None)
    }

    /// Create a leap handler with operator context
    #[must_use]
    pub const fn with_operator(
        direction: LeapDirection,
        operator: OperatorType,
        count: Option<usize>,
    ) -> Self {
        Self::new(direction, Some(operator), count)
    }
}

impl DeferredActionHandler for LeapStartHandler {
    fn handle(&self, ctx: &mut dyn RuntimeContext) {
        // Update leap state in plugin registry
        ctx.with_state_mut::<LeapState, _, _>(|leap_state| {
            if let Some(op) = self.operator {
                leap_state.activate_with_operator(self.direction, op, self.count);
            } else {
                leap_state.activate(self.direction);
            }
        });

        // Set mode to leap sub-mode using generic Interactor pattern
        // Leap-specific data (direction, operator, count) is stored in LeapState
        let new_mode = ModeState::new().with_sub(SubMode::Interactor(ComponentId("leap")));
        ctx.set_mode(new_mode);

        // Emit start event through event bus
        ctx.emit(LeapStartEvent {
            direction: self.direction,
            operator: self.operator,
            count: self.count,
        });

        tracing::debug!(
            direction = ?self.direction,
            operator = ?self.operator,
            "LeapStartHandler: activated leap mode"
        );
    }

    fn name(&self) -> &'static str {
        "leap_start"
    }
}

/// Helper to create a `CommandResult::Deferred` for leap start
///
/// This is a convenience function for commands that need to start leap mode.
#[must_use]
pub fn leap_start_result(
    direction: LeapDirection,
    operator: Option<OperatorType>,
    count: Option<usize>,
) -> CommandResult {
    CommandResult::Deferred(Box::new(LeapStartHandler::new(direction, operator, count)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leap_start_handler_creation() {
        let handler = LeapStartHandler::forward();
        assert_eq!(handler.direction, LeapDirection::Forward);
        assert!(handler.operator.is_none());
        assert!(handler.count.is_none());

        let handler = LeapStartHandler::backward();
        assert_eq!(handler.direction, LeapDirection::Backward);

        let handler =
            LeapStartHandler::with_operator(LeapDirection::Forward, OperatorType::Delete, Some(3));
        assert_eq!(handler.operator, Some(OperatorType::Delete));
        assert_eq!(handler.count, Some(3));
    }

    #[test]
    fn test_handler_name() {
        let handler = LeapStartHandler::forward();
        assert_eq!(handler.name(), "leap_start");
    }
}
