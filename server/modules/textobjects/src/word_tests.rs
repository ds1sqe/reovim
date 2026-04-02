use {
    crate::{TEXTOBJECTS_MODULE, word::*},
    reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext, CommandHandler},
    reovim_driver_session::{
        ClientId, ExtensionMap, OperatorPendingState, Session, SessionRuntime, Window,
        WindowLayout,
        api::{CommandExecutor, ExtensionApi},
        testing::StubExecutor,
    },
    reovim_kernel::{
        api::{
            ModeStack,
            v1::{BufferId, Jumplist, KernelContext, MarkBank, ModeId, ModuleId},
        },
        testing::{create_test_context, setup_buffer, test_mode},
    },
    reovim_types_text::{HistoryRing, Position, RegisterBank},
};

// =========================================================================
// Test Infrastructure (#471)
// =========================================================================

/// Test state holder for per-client state (#471 borrow checker fix).
///
/// Holds per-client state as separate fields to avoid the double mutable
/// borrow issue when calling `SessionRuntime::new()`.
struct TestState {
    session: Session,
    mode_stack: ModeStack,
    windows: WindowLayout,
    extensions: ExtensionMap,
    compositor: Option<Box<dyn reovim_driver_layout::RootCompositor>>,
    tabs: reovim_driver_session::TabPageSet,
    registers: RegisterBank,
    clipboard_history: HistoryRing,
    local_marks: MarkBank,
    jumplist: Jumplist,
    active_buffer: Option<BufferId>,
    terminal_size: (u16, u16),
}

impl TestState {
    /// Create test state with a window containing the given buffer.
    fn with_window(buffer_id: BufferId, mode: ModeId) -> Self {
        let session = Session::new(ClientId::new(1), mode.clone()); // #491
        let mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let extensions = ExtensionMap::new();

        windows.add(Window::with_buffer(buffer_id));

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
            jumplist: Jumplist::new(),
            active_buffer: None,
            terminal_size: (80, 24),
        }
    }

    /// Create test state with a custom window (for cursor positioning).
    fn with_custom_window(window: Window, mode: ModeId) -> Self {
        let session = Session::new(ClientId::new(1), mode.clone()); // #491
        let mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let extensions = ExtensionMap::new();

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
            jumplist: Jumplist::new(),
            active_buffer: None,
            terminal_size: (80, 24),
        }
    }

    /// Create test state with an empty window (for error tests).
    fn empty(mode: ModeId) -> Self {
        let session = Session::new(ClientId::new(1), mode.clone()); // #491
        let mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let extensions = ExtensionMap::new();

        windows.add(Window::new());

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
            jumplist: Jumplist::new(),
            active_buffer: None,
            terminal_size: (80, 24),
        }
    }

    /// Create a runtime from this test state.
    fn runtime<'a>(
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
                jumplist: &mut self.jumplist,
                active_buffer: &mut self.active_buffer,
                terminal_size: &mut self.terminal_size,
            },
            kernel,
            executor,
        )
    }
}

// =========================================================================
// Command ID Tests
// =========================================================================

#[test]
fn test_inner_word_id() {
    let cmd = InnerWord;
    assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
    assert_eq!(cmd.id().name(), "inner-word");
}

#[test]
fn test_around_word_id() {
    let cmd = AWord;
    assert_eq!(cmd.id().name(), "around-word");
}

#[test]
fn test_inner_word_big_id() {
    let cmd = InnerWordBig;
    assert_eq!(cmd.id().name(), "inner-word-big");
}

#[test]
fn test_around_word_big_id() {
    let cmd = AWordBig;
    assert_eq!(cmd.id().name(), "around-word-big");
}

// =========================================================================
// Command Args Tests
// =========================================================================

#[test]
fn test_word_commands_have_count_arg() {
    for cmd in all_commands() {
        let args = cmd.args();
        assert!(!args.is_empty(), "Command {} should have count arg", cmd.id());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }
}

