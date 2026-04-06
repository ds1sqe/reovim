use {
    crate::{ids, screen::*},
    reovim_driver_command::{
        ArgKind, ArgValue, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{
        ClientId, ExtensionMap, Jumplist, MarkBank, Session, SessionRuntime, Window, WindowLayout,
        testing::{StubExecutor, create_test_kernel, dual_register_buffer},
    },
    reovim_kernel::{
        api::{
            KernelContext, ModeStack,
            v1::{BufferId, ModeId, ModuleId},
        },
    },
    reovim_types_text::{HistoryRing, Position, RegisterBank},
};

// =========================================================================
// Test Infrastructure
// =========================================================================

struct TestSetup {
    ctx: KernelContext,
    session: Session,
    mode_stack: ModeStack,
    windows: WindowLayout,
    extensions: ExtensionMap,
    compositor: Option<Box<dyn reovim_driver_layout::RootCompositor>>,
    registers: RegisterBank,
    clipboard_history: HistoryRing,
    local_marks: MarkBank,
    jumplist: Jumplist,
    active_buffer: Option<BufferId>,
    terminal_size: (u16, u16),
    buffer_id: BufferId,
}

impl TestSetup {
    fn new(content: &str) -> Self {
        let ctx = create_test_kernel();
        let buffer_id = dual_register_buffer(&ctx, content);

        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let session = Session::new(ClientId::new(1), home_mode.clone());

        let mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let extensions = ExtensionMap::new();

        let mut window = Window::new();
        window.buffer_id = Some(buffer_id);
        windows.add(window);

        Self {
            ctx,
            session,
            mode_stack,
            windows,
            extensions,
            compositor: None,
            registers: RegisterBank::new(),
            clipboard_history: HistoryRing::new(),
            local_marks: MarkBank::new(),
            jumplist: Jumplist::new(),
            active_buffer: None,
            terminal_size: (80, 24),
            buffer_id,
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn set_cursor(&mut self, pos: Position) {
        if let Some(window) = self.windows.active_mut() {
            window.cursor = pos.into();
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn cursor(&self) -> Position {
        self.windows
            .active()
            .map_or_else(|| Position::new(0, 0), |w| Position::new(w.cursor.line, w.cursor.column))
    }

    fn args(&self) -> CommandContext {
        let mut args = CommandContext::new();
        args.set_buffer_id(self.buffer_id);
        args
    }

    fn args_with_count(&self, count: usize) -> CommandContext {
        let mut args = self.args();
        args.set("count", ArgValue::Count(count));
        args
    }

    fn set_viewport(&mut self, scroll_top: usize, height: u16) {
        if let Some(window) = self.windows.active_mut() {
            window.viewport.scroll_top = scroll_top;
            window.viewport.height = height;
        }
    }

    fn run<C: CommandHandler>(&mut self, cmd: &C, args: &CommandContext) -> CommandResult {
        let executor = StubExecutor;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut runtime = SessionRuntime::new(
            &mut self.session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut self.mode_stack,
                windows: &mut self.windows,
                extensions: &mut self.extensions,
                compositor: &mut self.compositor,
                tabs: &mut tabs,
                registers: &mut self.registers,
                clipboard_history: &mut self.clipboard_history,
                local_marks: &mut self.local_marks,
                jumplist: &mut self.jumplist,
                active_buffer: &mut self.active_buffer,
                terminal_size: &mut self.terminal_size,
            },
            &self.ctx,
            &executor,
        );
        cmd.execute(&mut runtime, args)
    }
}

fn run_command_no_buffer<C: CommandHandler>(cmd: &C) -> CommandResult {
    let ctx = KernelContext::default();
    let home_mode = ModeId::new(ModuleId::new("test"), "normal");
    let mut session = Session::new(ClientId::new(1), home_mode.clone());

    let mut mode_stack = ModeStack::new(home_mode);
    let mut windows = WindowLayout::empty();
    let mut extensions = ExtensionMap::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    windows.add(Window::new());

    let executor = StubExecutor;
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
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

/// Generate 50 lines of content for viewport testing.
fn fifty_lines() -> String {
    (0..50)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

// =========================================================================
// ScreenHigh (H) — Command Identity
// =========================================================================

#[test]
fn test_screen_high_id() {
    let cmd = ScreenHigh;
    assert_eq!(cmd.id().module(), &ids::MODULE);
    assert_eq!(cmd.id().name(), "screen-high");
}

#[test]
fn test_screen_high_description() {
    assert!(!ScreenHigh.description().is_empty());
}

#[test]
fn test_screen_high_has_count_arg() {
    let args = ScreenHigh.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_screen_high_debug() {
    assert!(!format!("{ScreenHigh:?}").is_empty());
}

// =========================================================================
// ScreenHigh (H) — Behavior
// =========================================================================

#[test]
fn test_screen_high_basic() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(0, 24);
    setup.set_cursor(Position::new(10, 5));

    let args = setup.args();
    let result = setup.run(&ScreenHigh, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 0);
    assert_eq!(setup.cursor().column, 0);
}

#[test]
fn test_screen_high_with_count() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(0, 24);
    setup.set_cursor(Position::new(10, 0));

    let args = setup.args_with_count(3);
    let result = setup.run(&ScreenHigh, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 2);
}

#[test]
fn test_screen_high_with_scroll() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(10, 24);
    setup.set_cursor(Position::new(20, 0));

    let args = setup.args();
    let result = setup.run(&ScreenHigh, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 10);
}

#[test]
fn test_screen_high_with_count_and_scroll() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(10, 24);
    setup.set_cursor(Position::new(20, 0));

    let args = setup.args_with_count(3);
    let result = setup.run(&ScreenHigh, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 12);
}

#[test]
fn test_screen_high_clamped_to_buffer_end() {
    let mut setup = TestSetup::new("line1\nline2\nline3\nline4\nline5");
    setup.set_viewport(0, 24);
    setup.set_cursor(Position::new(2, 0));

    let args = setup.args_with_count(100);
    let result = setup.run(&ScreenHigh, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 4); // Clamped to last line
}

#[test]
fn test_screen_high_already_at_target() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(0, 24);
    setup.set_cursor(Position::new(0, 0));

    let args = setup.args();
    let result = setup.run(&ScreenHigh, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 0); // No-op
}

#[test]
fn test_screen_high_no_buffer() {
    let result = run_command_no_buffer(&ScreenHigh);
    assert!(result.is_error());
}

// =========================================================================
// ScreenMiddle (M) — Command Identity
// =========================================================================

#[test]
fn test_screen_middle_id() {
    let cmd = ScreenMiddle;
    assert_eq!(cmd.id().name(), "screen-middle");
}

#[test]
fn test_screen_middle_description() {
    assert!(!ScreenMiddle.description().is_empty());
}

#[test]
fn test_screen_middle_no_args() {
    assert!(ScreenMiddle.args().is_empty());
}

#[test]
fn test_screen_middle_debug() {
    assert!(!format!("{ScreenMiddle:?}").is_empty());
}

// =========================================================================
// ScreenMiddle (M) — Behavior
// =========================================================================

#[test]
fn test_screen_middle_basic() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(0, 24);
    setup.set_cursor(Position::new(0, 5));

    let args = setup.args();
    let result = setup.run(&ScreenMiddle, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 12); // 24 / 2 = 12
    assert_eq!(setup.cursor().column, 0);
}

#[test]
fn test_screen_middle_with_scroll() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(10, 20);
    setup.set_cursor(Position::new(15, 0));

    let args = setup.args();
    let result = setup.run(&ScreenMiddle, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 20); // 10 + 20/2 = 20
}

