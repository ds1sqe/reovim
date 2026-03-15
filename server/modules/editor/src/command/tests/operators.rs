use {
    super::super::*,
    reovim_driver_command::{Command, CommandContext},
    reovim_driver_session::{
        ClientId, ExtensionMap, Session, SessionRuntime, WindowLayout, testing::StubExecutor,
    },
    reovim_kernel::{
        api::v1::{HistoryRing, Jumplist, KernelContext, MarkBank, ModeStack, RegisterBank},
        testing::test_mode,
    },
};

#[test]
fn test_enter_delete_operator_id_and_desc() {
    let cmd = EnterDeleteOperator;
    assert_eq!(cmd.id().name(), "enter-delete-operator");
    assert_eq!(cmd.description(), "Enter delete operator-pending mode");
}

#[test]
fn test_enter_yank_operator_id_and_desc() {
    let cmd = EnterYankOperator;
    assert_eq!(cmd.id().name(), "enter-yank-operator");
    assert_eq!(cmd.description(), "Enter yank operator-pending mode");
}

#[test]
fn test_enter_change_operator_id_and_desc() {
    let cmd = EnterChangeOperator;
    assert_eq!(cmd.id().name(), "enter-change-operator");
    assert_eq!(cmd.description(), "Enter change operator-pending mode");
}

#[test]
fn test_enter_indent_operator_id_and_desc() {
    let cmd = EnterIndentOperator;
    assert_eq!(cmd.id().name(), "enter-indent-operator");
    assert_eq!(cmd.description(), "Enter indent operator-pending mode");
}

#[test]
fn test_enter_dedent_operator_id_and_desc() {
    let cmd = EnterDedentOperator;
    assert_eq!(cmd.id().name(), "enter-dedent-operator");
    assert_eq!(cmd.description(), "Enter dedent operator-pending mode");
}

#[test]
fn test_enter_delete_operator_returns_success() {
    // Commands now return Success - actual operator-pending logic
    // will be handled by the vim resolver via SessionRuntime (see #394)
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone()); // #491
    let kernel = KernelContext::default();
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
    let args = CommandContext::new();
    let result = EnterDeleteOperator.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_enter_yank_operator_returns_success() {
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone()); // #491
    let kernel = KernelContext::default();
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
    let args = CommandContext::new();
    let result = EnterYankOperator.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_enter_change_operator_returns_success() {
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone()); // #491
    let kernel = KernelContext::default();
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
    let args = CommandContext::new();
    let result = EnterChangeOperator.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_enter_indent_operator_returns_success() {
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone()); // #491
    let kernel = KernelContext::default();
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
    let args = CommandContext::new();
    let result = EnterIndentOperator.execute(&mut runtime, &args);
    assert!(result.is_success());
}

#[test]
fn test_enter_dedent_operator_returns_success() {
    let mode = test_mode();
    let mut session = Session::new(ClientId::new(1), mode.clone()); // #491
    let kernel = KernelContext::default();
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
    let args = CommandContext::new();
    let result = EnterDedentOperator.execute(&mut runtime, &args);
    assert!(result.is_success());
}
