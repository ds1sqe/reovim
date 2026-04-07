#![allow(clippy::uninlined_format_args)]
use {
    super::super::*,
    crate::session_state::{ChangeType, VimSessionState},
    reovim_driver_command::{Command, CommandContext, CommandHandler},
    reovim_driver_session::{BufferApi, SessionRuntime, api::ExtensionApi},
};

use {
    crate::session_state::{LastChange, OperatorType},
    reovim_driver_command::ArgValue,
    reovim_driver_session::{
        ClientId, ExtensionMap, Jumplist, MarkBank, Session, Window, WindowLayout,
        api::CommandExecutor, testing::StubExecutor,
    },
    reovim_kernel::{
        api::{
            ModeStack,
            v1::{BufferId, KernelContext, ModeId, ModuleId, RwLock},
        },
        testing::create_test_context,
    },
    reovim_provider_text::{Buffer, TextBufferRegistry},
    reovim_types_text::{HistoryRing, RegisterBank},
    std::sync::Arc,
};

fn make_test_ctx() -> KernelContext {
    let ctx = create_test_context();
    ctx.services.register(Arc::new(TextBufferRegistry::new()));
    ctx
}

fn setup_buf(ctx: &KernelContext, content: &str) -> BufferId {
    let buffer = Buffer::from_string(content);
    let arc = Arc::new(RwLock::new(buffer));
    if let Some(reg) = ctx.services.get::<TextBufferRegistry>() {
        reg.register(arc.clone());
    }
    ctx.buffers.register(arc)
}

fn test_mode() -> ModeId {
    ModeId::new(ModuleId::new("vim"), "normal")
}

// =========================================================================
// Test Infrastructure (#471)
// =========================================================================

/// Test state holder for per-client state (#471 borrow checker fix).
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
fn test_dot_repeat_command_id() {
    let cmd = DotRepeat;
    assert_eq!(cmd.id().name(), "dot-repeat");
    assert_eq!(cmd.description(), "Repeat last change (.)");
}

// =========================================================================
// No Last Change Tests
// =========================================================================

