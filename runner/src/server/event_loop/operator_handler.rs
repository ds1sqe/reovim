//! Operator execution handling.
//!
//! Handles execution of pending operators (d, y, c) when a motion returns a range.

use {
    super::EventLoop,
    crate::server::AppState,
    reovim_kernel::api::v1::{ModeId, ModuleId, Position},
    reovim_module_operators::{
        ChangeOperator, DeleteOperator, Operator, OperatorContext, Range, YankOperator,
    },
};

impl<F: reovim_driver_input::InputFallbackHandler<AppState>> EventLoop<F> {
    /// Execute the pending operator with the given range.
    ///
    /// Called when a motion command returns `CommandResult::OperatorRange`.
    /// This takes the pending operator state, executes the operator on the range,
    /// handles undo recording, and returns to normal mode.
    pub(super) fn execute_pending_operator(
        &mut self,
        start: Position,
        end: Position,
        is_linewise: bool,
    ) {
        // Take the pending operator - this clears the state
        let Some(pending) = self.app.take_pending_operator() else {
            tracing::warn!("OperatorRange received but no pending operator");
            return;
        };

        // Get the active buffer
        let Some(buffer_id) = self.app.active_buffer else {
            tracing::warn!("No active buffer for operator execution");
            return;
        };

        // Create the range (normalize it so start <= end)
        let range = if is_linewise {
            Range::linewise(start, end).normalized()
        } else {
            Range::new(start, end).normalized()
        };

        // Skip empty ranges (no-op)
        if range.is_empty() && !is_linewise {
            tracing::debug!("Empty range, skipping operator");
            self.pop_operator_pending_mode();
            return;
        }

        // Get the operator
        let operator: Box<dyn Operator> = match pending.operator_id {
            "delete" => Box::new(DeleteOperator),
            "yank" => Box::new(YankOperator),
            "change" => Box::new(ChangeOperator),
            // TODO: Add indent/dedent operators
            unknown => {
                tracing::warn!(operator = unknown, "Unknown operator");
                self.pop_operator_pending_mode();
                return;
            }
        };

        // Execute the operator
        // Note: we need to use an unsafe trick here because OperatorContext holds &KernelContext
        // but KernelContext is inside self.app. We work around this by creating the context
        // just for the execute call.
        let operator_result = {
            let mut op_ctx = OperatorContext {
                kernel: &self.app.kernel,
                buffer_id,
                register: pending.register,
                count: pending.count,
            };
            operator.execute(&mut op_ctx, range)
        };

        match operator_result {
            Ok(()) => {
                tracing::debug!(
                    operator = pending.operator_id,
                    ?range,
                    "Operator executed successfully"
                );

                // For change operator, enter insert mode
                if pending.operator_id == "change" {
                    let insert_mode = ModeId::new(ModuleId::new("editor"), "insert");
                    self.app.mode_stack.set(insert_mode);
                    tracing::debug!("Entered insert mode after change operator");
                } else {
                    // For other operators, just pop back to normal mode
                    self.pop_operator_pending_mode();
                }
            }
            Err(err) => {
                tracing::error!(error = %err, "Operator execution failed");
                self.last_error = Some(format!("Operator failed: {err}"));
                self.pop_operator_pending_mode();
            }
        }
    }

    /// Pop operator-pending mode and return to normal mode.
    fn pop_operator_pending_mode(&mut self) {
        // Pop should return to normal mode (the mode before operator-pending)
        self.app.mode_stack.pop();
        tracing::debug!(
            mode = %self.app.current_mode(),
            "Returned from operator-pending mode"
        );
    }
}
