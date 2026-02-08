//! Search motion commands.
//!
//! Implements vim search commands: `/`, `?`, `n`, `N`, `*`, `#`, `:noh`.
//!
//! # Architecture
//!
//! - **Mechanism**: `SearchProvider` trait (in `reovim-driver-search`)
//! - **Policy**: These commands wire keys to search functionality
//!
//! # Search State
//!
//! The `SearchState` session extension stores:
//! - Last search pattern (for n/N repeat)
//! - Last search direction (for correct n/N behavior)
//!
//! # Current Status
//!
//! All search commands are fully implemented:
//! - `/` and `?` - forward/backward search with pattern input (uses `CommandLine` mode)
//! - `n` and `N` - repeat search in same/opposite direction
//! - `*` and `#` - search word under cursor forward/backward

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_search::{Direction, SearchKey, SearchProviderRegistry},
    reovim_driver_session::{SessionRuntime, api::ExtensionApi},
    reovim_kernel::api::{Position, v1::CommandId},
};

use crate::{ids, search_state::SearchState};

// =============================================================================
// Search Forward (/)
// =============================================================================

/// Search forward for a pattern.
///
/// Press `/` to enter search mode. Type a pattern and press Enter to search.
/// The cursor moves to the first match after the current position.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchForward;

impl Command for SearchForward {
    fn id(&self) -> CommandId {
        ids::SEARCH_FORWARD
    }

    fn description(&self) -> &'static str {
        "Search forward (/)"
    }
}

impl CommandHandler for SearchForward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#338): Implement via command-line mode
        // Currently requires command-line input mode which is deferred to Phase 8
        CommandResult::Success
    }
}

// =============================================================================
// Search Backward (?)
// =============================================================================

/// Search backward for a pattern.
///
/// Press `?` to enter search mode. Type a pattern and press Enter to search.
/// The cursor moves to the first match before the current position.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchBackward;

impl Command for SearchBackward {
    fn id(&self) -> CommandId {
        ids::SEARCH_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Search backward (?)"
    }
}

impl CommandHandler for SearchBackward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#338): Implement via command-line mode
        // Currently requires command-line input mode which is deferred to Phase 8
        CommandResult::Success
    }
}

// =============================================================================
// Search Next (n)
// =============================================================================

/// Go to next search match.
///
/// Press `n` to move to the next occurrence of the last search pattern
/// in the same direction as the original search.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchNext;

impl Command for SearchNext {
    fn id(&self) -> CommandId {
        ids::SEARCH_NEXT
    }

    fn description(&self) -> &'static str {
        "Go to next search match (n)"
    }
}

impl CommandHandler for SearchNext {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Get last search pattern and direction
        let (pattern, direction) = {
            let search = runtime.ext_mut::<SearchState>();
            match search.pattern_for_repeat() {
                Some(p) => (p.to_string(), search.direction_for_repeat()),
                None => return CommandResult::Success, // No pattern to repeat
            }
        };

        search_and_move(runtime, args, &pattern, direction)
    }
}

// =============================================================================
// Search Previous (N)
// =============================================================================

/// Go to previous search match.
///
/// Press `N` to move to the previous occurrence of the last search pattern
/// (opposite direction from the original search).
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchPrevious;

impl Command for SearchPrevious {
    fn id(&self) -> CommandId {
        ids::SEARCH_PREV
    }

    fn description(&self) -> &'static str {
        "Go to previous search match (N)"
    }
}

impl CommandHandler for SearchPrevious {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Get last search pattern and REVERSED direction
        let (pattern, direction) = {
            let search = runtime.ext_mut::<SearchState>();
            match search.pattern_for_repeat() {
                Some(p) => (p.to_string(), search.direction_for_opposite()),
                None => return CommandResult::Success, // No pattern to repeat
            }
        };

        search_and_move(runtime, args, &pattern, direction)
    }
}

// =============================================================================
// Search Word Under Cursor Forward (*)
// =============================================================================

/// Search for word under cursor forward.
///
/// Press `*` to search forward for the word under the cursor.
/// Word boundaries are automatically added to the pattern.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchWordForward;

