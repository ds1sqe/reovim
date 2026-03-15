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
        BufferApi, SessionRuntime, TransitionContext,
        api::{ChangeTracker, ExtensionApi, ModeApi, SearchState},
    },
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{CommandId, Position},
    reovim_module_cmdline::{CmdlineMessage, CmdlinePrompt, CmdlineState},
    std::sync::Arc,
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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

        // Record insert mode text for dot repeat (Epic #465, #577)
        if let Some(vim) = runtime.ext_mut::<crate::VimSessionState>().into() {
            let insert_text = std::mem::take(&mut vim.insert_buffer);
            if vim.recording_repeat {
                // #577: Operator-initiated insert (e.g., cwbar<Esc>) —
                // don't overwrite last_change (operator set it), just finish recording
                vim.finish_repeat_recording();
            } else if !insert_text.is_empty() {
                // Standalone insert (e.g., ihello<Esc>) — record as Insert change
                vim.last_change = Some(crate::session_state::LastChange {
                    change_type: crate::session_state::ChangeType::Insert { text: insert_text },
                    count: None,
                    register: None,
                    keys: Vec::new(),
                });
            }
        }

        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Enter replace mode (R in normal mode).
///
/// In replace mode, typed characters overwrite existing text at the cursor.
/// Backspace restores original characters. Escape returns to normal mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterReplaceMode;

impl Command for EnterReplaceMode {
    fn id(&self) -> CommandId {
        ids::ENTER_REPLACE_MODE
    }

    fn description(&self) -> &'static str {
        "Enter replace mode"
    }
}

impl CommandHandler for EnterReplaceMode {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Start undo batching for replace mode (like insert mode entry)
        if let Some(buffer_id) = args.buffer_id() {
            begin_insert_batch(runtime, buffer_id);
        }
        // Clear insert buffer and replace restore stack
        if let Some(vim) = runtime.ext_mut::<crate::VimSessionState>().into() {
            vim.insert_buffer.clear();
            vim.replace_restore_stack.clear();
        }
        runtime.set_mode(VimMode::REPLACE_ID, TransitionContext::new());
        CommandResult::Success
    }
}

/// Backspace in replace mode.
///
/// Restores the original character that was overwritten. Pops from
/// the replace restore stack in `VimSessionState`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReplaceBackspace;

impl Command for ReplaceBackspace {
    fn id(&self) -> CommandId {
        ids::REPLACE_BACKSPACE
    }

    fn description(&self) -> &'static str {
        "Restore original character in replace mode"
    }
}

impl CommandHandler for ReplaceBackspace {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let entry = runtime
            .ext_mut::<crate::VimSessionState>()
            .replace_restore_stack
            .pop();

        let Some(entry) = entry else {
            // Nothing to restore
            return CommandResult::Success;
        };

        let Some(window) = runtime.windows().active() else {
            return CommandResult::Success;
        };
        let Some(buffer_id) = window.buffer_id else {
            return CommandResult::Success;
        };
        let cursor = Position::new(window.cursor.line, window.cursor.column);

        if cursor.column > 0 {
            let delete_col = cursor.column - 1;
            // Delete the replacement character
            runtime.delete_range(buffer_id, Position::new(cursor.line, delete_col), cursor);

            // Restore original character if there was one
            if let Some(original) = entry.original {
                let restore_str = String::from(original);
                runtime.insert_text(buffer_id, Position::new(cursor.line, delete_col), &restore_str);
            }

            // Move cursor back
            if let Some(w) = runtime.windows_mut().active_mut() {
                w.cursor.column = delete_col;
            }
        } else if cursor.line > 0 {
            // At start of line — join with previous line
            let prev_line_len = runtime
                .buffer_line_len(buffer_id, cursor.line - 1)
                .unwrap_or(0);

            // Delete the newline
            runtime.delete_range(
                buffer_id,
                Position::new(cursor.line - 1, prev_line_len),
                Position::new(cursor.line, 0),
            );

            // Restore original char if there was one
            if let Some(original) = entry.original {
                let restore_str = String::from(original);
                runtime.insert_text(
                    buffer_id,
                    Position::new(cursor.line - 1, prev_line_len),
                    &restore_str,
                );
            }

            // Move cursor to join point
            if let Some(w) = runtime.windows_mut().active_mut() {
                w.cursor.line -= 1;
                w.cursor.column = prev_line_len;
            }
        }

