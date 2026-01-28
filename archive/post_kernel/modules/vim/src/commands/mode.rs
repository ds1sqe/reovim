//! Mode switching commands.
//!
//! Provides commands for switching between Vim modes:
//! - Enter insert mode (i, a)
//! - Exit to normal mode (Escape)
//! - Enter window mode (Ctrl-W)
//! - Enter/exit command-line mode
//!
//! # Epic #372 - Mode Ownership
//!
//! These commands use `VimMode::*_ID` constants directly, which is why they
//! belong in the vim module rather than the generic editor module.
//!
//! # SessionApi Migration (Epic #394)
//!
//! These commands use the `ModeApi` trait to switch modes, which updates the
//! session's mode stack directly. Changes are synced back to `AppState` after
//! command execution.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_search::Direction,
    reovim_driver_session::{
        BufferApi, CmdlinePrompt, CmdlineState, SessionRuntime, TransitionContext,
        api::{ExtensionApi, ModeApi, SearchState},
    },
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::{ids, modes::VimMode};

/// Start undo batching for insert mode.
///
/// All edits until `end_insert_batch` are grouped as a single undo entry.
fn begin_insert_batch(runtime: &SessionRuntime<'_>, buffer_id: reovim_kernel::api::v1::BufferId) {
    if let Some(pos) = runtime.buffer_position(buffer_id)
        && let Some(undo_registry) = runtime.kernel().services.get::<UndoProviderRegistry>()
        && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
    {
        undo_provider.begin_batch(buffer_id, pos);
    }
}

/// End undo batching for insert mode.
///
/// Commits all accumulated edits as a single undo entry.
fn end_insert_batch(runtime: &SessionRuntime<'_>, buffer_id: reovim_kernel::api::v1::BufferId) {
    if let Some(pos) = runtime.buffer_position(buffer_id)
        && let Some(undo_registry) = runtime.kernel().services.get::<UndoProviderRegistry>()
        && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
    {
        undo_provider.end_batch(buffer_id, pos);
    }
}

/// Enter insert mode (before cursor).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertMode;

impl Command for EnterInsertMode {
    fn id(&self) -> CommandId {
        ids::ENTER_INSERT
    }

    fn description(&self) -> &'static str {
        "Enter insert mode"
    }
}

impl CommandHandler for EnterInsertMode {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Start undo batching for insert mode
        if let Some(buffer_id) = args.buffer_id() {
            begin_insert_batch(runtime, buffer_id);
        }
        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Enter insert mode after cursor (a).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertModeAppend;

impl Command for EnterInsertModeAppend {
    fn id(&self) -> CommandId {
        ids::ENTER_INSERT_AFTER
    }

    fn description(&self) -> &'static str {
        "Enter insert mode after cursor (append)"
    }
}

impl CommandHandler for EnterInsertModeAppend {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Move cursor right first, then enter insert mode
        if let Some(buffer_id) = args.buffer_id() {
            if let Some(pos) = runtime.buffer_position(buffer_id)
                && let Some(line_len) = runtime.buffer_line_len(buffer_id, pos.line)
            {
                // Move right only if not at end of line
                if pos.column < line_len {
                    runtime.set_buffer_position(buffer_id, Position::new(pos.line, pos.column + 1));
                }
            }
            // Start undo batching for insert mode
            begin_insert_batch(runtime, buffer_id);
        }

        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Exit to normal mode (Escape from insert mode).
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitToNormal;

impl Command for ExitToNormal {
    fn id(&self) -> CommandId {
        ids::EXIT_INSERT
    }

    fn description(&self) -> &'static str {
        "Exit insert mode and return to normal mode"
    }
}

impl CommandHandler for ExitToNormal {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        if let Some(buffer_id) = args.buffer_id() {
            // End undo batching - commits all insert edits as one undo entry
            end_insert_batch(runtime, buffer_id);

            // Move cursor left one position when exiting insert mode (Vim behavior)
            if let Some(pos) = runtime.buffer_position(buffer_id)
                && pos.column > 0
            {
                runtime.set_buffer_position(buffer_id, Position::new(pos.line, pos.column - 1));
            }
        }

        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Enter window management mode (Ctrl-W in normal mode).
///
/// This pushes "window" mode onto the mode stack. In window mode,
/// subsequent keys (h/j/k/l for navigation, s/v for splits, etc.)
/// are handled by the layout module's keybindings.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterWindowMode;

impl Command for EnterWindowMode {
    fn id(&self) -> CommandId {
        ids::ENTER_WINDOW_MODE
    }

    fn description(&self) -> &'static str {
        "Enter window management mode"
    }
}