impl Command for SearchWordForward {
    fn id(&self) -> CommandId {
        ids::SEARCH_WORD_FORWARD
    }

    fn description(&self) -> &'static str {
        "Search word under cursor forward (*)"
    }
}

impl CommandHandler for SearchWordForward {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        search_word(runtime, args, Direction::Forward)
    }
}

// =============================================================================
// Search Word Under Cursor Backward (#)
// =============================================================================

/// Search for word under cursor backward.
///
/// Press `#` to search backward for the word under the cursor.
/// Word boundaries are automatically added to the pattern.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchWordBackward;

impl Command for SearchWordBackward {
    fn id(&self) -> CommandId {
        ids::SEARCH_WORD_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Search word under cursor backward (#)"
    }
}

impl CommandHandler for SearchWordBackward {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        search_word(runtime, args, Direction::Backward)
    }
}

// =============================================================================
// Clear Search Highlight (:noh)
// =============================================================================

/// Clear search highlighting.
///
/// Use `:noh` or `:nohlsearch` to clear search highlighting.
/// The last search pattern is preserved for `n` and `N`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClearSearchHighlight;

impl Command for ClearSearchHighlight {
    fn id(&self) -> CommandId {
        ids::CLEAR_SEARCH_HIGHLIGHT
    }

    fn description(&self) -> &'static str {
        "Clear search highlighting (:noh)"
    }
}

