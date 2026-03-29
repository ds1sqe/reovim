//! Operator command wrappers for Epic #415.
//!
//! These `CommandHandler` implementations wrap the `Operator` trait,
//! reading range information from `CommandContext` and delegating to
//! the appropriate operator.
//!
//! # Architecture
//!
//! When the runner receives `PopResult::ExecuteCommand`, it executes
//! the command with the provided arguments. This module provides the
//! command handlers that are registered with IDs like `vim:delete`
//! and perform the actual operation.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{BufferApi, SessionRuntime, TransitionContext, api::ModeApi},
    reovim_kernel::api::v1::{CommandId, Position},
};

use {
    super::{
        DeleteOperator, LowercaseOperator, Operator, OperatorContext, Range, ToggleCaseOperator,
        UppercaseOperator, YankOperator,
    },
    crate::ids::MODULE,
};

// =============================================================================
// Delete Command (vim:delete)
// =============================================================================

/// Delete operator command - wraps `DeleteOperator` for command execution.
///
/// This command is executed by the runner when `PopResult::ExecuteCommand`
/// is received with command `vim:delete`. It reads the range from the
/// command context and delegates to `DeleteOperator`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteCommand;

impl Command for DeleteCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE, "delete")
    }

    fn description(&self) -> &'static str {
        "Delete text in range"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for DeleteCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_operator(&DeleteOperator, runtime, args)
    }
}

// =============================================================================
// Yank Command (vim:yank)
// =============================================================================

/// Yank operator command - wraps `YankOperator` for command execution.
///
/// This command is executed by the runner when `PopResult::ExecuteCommand`
/// is received with command `vim:yank`. It reads the range from the
/// command context and delegates to `YankOperator`.
#[derive(Debug, Clone, Copy, Default)]
pub struct YankCommand;

impl Command for YankCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE, "yank")
    }

    fn description(&self) -> &'static str {
        "Yank text in range to register"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for YankCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // In Vim, after yank the cursor should be restored to the start of the yanked range.
        // Get the range start BEFORE executing the yank, so we can restore cursor after.
        let (start_line, start_col) = args.range_start().unwrap_or((0, 0));
        let restore_pos = Position::new(start_line, start_col);

        let cur_pos = runtime
            .windows()
            .active()
            .map(|w| Position::new(w.cursor.line, w.cursor.column));
        tracing::debug!(
            range_start = ?restore_pos,
            current_pos = ?cur_pos,
            "yank command: before execute"
        );

        let result = execute_operator(&YankOperator, runtime, args);

        // Restore cursor to start of yanked range (Vim behavior)
        if matches!(result, CommandResult::Success) && args.buffer_id().is_some() {
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = restore_pos.into();
            }

            let cur_pos = runtime
                .windows()
                .active()
                .map(|w| Position::new(w.cursor.line, w.cursor.column));
            tracing::debug!(
                restored_to = ?restore_pos,
                current_pos = ?cur_pos,
                "yank command: cursor restored"
            );

            // #657: Record yank flash state for client-side animation
            if let Some(buffer_id) = args.buffer_id() {
                use reovim_driver_session::api::ExtensionApi;
                let (end_line, end_col) = args.range_end().unwrap_or((0, 0));
                let state = runtime.ext_mut::<super::yank_flash::YankFlashState>();
                state.record(
                    buffer_id,
                    start_line,
                    start_col,
                    end_line,
                    end_col,
                    args.is_linewise(),
                );
            }
        }

        result
    }
}

// =============================================================================
// Change Command (vim:change)
// =============================================================================

/// Change operator command - wraps `ChangeOperator` for command execution.
///
/// This command is executed by the runner when `PopResult::ExecuteCommand`
/// is received with command `vim:change`. It reads the range from the
/// command context, deletes the text, and transitions to insert mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangeCommand;

impl Command for ChangeCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE, "change")
    }

    fn description(&self) -> &'static str {
        "Change text in range (delete and enter insert)"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for ChangeCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        use {
            super::ChangeOperator,
            crate::modes::VimMode,
            reovim_driver_undo::{UndoKey, UndoProviderRegistry},
        };

        // Start undo batching BEFORE the delete - both the delete and subsequent
        // insert edits should be grouped as a single undo entry
        if let Some(buffer_id) = args.buffer_id()
            && let Some(window) = runtime.windows().active()
            && let Some(undo_registry) = runtime.kernel().services.get::<UndoProviderRegistry>()
            && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
        {
            let pos = Position::new(window.cursor.line, window.cursor.column);
            undo_provider.begin_batch(buffer_id, pos);
        }

        // Execute the change operator (delete text)
        let result = execute_operator(&ChangeOperator, runtime, args);

        // If successful, enter insert mode (change = delete + insert)
        if matches!(result, CommandResult::Success) {
            runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
        }

        result
    }
}

// =============================================================================
// Lowercase Command (vim:lowercase)
// =============================================================================

/// Lowercase operator command - wraps `LowercaseOperator` for command execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct LowercaseCommand;

