//! Command handlers for the bufferline module.

use {
    reovim_driver_command::{Command, CommandHandler, CommandResult},
    reovim_driver_command_types::CommandContext,
    reovim_driver_session::{BufferApi, ExtensionApi, SessionRuntime},
    reovim_kernel::api::v1::CommandId,
};

use crate::{ids, state::BufferlineState};

// ============================================================================
// PinBuffer — toggle pin for active buffer
// ============================================================================

/// Toggle pin state for the active buffer.
#[derive(Debug, Clone, Copy, Default)]
pub struct PinBuffer;

impl Command for PinBuffer {
    fn id(&self) -> CommandId {
        ids::PIN_BUFFER
    }

    fn description(&self) -> &'static str {
        "Toggle pin for active buffer"
    }
}

impl CommandHandler for PinBuffer {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(buf_id) = runtime.active_buffer() else {
            return CommandResult::Success;
        };
        let id = buf_id.as_usize() as u64;

        let Some(state) = runtime.shared_ext_mut::<BufferlineState>() else {
            return CommandResult::Success;
        };

        if state.is_pinned(id) {
            state.unpin(id);
        } else {
            state.pin(id);
        }

        CommandResult::Success
    }
}

// ============================================================================
// UnpinBuffer — explicit unpin
// ============================================================================

/// Explicitly unpin the active buffer.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnpinBuffer;

impl Command for UnpinBuffer {
    fn id(&self) -> CommandId {
        ids::UNPIN_BUFFER
    }

    fn description(&self) -> &'static str {
        "Unpin active buffer"
    }
}

impl CommandHandler for UnpinBuffer {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(buf_id) = runtime.active_buffer() else {
            return CommandResult::Success;
        };
        let id = buf_id.as_usize() as u64;

        if let Some(state) = runtime.shared_ext_mut::<BufferlineState>() {
            state.unpin(id);
        }

        CommandResult::Success
    }
}

// ============================================================================
// CloseBuffer — close active buffer and remove from pins
// ============================================================================

/// Close the active buffer.
#[derive(Debug, Clone, Copy, Default)]
pub struct CloseBuffer;

impl Command for CloseBuffer {
    fn id(&self) -> CommandId {
        ids::CLOSE_BUFFER
    }

    fn description(&self) -> &'static str {
        "Close active buffer"
    }
}

impl CommandHandler for CloseBuffer {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(buf_id) = runtime.active_buffer() else {
            return CommandResult::Success;
        };
        let id = buf_id.as_usize() as u64;

        // Remove from pin list if pinned.
        if let Some(state) = runtime.shared_ext_mut::<BufferlineState>() {
            state.unpin(id);
        }

        // Delegate to kernel buffer close (deletes the buffer).
        let _ = runtime.kernel().buffers.unregister(buf_id);

        CommandResult::Success
    }
}

// ============================================================================
// NextBuffer — switch to next buffer in buffer list
// ============================================================================

/// Switch to the next buffer.
#[derive(Debug, Clone, Copy, Default)]
pub struct NextBuffer;

impl Command for NextBuffer {
    fn id(&self) -> CommandId {
        ids::NEXT_BUFFER
    }

    fn description(&self) -> &'static str {
        "Switch to next buffer"
    }
}

impl CommandHandler for NextBuffer {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        cycle_buffer(runtime, &Direction::Next)
    }
}

// ============================================================================
// PrevBuffer — switch to previous buffer in buffer list
// ============================================================================

/// Switch to the previous buffer.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrevBuffer;

impl Command for PrevBuffer {
    fn id(&self) -> CommandId {
        ids::PREV_BUFFER
    }

    fn description(&self) -> &'static str {
        "Switch to previous buffer"
    }
}

impl CommandHandler for PrevBuffer {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        cycle_buffer(runtime, &Direction::Prev)
    }
}

// ============================================================================
// Shared buffer cycling logic
// ============================================================================

enum Direction {
    Next,
    Prev,
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn cycle_buffer(runtime: &mut SessionRuntime<'_>, direction: &Direction) -> CommandResult {
    let Some(current) = runtime.active_buffer() else {
        return CommandResult::Success;
    };

    let mut buf_ids = runtime.kernel().buffers.list();
    if buf_ids.len() < 2 {
        return CommandResult::Success;
    }
    buf_ids.sort_unstable();

    let current_idx = buf_ids.iter().position(|&id| id == current).unwrap_or(0);

    let next_idx = match direction {
        Direction::Next => (current_idx + 1) % buf_ids.len(),
        Direction::Prev => {
            if current_idx == 0 {
                buf_ids.len() - 1
            } else {
                current_idx - 1
            }
        }
    };

    runtime.set_active_buffer(Some(buf_ids[next_idx]));
    CommandResult::Success
}

// ============================================================================
// Factory
// ============================================================================

/// Create all command handlers for the bufferline module.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(PinBuffer),
        Box::new(UnpinBuffer),
        Box::new(CloseBuffer),
        Box::new(NextBuffer),
        Box::new(PrevBuffer),
    ]
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