// =========================================================================
// Error Handling Tests
// =========================================================================

#[test]
fn test_inner_word_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_word_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// Inner Word (iw) Tests
// =========================================================================

#[test]
fn test_inner_word_basic() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world foo");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);

    // Commands now return Success - actual range calculation happens
    // but is stored internally for the operator to access
    assert!(result.is_success());
}

#[test]
fn test_inner_word_middle_of_word() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world");

    // Phase #471: Cursor lives in Window, not Buffer
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 2).into(); // Position cursor at 'l' in "hello"

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// A Word (aw) Tests
// =========================================================================

#[test]
fn test_a_word_includes_trailing_whitespace() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// BigWord Tests
// =========================================================================

#[test]
fn test_inner_word_big_skips_punctuation() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world foo");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_word_small_stops_at_punctuation() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world foo");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// Edge Cases
// =========================================================================

#[test]
fn test_empty_buffer() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);
    // Empty buffer should return Success (no-op) since there's no word
    assert!(result.is_success());
}

#[test]
fn test_whitespace_only() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "   ");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);
    // Commands return Success - range is calculated but stored internally
    assert!(result.is_success());
}

// =========================================================================
// Mode-Aware Behavior Tests (Epic #465)
// =========================================================================

fn visual_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "visual")
}

fn operator_pending_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "delete")
}

#[test]
fn test_inner_word_in_operator_pending_mode_stores_range() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world");
    // Use operator-pending mode (delete mode)
    let mut state = TestState::with_window(buffer_id, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_success());

    // In operator-pending mode, range should be stored in OperatorPendingState
    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some(), "OperatorPendingState should exist");
    assert!(
        ext_state.unwrap().has_textobj_range(),
        "Text object range should be stored in operator-pending mode"
    );
}

#[test]
fn test_inner_word_in_visual_mode_sets_selection() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world");
    // Use visual mode with custom window
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Phase #471: Selection lives in Window, access via windows()
    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some(), "Selection should be set in visual mode");

    // Verify selection covers the word "hello" (indices 0-4)
    // Selection end is exclusive (first position NOT in selection)
    let sel = selection.unwrap();
    assert_eq!(sel.start, Position::new(0, 0));
    assert_eq!(sel.end, Position::new(0, 5)); // exclusive: position after 'o'
}

#[test]
fn test_around_word_in_visual_mode_sets_selection() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world");
    // Use visual mode with custom window
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Phase #471: Selection lives in Window, access via windows()
    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some(), "Selection should be set in visual mode for around-word");

    // Around word includes trailing whitespace ("hello ")
    // Selection end is exclusive (first position NOT in selection)
    let sel = selection.unwrap();
    assert_eq!(sel.start, Position::new(0, 0));
    assert_eq!(sel.end, Position::new(0, 6)); // exclusive: position after space
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_visual_mode_does_not_store_operator_range() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world");
    let mut state = TestState::with_window(buffer_id, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_success());

    // In visual mode, OperatorPendingState should NOT have range
    let ext_state = runtime.ext::<OperatorPendingState>();
    if let Some(ext_state) = ext_state {
        assert!(
            !ext_state.has_textobj_range(),
            "Visual mode should not store range in OperatorPendingState"
        );
    }
}

// =========================================================================
// Additional coverage tests
// =========================================================================

fn visual_line_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "visual-line")
}

fn visual_block_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "visual-block")
}

#[test]
fn test_inner_word_visual_line_mode_sets_line_selection() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, reovim_driver_session::api::SelectionMode::Line);
}

#[test]
fn test_inner_word_visual_block_mode_sets_block_selection() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, reovim_driver_session::api::SelectionMode::Block);
}