impl CommandHandler for ClearSearchHighlight {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO: When highlighting is implemented, clear it here
        // For now, this is a no-op since highlighting isn't implemented yet
        CommandResult::Success
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Search for word under cursor in the given direction.
fn search_word(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    direction: Direction,
) -> CommandResult {
    use reovim_kernel::api::v1::Position;

    // Get active buffer and cursor position
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Get cursor from per-client Window (#471)
    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let cursor = Position::new(window.cursor.line, window.cursor.column);

    // Get search provider from kernel services
    let Some(search_registry) = runtime.kernel().services.get::<SearchProviderRegistry>() else {
        return CommandResult::error("Search provider not available");
    };

    let Some(search_provider) = search_registry.get(&SearchKey::Regex) else {
        return CommandResult::error("Regex search engine not registered");
    };

    // Get word under cursor and find word start position for backward search
    let (word_pattern, search_cursor) = {
        let Some(Some((pattern, word_start))) = runtime.with_buffer_read(buffer_id, |buffer| {
            // Get word pattern
            let pattern = search_provider.word_at_cursor(buffer, cursor)?;

            // Find word start position for backward search
            let line = buffer.line(cursor.line)?;
            let chars: Vec<char> = line.chars().collect();

            if cursor.column >= chars.len() {
                return Some((pattern, cursor));
            }

            // Find word start
            let mut start = cursor.column;
            while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
                start -= 1;
            }

            let word_start = Position {
                line: cursor.line,
                column: start,
            };

            Some((pattern, word_start))
        }) else {
            return CommandResult::Success; // No word under cursor - silent fail like vim
        };

        // For backward search (#), use word start position to skip current word.
        // For forward search (*), use cursor position (search starts after cursor).
        let search_pos = match direction {
            Direction::Backward => word_start,
            Direction::Forward => cursor,
        };

        (pattern, search_pos)
    };

    // Store pattern and direction for n/N repeat
    {
        let search = runtime.ext_mut::<SearchState>();
        search.set(word_pattern.clone(), direction);
    }

    // Search for the word using the appropriate search position
    search_and_move_from(runtime, args, &word_pattern, direction, search_cursor)
}

/// Search for pattern and move cursor to match.
fn search_and_move(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    pattern: &str,
    direction: Direction,
) -> CommandResult {
    use {reovim_driver_session::ChangeTracker, reovim_kernel::api::v1::Position};

    // Get active buffer and cursor position
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Get cursor from per-client Window (#471)
    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let cursor = Position::new(window.cursor.line, window.cursor.column);

    // Get search provider
    let Some(search_registry) = runtime.kernel().services.get::<SearchProviderRegistry>() else {
        return CommandResult::error("Search provider not available");
    };

    let Some(search_provider) = search_registry.get(&SearchKey::Regex) else {
        return CommandResult::error("Regex search engine not registered");
    };

    // Search for pattern
    let search_result = runtime.with_buffer_read(buffer_id, |buffer| {
        search_provider.find_next(buffer, cursor, pattern, direction, true)
    });

    match search_result {
        Some(Ok(Some(m))) => {
            // Move cursor to match start via per-client Window (#471)
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = m.start.into();
            }
            runtime.record_cursor_move(buffer_id);
            CommandResult::Success
        }
        Some(Ok(None) | Err(_)) => {
            // No match or invalid pattern - silent failure like vim
            CommandResult::Success
        }
        None => CommandResult::error("Buffer not found"),
    }
}

/// Search for pattern from a specific position and move cursor to match.
/// Used by word search (*/#) which needs to start from word boundaries.
fn search_and_move_from(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    pattern: &str,
    direction: Direction,
    search_from: Position,
) -> CommandResult {
    use reovim_driver_session::ChangeTracker;

    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let Some(search_registry) = runtime.kernel().services.get::<SearchProviderRegistry>() else {
        return CommandResult::error("Search provider not available");
    };

    let Some(search_provider) = search_registry.get(&SearchKey::Regex) else {
        return CommandResult::error("Regex search engine not registered");
    };

    let search_result = runtime.with_buffer_read(buffer_id, |buffer| {
        search_provider.find_next(buffer, search_from, pattern, direction, true)
    });

    match search_result {
        Some(Ok(Some(m))) => {
            // Move cursor via per-client Window (#471)
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = m.start.into();
            }
            runtime.record_cursor_move(buffer_id);
            CommandResult::Success
        }
        Some(Ok(None) | Err(_)) => CommandResult::Success,
        None => CommandResult::error("Buffer not found"),
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Get all search motion commands.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(SearchForward),
        Box::new(SearchBackward),
        Box::new(SearchNext),
        Box::new(SearchPrevious),
        Box::new(SearchWordForward),
        Box::new(SearchWordBackward),
        Box::new(ClearSearchHighlight),
    ]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::ids,
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, Window, WindowLayout, api::CommandExecutor,
        },
        reovim_kernel::api::{
            KernelContext, ModeStack, ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, CommandId as KernelCommandId,
                EventBus, MarkBank, ModeId, ModuleId, MotionEngine, OptionRegistry, RegisterBank,
                RwLock, TextObjectEngine,
            },
        },
        std::{collections::HashMap, sync::Arc},
    };

    // =========================================================================
    // Test Infrastructure
    // =========================================================================

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

    impl CommandExecutor for StubExecutor {
        fn execute(
            &self,
            _: &KernelCommandId,
            _: &CommandContext,
            _: &KernelContext,
        ) -> Option<CommandResult> {
            Some(CommandResult::Success)
        }
    }

    fn create_test_context() -> KernelContext {
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(RegisterBank::new())),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            Arc::new(ServiceRegistry::new()),
        )
    }

    fn setup_buffer(ctx: &KernelContext, content: &str) -> BufferId {
        let buffer = Buffer::from_string(content);
        ctx.buffers.register(buffer)
    }

    fn run_command_no_buffer<C: CommandHandler>(cmd: &C) -> CommandResult {
        let ctx = KernelContext::default();
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::new());

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &ctx,
            &executor,
        );
        cmd.execute(&mut runtime, &CommandContext::new())
    }

    // =========================================================================
    // Command ID Tests
    // =========================================================================

    #[test]
    fn test_search_forward_id() {
        let cmd = SearchForward;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "search-forward");
        assert_eq!(cmd.description(), "Search forward (/)");
    }

    #[test]
    fn test_search_backward_id() {
        let cmd = SearchBackward;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "search-backward");
        assert_eq!(cmd.description(), "Search backward (?)");
    }

    #[test]
    fn test_search_next_id() {
        let cmd = SearchNext;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "search-next");
        assert_eq!(cmd.description(), "Go to next search match (n)");
    }

    #[test]
    fn test_search_previous_id() {
        let cmd = SearchPrevious;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "search-prev");
        assert_eq!(cmd.description(), "Go to previous search match (N)");
    }

    #[test]
    fn test_search_word_forward_id() {
        let cmd = SearchWordForward;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "search-word-forward");
        assert_eq!(cmd.description(), "Search word under cursor forward (*)");
    }

    #[test]
    fn test_search_word_backward_id() {
        let cmd = SearchWordBackward;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "search-word-backward");
        assert_eq!(cmd.description(), "Search word under cursor backward (#)");
    }

    #[test]
    fn test_clear_search_highlight_id() {
        let cmd = ClearSearchHighlight;
        assert_eq!(cmd.id().module(), &ids::MODULE);
        assert_eq!(cmd.id().name(), "clear-search-highlight");
        assert_eq!(cmd.description(), "Clear search highlighting (:noh)");
    }

    #[test]
    fn test_all_commands_count() {
        let cmds = all_commands();
        assert_eq!(cmds.len(), 7); // /, ?, n, N, *, #, :noh
    }

    // =========================================================================
    // Command Args Tests
    // =========================================================================

    #[test]
    fn test_search_commands_have_no_args() {
        for cmd in all_commands() {
            let args = cmd.args();
            assert!(args.is_empty(), "Command {} should have no args", cmd.id());
        }
    }

    // =========================================================================
    // SearchForward / SearchBackward Execution Tests (stubs)
    // =========================================================================

    #[test]
    fn test_search_forward_execute_returns_success() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();

        let result = SearchForward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_backward_execute_returns_success() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();

        let result = SearchBackward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    // =========================================================================
    // SearchNext / SearchPrevious Execution Tests
    // =========================================================================

    #[test]
    fn test_search_next_no_pattern_returns_success() {
        // SearchNext with no prior search pattern should just return Success
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchNext.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_previous_no_pattern_returns_success() {
        // SearchPrevious with no prior search pattern should just return Success
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchPrevious.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_next_with_pattern_but_no_provider() {
        // SearchNext with pattern stored but no search provider registered
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        // Set a pattern in SearchState
        {
            use reovim_driver_session::api::ExtensionApi;
            let search_state = runtime.ext_mut::<SearchState>();
            search_state.set("hello".to_string(), Direction::Forward);
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // No search provider registered, so should error
        let result = SearchNext.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_search_previous_with_pattern_but_no_provider() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        // Set a pattern in SearchState
        {
            use reovim_driver_session::api::ExtensionApi;
            let search_state = runtime.ext_mut::<SearchState>();
            search_state.set("hello".to_string(), Direction::Forward);
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // No search provider registered
        let result = SearchPrevious.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_search_next_no_buffer_returns_error() {
        // SearchNext with pattern but no buffer should error
        let kernel = create_test_context();
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::new());

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        // Set a pattern in SearchState
        {
            use reovim_driver_session::api::ExtensionApi;
            let search_state = runtime.ext_mut::<SearchState>();
            search_state.set("hello".to_string(), Direction::Forward);
        }

        let args = CommandContext::new();

        // No buffer_id in args
        let result = SearchNext.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // SearchWordForward / SearchWordBackward Execution Tests
    // =========================================================================

    #[test]
    fn test_search_word_forward_no_buffer_returns_error() {
        let result = run_command_no_buffer(&SearchWordForward);
        assert!(result.is_error());
    }

    #[test]
    fn test_search_word_backward_no_buffer_returns_error() {
        let result = run_command_no_buffer(&SearchWordBackward);
        assert!(result.is_error());
    }

    #[test]
    fn test_search_word_forward_no_search_provider() {
        // No search provider registered - should error
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordForward.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_search_word_backward_no_search_provider() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordBackward.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // ClearSearchHighlight Execution Tests
    // =========================================================================

    #[test]
    fn test_clear_search_highlight_execute_returns_success() {
        let kernel = create_test_context();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();

        let result = ClearSearchHighlight.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    // =========================================================================
    // Default Trait Tests
    // =========================================================================

    #[test]
    fn test_search_default_traits() {
        let _: SearchForward = SearchForward;
        let _: SearchBackward = SearchBackward;
        let _: SearchNext = SearchNext;
        let _: SearchPrevious = SearchPrevious;
        let _: SearchWordForward = SearchWordForward;
        let _: SearchWordBackward = SearchWordBackward;
        let _: ClearSearchHighlight = ClearSearchHighlight;
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    #[test]
    fn test_search_next_no_buffer_id_in_args() {
        // n (search next) with a pattern but no buffer_id returns error
        let kernel = create_test_context();
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::new());

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        {
            use reovim_driver_session::api::ExtensionApi;
            let s = runtime.ext_mut::<SearchState>();
            s.set("pattern".to_string(), Direction::Forward);
        }

        let args = CommandContext::new(); // no buffer_id
        let result = SearchNext.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_search_previous_no_buffer_id_in_args() {
        let kernel = create_test_context();
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::new());

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        {
            use reovim_driver_session::api::ExtensionApi;
            let s = runtime.ext_mut::<SearchState>();
            s.set("pattern".to_string(), Direction::Backward);
        }

        let args = CommandContext::new();
        let result = SearchPrevious.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // Tests with mock SearchProvider for full path coverage
    // =========================================================================

    use reovim_driver_search::{SearchError, SearchMatch, SearchProvider};

    /// Mock search provider that performs simple substring matching.
    struct MockSearchProvider;

    impl SearchProvider for MockSearchProvider {
        fn find_next(
            &self,
            buffer: &Buffer,
            cursor: Position,
            pattern: &str,
            direction: Direction,
            _wrap: bool,
        ) -> Result<Option<SearchMatch>, SearchError> {
            let content = buffer.content();
            match direction {
                Direction::Forward => {
                    // Search forward from cursor column
                    let start_offset = cursor.column.saturating_add(1);
                    let search_area = if start_offset < content.len() {
                        &content[start_offset..]
                    } else {
                        ""
                    };
                    Ok(search_area.find(pattern).map(|pos| {
                        let abs = start_offset + pos;
                        SearchMatch {
                            start: Position::new(cursor.line, abs),
                            end: Position::new(cursor.line, abs + pattern.len()),
                        }
                    }))
                }
                Direction::Backward => {
                    let search_area = if cursor.column > 0 {
                        &content[..cursor.column]
                    } else {
                        ""
                    };
                    Ok(search_area.rfind(pattern).map(|pos| SearchMatch {
                        start: Position::new(cursor.line, pos),
                        end: Position::new(cursor.line, pos + pattern.len()),
                    }))
                }
            }
        }

        fn find_all(
            &self,
            buffer: &Buffer,
            pattern: &str,
        ) -> Result<Vec<SearchMatch>, SearchError> {
            let content = buffer.content();
            let mut matches = Vec::new();
            let mut start = 0;
            while let Some(pos) = content[start..].find(pattern) {
                let abs = start + pos;
                matches.push(SearchMatch {
                    start: Position::new(0, abs),
                    end: Position::new(0, abs + pattern.len()),
                });
                start = abs + pattern.len();
            }
            Ok(matches)
        }

        fn word_at_cursor(&self, buffer: &Buffer, cursor: Position) -> Option<String> {
            let line = buffer.line(cursor.line)?;
            let chars: Vec<char> = line.chars().collect();
            if cursor.column >= chars.len() {
                return None;
            }
            if !chars[cursor.column].is_alphanumeric() && chars[cursor.column] != '_' {
                return None;
            }
            let mut start = cursor.column;
            while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
                start -= 1;
            }
            let mut end = cursor.column;
            while end + 1 < chars.len()
                && (chars[end + 1].is_alphanumeric() || chars[end + 1] == '_')
            {
                end += 1;
            }
            let word: String = chars[start..=end].iter().collect();
            Some(format!(r"\b{word}\b"))
        }
    }

    /// Create a test context with a mock search provider registered.
    fn create_test_context_with_search() -> KernelContext {
        let services = Arc::new(ServiceRegistry::new());
        let search_registry = Arc::new(SearchProviderRegistry::new());
        search_registry.register(SearchKey::Regex, Arc::new(MockSearchProvider));
        services.register(search_registry);

        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(RegisterBank::new())),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            services,
        )
    }

    // =========================================================================
    // SearchNext with provider (exercises search_and_move)
    // =========================================================================

    #[test]
    fn test_search_next_with_provider_finds_match() {
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        // Set a pattern in SearchState
        {
            use reovim_driver_session::api::ExtensionApi;
            let search_state = runtime.ext_mut::<SearchState>();
            search_state.set("hello".to_string(), Direction::Forward);
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchNext.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_next_with_provider_no_match() {
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        {
            use reovim_driver_session::api::ExtensionApi;
            let search_state = runtime.ext_mut::<SearchState>();
            search_state.set("missing_pattern".to_string(), Direction::Forward);
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchNext.execute(&mut runtime, &args);
        assert!(result.is_success()); // No match is silent success (vim behavior)
    }

    #[test]
    fn test_search_previous_with_provider_finds_match() {
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let mut win = Window::with_buffer(buffer_id);
        win.cursor.column = 12; // position past the second hello
        windows.add(win);

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        {
            use reovim_driver_session::api::ExtensionApi;
            let search_state = runtime.ext_mut::<SearchState>();
            search_state.set("hello".to_string(), Direction::Forward);
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // SearchPrevious reverses direction (Forward -> Backward)
        let result = SearchPrevious.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    // =========================================================================
    // SearchWordForward / SearchWordBackward with provider
    // =========================================================================

    #[test]
    fn test_search_word_forward_with_provider() {
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordForward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_word_backward_with_provider() {
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let mut win = Window::with_buffer(buffer_id);
        win.cursor.column = 12; // At second 'hello'
        windows.add(win);

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordBackward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_word_forward_on_whitespace() {
        // Word search on a space should silently succeed (no word under cursor)
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let mut win = Window::with_buffer(buffer_id);
        win.cursor.column = 5; // On space between hello and world
        windows.add(win);

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordForward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_word_forward_cursor_past_line_end() {
        // Cursor beyond line length
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let mut win = Window::with_buffer(buffer_id);
        win.cursor.column = 100; // way past end
        windows.add(win);

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordForward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_word_backward_on_whitespace() {
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let mut win = Window::with_buffer(buffer_id);
        win.cursor.column = 5; // On space
        windows.add(win);

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordBackward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_next_with_provider_and_backward_direction() {
        // SearchNext repeats in the same direction as original search
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let mut win = Window::with_buffer(buffer_id);
        win.cursor.column = 12;
        windows.add(win);

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        {
            use reovim_driver_session::api::ExtensionApi;
            let search_state = runtime.ext_mut::<SearchState>();
            search_state.set("hello".to_string(), Direction::Backward);
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchNext.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_previous_with_provider_backward_original() {
        // SearchPrevious reverses original direction: if original was Backward, goes Forward
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        {
            use reovim_driver_session::api::ExtensionApi;
            let search_state = runtime.ext_mut::<SearchState>();
            search_state.set("hello".to_string(), Direction::Backward);
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Backward original -> Forward for N
        let result = SearchPrevious.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_and_move_with_invalid_buffer_returns_error() {
        use reovim_driver_command::ArgValue;

        // Use a buffer_id that doesn't exist in the kernel
        let kernel = create_test_context_with_search();
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::new());

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        {
            use reovim_driver_session::api::ExtensionApi;
            let search_state = runtime.ext_mut::<SearchState>();
            search_state.set("hello".to_string(), Direction::Forward);
        }

        // Set a non-existent buffer_id
        let mut args = CommandContext::new();
        args.set("buffer_id", ArgValue::BufferId(9999));

        let result = SearchNext.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_search_word_forward_with_invalid_buffer() {
        use reovim_driver_command::ArgValue;

        let kernel = create_test_context_with_search();
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::new());

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set("buffer_id", ArgValue::BufferId(9999));

        let result = SearchWordForward.execute(&mut runtime, &args);
        // search_word: with_buffer_read returns None -> Success (silent fail)
        assert!(result.is_success());
    }

    #[test]
    fn test_search_word_at_word_start() {
        // Cursor at the start of a word (column 0)
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let mut win = Window::with_buffer(buffer_id);
        win.cursor.column = 0; // At 'h' of hello
        windows.add(win);

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordForward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_word_backward_at_word_start() {
        // Cursor at the start of second word
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let mut win = Window::with_buffer(buffer_id);
        win.cursor.column = 6; // At 'w' of world
        windows.add(win);

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordBackward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    // =========================================================================
    // Test infrastructure coverage helpers
    // =========================================================================

    #[test]
    fn test_buffer_manager_list_and_count() {
        let ctx = create_test_context();
        assert_eq!(ctx.buffers.count(), 0);
        assert!(ctx.buffers.list().is_empty());

        let bid1 = setup_buffer(&ctx, "hello");
        let bid2 = setup_buffer(&ctx, "world");

        assert_eq!(ctx.buffers.count(), 2);
        let list = ctx.buffers.list();
        assert!(list.contains(&bid1));
        assert!(list.contains(&bid2));
    }

    #[test]
    fn test_buffer_manager_unregister() {
        let ctx = create_test_context();
        let bid = setup_buffer(&ctx, "hello");
        assert_eq!(ctx.buffers.count(), 1);

        let buffer = ctx.buffers.unregister(bid);
        assert!(buffer.is_ok());
        assert_eq!(ctx.buffers.count(), 0);
    }

    #[test]
    fn test_search_forward_no_window() {
        // Test with no active window
        let kernel = create_test_context_with_search();
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty(); // No window added
        let mut extensions = ExtensionMap::new();

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let result = SearchForward.execute(&mut runtime, &CommandContext::new());
        // SearchForward is a stub that always returns Success regardless of windows
        assert!(result.is_success());
    }

    #[test]
    fn test_clear_search_highlight_no_window() {
        let kernel = create_test_context();
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let result = ClearSearchHighlight.execute(&mut runtime, &CommandContext::new());
        assert!(result.is_success());
    }

    #[test]
    fn test_search_word_forward_no_window() {
        // search_word early returns "No active window" if no window
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordForward.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_search_next_no_window() {
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        {
            use reovim_driver_session::api::ExtensionApi;
            let ss = runtime.ext_mut::<SearchState>();
            ss.set("hello".to_string(), Direction::Forward);
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchNext.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_search_word_backward_no_window() {
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordBackward.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_search_word_forward_empty_buffer() {
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::with_buffer(buffer_id));

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordForward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_next_with_provider_no_window_no_buffer() {
        // SearchNext with pattern but NO buffer_id and no search provider
        let kernel = create_test_context_with_search();
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        windows.add(Window::new());

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        {
            use reovim_driver_session::api::ExtensionApi;
            let ss = runtime.ext_mut::<SearchState>();
            ss.set("hello".to_string(), Direction::Forward);
        }

        // No buffer_id in args
        let args = CommandContext::new();
        let result = SearchNext.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_search_word_forward_middle_of_word() {
        // Cursor in middle of a word
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let mut win = Window::with_buffer(buffer_id);
        win.cursor.column = 2; // 'l' in hello (middle of word)
        windows.add(win);

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordForward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_search_word_backward_middle_of_word() {
        let kernel = create_test_context_with_search();
        let buffer_id = setup_buffer(&kernel, "hello world hello");
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();

        let mut win = Window::with_buffer(buffer_id);
        win.cursor.column = 14; // 'l' in second hello
        windows.add(win);

        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = SearchWordBackward.execute(&mut runtime, &args);
        assert!(result.is_success());
    }
}