        // Pop from insert buffer for dot repeat
        runtime.ext_mut::<crate::VimSessionState>().insert_buffer.pop();

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

        // Push input to history before taking it (#451)
        runtime.ext_mut::<CmdlineState>().push_to_history();

        // Get the cmdline input
        let cmdline = runtime.ext_mut::<CmdlineState>().take_cmdline_input();

        // Exit cmdline and return to normal BEFORE dispatching the action.
        // This ensures any mode transition made by the action (e.g., push_mode)
        // stacks on top of NORMAL rather than being clobbered by set_mode below.
        runtime.ext_mut::<CmdlineState>().exit();
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

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

        CommandResult::Success
    }
}

/// Strip range prefix from a command line string.
///
/// Detects and removes range prefixes like `%`, returning the range
/// as `(start_line, end_line)` and the remaining command string.
///
/// Supported prefixes:
/// - `%` — entire buffer (0, `last_line`)
/// - No prefix — returns `None` (command handles default)
#[cfg_attr(coverage_nightly, coverage(off))]
fn strip_range_prefix<'a>(
    runtime: &SessionRuntime<'_>,
    args: &CommandContext,
    cmdline: &'a str,
) -> (Option<(usize, usize)>, &'a str) {
    let trimmed = cmdline.trim_start();

    // % prefix means entire buffer
    if let Some(rest) = trimmed.strip_prefix('%') {
        let line_count = args
            .buffer_id()
            .and_then(|bid| runtime.buffer_line_count(bid))
            .unwrap_or(1);
        let last_line = line_count.saturating_sub(1);
        return (Some((0, last_line)), rest);
    }

    (None, cmdline)
}

/// Execute an ex-command via `CommandNameIndex` and `runtime.execute_command()`.
///
/// Parses the command line, resolves the command name via `CommandNameIndex`,
/// binds arguments to the command's `ArgSpec` declarations via `bind_args()`,
/// and dispatches through the unified command system.
fn execute_ex_command(runtime: &mut SessionRuntime<'_>, args: &CommandContext, cmdline: &str) {
    use {
        reovim_driver_command::{CommandNameIndex, bind_args, parse_cmdline},
        reovim_driver_session::CommandApi,
    };

    // Strip range prefix (%, line numbers) from cmdline (#666)
    let (range, effective_cmdline) = strip_range_prefix(runtime, args, cmdline);

    let Some(parsed) = parse_cmdline(effective_cmdline) else {
        return;
    };

    let Some(name_index) = runtime.kernel().services.get::<CommandNameIndex>() else {
        tracing::warn!("CommandNameIndex not registered - ex-commands not available");
        return;
    };

    let (cmd_id, specs) = match name_index.resolve_prefix(&parsed.name) {
        Ok(Some((id, cmd))) => (id.clone(), cmd.args()),
        Ok(None) => {
            let msg = format!("E492: Not an editor command: {}", parsed.name);
            runtime
                .ext_mut::<CmdlineState>()
                .set_message(CmdlineMessage::Error(msg));
            return;
        }
        Err(ambiguous) => {
            runtime
                .ext_mut::<CmdlineState>()
                .set_message(CmdlineMessage::Error(ambiguous.to_string()));
            return;
        }
    };
    // Drop the borrow on name_index before calling execute_command
    drop(name_index);

    let bound = match bind_args(&specs, &parsed.raw_args, parsed.bang) {
        Ok(map) => map,
        Err(e) => {
            runtime
                .ext_mut::<CmdlineState>()
                .set_message(CmdlineMessage::Error(e.to_string()));
            return;
        }
    };

    // Build command context from bound arguments
    let mut ctx = CommandContext::new();
    for (name, value) in bound {
        ctx.set(&name, value);
    }
    // Propagate buffer_id and VFS from outer args
    if let Some(bid) = args.buffer_id() {
        ctx.set_buffer_id(bid);
    }
    if let Some(vfs) = args.vfs() {
        ctx.set_vfs(Arc::clone(vfs));
    }
    // Propagate range if detected (#666)
    if let Some((start, end)) = range {
        ctx.set("range", reovim_driver_command_types::ArgValue::Range(start, end));
    }

    let result = runtime.execute_command(cmd_id, ctx);
    if let CommandResult::Error(msg) = result {
        runtime
            .ext_mut::<CmdlineState>()
            .set_message(CmdlineMessage::Error(msg));
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
// Command-line editing commands (#451)
// =============================================================================

/// Move cursor left in command-line.
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineCursorLeft;

impl Command for CmdlineCursorLeft {
    fn id(&self) -> CommandId {
        ids::CMDLINE_CURSOR_LEFT
    }
    fn description(&self) -> &'static str {
        "Move cursor left in command-line"
    }
}

impl CommandHandler for CmdlineCursorLeft {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.ext_mut::<CmdlineState>().move_cursor_left();
        CommandResult::Success
    }
}