impl Command for LowercaseCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE, "lowercase")
    }

    fn description(&self) -> &'static str {
        "Lowercase text in range"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for LowercaseCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_operator(&LowercaseOperator, runtime, args)
    }
}

// =============================================================================
// Uppercase Command (vim:uppercase)
// =============================================================================

/// Uppercase operator command - wraps `UppercaseOperator` for command execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct UppercaseCommand;

impl Command for UppercaseCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE, "uppercase")
    }

    fn description(&self) -> &'static str {
        "Uppercase text in range"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for UppercaseCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_operator(&UppercaseOperator, runtime, args)
    }
}

// =============================================================================
// Toggle Case Command (vim:toggle-case-op)
// =============================================================================

/// Toggle case operator command - wraps `ToggleCaseOperator` for command execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleCaseCommand;

impl Command for ToggleCaseCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE, "toggle-case-op")
    }

    fn description(&self) -> &'static str {
        "Toggle case of text in range"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for ToggleCaseCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_operator(&ToggleCaseOperator, runtime, args)
    }
}

// =============================================================================
// Helper Function
// =============================================================================

/// Execute an operator with range information from `CommandContext`.
///
/// This function bridges the `CommandHandler` interface to the `Operator` trait:
/// 1. Extracts `range_start`, `range_end`, linewise from args
/// 2. Builds an `OperatorContext` with kernel access
/// 3. Calls `operator.execute()`
/// 4. Updates cursor in window for text-modifying operators
#[cfg_attr(coverage_nightly, coverage(off))]
fn execute_operator(
    operator: &dyn Operator,
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
) -> CommandResult {
    // Get buffer ID
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Get range from context (Epic #415 Phase 5)
    let (start_line, start_col) = args.range_start().unwrap_or((0, 0));
    let (end_line, end_col) = args.range_end().unwrap_or((0, 0));
    let linewise = args.is_linewise();

    let start = Position::new(start_line, start_col);
    let end = Position::new(end_line, end_col);

    // Build range
    let range = if linewise {
        Range::linewise(start, end)
    } else {
        Range::new(start, end)
    };

    // Get count and register (#515: convert Option<char> → Register)
    let count = args.count().unwrap_or(1);
    let register = super::registers::option_char_to_register(args.register());

    // Get cursor position from window (for undo tracking)
    let cursor_position = runtime
        .windows()
        .active()
        .map_or_else(Position::origin, |w| Position::new(w.cursor.line, w.cursor.column));

    // Build operator context using runtime's kernel and per-client registers (#515)
    // Use split-borrow helper to avoid conflicting borrows on runtime
    let (kernel, registers, clipboard_history) = runtime.kernel_and_registers();
    let mut op_ctx = OperatorContext {
        kernel,
        registers,
        clipboard_history,
        buffer_id,
        register,
        count,
        cursor_position,
        cursor_after: None,
    };

    // Execute operator
    match operator.execute(&mut op_ctx, range) {
        Ok(()) => {
            // Copy cursor_after before releasing op_ctx borrows (#552).
            // op_ctx holds split-borrows from runtime.kernel_and_registers();
            // copying the Copy field lets NLL release those borrows so we can
            // call runtime.buffer_line_count() and runtime.windows_mut() below.
            let cursor_after = op_ctx.cursor_after;

            // Update cursor for text-modifying operators (#471, #552)
            if operator.is_text_modifying() {
                // Use operator's computed cursor if available,
                // otherwise clamp range start to valid bounds (#552)
                let new_cursor = cursor_after.unwrap_or_else(|| {
                    let line_count = runtime.buffer_line_count(buffer_id).unwrap_or(1);
                    let clamped_line = start.line.min(line_count.saturating_sub(1));
                    let line_len = runtime
                        .buffer_line_len(buffer_id, clamped_line)
                        .unwrap_or(0);
                    let clamped_col = if line_len == 0 {
                        0
                    } else {
                        start.column.min(line_len.saturating_sub(1))
                    };
                    Position::new(clamped_line, clamped_col)
                });

                if let Some(window) = runtime.windows_mut().active_mut() {
                    window.cursor = new_cursor.into();
                    tracing::debug!(
                        ?new_cursor,
                        operator = operator.id(),
                        "Updated cursor after text-modifying operator"
                    );
                }
            }

            // Record buffer modification for notification pipeline.
            // The operator writes directly to the kernel buffer via OperatorContext,
            // bypassing SessionRuntime::delete_range() which normally calls
            // record_buffer_modified(). We must record it explicitly so that
            // StateChanges propagates to TUI for display refresh.
            if operator.is_text_modifying() {
                runtime.record_buffer_modified(buffer_id);
            }

            CommandResult::Success
        }
        Err(e) => CommandResult::error(&e.to_string()),
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all operator command handlers.
#[must_use]
pub fn operator_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(DeleteCommand),
        Box::new(YankCommand),
        Box::new(ChangeCommand),
        Box::new(LowercaseCommand),
        Box::new(UppercaseCommand),
        Box::new(ToggleCaseCommand),
    ]
}

// =============================================================================
// Tests
// =============================================================================
