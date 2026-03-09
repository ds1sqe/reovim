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
    reovim_module_cmdline::{CmdlinePrompt, CmdlineState},
    std::sync::Arc,
};

/// Helper to get cursor position from the active window.
fn get_cursor_position(runtime: &SessionRuntime<'_>) -> Option<Position> {
    let window = runtime.windows().active()?;
    Some(Position::new(window.cursor.line, window.cursor.column))
}

/// Helper to set cursor position on the active window.
#[cfg_attr(coverage_nightly, coverage(off))]
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

/// Execute an ex-command via `CommandNameIndex` and `runtime.execute_command()`.
///
/// Parses the command line, resolves the command name via `CommandNameIndex`,
/// builds a `CommandContext`, and dispatches through the unified command system.
fn execute_ex_command(runtime: &mut SessionRuntime<'_>, args: &CommandContext, cmdline: &str) {
    use {
        reovim_driver_command::{CommandNameIndex, parse_cmdline},
        reovim_driver_command_types::ArgValue,
        reovim_driver_session::CommandApi,
    };

    let Some(parsed) = parse_cmdline(cmdline) else {
        return;
    };

    let Some(name_index) = runtime.kernel().services.get::<CommandNameIndex>() else {
        tracing::warn!("CommandNameIndex not registered - ex-commands not available");
        return;
    };

    let Some(cmd_id) = name_index.resolve(&parsed.name).cloned() else {
        tracing::warn!(name = parsed.name, "E492: Not an editor command");
        return;
    };

    // Drop the borrow on name_index before calling execute_command
    drop(name_index);

    // Build command context
    let mut ctx = CommandContext::new();
    if parsed.bang {
        ctx.set("bang", ArgValue::Bang(true));
    }
    if !parsed.args.is_empty() {
        ctx.set("file", ArgValue::String(parsed.args.join(" ")));
    }
    // Propagate buffer_id and VFS from outer args
    if let Some(bid) = args.buffer_id() {
        ctx.set_buffer_id(bid);
    }
    if let Some(vfs) = args.vfs() {
        ctx.set_vfs(Arc::clone(vfs));
    }

    let result = runtime.execute_command(cmd_id, ctx);
    if let CommandResult::Error(msg) = result {
        tracing::warn!(cmdline, msg = %msg, "Ex-command failed");
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

#[cfg(test)]
#[allow(clippy::significant_drop_tightening, clippy::uninlined_format_args)]
mod tests {
    use {
        super::*,
        reovim_driver_command::Command,
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, WindowLayout,
            api::{CommandExecutor, CommandHandle},
        },
        reovim_kernel::api::{
            ModeStack,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, CommandId, EventBus, HistoryRing,
                KernelContext, MarkBank, MotionEngine, OptionRegistry, RegisterBank, RwLock,
                ServiceRegistry, TextObjectEngine,
            },
        },
        std::{collections::HashMap, sync::Arc},
    };

    use reovim_driver_session::api::ModeApi;

    // ========================================================================
    // Test infrastructure
    // ========================================================================

    /// Test buffer manager that actually stores buffers.
    struct TestBufferManager {
        buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
    }

    impl TestBufferManager {
        fn new() -> Self {
            Self {
                buffers: RwLock::new(HashMap::new()),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl BufferManager for TestBufferManager {
        fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
            self.buffers.read().get(&id).cloned()
        }

        fn create(&self) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(Buffer::new()));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn register(&self, buffer: Buffer) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(buffer));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError> {
            self.buffers
                .write()
                .remove(&id)
                .map_or(Err(BufferError::NotFound(id)), |arc_buffer| {
                    Arc::try_unwrap(arc_buffer)
                        .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
                })
        }

        fn list(&self) -> Vec<BufferId> {
            self.buffers.read().keys().copied().collect()
        }

        fn count(&self) -> usize {
            self.buffers.read().len()
        }
    }

    struct StubExecutor;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CommandExecutor for StubExecutor {
        fn get_handle(&self, _id: &CommandId) -> Option<std::sync::Arc<dyn CommandHandle>> {
            None
        }
    }

    /// Test executor that wraps `CommandHandler` instances for name-based dispatch tests.
    struct TestExecutor {
        handlers: HashMap<CommandId, Arc<dyn CommandHandler>>,
    }

    impl TestExecutor {
        fn new() -> Self {
            Self {
                handlers: HashMap::new(),
            }
        }

        fn register(&mut self, handler: Arc<dyn CommandHandler>) {
            self.handlers.insert(handler.id(), handler);
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CommandExecutor for TestExecutor {
        fn get_handle(&self, id: &CommandId) -> Option<std::sync::Arc<dyn CommandHandle>> {
            self.handlers.get(id).map(|h| {
                let handler = Arc::clone(h);
                Arc::new(TestHandleBridge(handler)) as Arc<dyn CommandHandle>
            })
        }
    }

    struct TestHandleBridge(Arc<dyn CommandHandler>);

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CommandHandle for TestHandleBridge {
        fn execute(
            &self,
            runtime: &mut SessionRuntime<'_>,
            ctx: &reovim_driver_command::CommandContext,
        ) -> CommandResult {
            self.0.execute(runtime, ctx)
        }
    }

    struct TestState {
        session: Session,
        mode_stack: ModeStack,
        windows: WindowLayout,
        extensions: ExtensionMap,
        compositor: Option<Box<dyn reovim_driver_display::layout::RootCompositor>>,
        tabs: reovim_driver_session::TabPageSet,
        registers: RegisterBank,
        clipboard_history: HistoryRing,
        local_marks: MarkBank,
        active_buffer: Option<BufferId>,
        terminal_size: (u16, u16),
    }

    impl TestState {
        fn with_buffer(buffer_id: Option<BufferId>) -> Self {
            let home_mode = VimMode::NORMAL_ID;
            let session = Session::new(ClientId::new(1), home_mode.clone());
            let mode_stack = ModeStack::new(home_mode);
            let mut windows = WindowLayout::empty();
            let extensions = ExtensionMap::new();

            let mut window = reovim_driver_session::Window::new();
            if let Some(buffer_id) = buffer_id {
                window.buffer_id = Some(buffer_id);
            }
            windows.add(window);

            Self {
                session,
                mode_stack,
                windows,
                extensions,
                compositor: None,
                tabs: reovim_driver_session::TabPageSet::new(),
                registers: RegisterBank::new(),
                clipboard_history: HistoryRing::new(),
                local_marks: MarkBank::new(),
                active_buffer: None,
                terminal_size: (80, 24),
            }
        }

        fn runtime<'a>(&'a mut self, kernel: &'a KernelContext) -> SessionRuntime<'a> {
            self.runtime_with_executor(kernel, &StubExecutor)
        }

        fn runtime_with_executor<'a>(
            &'a mut self,
            kernel: &'a KernelContext,
            executor: &'a dyn CommandExecutor,
        ) -> SessionRuntime<'a> {
            SessionRuntime::new(
                &mut self.session,
                reovim_driver_session::ClientContext {
                    mode_stack: &mut self.mode_stack,
                    windows: &mut self.windows,
                    extensions: &mut self.extensions,
                    compositor: &mut self.compositor,
                    tabs: &mut self.tabs,
                    registers: &mut self.registers,
                    clipboard_history: &mut self.clipboard_history,
                    local_marks: &mut self.local_marks,
                    active_buffer: &mut self.active_buffer,
                    terminal_size: &mut self.terminal_size,
                },
                kernel,
                executor,
            )
        }
    }

    fn create_test_context() -> KernelContext {
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            Arc::new(ServiceRegistry::new()),
        )
    }

    // ========================================================================
    // Metadata tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_id() {
        let cmd = EnterInsertMode;
        assert_eq!(cmd.id(), ids::ENTER_INSERT);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_description() {
        let cmd = EnterInsertMode;
        assert!(cmd.description().contains("insert"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_append_id() {
        let cmd = EnterInsertModeAppend;
        assert_eq!(cmd.id(), ids::ENTER_INSERT_AFTER);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_append_description() {
        let cmd = EnterInsertModeAppend;
        assert!(cmd.description().contains("append"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_to_normal_id() {
        let cmd = ExitToNormal;
        assert_eq!(cmd.id(), ids::EXIT_INSERT);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_to_normal_description() {
        let cmd = ExitToNormal;
        assert!(cmd.description().contains("normal"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_window_mode_id() {
        let cmd = EnterWindowMode;
        assert_eq!(cmd.id(), ids::ENTER_WINDOW_MODE);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_window_mode_description() {
        let cmd = EnterWindowMode;
        assert!(cmd.description().contains("window"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cancel_to_normal_id() {
        let cmd = CancelToNormal;
        assert_eq!(cmd.id(), ids::CANCEL_TO_NORMAL);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cancel_to_normal_description() {
        let cmd = CancelToNormal;
        assert!(cmd.description().contains("normal"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_commandline_mode_id() {
        let cmd = EnterCommandLineMode;
        assert_eq!(cmd.id(), ids::ENTER_COMMANDLINE);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_commandline_mode_description() {
        let cmd = EnterCommandLineMode;
        assert!(cmd.description().contains("command-line"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_mode_id() {
        let cmd = ExitCommandLineMode;
        assert_eq!(cmd.id(), ids::EXIT_COMMANDLINE);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_mode_description() {
        let cmd = ExitCommandLineMode;
        assert!(cmd.description().contains("command-line"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cancel_commandline_mode_id() {
        let cmd = CancelCommandLineMode;
        assert_eq!(cmd.id(), ids::CANCEL_COMMANDLINE);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cancel_commandline_mode_description() {
        let cmd = CancelCommandLineMode;
        assert!(cmd.description().contains("Cancel"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_search_forward_id() {
        let cmd = EnterSearchForward;
        assert_eq!(cmd.id(), ids::ENTER_SEARCH_FORWARD);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_search_forward_description() {
        let cmd = EnterSearchForward;
        assert!(cmd.description().contains("search"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_search_backward_id() {
        let cmd = EnterSearchBackward;
        assert_eq!(cmd.id(), ids::ENTER_SEARCH_BACKWARD);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_search_backward_description() {
        let cmd = EnterSearchBackward;
        assert!(cmd.description().contains("search"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_all_mode_commands_debug() {
        let debug_insert = format!("{:?}", EnterInsertMode);
        assert!(debug_insert.contains("EnterInsertMode"));

        let debug_append = format!("{:?}", EnterInsertModeAppend);
        assert!(debug_append.contains("EnterInsertModeAppend"));

        let debug_exit = format!("{:?}", ExitToNormal);
        assert!(debug_exit.contains("ExitToNormal"));

        let debug_window = format!("{:?}", EnterWindowMode);
        assert!(debug_window.contains("EnterWindowMode"));

        let debug_cancel = format!("{:?}", CancelToNormal);
        assert!(debug_cancel.contains("CancelToNormal"));

        let debug_cmdline = format!("{:?}", EnterCommandLineMode);
        assert!(debug_cmdline.contains("EnterCommandLineMode"));

        let debug_exit_cmd = format!("{:?}", ExitCommandLineMode);
        assert!(debug_exit_cmd.contains("ExitCommandLineMode"));

        let debug_cancel_cmd = format!("{:?}", CancelCommandLineMode);
        assert!(debug_cancel_cmd.contains("CancelCommandLineMode"));

        let debug_search_forward = format!("{:?}", EnterSearchForward);
        assert!(debug_search_forward.contains("EnterSearchForward"));

        let debug_search_backward = format!("{:?}", EnterSearchBackward);
        assert!(debug_search_backward.contains("EnterSearchBackward"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_all_mode_commands_default() {
        let _ = EnterInsertMode;
        let _ = EnterInsertModeAppend;
        let _ = ExitToNormal;
        let _ = EnterWindowMode;
        let _ = CancelToNormal;
        let _ = EnterCommandLineMode;
        let _ = ExitCommandLineMode;
        let _ = CancelCommandLineMode;
        let _ = EnterSearchForward;
        let _ = EnterSearchBackward;
    }

    // ========================================================================
    // Execute tests - mode transitions
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_execute() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_without_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_append_execute() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertModeAppend.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_append_moves_cursor() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Set cursor to position (0, 2) - on 'l'
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 2).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertModeAppend.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor should move right by 1
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 3);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_to_normal_execute() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Set cursor at column 3 so exiting insert moves it to 2
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 3).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ExitToNormal.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);

        // Cursor should move left by 1 (Vim behavior on exit insert)
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 2);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_to_normal_cursor_at_col_zero() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Cursor at column 0 should stay at 0
        let mut runtime = state.runtime(&ctx);
        let result = ExitToNormal.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_window_mode_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = EnterWindowMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::WINDOW_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cancel_to_normal_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = CancelToNormal.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_commandline_mode_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = EnterCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::COMMANDLINE_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_mode_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cancel_commandline_mode_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = CancelCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_search_forward_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = EnterSearchForward.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::COMMANDLINE_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_search_backward_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = EnterSearchBackward.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::COMMANDLINE_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_to_normal_without_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = ExitToNormal.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_append_at_end_of_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Cursor at end of line (column 4 = last char 'o', line_len = 5)
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 4).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertModeAppend.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Should move to column 5 (past end of line for insert)
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 5);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_append_without_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertModeAppend.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
    }

    // ========================================================================
    // Additional execute tests - edge cases
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_append_at_start_of_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Cursor at column 0
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertModeAppend.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 1); // Moved right
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_append_empty_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertModeAppend.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor stays at 0 (line_len is 0, so not < line_len)
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_to_normal_from_middle_of_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 5).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ExitToNormal.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 4); // Moved left by 1
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_to_normal_from_column_one() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("ab");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 1).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ExitToNormal.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_mode_returns_to_normal() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        // First enter commandline mode
        EnterCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(runtime.current_mode(), &VimMode::COMMANDLINE_ID);

        // Then exit
        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cancel_commandline_mode_returns_to_normal() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        // First enter commandline mode
        EnterCommandLineMode.execute(&mut runtime, &args);

        // Cancel
        let result = CancelCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_on_second_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(1, 2).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_append_on_second_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(1, 2).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertModeAppend.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 3); // Moved right
    }

    // ========================================================================
    // Undo batching tests - exercises begin_insert_batch / end_insert_batch
    // ========================================================================

    use {
        reovim_driver_undo::{UndoKey, UndoPersistError, UndoProvider, UndoProviderRegistry},
        reovim_driver_vfs::VfsDriver,
        reovim_kernel::api::v1::{Edit, UndoResult, UndoTree},
    };

    /// Minimal mock undo provider that tracks `begin_batch`/`end_batch` calls.
    struct MockUndoProvider {
        batch_begins: RwLock<Vec<(BufferId, Position)>>,
        batch_ends: RwLock<Vec<(BufferId, Position)>>,
        records: RwLock<Vec<BufferId>>,
    }

    impl MockUndoProvider {
        fn new() -> Self {
            Self {
                batch_begins: RwLock::new(Vec::new()),
                batch_ends: RwLock::new(Vec::new()),
                records: RwLock::new(Vec::new()),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl UndoProvider for MockUndoProvider {
        fn undo(&self, _: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo(&self, _: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo_branch(&self, _: BufferId, _: usize) -> Option<UndoResult> {
            None
        }
        fn record(&self, buffer_id: BufferId, _: Vec<Edit>, _: Position, _: Position) {
            self.records.write().push(buffer_id);
        }
        fn has_history(&self, _: BufferId) -> bool {
            false
        }
        fn remove(&self, _: BufferId) {}
        fn buffer_count(&self) -> usize {
            0
        }
        fn get_tree(&self, _: BufferId) -> Option<UndoTree> {
            None
        }
        fn begin_batch(&self, buffer_id: BufferId, cursor_before: Position) {
            self.batch_begins.write().push((buffer_id, cursor_before));
        }
        fn end_batch(&self, buffer_id: BufferId, cursor_after: Position) {
            self.batch_ends.write().push((buffer_id, cursor_after));
        }
        fn is_batching(&self, _: BufferId) -> bool {
            false
        }
        fn persist(&self, _: BufferId, _: &str, _: &dyn VfsDriver) -> Result<(), UndoPersistError> {
            Ok(())
        }
        fn load(&self, _: BufferId, _: &str, _: &dyn VfsDriver) -> Result<bool, UndoPersistError> {
            Ok(false)
        }
    }

    fn create_test_context_with_undo() -> (KernelContext, Arc<MockUndoProvider>) {
        let services = Arc::new(ServiceRegistry::new());
        let mock_undo = Arc::new(MockUndoProvider::new());
        let undo_registry = Arc::new(UndoProviderRegistry::new());
        undo_registry.register(UndoKey::Buffer, mock_undo.clone() as Arc<dyn UndoProvider>);
        services.register(undo_registry);

        let ctx = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            services,
        );
        (ctx, mock_undo)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_calls_begin_batch() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // begin_batch should have been called
        let begins = mock_undo.batch_begins.read();
        assert_eq!(begins.len(), 1);
        assert_eq!(begins[0].0, buffer_id);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_append_calls_begin_batch() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertModeAppend.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let begins = mock_undo.batch_begins.read();
        assert_eq!(begins.len(), 1);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_to_normal_calls_end_batch() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 3).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ExitToNormal.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // end_batch should have been called
        let ends = mock_undo.batch_ends.read();
        assert_eq!(ends.len(), 1);
        assert_eq!(ends[0].0, buffer_id);
    }

    // ========================================================================
    // VimSessionState dot-repeat recording tests
    // ========================================================================

    use reovim_driver_session::api::ExtensionApi;

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_mode_clears_insert_buffer() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        // Pre-populate insert_buffer via VimSessionState
        {
            let vim = runtime.ext_mut::<crate::VimSessionState>();
            vim.insert_buffer.push_str("previous text");
        }

        EnterInsertMode.execute(&mut runtime, &args);

        // insert_buffer should be cleared
        let vim = runtime.ext_mut::<crate::VimSessionState>();
        assert!(vim.insert_buffer.is_empty());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_to_normal_records_dot_repeat_for_insert() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        // Simulate insert buffer with typed text
        {
            let vim = runtime.ext_mut::<crate::VimSessionState>();
            vim.insert_buffer.push_str("world");
        }

        ExitToNormal.execute(&mut runtime, &args);

        // last_change should record the insert text
        let vim = runtime.ext_mut::<crate::VimSessionState>();
        assert!(vim.last_change.is_some());
        let last_change = vim.last_change.as_ref().unwrap();
        match &last_change.change_type {
            crate::session_state::ChangeType::Insert { text } => {
                assert_eq!(text, "world");
            }
            _ => panic!("Expected Insert change type"),
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_to_normal_does_not_record_empty_insert() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        // insert_buffer is empty (no text typed)
        ExitToNormal.execute(&mut runtime, &args);

        // last_change should NOT be set for empty insert
        let vim = runtime.ext_mut::<crate::VimSessionState>();
        assert!(vim.last_change.is_none());
    }

    // ========================================================================
    // ExitCommandLineMode search handling tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_mode_search_forward_with_pending() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        // Set up search forward mode: pending search + cmdline input
        {
            use reovim_driver_session::api::SearchState;
            runtime
                .ext_mut::<SearchState>()
                .start_pending_search(reovim_driver_search::Direction::Forward);
            runtime
                .ext_mut::<CmdlineState>()
                .enter(CmdlinePrompt::SearchForward);
            for ch in "world".chars() {
                runtime.ext_mut::<CmdlineState>().insert_char(ch);
            }
        }

        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::NORMAL_ID);

        // Search state should have stored pattern/direction
        let search_state = runtime.ext_mut::<reovim_driver_session::api::SearchState>();
        assert!(search_state.pattern_for_repeat().is_some());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_mode_search_backward_with_pending() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        // Set up search backward mode
        {
            use reovim_driver_session::api::SearchState;
            runtime
                .ext_mut::<SearchState>()
                .start_pending_search(reovim_driver_search::Direction::Backward);
            runtime
                .ext_mut::<CmdlineState>()
                .enter(CmdlinePrompt::SearchBackward);
            for ch in "hello".chars() {
                runtime.ext_mut::<CmdlineState>().insert_char(ch);
            }
        }

        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_mode_search_empty_cmdline() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        // Set up search forward but with empty cmdline
        {
            use reovim_driver_session::api::SearchState;
            runtime
                .ext_mut::<SearchState>()
                .start_pending_search(reovim_driver_search::Direction::Forward);
            runtime
                .ext_mut::<CmdlineState>()
                .enter(CmdlinePrompt::SearchForward);
            // Don't set cmdline input - it will be empty
        }

        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_mode_command_with_input() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        // Enter command mode with some input (no CommandNameIndex registered)
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        for ch in "w".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }

        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        // execute_ex_command logs warning but doesn't fail
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_mode_command_empty_input() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        // Enter command mode with empty input
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);

        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    // ========================================================================
    // execute_search edge cases
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_search_no_buffer_id() {
        let ctx = create_test_context();
        let args = CommandContext::new(); // no buffer_id

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        // Search forward pending with cmdline, but no buffer_id
        {
            use reovim_driver_session::api::SearchState;
            runtime
                .ext_mut::<SearchState>()
                .start_pending_search(reovim_driver_search::Direction::Forward);
            runtime
                .ext_mut::<CmdlineState>()
                .enter(CmdlinePrompt::SearchForward);
            for ch in "test".chars() {
                runtime.ext_mut::<CmdlineState>().insert_char(ch);
            }
        }

        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_search_no_pending() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        // Enter search forward mode but no pending search set
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::SearchForward);
        for ch in "test".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }

        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    // ========================================================================
    // execute_search with SearchProviderRegistry tests
    // ========================================================================

    use reovim_driver_search::{
        SearchError, SearchKey, SearchMatch, SearchProvider, SearchProviderRegistry,
    };

    /// Mock search provider that can be configured to return specific results.
    struct MockSearchProvider {
        result: RwLock<Option<Result<Option<SearchMatch>, SearchError>>>,
    }

    impl MockSearchProvider {
        fn with_match(m: SearchMatch) -> Self {
            Self {
                result: RwLock::new(Some(Ok(Some(m)))),
            }
        }

        fn with_no_match() -> Self {
            Self {
                result: RwLock::new(Some(Ok(None))),
            }
        }

        fn with_error() -> Self {
            Self {
                result: RwLock::new(Some(Err(SearchError::InvalidPattern("test".to_string())))),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl SearchProvider for MockSearchProvider {
        fn find_next(
            &self,
            _buffer: &Buffer,
            _cursor: Position,
            _pattern: &str,
            _direction: reovim_driver_search::Direction,
            _wrap: bool,
        ) -> Result<Option<SearchMatch>, SearchError> {
            self.result.read().clone().unwrap_or(Ok(None))
        }

        fn find_all(
            &self,
            _buffer: &Buffer,
            _pattern: &str,
        ) -> Result<Vec<SearchMatch>, SearchError> {
            Ok(vec![])
        }

        fn word_at_cursor(&self, _buffer: &Buffer, _cursor: Position) -> Option<String> {
            None
        }
    }

    fn create_test_context_with_search(provider: Arc<dyn SearchProvider>) -> KernelContext {
        let services = Arc::new(ServiceRegistry::new());
        let search_registry = Arc::new(SearchProviderRegistry::new());
        search_registry.register(SearchKey::Regex, provider);
        services.register(search_registry);

        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            services,
        )
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_search_with_match_found() {
        let mock = Arc::new(MockSearchProvider::with_match(SearchMatch {
            start: Position::new(0, 6),
            end: Position::new(0, 11),
        }));
        let ctx = create_test_context_with_search(mock);
        let buffer = Buffer::from_string("hello world hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        // Set up search forward with pattern
        {
            runtime
                .ext_mut::<reovim_driver_session::api::SearchState>()
                .start_pending_search(reovim_driver_search::Direction::Forward);
            runtime
                .ext_mut::<CmdlineState>()
                .enter(CmdlinePrompt::SearchForward);
            for ch in "world".chars() {
                runtime.ext_mut::<CmdlineState>().insert_char(ch);
            }
        }

        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor should have moved to match position
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 6);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_search_no_match() {
        let mock = Arc::new(MockSearchProvider::with_no_match());
        let ctx = create_test_context_with_search(mock);
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        {
            runtime
                .ext_mut::<reovim_driver_session::api::SearchState>()
                .start_pending_search(reovim_driver_search::Direction::Forward);
            runtime
                .ext_mut::<CmdlineState>()
                .enter(CmdlinePrompt::SearchForward);
            for ch in "zzz".chars() {
                runtime.ext_mut::<CmdlineState>().insert_char(ch);
            }
        }

        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor should remain at original position
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_search_invalid_pattern() {
        let mock = Arc::new(MockSearchProvider::with_error());
        let ctx = create_test_context_with_search(mock);
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        {
            runtime
                .ext_mut::<reovim_driver_session::api::SearchState>()
                .start_pending_search(reovim_driver_search::Direction::Forward);
            runtime
                .ext_mut::<CmdlineState>()
                .enter(CmdlinePrompt::SearchForward);
            for ch in "[invalid".chars() {
                runtime.ext_mut::<CmdlineState>().insert_char(ch);
            }
        }

        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor should remain at original position despite error
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_search_backward_with_match() {
        let mock = Arc::new(MockSearchProvider::with_match(SearchMatch {
            start: Position::new(0, 0),
            end: Position::new(0, 5),
        }));
        let ctx = create_test_context_with_search(mock);
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 8).into();
        }
        let mut runtime = state.runtime(&ctx);

        {
            runtime
                .ext_mut::<reovim_driver_session::api::SearchState>()
                .start_pending_search(reovim_driver_search::Direction::Backward);
            runtime
                .ext_mut::<CmdlineState>()
                .enter(CmdlinePrompt::SearchBackward);
            for ch in "hello".chars() {
                runtime.ext_mut::<CmdlineState>().insert_char(ch);
            }
        }

        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor should have moved to match
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_search_no_cursor_position() {
        // Test execute_search early return when no active window (no cursor)
        let mock = Arc::new(MockSearchProvider::with_no_match());
        let ctx = create_test_context_with_search(mock);
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Create state with no windows (no active window)
        let home_mode = VimMode::NORMAL_ID;
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let mut runtime = SessionRuntime::new(
            &mut session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &ctx,
            &StubExecutor,
        );

        {
            runtime
                .ext_mut::<reovim_driver_session::api::SearchState>()
                .start_pending_search(reovim_driver_search::Direction::Forward);
            runtime
                .ext_mut::<CmdlineState>()
                .enter(CmdlinePrompt::SearchForward);
            for ch in "test".chars() {
                runtime.ext_mut::<CmdlineState>().insert_char(ch);
            }
        }

        // Should succeed without error even though no cursor available
        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    // ========================================================================
    // execute_ex_command with CommandNameIndex tests (#547)
    // ========================================================================

    use reovim_driver_command::CommandNameIndex;

    /// Simple test command for ex-command dispatch tests.
    struct ExTestCmd {
        cmd_id: CommandId,
        result: CommandResult,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ExTestCmd {
        fn success(name: &'static str) -> Self {
            Self {
                cmd_id: CommandId::new(reovim_kernel::api::v1::ModuleId::new("test"), name),
                result: CommandResult::Success,
            }
        }

        fn failing(name: &'static str) -> Self {
            Self {
                cmd_id: CommandId::new(reovim_kernel::api::v1::ModuleId::new("test"), name),
                result: CommandResult::error("test failure"),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Command for ExTestCmd {
        fn id(&self) -> CommandId {
            self.cmd_id.clone()
        }
        fn description(&self) -> &'static str {
            "test"
        }
        fn names(&self) -> &[&'static str] {
            // Determined by the actual test setup (name_index controls resolution)
            &[]
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CommandHandler for ExTestCmd {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            self.result.clone()
        }
    }

    fn create_test_context_with_name_index(name_index: CommandNameIndex) -> KernelContext {
        let services = Arc::new(ServiceRegistry::new());
        services.register(Arc::new(name_index));

        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            services,
        )
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_ex_command_success() {
        let cmd = Arc::new(ExTestCmd::success("test-cmd"));
        let cmd_handler: Arc<dyn CommandHandler> = Arc::clone(&cmd) as Arc<dyn CommandHandler>;

        // Build name index
        let mut name_index = CommandNameIndex::new();
        let cmd_as_command: Arc<dyn Command> = Arc::clone(&cmd) as Arc<dyn Command>;
        name_index.insert("test".to_string(), cmd.id(), Arc::clone(&cmd_as_command));
        name_index.insert("t".to_string(), cmd.id(), cmd_as_command);

        // Build executor
        let mut executor = TestExecutor::new();
        executor.register(cmd_handler);

        let ctx = create_test_context_with_name_index(name_index);
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime_with_executor(&ctx, &executor);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        for ch in "test".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }

        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_ex_command_not_found() {
        // Empty name index — no commands registered
        let name_index = CommandNameIndex::new();
        let ctx = create_test_context_with_name_index(name_index);
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        for ch in "nonexistent".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }

        // Should succeed (logs warning but doesn't fail)
        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_ex_command_error() {
        let cmd = Arc::new(ExTestCmd::failing("fail-cmd"));
        let cmd_handler: Arc<dyn CommandHandler> = Arc::clone(&cmd) as Arc<dyn CommandHandler>;

        let mut name_index = CommandNameIndex::new();
        let cmd_as_command: Arc<dyn Command> = Arc::clone(&cmd) as Arc<dyn Command>;
        name_index.insert("fail".to_string(), cmd.id(), cmd_as_command);

        let mut executor = TestExecutor::new();
        executor.register(cmd_handler);

        let ctx = create_test_context_with_name_index(name_index);
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime_with_executor(&ctx, &executor);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        for ch in "fail".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }

        // Should succeed (logs error but doesn't fail)
        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_search_no_search_registry() {
        // No SearchProviderRegistry registered in services
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        {
            runtime
                .ext_mut::<reovim_driver_session::api::SearchState>()
                .start_pending_search(reovim_driver_search::Direction::Forward);
            runtime
                .ext_mut::<CmdlineState>()
                .enter(CmdlinePrompt::SearchForward);
            for ch in "world".chars() {
                runtime.ext_mut::<CmdlineState>().insert_char(ch);
            }
        }

        // Should succeed even without search registry (logs warning)
        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor should not have moved
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_search_no_search_provider_for_key() {
        // Register an empty SearchProviderRegistry (no Regex key)
        let services = Arc::new(ServiceRegistry::new());
        let search_registry = Arc::new(SearchProviderRegistry::new());
        // Don't register any provider
        services.register(search_registry);

        let ctx = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            services,
        );

        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        {
            runtime
                .ext_mut::<reovim_driver_session::api::SearchState>()
                .start_pending_search(reovim_driver_search::Direction::Forward);
            runtime
                .ext_mut::<CmdlineState>()
                .enter(CmdlinePrompt::SearchForward);
            for ch in "test".chars() {
                runtime.ext_mut::<CmdlineState>().insert_char(ch);
            }
        }

        // Should succeed even without search provider (logs warning)
        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_commandline_search_buffer_not_found() {
        // Register a search provider but use a buffer_id that doesn't exist
        let mock = Arc::new(MockSearchProvider::with_no_match());
        let ctx = create_test_context_with_search(mock);
        // Don't register a buffer - use a fake buffer_id
        let fake_buffer_id = BufferId::from_raw(999);

        let mut args = CommandContext::new();
        args.set_buffer_id(fake_buffer_id);

        let mut state = TestState::with_buffer(Some(fake_buffer_id));
        let mut runtime = state.runtime(&ctx);

        {
            runtime
                .ext_mut::<reovim_driver_session::api::SearchState>()
                .start_pending_search(reovim_driver_search::Direction::Forward);
            runtime
                .ext_mut::<CmdlineState>()
                .enter(CmdlinePrompt::SearchForward);
            for ch in "test".chars() {
                runtime.ext_mut::<CmdlineState>().insert_char(ch);
            }
        }

        // Should succeed (buffer not found for search is handled gracefully)
        let result = ExitCommandLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    // ========================================================================
    // Command-line editing commands (#451)
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_cursor_left_id() {
        assert_eq!(CmdlineCursorLeft.id(), ids::CMDLINE_CURSOR_LEFT);
        assert!(CmdlineCursorLeft.description().contains("left"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_cursor_right_id() {
        assert_eq!(CmdlineCursorRight.id(), ids::CMDLINE_CURSOR_RIGHT);
        assert!(CmdlineCursorRight.description().contains("right"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_cursor_home_id() {
        assert_eq!(CmdlineCursorHome.id(), ids::CMDLINE_CURSOR_HOME);
        assert!(CmdlineCursorHome.description().contains("start"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_cursor_end_id() {
        assert_eq!(CmdlineCursorEnd.id(), ids::CMDLINE_CURSOR_END);
        assert!(CmdlineCursorEnd.description().contains("end"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_delete_char_id() {
        assert_eq!(CmdlineDeleteChar.id(), ids::CMDLINE_DELETE_CHAR);
        assert!(CmdlineDeleteChar.description().contains("cursor"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_backspace_id() {
        assert_eq!(CmdlineBackspace.id(), ids::CMDLINE_BACKSPACE);
        assert!(CmdlineBackspace.description().contains("before"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_delete_word_id() {
        assert_eq!(CmdlineDeleteWord.id(), ids::CMDLINE_DELETE_WORD);
        assert!(CmdlineDeleteWord.description().contains("word"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_delete_to_start_id() {
        assert_eq!(CmdlineDeleteToStart.id(), ids::CMDLINE_DELETE_TO_START);
        assert!(CmdlineDeleteToStart.description().contains("start"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_editing_debug_and_default() {
        let _ = format!("{:?}", CmdlineCursorLeft);
        let _ = format!("{:?}", CmdlineCursorRight);
        let _ = format!("{:?}", CmdlineCursorHome);
        let _ = format!("{:?}", CmdlineCursorEnd);
        let _ = format!("{:?}", CmdlineDeleteChar);
        let _ = format!("{:?}", CmdlineBackspace);
        let _ = format!("{:?}", CmdlineDeleteWord);
        let _ = format!("{:?}", CmdlineDeleteToStart);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_cursor_left_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        // Enter cmdline mode and type some chars
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        runtime.ext_mut::<CmdlineState>().insert_char('a');
        runtime.ext_mut::<CmdlineState>().insert_char('b');
        assert_eq!(runtime.ext_mut::<CmdlineState>().cursor(), 2);

        let result = CmdlineCursorLeft.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.ext_mut::<CmdlineState>().cursor(), 1);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_cursor_right_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        runtime.ext_mut::<CmdlineState>().insert_char('a');
        runtime.ext_mut::<CmdlineState>().move_cursor_left();

        let result = CmdlineCursorRight.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.ext_mut::<CmdlineState>().cursor(), 1);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_cursor_home_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        runtime.ext_mut::<CmdlineState>().insert_char('a');

        let result = CmdlineCursorHome.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.ext_mut::<CmdlineState>().cursor(), 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_cursor_end_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        runtime.ext_mut::<CmdlineState>().insert_char('a');
        runtime.ext_mut::<CmdlineState>().move_to_start();

        let result = CmdlineCursorEnd.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.ext_mut::<CmdlineState>().cursor(), 1);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_delete_char_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        runtime.ext_mut::<CmdlineState>().insert_char('a');
        runtime.ext_mut::<CmdlineState>().insert_char('b');
        runtime.ext_mut::<CmdlineState>().move_to_start();

        let result = CmdlineDeleteChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "b");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_backspace_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        runtime.ext_mut::<CmdlineState>().insert_char('a');
        runtime.ext_mut::<CmdlineState>().insert_char('b');

        let result = CmdlineBackspace.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "a");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_delete_word_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        for ch in "hello world".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }

        let result = CmdlineDeleteWord.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "hello ");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_history_up_id() {
        assert_eq!(CmdlineHistoryUp.id(), ids::CMDLINE_HISTORY_UP);
        assert!(CmdlineHistoryUp.description().contains("older"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_history_down_id() {
        assert_eq!(CmdlineHistoryDown.id(), ids::CMDLINE_HISTORY_DOWN);
        assert!(CmdlineHistoryDown.description().contains("newer"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_history_debug_and_default() {
        let _ = format!("{:?}", CmdlineHistoryUp);
        let _ = format!("{:?}", CmdlineHistoryDown);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_history_up_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        // Enter and add history
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        runtime.ext_mut::<CmdlineState>().insert_char('w');
        runtime.ext_mut::<CmdlineState>().push_to_history();

        // New cmdline session
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);

        let result = CmdlineHistoryUp.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "w");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_history_down_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        // Enter and add history
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        runtime.ext_mut::<CmdlineState>().insert_char('w');
        runtime.ext_mut::<CmdlineState>().push_to_history();

        // New cmdline, navigate up then down
        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        for ch in "new".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }
        runtime.ext_mut::<CmdlineState>().history_up();
        assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "w");

        let result = CmdlineHistoryDown.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "new");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_delete_to_start_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        for ch in "hello".chars() {
            runtime.ext_mut::<CmdlineState>().insert_char(ch);
        }

        let result = CmdlineDeleteToStart.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "");
        assert_eq!(runtime.ext_mut::<CmdlineState>().cursor(), 0);
    }

    // ========================================================================
    // Command-line completion commands (#451)
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_complete_next_id() {
        assert_eq!(CmdlineCompleteNext.id(), ids::CMDLINE_COMPLETE_NEXT);
        assert!(CmdlineCompleteNext.description().contains("next"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_complete_prev_id() {
        assert_eq!(CmdlineCompletePrev.id(), ids::CMDLINE_COMPLETE_PREV);
        assert!(CmdlineCompletePrev.description().contains("previous"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_completion_debug_and_default() {
        let _ = format!("{:?}", CmdlineCompleteNext);
        let _ = format!("{:?}", CmdlineCompletePrev);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_complete_next_with_name_index() {
        struct WriteCmd;
        impl Command for WriteCmd {
            fn id(&self) -> CommandId {
                CommandId::new(reovim_kernel::api::v1::ModuleId::new("test"), "write")
            }
            fn description(&self) -> &'static str {
                "Write"
            }
            fn names(&self) -> &[&'static str] {
                &["w", "write"]
            }
        }

        struct WqCmd;
        impl Command for WqCmd {
            fn id(&self) -> CommandId {
                CommandId::new(reovim_kernel::api::v1::ModuleId::new("test"), "wq")
            }
            fn description(&self) -> &'static str {
                "Write and quit"
            }
            fn names(&self) -> &[&'static str] {
                &["wq"]
            }
        }

        // Build a CommandNameIndex with "w", "write", "wq" names
        let mut name_index = CommandNameIndex::new();
        let write_cmd: Arc<dyn Command> = Arc::new(WriteCmd);
        let wq_cmd: Arc<dyn Command> = Arc::new(WqCmd);

        name_index.insert("w".to_string(), write_cmd.id(), Arc::clone(&write_cmd));
        name_index.insert("write".to_string(), write_cmd.id(), write_cmd);
        name_index.insert("wq".to_string(), wq_cmd.id(), wq_cmd);

        let ctx = create_test_context_with_name_index(name_index);
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        runtime.ext_mut::<CmdlineState>().insert_char('w');

        // First Tab populates and selects
        let result = CmdlineCompleteNext.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert!(!runtime.ext_mut::<CmdlineState>().completions().is_empty());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_complete_next_empty_input() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        // Empty input — should not populate

        let result = CmdlineCompleteNext.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert!(runtime.ext_mut::<CmdlineState>().completions().is_empty());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_complete_prev_execute() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        runtime
            .ext_mut::<CmdlineState>()
            .set_completions("w".to_string(), vec!["write".to_string(), "wq".to_string()]);

        let result = CmdlineCompletePrev.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        // Should select last item
        assert_eq!(runtime.ext_mut::<CmdlineState>().input(), "wq");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_cmdline_complete_no_registry() {
        let ctx = create_test_context(); // no CommandNameIndex
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        runtime
            .ext_mut::<CmdlineState>()
            .enter(CmdlinePrompt::Command);
        runtime.ext_mut::<CmdlineState>().insert_char('w');

        let result = CmdlineCompleteNext.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        // No registry → empty completions
        assert!(runtime.ext_mut::<CmdlineState>().completions().is_empty());
    }
}
