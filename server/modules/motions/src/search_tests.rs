use {
    crate::{SearchState, ids, search::*},
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_search::{Direction, SearchKey, SearchProviderRegistry},
    reovim_driver_session::{
        ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::{
            KernelContext, ModeStack,
            v1::{Buffer, Jumplist, MarkBank, ModeId, ModuleId},
        },
        testing::{create_test_context, setup_buffer},
    },
    reovim_types_text::{HistoryRing, Position, RegisterBank},
    std::sync::Arc,
};

// =========================================================================
// Test Infrastructure
// =========================================================================

fn run_command_no_buffer<C: CommandHandler>(cmd: &C) -> CommandResult {
    let ctx = KernelContext::default();
    let home_mode = ModeId::new(ModuleId::new("test"), "normal");
    let mut session = Session::new(ClientId::new(1), home_mode.clone());
    let mut mode_stack = ModeStack::new(home_mode);
    let mut windows = WindowLayout::empty();
    let mut extensions = ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::new());

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::new());

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::new());

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::new());

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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

#[cfg_attr(coverage_nightly, coverage(off))]
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

    fn find_all(&self, buffer: &Buffer, pattern: &str) -> Result<Vec<SearchMatch>, SearchError> {
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
        while end + 1 < chars.len() && (chars[end + 1].is_alphanumeric() || chars[end + 1] == '_') {
            end += 1;
        }
        let word: String = chars[start..=end].iter().collect();
        Some(format!(r"\b{word}\b"))
    }

    fn find_next_source(
        &self,
        source: &dyn reovim_driver_search::LineSource,
        cursor: Position,
        pattern: &str,
        direction: Direction,
        _wrap: bool,
    ) -> Result<Option<SearchMatch>, SearchError> {
        let content = source.content();
        match direction {
            Direction::Forward => {
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

    fn find_all_source(
        &self,
        source: &dyn reovim_driver_search::LineSource,
        pattern: &str,
    ) -> Result<Vec<SearchMatch>, SearchError> {
        let content = source.content();
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

    fn word_at_cursor_source(
        &self,
        source: &dyn reovim_driver_search::LineSource,
        cursor: Position,
    ) -> Option<String> {
        let line = source.line(cursor.line)?;
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
        while end + 1 < chars.len() && (chars[end + 1].is_alphanumeric() || chars[end + 1] == '_') {
            end += 1;
        }
        let word: String = chars[start..=end].iter().collect();
        Some(format!(r"\b{word}\b"))
    }
}

/// Create a test context with a mock search provider registered.
fn create_test_context_with_search() -> KernelContext {
    let ctx = create_test_context();
    let search_registry = Arc::new(SearchProviderRegistry::new());
    search_registry.register(SearchKey::Regex, Arc::new(MockSearchProvider));
    ctx.services.register(search_registry);
    ctx
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut win = Window::with_buffer(buffer_id);
    win.cursor.column = 12; // position past the second hello
    windows.add(win);

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut win = Window::with_buffer(buffer_id);
    win.cursor.column = 12; // At second 'hello'
    windows.add(win);

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut win = Window::with_buffer(buffer_id);
    win.cursor.column = 5; // On space between hello and world
    windows.add(win);

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut win = Window::with_buffer(buffer_id);
    win.cursor.column = 100; // way past end
    windows.add(win);

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut win = Window::with_buffer(buffer_id);
    win.cursor.column = 5; // On space
    windows.add(win);

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut win = Window::with_buffer(buffer_id);
    win.cursor.column = 12;
    windows.add(win);

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::new());

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::new());

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut win = Window::with_buffer(buffer_id);
    win.cursor.column = 0; // At 'h' of hello
    windows.add(win);

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut win = Window::with_buffer(buffer_id);
    win.cursor.column = 6; // At 'w' of world
    windows.add(win);

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    assert!(buffer.is_some());
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::with_buffer(buffer_id));

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    windows.add(Window::new());

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut win = Window::with_buffer(buffer_id);
    win.cursor.column = 2; // 'l' in hello (middle of word)
    windows.add(win);

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
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
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    let mut win = Window::with_buffer(buffer_id);
    win.cursor.column = 14; // 'l' in second hello
    windows.add(win);

    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let executor = StubExecutor;
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = SearchWordBackward.execute(&mut runtime, &args);
    assert!(result.is_success());
}
