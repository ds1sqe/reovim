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
// Factory
// ============================================================================

/// Create all command handlers for the bufferline module.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(PinBuffer),
        Box::new(UnpinBuffer),
        Box::new(CloseBuffer),
    ]
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
