//! Buffer navigation ex-commands: `:bnext`, `:bprevious`, `:bd`.

use {
    reovim_content_codec_text::CodecSessionState,
    reovim_driver_command::{Command, CommandHandler, CommandResult},
    reovim_driver_text_session::{BufferApi, ExtensionApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, ModuleId},
    reovim_subsys_command_types::CommandContext,
};

const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

// ============================================================================
// BnextCommand — :bnext / :bn
// ============================================================================

/// Switch to the next buffer in the buffer list.
#[derive(Debug, Clone, Copy)]
pub struct BnextCommand;

impl Command for BnextCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "bnext")
    }

    fn description(&self) -> &'static str {
        "Switch to next buffer"
    }

    fn names(&self) -> &[&'static str] {
        &["bn", "bnext"]
    }
}

impl CommandHandler for BnextCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        cycle_buffer(runtime, &Direction::Next)
    }
}

// ============================================================================
// BpreviousCommand — :bprevious / :bp / :bN
// ============================================================================

/// Switch to the previous buffer in the buffer list.
#[derive(Debug, Clone, Copy)]
pub struct BpreviousCommand;

impl Command for BpreviousCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "bprevious")
    }

    fn description(&self) -> &'static str {
        "Switch to previous buffer"
    }

    fn names(&self) -> &[&'static str] {
        &["bp", "bprevious", "bN", "bNext"]
    }
}

impl CommandHandler for BpreviousCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        cycle_buffer(runtime, &Direction::Prev)
    }
}

// ============================================================================
// BdeleteCommand — :bd / :bdelete
// ============================================================================

/// Delete (close) the active buffer.
#[derive(Debug, Clone, Copy)]
pub struct BdeleteCommand;

impl Command for BdeleteCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "bdelete")
    }

    fn description(&self) -> &'static str {
        "Delete (close) active buffer"
    }

    fn names(&self) -> &[&'static str] {
        &["bd", "bdelete"]
    }
}

impl CommandHandler for BdeleteCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(buf_id) = runtime.active_buffer() else {
            return CommandResult::Success;
        };

        // Route through `BufferApi::delete_buffer` so session-layer
        // registries (`TextBufferRegistry`) are released and
        // last-buffer protection fires.
        if let Err(e) = runtime.delete_buffer(buf_id) {
            return CommandResult::Error(format!("execution failed: {e}"));
        }

        // #740 Phase 0 (B3): the codec session state lives in the
        // session-wide shared extensions, which `reovim-driver-session`
        // cannot name without reversing the dependency graph
        // (codec → session). Clean up here in the commands layer where the
        // codec import already exists. Phase 3 migrates this responsibility
        // into the runtime via the InodeTable extension.
        // Use `.map()` instead of `if let Some` to avoid an untestable
        // MC/DC branch on the pattern match.
        let _ = runtime
            .shared_ext_mut::<CodecSessionState>()
            .map(|codec_state| codec_state.remove(buf_id));

        CommandResult::Success
    }
}

// ============================================================================
// QuitAllCommand — :qa / :qall
// ============================================================================

/// Quit all clients (shut down the server).
#[derive(Debug, Clone, Copy)]
pub struct QuitAllCommand;

impl Command for QuitAllCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "qall")
    }

    fn description(&self) -> &'static str {
        "Quit all — shut down the server"
    }

    fn names(&self) -> &[&'static str] {
        &["qa", "qall"]
    }
}

impl CommandHandler for QuitAllCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.signal(reovim_driver_command::RuntimeSignal::Quit);
        CommandResult::Success
    }
}

// ============================================================================
// Shared buffer cycling
// ============================================================================

enum Direction {
    Next,
    Prev,
}

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

/// Create all buffer navigation command handlers.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(BnextCommand),
        Box::new(BpreviousCommand),
        Box::new(BdeleteCommand),
        Box::new(QuitAllCommand),
    ]
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