#[test]
fn test_screen_middle_clamped_to_buffer_end() {
    let mut setup = TestSetup::new("line1\nline2\nline3");
    setup.set_viewport(0, 100);
    setup.set_cursor(Position::new(0, 0));

    let args = setup.args();
    let result = setup.run(&ScreenMiddle, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 2); // Clamped to last line (3 lines total)
}

#[test]
fn test_screen_middle_already_at_target() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(0, 24);
    setup.set_cursor(Position::new(12, 0));

    let args = setup.args();
    let result = setup.run(&ScreenMiddle, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 12); // No-op
}

#[test]
fn test_screen_middle_no_buffer() {
    let result = run_command_no_buffer(&ScreenMiddle);
    assert!(result.is_error());
}

// =========================================================================
// ScreenLow (L) — Command Identity
// =========================================================================

#[test]
fn test_screen_low_id() {
    let cmd = ScreenLow;
    assert_eq!(cmd.id().name(), "screen-low");
}

#[test]
fn test_screen_low_description() {
    assert!(!ScreenLow.description().is_empty());
}

#[test]
fn test_screen_low_has_count_arg() {
    let args = ScreenLow.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].kind, ArgKind::Count);
}

#[test]
fn test_screen_low_debug() {
    assert!(!format!("{ScreenLow:?}").is_empty());
}

// =========================================================================
// ScreenLow (L) — Behavior
// =========================================================================

#[test]
fn test_screen_low_basic() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(0, 24);
    setup.set_cursor(Position::new(5, 3));

    let args = setup.args();
    let result = setup.run(&ScreenLow, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 23); // 0 + 24 - 1 = 23
    assert_eq!(setup.cursor().column, 0);
}

#[test]
fn test_screen_low_with_count() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(0, 24);
    setup.set_cursor(Position::new(5, 0));

    let args = setup.args_with_count(3);
    let result = setup.run(&ScreenLow, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 21); // 0 + 24 - 3 = 21
}

#[test]
fn test_screen_low_with_scroll() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(10, 20);
    setup.set_cursor(Position::new(15, 0));

    let args = setup.args();
    let result = setup.run(&ScreenLow, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 29); // 10 + 20 - 1 = 29
}

#[test]
fn test_screen_low_clamped_to_buffer_end() {
    let mut setup = TestSetup::new("line1\nline2\nline3\nline4\nline5");
    setup.set_viewport(0, 100);
    setup.set_cursor(Position::new(0, 0));

    let args = setup.args();
    let result = setup.run(&ScreenLow, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 4); // Clamped to last line
}

#[test]
fn test_screen_low_count_larger_than_height() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(5, 24);
    setup.set_cursor(Position::new(20, 0));

    let args = setup.args_with_count(100);
    let result = setup.run(&ScreenLow, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 5); // Clamped to scroll_top
}

#[test]
fn test_screen_low_already_at_target() {
    let mut setup = TestSetup::new(&fifty_lines());
    setup.set_viewport(0, 24);
    setup.set_cursor(Position::new(23, 0));

    let args = setup.args();
    let result = setup.run(&ScreenLow, &args);
    assert!(result.is_success());
    assert_eq!(setup.cursor().line, 23); // No-op
}

#[test]
fn test_screen_low_no_buffer() {
    let result = run_command_no_buffer(&ScreenLow);
    assert!(result.is_error());
}

// =========================================================================
// all_commands
// =========================================================================

#[test]
fn test_screen_all_commands_count() {
    assert_eq!(all_commands().len(), 3);
}