/// Move cursor right in command-line.
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineCursorRight;

impl Command for CmdlineCursorRight {
    fn id(&self) -> CommandId {
        ids::CMDLINE_CURSOR_RIGHT
    }
    fn description(&self) -> &'static str {
        "Move cursor right in command-line"
    }
}

impl CommandHandler for CmdlineCursorRight {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.ext_mut::<CmdlineState>().move_cursor_right();
        CommandResult::Success
    }
}

/// Move cursor to start of command-line.
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineCursorHome;

impl Command for CmdlineCursorHome {
    fn id(&self) -> CommandId {
        ids::CMDLINE_CURSOR_HOME
    }
    fn description(&self) -> &'static str {
        "Move cursor to start of command-line"
    }
}

impl CommandHandler for CmdlineCursorHome {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.ext_mut::<CmdlineState>().move_to_start();
        CommandResult::Success
    }
}

/// Move cursor to end of command-line.
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineCursorEnd;

impl Command for CmdlineCursorEnd {
    fn id(&self) -> CommandId {
        ids::CMDLINE_CURSOR_END
    }
    fn description(&self) -> &'static str {
        "Move cursor to end of command-line"
    }
}

impl CommandHandler for CmdlineCursorEnd {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.ext_mut::<CmdlineState>().move_to_end();
        CommandResult::Success
    }
}

/// Delete character at cursor in command-line (Del key).
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineDeleteChar;

impl Command for CmdlineDeleteChar {
    fn id(&self) -> CommandId {
        ids::CMDLINE_DELETE_CHAR
    }
    fn description(&self) -> &'static str {
        "Delete character at cursor in command-line"
    }
}

impl CommandHandler for CmdlineDeleteChar {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.ext_mut::<CmdlineState>().delete_at_cursor();
        CommandResult::Success
    }
}

/// Delete character before cursor in command-line (Backspace).
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineBackspace;

impl Command for CmdlineBackspace {
    fn id(&self) -> CommandId {
        ids::CMDLINE_BACKSPACE
    }
    fn description(&self) -> &'static str {
        "Delete character before cursor in command-line"
    }
}

impl CommandHandler for CmdlineBackspace {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.ext_mut::<CmdlineState>().backspace();
        CommandResult::Success
    }
}

/// Delete word before cursor in command-line (Ctrl-W).
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineDeleteWord;

impl Command for CmdlineDeleteWord {
    fn id(&self) -> CommandId {
        ids::CMDLINE_DELETE_WORD
    }
    fn description(&self) -> &'static str {
        "Delete word before cursor in command-line"
    }
}

