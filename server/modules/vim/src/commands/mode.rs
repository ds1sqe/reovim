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
//! # `SessionApi` Migration (Epic #394)
//!
//! These commands use the `ModeApi` trait to switch modes, which updates the
//! session's mode stack directly. Changes are synced back to `AppState` after
//! command execution.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_search::Direction,
    reovim_driver_session::{
        BufferApi, CmdlinePrompt, CmdlineState, SessionRuntime, TransitionContext,
        api::{ChangeTracker, ExtensionApi, ModeApi, SearchState},
    },
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{CommandId, Position},
};

/// Helper to get cursor position from the active window.
fn get_cursor_position(runtime: &SessionRuntime<'_>) -> Option<Position> {
    let window = runtime.windows().active()?;
    Some(Position::new(window.cursor.line, window.cursor.column))
}

/// Helper to set cursor position on the active window.
fn set_cursor_position(runtime: &mut SessionRuntime<'_>, pos: Position) {
    if let Some(window) = runtime.windows_mut().active_mut() {
        window.cursor = pos.into();
    }
}

use crate::{ids, modes::VimMode};

/// Start undo batching for insert mode.
///
/// All edits until `end_insert_batch` are grouped as a single undo entry.
fn begin_insert_batch(runtime: &SessionRuntime<'_>, buffer_id: reovim_kernel::api::v1::BufferId) {
    if let Some(pos) = get_cursor_position(runtime)
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
    if let Some(pos) = get_cursor_position(runtime)
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
        // Clear insert buffer for dot repeat tracking (Epic #465)
        if let Some(vim) = runtime.ext_mut::<crate::VimSessionState>().into() {
            vim.insert_buffer.clear();
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
            if let Some(pos) = get_cursor_position(runtime)
                && let Some(line_len) = runtime.buffer_line_len(buffer_id, pos.line)
            {
                // Move right only if not at end of line
                if pos.column < line_len {
                    set_cursor_position(runtime, Position::new(pos.line, pos.column + 1));
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
            if let Some(pos) = get_cursor_position(runtime)
                && pos.column > 0
            {
                set_cursor_position(runtime, Position::new(pos.line, pos.column - 1));
            }
        }

        // Record insert mode text for dot repeat (Epic #465)
        // Only record if there was actual text inserted
        if let Some(vim) = runtime.ext_mut::<crate::VimSessionState>().into() {
            let insert_text = std::mem::take(&mut vim.insert_buffer);
            if !insert_text.is_empty() {
                vim.last_change = Some(crate::session_state::LastChange {
                    change_type: crate::session_state::ChangeType::Insert { text: insert_text },
                    count: None,
                    register: None,
                });
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
/// If a search was pending, executes the search with the entered pattern.
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Get the prompt type to determine how to handle the input
        let prompt = runtime.ext_mut::<CmdlineState>().prompt();

        // Get the cmdline input
        let cmdline = runtime.ext_mut::<CmdlineState>().take_cmdline_input();

        match prompt {
            CmdlinePrompt::SearchForward | CmdlinePrompt::SearchBackward => {
                // Handle search
                let pending = {
                    let search_state = runtime.ext_mut::<SearchState>();
                    search_state.take_pending_search()
                };

                if let Some(direction) = pending
                    && !cmdline.is_empty()
                {
                    // Store pattern and direction for n/N repeat
                    runtime
                        .ext_mut::<SearchState>()
                        .set(cmdline.clone(), direction);

                    // Execute the search
                    execute_search(runtime, args, &cmdline, direction);
                }
            }
            CmdlinePrompt::Command => {
                // Handle ex-command (e.g., :w, :q, :e)
                if !cmdline.is_empty() {
                    execute_ex_command(runtime, args, &cmdline);
                }
            }
        }

        // Deactivate cmdline and clear input
        runtime.ext_mut::<CmdlineState>().exit();
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Execute an ex-command via the `ExCommandRegistry` service.
///
/// Looks up the registry in `ServiceRegistry` and dispatches the command.
/// If the registry is not registered (no ex-commands loaded), logs a warning.
fn execute_ex_command(runtime: &SessionRuntime<'_>, args: &CommandContext, cmdline: &str) {
    use reovim_driver_command::{
        ExCommandDispatcher, ExCommandRegistry, ExCommandResult, ExDispatchContext,
    };

    // Get registry from ServiceRegistry
    let Some(registry) = runtime.kernel().services.get::<ExCommandRegistry>() else {
        tracing::warn!("ExCommandRegistry not registered - ex-commands not available");
        return;
    };

    // Build dispatch context
    let ctx = ExDispatchContext::new(args.buffer_id(), None);

    // Dispatch the command
    match registry.dispatch(cmdline, runtime.kernel(), &ctx) {
        ExCommandResult::Success => {
            tracing::debug!(cmdline, "Ex-command executed successfully");
        }
        ExCommandResult::NotFound(name) => {
            tracing::warn!(name, "Unknown ex-command");
            // TODO: Show error message to user via status line
        }
        ExCommandResult::Error(msg) => {
            tracing::warn!(cmdline, msg, "Ex-command failed");
            // TODO: Show error message to user via status line
        }
    }
}

/// Execute a search and move cursor to the first match.
fn execute_search(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    pattern: &str,
    direction: Direction,
) {
    use reovim_driver_search::{SearchKey, SearchProviderRegistry};

    let Some(buffer_id) = args.buffer_id() else {
        return;
    };

    let Some(cursor) = get_cursor_position(runtime) else {
        return;
    };

    let Some(search_registry) = runtime.kernel().services.get::<SearchProviderRegistry>() else {
        tracing::warn!("Search provider not available");
        return;
    };

    let Some(search_provider) = search_registry.get(&SearchKey::Regex) else {
        tracing::warn!("Regex search engine not registered");
        return;
    };

    // Search for pattern
    let search_result = runtime.with_buffer_read(buffer_id, |buffer| {
        search_provider.find_next(buffer, cursor, pattern, direction, true)
    });

    match search_result {
        Some(Ok(Some(m))) => {
            // Move cursor to match start
            set_cursor_position(runtime, m.start);
            runtime.record_cursor_move(buffer_id);
            tracing::debug!(
                pattern,
                ?direction,
                match_start = ?m.start,
                "Search found match"
            );
        }
        Some(Ok(None)) => {
            tracing::debug!(pattern, "Search: no match found");
        }
        Some(Err(e)) => {
            tracing::debug!(pattern, ?e, "Search: invalid pattern");
        }
        None => {
            tracing::warn!("Buffer not found for search");
        }
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