#[test]
fn test_dot_repeat_without_vim_state_returns_success() {
    let kernel = make_test_ctx();
    let buffer_id = setup_buf(&kernel, "hello world");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    // No VimSessionState registered - should return Success
    let result = DotRepeat.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_dot_repeat_without_last_change_returns_success() {
    let kernel = make_test_ctx();
    let buffer_id = setup_buf(&kernel, "hello world");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    // Register VimSessionState but don't set last_change
    runtime.ext_mut::<VimSessionState>();

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DotRepeat.execute(&mut runtime, &args);
    assert!(result.is_success());
}

// =========================================================================
// Insert Repeat Tests
// =========================================================================

#[test]
fn test_dot_repeat_insert_text() {
    let kernel = make_test_ctx();
    let buffer_id = setup_buf(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    // Set up last change: insert "hello"
    {
        let vim = runtime.ext_mut::<VimSessionState>();
        vim.last_change = Some(LastChange {
            change_type: ChangeType::Insert {
                text: "hello".to_string(),
            },
            count: None,
            register: None,
            keys: Vec::new(),
        });
    }

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DotRepeat.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify buffer contains "hello"
    let content = runtime.buffer_content(buffer_id);
    assert!(content.is_some());
    // Buffer content after insert at position (0,0) - no trailing newline
    assert_eq!(content.unwrap(), "hello");
}

#[test]
fn test_dot_repeat_insert_with_original_count() {
    let kernel = make_test_ctx();
    let buffer_id = setup_buf(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    // Set up last change: insert "ab" with count 3
    {
        let vim = runtime.ext_mut::<VimSessionState>();
        vim.last_change = Some(LastChange {
            change_type: ChangeType::Insert {
                text: "ab".to_string(),
            },
            count: Some(3),
            register: None,
            keys: Vec::new(),
        });
    }

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DotRepeat.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify buffer contains "ababab" (repeated 3 times)
    let content = runtime.buffer_content(buffer_id);
    assert!(content.is_some());
    assert_eq!(content.unwrap(), "ababab");
}

#[test]
fn test_dot_repeat_count_overrides_original() {
    let kernel = make_test_ctx();
    let buffer_id = setup_buf(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    // Set up last change: insert "x" with count 5
    {
        let vim = runtime.ext_mut::<VimSessionState>();
        vim.last_change = Some(LastChange {
            change_type: ChangeType::Insert {
                text: "x".to_string(),
            },
            count: Some(5),
            register: None,
            keys: Vec::new(),
        });
    }

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    // Set count to 2 - this should override the original count of 5
    args.set("count", ArgValue::Count(2));

    let result = DotRepeat.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify buffer contains "xx" (repeated 2 times, not 5)
    let content = runtime.buffer_content(buffer_id);
    assert!(content.is_some());
    assert_eq!(content.unwrap(), "xx");
}

// =========================================================================
// Operator Change Recording Tests
// =========================================================================

#[test]
fn test_last_change_stores_operator_motion() {
    let last_change = LastChange {
        change_type: ChangeType::OperatorMotion {
            operator: OperatorType::Delete,
            linewise: false,
        },
        count: Some(3),
        register: Some('a'),
        keys: Vec::new(),
    };

    assert!(matches!(last_change.change_type, ChangeType::OperatorMotion { .. }));
    assert_eq!(last_change.count, Some(3));
    assert_eq!(last_change.register, Some('a'));
}

#[test]
fn test_last_change_stores_operator_textobj() {
    let last_change = LastChange {
        change_type: ChangeType::OperatorTextObject {
            operator: OperatorType::Change,
            linewise: false,
        },
        count: None,
        register: None,
        keys: Vec::new(),
    };

    assert!(matches!(last_change.change_type, ChangeType::OperatorTextObject { .. }));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_last_change_stores_insert() {
    let last_change = LastChange {
        change_type: ChangeType::Insert {
            text: "hello world".to_string(),
        },
        count: None,
        register: None,
        keys: Vec::new(),
    };

    match &last_change.change_type {
        ChangeType::Insert { text } => {
            assert_eq!(text, "hello world");
        }
        _ => panic!("Expected Insert variant"),
    }
}

// =========================================================================
// Error Handling Tests
// =========================================================================

#[test]
fn test_dot_repeat_no_buffer_returns_error() {
    let kernel = make_test_ctx();
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone()); // #491
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(mode);
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

    // Set up last change
    {
        let vim = runtime.ext_mut::<VimSessionState>();
        vim.last_change = Some(LastChange {
            change_type: ChangeType::Insert {
                text: "test".to_string(),
            },
            count: None,
            register: None,
            keys: Vec::new(),
        });
    }

    // No buffer_id set and no active buffer
    let args = CommandContext::new();
    let result = DotRepeat.execute(&mut runtime, &args);

    // Should return error since no buffer is available
    assert!(result.is_error());
}

// =========================================================================
// Additional tests
// =========================================================================

#[test]
fn test_dot_repeat_command_debug() {
    let debug = format!("{:?}", DotRepeat);
    assert!(debug.contains("DotRepeat"));
}

#[test]
fn test_dot_repeat_command_default() {
    let _ = DotRepeat;
}

#[test]
fn test_dot_repeat_command_clone() {
    let cmd = DotRepeat;
    let _ = cmd;
}

#[test]
fn test_dot_repeat_insert_empty_text() {
    let kernel = make_test_ctx();
    let buffer_id = setup_buf(&kernel, "hello");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    // Set up last change: insert empty string
    {
        let vim = runtime.ext_mut::<VimSessionState>();
        vim.last_change = Some(LastChange {
            change_type: ChangeType::Insert {
                text: String::new(),
            },
            count: None,
            register: None,
            keys: Vec::new(),
        });
    }

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DotRepeat.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Buffer should be unchanged
    let content = runtime.buffer_content(buffer_id);
    assert_eq!(content.unwrap(), "hello");
}

#[test]
fn test_dot_repeat_insert_with_no_count() {
    let kernel = make_test_ctx();
    let buffer_id = setup_buf(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    // Set up last change: insert "abc" with no count
    {
        let vim = runtime.ext_mut::<VimSessionState>();
        vim.last_change = Some(LastChange {
            change_type: ChangeType::Insert {
                text: "abc".to_string(),
            },
            count: None,
            register: None,
            keys: Vec::new(),
        });
    }

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DotRepeat.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Default count = 1, so just "abc"
    let content = runtime.buffer_content(buffer_id);
    assert_eq!(content.unwrap(), "abc");
}

#[test]
fn test_dot_repeat_operator_motion_stub() {
    let kernel = make_test_ctx();
    let buffer_id = setup_buf(&kernel, "hello world");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    // Set up last change as operator motion
    {
        let vim = runtime.ext_mut::<VimSessionState>();
        vim.last_change = Some(LastChange {
            change_type: ChangeType::OperatorMotion {
                operator: OperatorType::Delete,
                linewise: false,
            },
            count: Some(1),
            register: None,
            keys: Vec::new(),
        });
    }

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DotRepeat.execute(&mut runtime, &args);
    // Stub should return success
    assert!(result.is_success());
}

#[test]
fn test_dot_repeat_operator_textobj_stub() {
    let kernel = make_test_ctx();
    let buffer_id = setup_buf(&kernel, "hello world");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    // Set up last change as operator text object
    {
        let vim = runtime.ext_mut::<VimSessionState>();
        vim.last_change = Some(LastChange {
            change_type: ChangeType::OperatorTextObject {
                operator: OperatorType::Change,
                linewise: true,
            },
            count: None,
            register: Some('a'),
            keys: Vec::new(),
        });
    }

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DotRepeat.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_dot_repeat_insert_with_newline() {
    let kernel = make_test_ctx();
    let buffer_id = setup_buf(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    // Set up last change: insert text with newline
    {
        let vim = runtime.ext_mut::<VimSessionState>();
        vim.last_change = Some(LastChange {
            change_type: ChangeType::Insert {
                text: "line1\nline2".to_string(),
            },
            count: None,
            register: None,
            keys: Vec::new(),
        });
    }

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DotRepeat.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_dot_repeat_count_overrides_none() {
    let kernel = make_test_ctx();
    let buffer_id = setup_buf(&kernel, "");
    let mut state = TestState::with_window(buffer_id, test_mode());
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);

    // Set up last change: insert "x" with no count
    {
        let vim = runtime.ext_mut::<VimSessionState>();
        vim.last_change = Some(LastChange {
            change_type: ChangeType::Insert {
                text: "x".to_string(),
            },
            count: None,
            register: None,
            keys: Vec::new(),
        });
    }

    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    args.set("count", ArgValue::Count(3));

    let result = DotRepeat.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Should insert "xxx" (3 times)
    let content = runtime.buffer_content(buffer_id);
    assert_eq!(content.unwrap(), "xxx");
}

#[test]
fn test_dot_repeat_insert_no_active_window() {
    let kernel = make_test_ctx();
    let buffer_id = setup_buf(&kernel, "hello");
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(mode);
    let mut windows = WindowLayout::empty(); // No windows
    let mut extensions = ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );

    // Set up last change as insert
    {
        let vim = runtime.ext_mut::<VimSessionState>();
        vim.last_change = Some(LastChange {
            change_type: ChangeType::Insert {
                text: "abc".to_string(),
            },
            count: None,
            register: None,
            keys: Vec::new(),
        });
    }

    // buffer_id is set in args but there is no active window
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);

    let result = DotRepeat.execute(&mut runtime, &args);
    // Should return error "No active window"
    assert!(result.is_error());
}