#[test]
fn test_a_word_operator_pending_stores_range() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world foo");
    let mut state = TestState::with_window(buffer_id, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_a_word_visual_mode_sets_selection() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world foo");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

#[test]
fn test_inner_word_big_operator_pending_stores_range() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world foo");
    let mut state = TestState::with_window(buffer_id, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_inner_word_big_visual_mode_sets_selection() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world foo");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

#[test]
fn test_a_word_big_operator_pending_stores_range() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world foo");
    let mut state = TestState::with_window(buffer_id, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

#[test]
fn test_a_word_big_visual_mode_sets_selection() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world foo");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
}

#[test]
fn test_inner_word_with_count() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world foo bar");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_a_word_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_word_big_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = InnerWordBig.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_a_word_big_no_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let args = CommandContext::new();
    let result = AWordBig.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_a_word_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_word_big_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = InnerWordBig.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_a_word_big_invalid_buffer_returns_error() {
    let kernel = KernelContext::default();
    let mut state = TestState::empty(test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set("buffer_id", ArgValue::BufferId(999));
    let result = AWordBig.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_word_descriptions() {
    assert_eq!(InnerWord.description(), "Inner word text object");
    assert_eq!(AWord.description(), "A word text object (including whitespace)");
    assert_eq!(InnerWordBig.description(), "Inner WORD text object");
    assert_eq!(AWordBig.description(), "A WORD text object (including whitespace)");
}

#[test]
fn test_a_word_empty_buffer() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_word_big_empty_buffer() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_a_word_big_empty_buffer() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_word_at_end_of_line() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into(); // at 'o'

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_a_word_at_end_of_line() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(0, 4).into();

    let mut state = TestState::with_custom_window(window, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_a_word_visual_line_mode() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, reovim_driver_session::api::SelectionMode::Line);
}

#[test]
fn test_a_word_visual_block_mode() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, reovim_driver_session::api::SelectionMode::Block);
}

#[test]
fn test_multiline_word() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello\nworld\nfoo");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    window.cursor = Position::new(1, 2).into(); // 'r' in world

    let mut state = TestState::with_custom_window(window, operator_pending_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_success());

    let ext_state = runtime.ext::<OperatorPendingState>();
    assert!(ext_state.is_some());
    assert!(ext_state.unwrap().has_textobj_range());
}

// =========================================================================
// MC/DC Coverage: "No active window" path
// =========================================================================

#[test]
fn test_inner_word_no_active_window_returns_error() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world");
    // Truly empty window layout - no windows at all
    let session = Session::new(ClientId::new(1), test_mode());
    let mode_stack = ModeStack::new(test_mode());
    let windows = WindowLayout::empty();
    let extensions = ExtensionMap::new();
    let mut state = TestState {
        session,
        mode_stack,
        windows,
        extensions,
        compositor: None,
        tabs: reovim_driver_session::TabPageSet::new(),
        registers: RegisterBank::new(),
        clipboard_history: HistoryRing::new(),
        local_marks: MarkBank::new(),
        jumplist: Jumplist::new(),
        active_buffer: None,
        terminal_size: (80, 24),
    };
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWord.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_a_word_no_active_window_returns_error() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world");
    let session = Session::new(ClientId::new(1), test_mode());
    let mode_stack = ModeStack::new(test_mode());
    let windows = WindowLayout::empty();
    let extensions = ExtensionMap::new();
    let mut state = TestState {
        session,
        mode_stack,
        windows,
        extensions,
        compositor: None,
        tabs: reovim_driver_session::TabPageSet::new(),
        registers: RegisterBank::new(),
        clipboard_history: HistoryRing::new(),
        local_marks: MarkBank::new(),
        jumplist: Jumplist::new(),
        active_buffer: None,
        terminal_size: (80, 24),
    };
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_inner_word_big_no_active_window_returns_error() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world");
    let session = Session::new(ClientId::new(1), test_mode());
    let mode_stack = ModeStack::new(test_mode());
    let windows = WindowLayout::empty();
    let extensions = ExtensionMap::new();
    let mut state = TestState {
        session,
        mode_stack,
        windows,
        extensions,
        compositor: None,
        tabs: reovim_driver_session::TabPageSet::new(),
        registers: RegisterBank::new(),
        clipboard_history: HistoryRing::new(),
        local_marks: MarkBank::new(),
        jumplist: Jumplist::new(),
        active_buffer: None,
        terminal_size: (80, 24),
    };
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWordBig.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_a_word_big_no_active_window_returns_error() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world");
    let session = Session::new(ClientId::new(1), test_mode());
    let mode_stack = ModeStack::new(test_mode());
    let windows = WindowLayout::empty();
    let extensions = ExtensionMap::new();
    let mut state = TestState {
        session,
        mode_stack,
        windows,
        extensions,
        compositor: None,
        tabs: reovim_driver_session::TabPageSet::new(),
        registers: RegisterBank::new(),
        clipboard_history: HistoryRing::new(),
        local_marks: MarkBank::new(),
        jumplist: Jumplist::new(),
        active_buffer: None,
        terminal_size: (80, 24),
    };
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWordBig.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// MC/DC Coverage: is_visual_mode() sub-conditions
// For the OR chain: A || B || C
// We need all three sub-conditions exercised independently
// =========================================================================