impl CommandHandler for CmdlineDeleteWord {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.ext_mut::<CmdlineState>().delete_word_back();
        CommandResult::Success
    }
}

/// Delete to start of command-line (Ctrl-U).
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineDeleteToStart;

impl Command for CmdlineDeleteToStart {
    fn id(&self) -> CommandId {
        ids::CMDLINE_DELETE_TO_START
    }
    fn description(&self) -> &'static str {
        "Delete to start of command-line"
    }
}

impl CommandHandler for CmdlineDeleteToStart {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.ext_mut::<CmdlineState>().delete_to_start();
        CommandResult::Success
    }
}

// =============================================================================
// Command-line history commands (#451)
// =============================================================================

/// Navigate to older history entry (Up / Ctrl-P).
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineHistoryUp;

impl Command for CmdlineHistoryUp {
    fn id(&self) -> CommandId {
        ids::CMDLINE_HISTORY_UP
    }
    fn description(&self) -> &'static str {
        "Navigate to older history entry"
    }
}

impl CommandHandler for CmdlineHistoryUp {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.ext_mut::<CmdlineState>().history_up();
        CommandResult::Success
    }
}

/// Navigate to newer history entry (Down / Ctrl-N).
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineHistoryDown;

impl Command for CmdlineHistoryDown {
    fn id(&self) -> CommandId {
        ids::CMDLINE_HISTORY_DOWN
    }
    fn description(&self) -> &'static str {
        "Navigate to newer history entry"
    }
}

impl CommandHandler for CmdlineHistoryDown {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        runtime.ext_mut::<CmdlineState>().history_down();
        CommandResult::Success
    }
}

// =============================================================================
// Command-line completion commands (#451)
// =============================================================================

/// Cycle to next completion (Tab).
///
/// On first press, queries `CommandNameIndex` for candidates matching
/// the current input prefix. On subsequent presses, cycles forward.
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineCompleteNext;

impl Command for CmdlineCompleteNext {
    fn id(&self) -> CommandId {
        ids::CMDLINE_COMPLETE_NEXT
    }
    fn description(&self) -> &'static str {
        "Cycle to next command-line completion"
    }
}

impl CommandHandler for CmdlineCompleteNext {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        populate_completions_if_needed(runtime);
        runtime.ext_mut::<CmdlineState>().complete_next();
        CommandResult::Success
    }
}

/// Cycle to previous completion (Shift-Tab).
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdlineCompletePrev;

impl Command for CmdlineCompletePrev {
    fn id(&self) -> CommandId {
        ids::CMDLINE_COMPLETE_PREV
    }
    fn description(&self) -> &'static str {
        "Cycle to previous command-line completion"
    }
}

impl CommandHandler for CmdlineCompletePrev {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        populate_completions_if_needed(runtime);
        runtime.ext_mut::<CmdlineState>().complete_prev();
        CommandResult::Success
    }
}

/// Populate completions from `CommandNameIndex` if not already populated.
fn populate_completions_if_needed(runtime: &mut SessionRuntime<'_>) {
    // Only populate when completions list is empty
    if !runtime.ext_mut::<CmdlineState>().completions().is_empty() {
        return;
    }

    let prefix = runtime.ext_mut::<CmdlineState>().input().to_string();
    if prefix.is_empty() {
        return;
    }

    // Query CommandNameIndex for matching commands (#547)
    let candidates = {
        use reovim_driver_command::CommandNameIndex;
        runtime
            .kernel()
            .services
            .get::<CommandNameIndex>()
            .map_or_else(Vec::new, |index| {
                index
                    .search_by_prefix(&prefix)
                    .into_iter()
                    .flat_map(|(_, cmd)| cmd.names().iter().copied().map(String::from))
                    .filter(|name| name.starts_with(&prefix))
                    .collect()
            })
    };

    if !candidates.is_empty() {
        runtime
            .ext_mut::<CmdlineState>()
            .set_completions(prefix, candidates);
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