impl CommandHandler for EnterWindowMode {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Window mode is pushed onto the stack (can be exited to return to normal)
        runtime.push_mode(VimMode::WINDOW_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Cancel and return to normal mode (no cursor adjustment).
///
/// Used by operator modes (delete, yank, change) when the user presses Escape.
/// Unlike `ExitToNormal`, this does not adjust the cursor position.
#[derive(Debug, Clone, Copy, Default)]
pub struct CancelToNormal;

impl Command for CancelToNormal {
    fn id(&self) -> CommandId {
        ids::CANCEL_TO_NORMAL
    }

    fn description(&self) -> &'static str {
        "Cancel and return to normal mode"
    }
}

impl CommandHandler for CancelToNormal {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Enter command-line mode (`:` in normal mode).
///
/// This pushes "commandline" mode onto the mode stack. In command-line mode,
/// the user can type Ex commands like `:w`, `:q`, `:set`, etc.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterCommandLineMode;

impl Command for EnterCommandLineMode {
    fn id(&self) -> CommandId {
        ids::ENTER_COMMANDLINE
    }

    fn description(&self) -> &'static str {
        "Enter command-line mode"
    }
}

impl CommandHandler for EnterCommandLineMode {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Activate cmdline with command prompt
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        runtime.set_mode(VimMode::COMMANDLINE_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Exit command-line mode (Escape or Enter).
///
/// Returns to normal mode from command-line mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitCommandLineMode;

impl Command for ExitCommandLineMode {
    fn id(&self) -> CommandId {
        ids::EXIT_COMMANDLINE
    }

    fn description(&self) -> &'static str {
        "Exit command-line mode and return to normal mode"
    }
}

impl CommandHandler for ExitCommandLineMode {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Check if this is a search (pending_search set)
        let pending = {
            let search_state = runtime.ext_mut::<SearchState>();
            search_state.take_pending_search()
        };

        if let Some(direction) = pending {
            // Store the direction for the runner to execute search
            runtime.ext_mut::<SearchState>().last_direction = direction;
        }

        // Deactivate cmdline
        runtime.ext_mut::<CmdlineState>().exit();
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Cancel command-line mode without executing (Escape).
///
/// Unlike `ExitCommandLineMode`, this clears the pending search and cmdline
/// input without executing any action.
#[derive(Debug, Clone, Copy, Default)]
pub struct CancelCommandLineMode;

impl Command for CancelCommandLineMode {
    fn id(&self) -> CommandId {
        ids::CANCEL_COMMANDLINE
    }

    fn description(&self) -> &'static str {
        "Cancel command-line mode without executing"
    }
}

impl CommandHandler for CancelCommandLineMode {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Clear any pending search - we're canceling, not executing
        runtime.ext_mut::<SearchState>().clear_pending_search();

        // Cancel cmdline (signals to runner: don't execute)
        runtime.ext_mut::<CmdlineState>().cancel();
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());
        CommandResult::Success
    }
}

// =============================================================================
// Search Mode Entry Commands (#435)
// =============================================================================

/// Enter search forward mode (`/`).
///
/// Sets pending search direction to Forward and enters command-line mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterSearchForward;

impl Command for EnterSearchForward {
    fn id(&self) -> CommandId {
        ids::ENTER_SEARCH_FORWARD
    }

    fn description(&self) -> &'static str {
        "Enter search forward mode (/)"
    }
}

impl CommandHandler for EnterSearchForward {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        eprintln!("[DEBUG] EnterSearchForward command executed");
        // Set pending search direction
        runtime
            .ext_mut::<SearchState>()
            .start_pending_search(Direction::Forward);
        // Activate cmdline with search forward prompt
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchForward);
        // Enter command-line mode
        runtime.set_mode(VimMode::COMMANDLINE_ID, TransitionContext::new());
        eprintln!("[DEBUG] Entered commandline mode with SearchForward prompt");
        CommandResult::Success
    }
}

/// Enter search backward mode (`?`).
///
/// Sets pending search direction to Backward and enters command-line mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterSearchBackward;

impl Command for EnterSearchBackward {
    fn id(&self) -> CommandId {
        ids::ENTER_SEARCH_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Enter search backward mode (?)"
    }
}

impl CommandHandler for EnterSearchBackward {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Set pending search direction
        runtime
            .ext_mut::<SearchState>()
            .start_pending_search(Direction::Backward);
        // Activate cmdline with search backward prompt
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchBackward);
        // Enter command-line mode
        runtime.set_mode(VimMode::COMMANDLINE_ID, TransitionContext::new());
        CommandResult::Success
    }
}