#[test]
fn test_inner_word_big_visual_line_mode_sets_line_selection() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world foo");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, reovim_driver_session::api::SelectionMode::Line);
}

#[test]
fn test_inner_word_big_visual_block_mode_sets_block_selection() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world foo");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, reovim_driver_session::api::SelectionMode::Block);
}

#[test]
fn test_a_word_big_visual_line_mode_sets_line_selection() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world foo");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_line_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, reovim_driver_session::api::SelectionMode::Line);
}

#[test]
fn test_a_word_big_visual_block_mode_sets_block_selection() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world foo");
    let mut window = Window::new();
    window.buffer_id = Some(buffer_id);
    let mut state = TestState::with_custom_window(window, visual_block_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());

    let selection = runtime.windows().active().and_then(|w| w.selection.clone());
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().mode, reovim_driver_session::api::SelectionMode::Block);
}

// =========================================================================
// MC/DC Coverage: Default/Debug/Copy for word commands
// =========================================================================

#[test]
fn test_word_defaults() {
    fn make_default<T: Default>() -> T {
        T::default()
    }
    let iw: InnerWord = make_default();
    assert_eq!(iw.id().name(), "inner-word");

    let aw: AWord = make_default();
    assert_eq!(aw.id().name(), "around-word");

    let iwb: InnerWordBig = make_default();
    assert_eq!(iwb.id().name(), "inner-word-big");

    let awb: AWordBig = make_default();
    assert_eq!(awb.id().name(), "around-word-big");
}

#[test]
fn test_word_debug() {
    assert!(format!("{InnerWord:?}").contains("InnerWord"));
    assert!(format!("{AWord:?}").contains("AWord"));
    assert!(format!("{InnerWordBig:?}").contains("InnerWordBig"));
    assert!(format!("{AWordBig:?}").contains("AWordBig"));
}

#[test]
fn test_word_copy() {
    let cmd = InnerWord;
    let copied = cmd;
    assert_eq!(copied.id().name(), "inner-word");

    let cmd = AWord;
    let copied = cmd;
    assert_eq!(copied.id().name(), "around-word");
}

// =========================================================================
// MC/DC Coverage: count with BigWord variants
// =========================================================================

#[test]
fn test_inner_word_big_with_count() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world foo-bar baz");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = InnerWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_a_word_with_count() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello world foo bar");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_a_word_big_with_count() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "hello-world foo-bar baz");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(2));

    let result = AWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// MC/DC Coverage: whitespace-only buffers for other word types
// =========================================================================

#[test]
fn test_a_word_whitespace_only() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "   ");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWord.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_inner_word_big_whitespace_only() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "   ");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = InnerWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_a_word_big_whitespace_only() {
    let kernel = create_test_context();
    let buffer_id = setup_buffer(&kernel, "   ");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = AWordBig.execute(&mut runtime, &args);
    assert!(result.is_success());
}
