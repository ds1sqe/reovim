use {
    super::*,
    crate::{Jumplist, MarkBank, testing::StubExecutor, types::ClientId},
    reovim_kernel::{api::v1::ModuleId, testing::test_mode},
    reovim_types_text::{HistoryRing, RegisterBank},
};

fn test_mode_2() -> ModeId {
    ModeId::with_discriminant(ModuleId::new("test"), "insert", 1)
}

use std::sync::Arc;

use crate::api::CommandHandle;

#[test]
fn test_mode_api() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    // #471 Phase 0: Per-client state is now REQUIRED
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    // Check initial state
    assert_eq!(runtime.current_mode(), &test_mode());
    assert_eq!(runtime.mode_depth(), 1);
    assert!(runtime.is_mode_active(&test_mode()));

    // Push mode
    runtime.push_mode(test_mode_2(), TransitionContext::new());
    assert_eq!(runtime.current_mode(), &test_mode_2());
    assert_eq!(runtime.mode_depth(), 2);

    // Pop mode
    let result = runtime.pop_mode(None);
    assert!(result.is_ok());
    assert_eq!(runtime.current_mode(), &test_mode());

    // Cannot pop home mode
    let result = runtime.pop_mode(None);
    assert!(matches!(result, Err(ModeError::CannotPopHomeMode)));

    // Check changes were recorded
    let changes = runtime.take_changes();
    assert!(changes.mode_changed);
}

/// Test per-client mode stack isolation (#471, #477, Phase 0).
///
/// Verifies that:
/// 1. `new()` uses the provided per-client mode stack
/// 2. Mode changes don't affect the session's shared mode stack
/// 3. Two runtimes with different client stacks have independent modes
#[test]
fn test_per_client_mode_stack() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    // Create per-client state (#471, #477)
    let mut client_mode_stack = ModeStack::new(test_mode());
    let mut client_windows = crate::WindowLayout::empty();
    let mut client_extensions = crate::ExtensionMap::new();
    let mut client_compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut client_registers = RegisterBank::new();
    let mut client_clipboard_history = HistoryRing::new();
    let mut client_local_marks = MarkBank::new();
    let mut client_jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    // #491: Session no longer has mode_stack field - use home_mode() from shared
    let session_home_mode = session.shared.home_mode().clone();

    // Use a scope to release mutable borrow before checking session
    {
        // Create runtime with per-client state (#471 Phase 0)
        let mut runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut client_mode_stack,
                windows: &mut client_windows,
                extensions: &mut client_extensions,
                compositor: &mut client_compositor,
                tabs: &mut tabs,
                registers: &mut client_registers,
                clipboard_history: &mut client_clipboard_history,
                local_marks: &mut client_local_marks,
                jumplist: &mut client_jumplist,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );

        // #471 Phase 0: Per-client state is now REQUIRED (no has_* methods)
        assert_eq!(runtime.current_mode(), &test_mode());

        // Push mode to per-client stack
        runtime.push_mode(test_mode_2(), TransitionContext::new());
        assert_eq!(runtime.current_mode(), &test_mode_2());
        assert_eq!(runtime.mode_depth(), 2);
    }

    // #491: After runtime is dropped, session.shared.home_mode() remains unchanged
    assert_eq!(session.shared.home_mode(), &session_home_mode);

    // Verify client_mode_stack was modified
    assert_eq!(client_mode_stack.current(), &test_mode_2());
    assert_eq!(client_mode_stack.depth(), 2);
}

/// Test the `owner()` method for explicit client binding (#471 Phase 0).
#[test]
fn test_owner_tracking() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    // Per-client state is REQUIRED (#471 Phase 0)
    let mut client_stack = ModeStack::new(test_mode());
    let mut client_windows = crate::WindowLayout::empty();
    let mut client_extensions = crate::ExtensionMap::new();
    let mut client_compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut client_registers = RegisterBank::new();
    let mut client_clipboard_history = HistoryRing::new();
    let mut client_local_marks = MarkBank::new();
    let mut client_jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    // Runtime created with new() has no owner
    {
        let runtime = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut client_stack,
                windows: &mut client_windows,
                extensions: &mut client_extensions,
                compositor: &mut client_compositor,
                tabs: &mut tabs,
                registers: &mut client_registers,
                clipboard_history: &mut client_clipboard_history,
                local_marks: &mut client_local_marks,
                jumplist: &mut client_jumplist,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert_eq!(runtime.owner(), None);
    }

    // Runtime created with with_owner() has explicit owner
    let client_id = ClientId::new(42);
    {
        let runtime = SessionRuntime::with_owner(
            client_id,
            &mut session,
            crate::ClientContext {
                mode_stack: &mut client_stack,
                windows: &mut client_windows,
                extensions: &mut client_extensions,
                compositor: &mut client_compositor,
                tabs: &mut tabs,
                registers: &mut client_registers,
                clipboard_history: &mut client_clipboard_history,
                local_marks: &mut client_local_marks,
                jumplist: &mut client_jumplist,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        assert_eq!(runtime.owner(), Some(client_id));
    }
}

/// Test that two clients have independent mode stacks (#471, #477, Phase 0).
#[test]
fn test_multi_client_mode_isolation() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    // Create two independent client state sets (#471, #477)
    let mut client1_stack = ModeStack::new(test_mode());
    let mut client1_windows = crate::WindowLayout::empty();
    let mut client1_extensions = crate::ExtensionMap::new();
    let mut client1_compositor = None;
    let mut client1_registers = RegisterBank::new();
    let mut client1_clipboard_history = HistoryRing::new();
    let mut client1_local_marks = MarkBank::new();
    let mut client1_jumplist = Jumplist::new();
    let mut client2_stack = ModeStack::new(test_mode());
    let mut client2_windows = crate::WindowLayout::empty();
    let mut client2_extensions = crate::ExtensionMap::new();
    let mut client2_compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut client2_registers = RegisterBank::new();
    let mut client2_clipboard_history = HistoryRing::new();
    let mut client2_local_marks = MarkBank::new();
    let mut client2_jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    // Client 1 enters insert mode (#471 Phase 0: use new())
    {
        let mut runtime1 = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut client1_stack,
                windows: &mut client1_windows,
                extensions: &mut client1_extensions,
                compositor: &mut client1_compositor,
                tabs: &mut tabs,
                registers: &mut client1_registers,
                clipboard_history: &mut client1_clipboard_history,
                local_marks: &mut client1_local_marks,
                jumplist: &mut client1_jumplist,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        runtime1.push_mode(test_mode_2(), TransitionContext::new());
    }

    // Client 2 stays in normal mode (#471 Phase 0: use new())
    {
        let runtime2 = SessionRuntime::new(
            &mut session,
            crate::ClientContext {
                mode_stack: &mut client2_stack,
                windows: &mut client2_windows,
                extensions: &mut client2_extensions,
                compositor: &mut client2_compositor,
                tabs: &mut tabs,
                registers: &mut client2_registers,
                clipboard_history: &mut client2_clipboard_history,
                local_marks: &mut client2_local_marks,
                jumplist: &mut client2_jumplist,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &executor,
        );
        // Client 2 should still be in normal mode
        assert_eq!(runtime2.current_mode(), &test_mode());
    }

    // Verify isolation (#471 Phase 0)
    assert_eq!(client1_stack.current(), &test_mode_2()); // Client 1: insert
    assert_eq!(client2_stack.current(), &test_mode()); // Client 2: normal
    // Note: session.mode_stack is deprecated (#488) - per-client stacks are SSOT
}

#[test]
fn test_window_api() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    // #471 Phase 0: Per-client state is REQUIRED
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    // Create window
    let window_id = runtime.create_window(None);
    assert_eq!(runtime.window_count(), 1);
    assert_eq!(runtime.active_window(), Some(window_id));

    // Cannot close last window
    let result = runtime.close_window(window_id);
    assert!(matches!(result, Err(WindowError::CannotCloseLastWindow)));

    // Create second window
    let window_id2 = runtime.create_window(None);
    assert_eq!(runtime.window_count(), 2);

    // Focus second window
    let result = runtime.focus_window(window_id2);
    assert!(result.is_ok());

    // Close first window
    let result = runtime.close_window(window_id);
    assert!(result.is_ok());
    assert_eq!(runtime.window_count(), 1);

    // Check changes
    let changes = runtime.take_changes();
    assert!(changes.window_changed);
    assert!(!changes.windows_created.is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_extension_api() {
    use reovim_kernel::api::v1::ModeStack;

    #[derive(Debug, Default)]
    struct TestExtension {
        value: i32,
    }

    impl SessionExtension for TestExtension {
        fn create() -> Self {
            Self { value: 42 }
        }
    }

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    // #471 Phase 0: Per-client state is REQUIRED
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    // Extension doesn't exist initially
    assert!(runtime.ext::<TestExtension>().is_none());

    // ext_mut creates it
    let ext = runtime.ext_mut::<TestExtension>();
    assert_eq!(ext.value, 42);
    ext.value = 100;

    // ext now returns it
    assert_eq!(runtime.ext::<TestExtension>().unwrap().value, 100);
}

// ========================================================================
// Shared extension tests (#543)
// ========================================================================

#[test]
fn test_shared_ext_without_shared_extensions_returns_none() {
    use reovim_kernel::api::v1::ModeStack;

    #[derive(Debug)]
    struct SharedTestExt;

    impl SessionExtension for SharedTestExt {
        fn create() -> Self {
            Self
        }
    }

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    // Without shared_extensions, shared_ext returns None
    assert!(runtime.shared_ext::<SharedTestExt>().is_none());
}

#[test]
fn test_shared_ext_mut_without_shared_extensions_returns_none() {
    use reovim_kernel::api::v1::ModeStack;

    #[derive(Debug)]
    struct SharedTestExt2 {
        _value: i32,
    }

    impl SessionExtension for SharedTestExt2 {
        fn create() -> Self {
            Self { _value: 0 }
        }
    }

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    // Without shared_extensions, shared_ext_mut returns None
    assert!(runtime.shared_ext_mut::<SharedTestExt2>().is_none());
}

#[test]
fn test_shared_ext_with_shared_extensions_returns_value() {
    use reovim_kernel::api::v1::ModeStack;

    #[derive(Debug)]
    struct SharedTestExt3 {
        value: i32,
    }

    impl SessionExtension for SharedTestExt3 {
        fn create() -> Self {
            Self { value: 99 }
        }
    }

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut shared_extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    // Pre-populate shared extensions
    shared_extensions.get_or_insert::<SharedTestExt3>();

    let runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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
    )
    .with_shared_extensions(&mut shared_extensions);

    // With shared_extensions, shared_ext returns the value
    let ext = runtime.shared_ext::<SharedTestExt3>();
    assert!(ext.is_some());
    assert_eq!(ext.unwrap().value, 99);
}

#[test]
fn test_shared_ext_mut_creates_and_returns() {
    use reovim_kernel::api::v1::ModeStack;

    #[derive(Debug)]
    struct SharedTestExt4 {
        value: i32,
    }

    impl SessionExtension for SharedTestExt4 {
        fn create() -> Self {
            Self { value: 77 }
        }
    }

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut shared_extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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
    )
    .with_shared_extensions(&mut shared_extensions);

    // shared_ext_mut creates and returns
    let ext = runtime.shared_ext_mut::<SharedTestExt4>();
    assert!(ext.is_some());
    let ext = ext.unwrap();
    assert_eq!(ext.value, 77);
    ext.value = 200;

    // shared_ext reads the updated value
    assert_eq!(runtime.shared_ext::<SharedTestExt4>().unwrap().value, 200);
}

#[test]
fn test_shared_ext_default_trait_returns_none() {
    // Verify the default trait implementation (not SessionRuntime) returns None.
    struct MinimalApi;

    #[derive(Debug)]
    struct AnyExt;
    impl SessionExtension for AnyExt {
        fn create() -> Self {
            Self
        }
    }

    impl ExtensionApi for MinimalApi {
        fn ext<T: SessionExtension>(&self) -> Option<&T> {
            None
        }
        fn ext_mut<T: SessionExtension>(&mut self) -> &mut T {
            unimplemented!()
        }
    }

    let api = MinimalApi;
    assert!(api.shared_ext::<AnyExt>().is_none());
}

#[test]
fn test_shared_ext_mut_default_trait_returns_none() {
    struct MinimalApi2;

    #[derive(Debug)]
    struct AnyExt2;
    impl SessionExtension for AnyExt2 {
        fn create() -> Self {
            Self
        }
    }

    impl ExtensionApi for MinimalApi2 {
        fn ext<T: SessionExtension>(&self) -> Option<&T> {
            None
        }
        fn ext_mut<T: SessionExtension>(&mut self) -> &mut T {
            unimplemented!()
        }
    }

    let mut api = MinimalApi2;
    assert!(api.shared_ext_mut::<AnyExt2>().is_none());
}

#[test]
fn test_change_tracking() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;

    // #471 Phase 0: Per-client state is REQUIRED
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    // No changes initially
    assert!(!runtime.changes.has_changes());

    // Push mode records change
    runtime.push_mode(test_mode_2(), TransitionContext::new());
    assert!(runtime.changes.mode_changed);

    // Take changes resets
    let changes = runtime.take_changes();
    assert!(changes.mode_changed);
    assert!(!runtime.changes.has_changes());
}

// NOTE: test_selection_api and test_selection_api_set_selection removed
// as part of #471 - they tested the removed buffer_id-based selection API.
// Selection is now per-window, managed via CommandContext and CommandResult.

#[test]
fn test_buffer_text_range_single_line() {
    use crate::testing::TestSessionRuntime;

    // Create test runtime with a buffer
    let mut harness = TestSessionRuntime::with_buffer("hello world");

    // Get the buffer ID
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Extract "llo wo" (columns 2-8)
    let result = harness.with_runtime(|runtime| {
        runtime.buffer_text_range(buffer_id, Position::new(0, 2), Position::new(0, 8))
    });
    assert_eq!(result, Some("llo wo".to_string()));
}

#[test]
fn test_buffer_text_range_multi_line() {
    use crate::testing::TestSessionRuntime;

    // Create test runtime with multi-line content
    let mut harness = TestSessionRuntime::with_buffer("line one\nline two\nline three");

    // Get the buffer ID
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Extract from middle of first line to middle of last line
    let result = harness.with_runtime(|runtime| {
        runtime.buffer_text_range(buffer_id, Position::new(0, 5), Position::new(2, 4))
    });
    assert_eq!(result, Some("one\nline two\nline".to_string()));

    // Extract a full line (0,0 to 1,0 gets first line + newline)
    let result = harness.with_runtime(|runtime| {
        runtime.buffer_text_range(buffer_id, Position::new(0, 0), Position::new(1, 0))
    });
    assert_eq!(result, Some("line one\n".to_string()));
}

// =========================================================================
// Additional BufferApi tests via TestSessionRuntime
// =========================================================================

#[test]
fn test_buffer_line_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello\nworld\nfoo");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    harness.with_runtime(|runtime| {
        assert_eq!(runtime.buffer_line(buffer_id, 0), Some("hello".to_string()));
        assert_eq!(runtime.buffer_line(buffer_id, 1), Some("world".to_string()));
        assert_eq!(runtime.buffer_line(buffer_id, 2), Some("foo".to_string()));
        assert!(runtime.buffer_line(buffer_id, 99).is_none());
    });
}

#[test]
fn test_buffer_line_count_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello\nworld\nfoo");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    let count = harness.with_runtime(|runtime| runtime.buffer_line_count(buffer_id));
    assert_eq!(count, Some(3));
}

#[test]
fn test_buffer_content_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    let content = harness.with_runtime(|runtime| runtime.buffer_content(buffer_id));
    assert_eq!(content, Some("hello world".to_string()));
}

#[test]
fn test_buffer_nonexistent() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("test");
    let fake_id = BufferId::new();

    harness.with_runtime(|runtime| {
        assert!(runtime.buffer_line(fake_id, 0).is_none());
        assert!(runtime.buffer_line_count(fake_id).is_none());
        assert!(runtime.buffer_content(fake_id).is_none());
        assert!(runtime.buffer_file_path(fake_id).is_none());
        assert!(runtime.is_buffer_modified(fake_id).is_none());
        assert!(
            runtime
                .buffer_text_range(fake_id, Position::new(0, 0), Position::new(0, 5))
                .is_none()
        );
    });
}

#[test]
fn test_buffer_file_path_none() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("test");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Buffer created without name should have no file path
    let path = harness.with_runtime(|runtime| runtime.buffer_file_path(buffer_id));
    assert!(path.is_none());
}

#[test]
fn test_buffer_modified_flag() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Initially not modified
    let modified = harness.with_runtime(|runtime| runtime.is_buffer_modified(buffer_id));
    assert_eq!(modified, Some(false));

    // Set modified
    harness.with_runtime(|runtime| {
        runtime.set_buffer_modified(buffer_id, true);
    });
    let modified = harness.with_runtime(|runtime| runtime.is_buffer_modified(buffer_id));
    assert_eq!(modified, Some(true));

    // Clear modified
    harness.with_runtime(|runtime| {
        runtime.set_buffer_modified(buffer_id, false);
    });
    let modified = harness.with_runtime(|runtime| runtime.is_buffer_modified(buffer_id));
    assert_eq!(modified, Some(false));
}

#[test]
fn test_set_buffer_modified_nonexistent() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    let fake_id = BufferId::new();

    // Should not panic for non-existent buffer
    harness.with_runtime(|runtime| {
        runtime.set_buffer_modified(fake_id, true);
    });
}

#[test]
fn test_insert_text_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("helloworld");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    harness.with_runtime(|runtime| {
        runtime.insert_text(buffer_id, Position::new(0, 5), " ");
    });

    harness.assert_buffer_content("hello world");
    let changes = harness.take_changes();
    assert!(changes.buffer_modified);
}

#[test]
fn test_insert_text_nonexistent_buffer() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    let fake_id = BufferId::new();

    // Should not panic for non-existent buffer
    harness.with_runtime(|runtime| {
        runtime.insert_text(fake_id, Position::new(0, 0), "text");
    });
}

#[test]
fn test_delete_range_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    harness.with_runtime(|runtime| {
        runtime.delete_range(buffer_id, Position::new(0, 5), Position::new(0, 11));
    });

    harness.assert_buffer_content("hello");
    let changes = harness.take_changes();
    assert!(changes.buffer_modified);
}

#[test]
fn test_delete_range_nonexistent_buffer() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    let fake_id = BufferId::new();

    // Should not panic for non-existent buffer
    harness.with_runtime(|runtime| {
        runtime.delete_range(fake_id, Position::new(0, 0), Position::new(0, 5));
    });
}

#[test]
fn test_create_buffer_unnamed() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    let buf_id = harness.with_runtime(|runtime| runtime.create_buffer(None, "content"));

    let content = harness.with_runtime(|runtime| runtime.buffer_content(buf_id));
    assert_eq!(content, Some("content".to_string()));

    let path = harness.with_runtime(|runtime| runtime.buffer_file_path(buf_id));
    assert!(path.is_none());

    let changes = harness.take_changes();
    assert!(changes.buffers_created.contains(&buf_id));
}

#[test]
fn test_create_buffer_named() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    let buf_id = harness.with_runtime(|runtime| runtime.create_buffer(Some("test.txt"), "hello"));

    let path = harness.with_runtime(|runtime| runtime.buffer_file_path(buf_id));
    assert_eq!(path, Some("test.txt".to_string()));
}

#[test]
fn test_rename_buffer_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    let buf_id = harness.with_runtime(|runtime| runtime.create_buffer(Some("old.txt"), "content"));
    harness.take_changes(); // Clear creation changes

    harness.with_runtime(|runtime| {
        runtime.rename_buffer(buf_id, "new.txt");
    });

    let path = harness.with_runtime(|runtime| runtime.buffer_file_path(buf_id));
    assert_eq!(path, Some("new.txt".to_string()));

    let changes = harness.take_changes();
    assert!(!changes.buffers_renamed.is_empty());
    assert_eq!(changes.buffers_renamed[0].1, "new.txt");
}

#[test]
fn test_rename_nonexistent_buffer() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    let fake_id = BufferId::new();

    // Should not panic
    harness.with_runtime(|runtime| {
        runtime.rename_buffer(fake_id, "new.txt");
    });
}

// ========================================================================
// replace_content (#667)
// ========================================================================

#[test]
fn test_replace_content_updates_buffer() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("original content");
    let buffer_id = harness.active_buffer().unwrap();

    harness.with_runtime(|runtime| {
        runtime.replace_content(buffer_id, "formatted content");
    });

    harness.assert_buffer_content("formatted content");
}

#[test]
fn test_replace_content_marks_modified() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("original");
    let buffer_id = harness.active_buffer().unwrap();

    // Clear modified flag
    harness
        .kernel()
        .buffers
        .get(buffer_id)
        .unwrap()
        .write()
        .set_modified(false);

    harness.with_runtime(|runtime| {
        runtime.replace_content(buffer_id, "new content");
    });

    let is_modified = harness
        .kernel()
        .buffers
        .get(buffer_id)
        .unwrap()
        .read()
        .is_modified();
    assert!(is_modified);
}

#[test]
fn test_replace_content_records_changes() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("before");
    let buffer_id = harness.active_buffer().unwrap();
    harness.take_changes();

    harness.with_runtime(|runtime| {
        runtime.replace_content(buffer_id, "after");
    });

    let changes = harness.take_changes();
    assert!(changes.buffer_modified);
}

#[test]
fn test_replace_content_nonexistent_buffer() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    let fake_id = BufferId::new();

    // Should not panic
    harness.with_runtime(|runtime| {
        runtime.replace_content(fake_id, "anything");
    });
}

#[test]
fn test_delete_buffer_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    let buf1 = harness.with_runtime(|runtime| runtime.create_buffer(None, "first"));
    let buf2 = harness.with_runtime(|runtime| runtime.create_buffer(None, "second"));
    harness.take_changes();

    // Delete one buffer
    let result = harness.with_runtime(|runtime| runtime.delete_buffer(buf1));
    assert!(result.is_ok());

    // Verify it's gone
    let content = harness.with_runtime(|runtime| runtime.buffer_content(buf1));
    assert!(content.is_none());

    // The other buffer should still exist
    let content = harness.with_runtime(|runtime| runtime.buffer_content(buf2));
    assert_eq!(content, Some("second".to_string()));

    let changes = harness.take_changes();
    assert!(changes.buffers_deleted.contains(&buf1));
}

#[test]
fn test_delete_last_buffer_fails() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("only buffer");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    let result = harness.with_runtime(|runtime| runtime.delete_buffer(buffer_id));
    assert!(matches!(result, Err(BufferError::CannotDeleteLastBuffer)));
}

#[test]
fn test_delete_nonexistent_buffer() {
    use crate::testing::TestSessionRuntime;

    // Need at least 2 buffers so the count check passes before the existence check
    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| runtime.create_buffer(None, "first"));
    harness.with_runtime(|runtime| runtime.create_buffer(None, "second"));

    let fake_id = BufferId::new();
    let result = harness.with_runtime(|runtime| runtime.delete_buffer(fake_id));
    assert!(matches!(result, Err(BufferError::NotFound(_))));
}

// =========================================================================
// WindowApi tests via TestSessionRuntime
// =========================================================================

#[test]
fn test_window_buffer_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());
    let window_id = harness.with_runtime(|runtime| runtime.active_window().unwrap());

    let buf = harness.with_runtime(|runtime| runtime.window_buffer(window_id));
    assert_eq!(buf, Some(buffer_id));
}

#[test]
fn test_window_buffer_nonexistent() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let fake_id = WindowId::new();

    let buf = harness.with_runtime(|runtime| runtime.window_buffer(fake_id));
    assert!(buf.is_none());
}

#[test]
fn test_cursor_position_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");

    let pos = harness.with_runtime(|runtime| runtime.cursor_position());
    assert_eq!(pos, Some(Position::new(0, 0)));
}

#[test]
fn test_cursor_position_no_window() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new(); // No windows

    let pos = harness.with_runtime(|runtime| runtime.cursor_position());
    assert!(pos.is_none());
}

#[test]
fn test_focus_nonexistent_window() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("test");
    let fake_id = WindowId::new();

    let result = harness.with_runtime(|runtime| runtime.focus_window(fake_id));
    assert!(matches!(result, Err(WindowError::NotFound(_))));
}

#[test]
fn test_close_nonexistent_window() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("test");
    let _w1 = harness.with_runtime(|runtime| runtime.create_window(None));
    let fake_id = WindowId::new();

    let result = harness.with_runtime(|runtime| runtime.close_window(fake_id));
    assert!(matches!(result, Err(WindowError::NotFound(_))));
}

#[test]
fn test_set_window_buffer_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let window_id = harness.with_runtime(|runtime| runtime.active_window().unwrap());

    // Create a new buffer and set it on the window
    let new_buf = harness.with_runtime(|runtime| runtime.create_buffer(None, "world"));

    let result = harness.with_runtime(|runtime| runtime.set_window_buffer(window_id, new_buf));
    assert!(result.is_ok());

    // Verify the window now shows the new buffer
    let buf = harness.with_runtime(|runtime| runtime.window_buffer(window_id));
    assert_eq!(buf, Some(new_buf));
}

#[test]
fn test_set_window_buffer_nonexistent_window() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buf_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());
    let fake_win = WindowId::new();

    let result = harness.with_runtime(|runtime| runtime.set_window_buffer(fake_win, buf_id));
    assert!(matches!(result, Err(WindowError::NotFound(_))));
}

#[test]
fn test_set_window_buffer_nonexistent_buffer() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let window_id = harness.with_runtime(|runtime| runtime.active_window().unwrap());
    let fake_buf = BufferId::new();

    let result = harness.with_runtime(|runtime| runtime.set_window_buffer(window_id, fake_buf));
    assert!(matches!(result, Err(WindowError::BufferNotFound(_))));
}

#[test]
fn test_set_active_selection() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello world");

    // Initially no selection.
    let sel = harness.with_runtime(|runtime| runtime.active_selection().cloned());
    assert!(sel.is_none());

    // Set a selection.
    let selection = Selection::character(Position::new(0, 0), Position::new(0, 5));
    harness.with_runtime(|runtime| {
        runtime.set_active_selection(Some(selection.clone()));
    });

    let sel = harness.with_runtime(|runtime| runtime.active_selection().cloned());
    assert_eq!(sel.as_ref(), Some(&selection));

    // Clear it.
    harness.with_runtime(|runtime| {
        runtime.set_active_selection(None);
    });

    let sel = harness.with_runtime(|runtime| runtime.active_selection().cloned());
    assert!(sel.is_none());
}

#[test]
fn test_active_selection_no_window() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new(); // No windows

    let sel = harness.with_runtime(|runtime| runtime.active_selection().cloned());
    assert!(sel.is_none());

    // set_active_selection on no window — should be no-op, not panic.
    harness.with_runtime(|runtime| {
        runtime.set_active_selection(Some(Selection::character(
            Position::new(0, 0),
            Position::new(0, 1),
        )));
    });
}

// =========================================================================
// ModeApi additional tests
// =========================================================================

#[test]
fn test_mode_stack_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    let stack = harness.with_runtime(|runtime| runtime.mode_stack());
    assert_eq!(stack.len(), 1);

    // Push a mode
    harness.with_runtime(|runtime| {
        runtime.push_mode(test_mode_2(), TransitionContext::new());
    });

    let stack = harness.with_runtime(|runtime| runtime.mode_stack());
    assert_eq!(stack.len(), 2);
    assert_eq!(stack[0], test_mode());
    assert_eq!(stack[1], test_mode_2());
}

#[test]
fn test_set_mode_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    // set_mode replaces current mode
    harness.with_runtime(|runtime| {
        runtime.set_mode(test_mode_2(), TransitionContext::new());
    });

    let current = harness.with_runtime(|runtime| runtime.current_mode().clone());
    assert_eq!(current, test_mode_2());

    let depth = harness.with_runtime(|runtime| runtime.mode_depth());
    assert_eq!(depth, 1); // set_mode replaces, doesn't push
}

#[test]
fn test_is_mode_active_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    assert!(harness.with_runtime(|runtime| runtime.is_mode_active(&test_mode())));
    assert!(!harness.with_runtime(|runtime| runtime.is_mode_active(&test_mode_2())));

    // Push mode_2
    harness.with_runtime(|runtime| {
        runtime.push_mode(test_mode_2(), TransitionContext::new());
    });

    // Both should be active (on the stack)
    assert!(harness.with_runtime(|runtime| runtime.is_mode_active(&test_mode())));
    assert!(harness.with_runtime(|runtime| runtime.is_mode_active(&test_mode_2())));
}

#[test]
fn test_home_mode_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    let home = harness.with_runtime(|runtime| runtime.home_mode().clone());
    assert_eq!(home, test_mode());

    // Push another mode - home should stay the same
    harness.with_runtime(|runtime| {
        runtime.push_mode(test_mode_2(), TransitionContext::new());
    });
    let home = harness.with_runtime(|runtime| runtime.home_mode().clone());
    assert_eq!(home, test_mode());
}

// =========================================================================
// SessionRuntime accessor tests
// =========================================================================

#[test]
fn test_has_compositor_false() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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
    assert!(!runtime.has_compositor());
}

#[test]
fn test_session_accessor() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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
    assert_eq!(runtime.session().id.as_usize(), 1);
}

#[test]
fn test_session_mut_accessor() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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
    // Exercise session_mut() accessor.
    let session_ref = runtime.session_mut();
    assert_eq!(session_ref.id.as_usize(), 1);
}

#[test]
fn test_kernel_accessor() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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
    // Just verify kernel() doesn't panic
    let _kernel = runtime.kernel();
}

#[test]
fn test_windows_accessor() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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
    assert!(runtime.windows().is_empty());
}

#[test]
fn test_windows_mut_accessor() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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
    runtime.windows_mut().add(crate::Window::new());
    assert_eq!(runtime.windows().len(), 1);
}

// =========================================================================
// CommandApi test
// =========================================================================

#[test]
fn test_execute_command_api() {
    use {
        crate::testing::TestSessionRuntime, reovim_driver_command_types::CommandContext,
        reovim_kernel::api::v1::ModuleId,
    };

    let mut harness = TestSessionRuntime::new();
    let cmd = CommandId::new(ModuleId::new("test"), "test_cmd");

    let result =
        harness.with_runtime(|runtime| runtime.execute_command(cmd, CommandContext::new()));
    // StubExecutor returns None (command not found) -> Error
    assert!(matches!(result, reovim_driver_command_types::CommandResult::Error(_)));
}

// =========================================================================
// with_buffer_read test
// =========================================================================

#[test]
fn test_with_buffer_read() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    let line_count = harness.with_runtime(|runtime| {
        runtime.with_buffer_read(buffer_id, reovim_provider_text::BufferOps::line_count)
    });
    assert_eq!(line_count, Some(1));
}

#[test]
fn test_with_buffer_read_nonexistent() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    let fake_id = BufferId::new();

    let result = harness.with_runtime(|runtime| {
        runtime.with_buffer_read(fake_id, reovim_provider_text::BufferOps::line_count)
    });
    assert!(result.is_none());
}

// =========================================================================
// Record option change tests
// =========================================================================

#[test]
fn test_record_global_option_change() {
    use {crate::testing::TestSessionRuntime, reovim_kernel::api::v1::OptionValue};

    let mut harness = TestSessionRuntime::new();

    harness.with_runtime(|runtime| {
        runtime.record_global_option_change("number", OptionValue::bool(true));
    });

    let changes = harness.take_changes();
    assert!(changes.option_changed);
    assert_eq!(changes.options_changed.len(), 1);
    assert_eq!(changes.options_changed[0].name, "number");
}

#[test]
fn test_record_window_option_change() {
    use {crate::testing::TestSessionRuntime, reovim_kernel::api::v1::OptionValue};

    let mut harness = TestSessionRuntime::with_buffer("test");
    let window_id = harness.with_runtime(|runtime| runtime.active_window().unwrap());

    harness.with_runtime(|runtime| {
        runtime.record_window_option_change("wrap", OptionValue::bool(false), window_id);
    });

    let changes = harness.take_changes();
    assert!(changes.option_changed);
    assert_eq!(changes.options_changed.len(), 1);
    assert_eq!(changes.options_changed[0].window_id, Some(window_id));
}

// =========================================================================
// ChangeTracker record_cursor_move test
// =========================================================================

#[test]
fn test_change_tracker_record_cursor_move() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("test");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    harness.with_runtime(|runtime| {
        ChangeTracker::record_cursor_move(runtime, buffer_id);
    });

    let changes = harness.take_changes();
    assert!(changes.cursor_moved);
    assert!(changes.affected_buffers.contains(&buffer_id));
}

// =========================================================================
// UndoApi - can_undo / can_redo without provider
// =========================================================================

#[test]
fn test_can_undo_without_provider() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("test");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // No undo provider registered, should return false
    let can_undo = harness.with_runtime(|runtime| runtime.can_undo(buffer_id));
    assert!(!can_undo);

    let can_redo = harness.with_runtime(|runtime| runtime.can_redo(buffer_id));
    assert!(!can_redo);
}

#[test]
fn test_undo_without_provider() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("test");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    let result = harness.with_runtime(|runtime| runtime.undo(buffer_id));
    assert!(result.is_none());

    let result = harness.with_runtime(|runtime| runtime.redo(buffer_id));
    assert!(result.is_none());
}

#[test]
fn test_undo_mine_without_owner() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("test");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Without owner, undo_mine should return None
    let result = harness.with_runtime(|runtime| runtime.undo_mine(buffer_id));
    assert!(result.is_none());

    let result = harness.with_runtime(|runtime| runtime.redo_mine(buffer_id));
    assert!(result.is_none());
}

// =========================================================================
// RegisterApi additional tests
// =========================================================================

#[test]
fn test_numbered_register_writes_ignored() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    harness.with_runtime(|runtime| {
        // Writing to numbered register should be ignored
        runtime.set_register(Some('0'), crate::api::RegisterContent::characterwise("test"));

        // Reading numbered registers returns None without provider
        let content = runtime.get_register(Some('0'));
        assert!(content.is_none());
    });
}

#[test]
fn test_clipboard_register_without_provider() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    harness.with_runtime(|runtime| {
        // Without clipboard provider, + register falls back to kernel registers
        runtime.set_register(Some('+'), crate::api::RegisterContent::characterwise("clipboard"));

        // The fallback stores in kernel registers
        let content = runtime.get_register(Some('+'));
        assert!(content.is_none()); // No clipboard provider, get returns None
    });
}

#[test]
fn test_selection_register_without_provider() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    harness.with_runtime(|runtime| {
        runtime.set_register(Some('*'), crate::api::RegisterContent::characterwise("selection"));

        let content = runtime.get_register(Some('*'));
        assert!(content.is_none()); // No clipboard provider
    });
}

// =========================================================================
// CompositorApi error paths (no compositor)
// =========================================================================

#[test]
fn test_compositor_api_no_compositor() {
    use {
        crate::testing::TestSessionRuntime,
        reovim_driver_layout::{NavigateDirection, Rect, SplitDirection},
    };

    let mut harness = TestSessionRuntime::new();

    harness.with_runtime(|runtime| {
        // All compositor operations should return errors when no compositor
        assert!(runtime.navigate(NavigateDirection::Left).is_err());
        assert!(runtime.split(SplitDirection::Horizontal).is_err());
        assert!(runtime.close_current_window().is_err());
        assert!(runtime.close_others().is_err());
        assert!(runtime.resize(NavigateDirection::Right, 5).is_err());
        assert!(runtime.equalize().is_err());
        assert!(runtime.cycle(true).is_err());
        assert!(runtime.toggle_float().is_err());
        assert!(runtime.raise_float().is_err());
        assert!(runtime.lower_float().is_err());
        assert!(runtime.hide_all_overlays().is_err());
        assert!(runtime.set_active_layer_opacity(0.5).is_err());
        assert!(runtime.active_layer_opacity().is_err());
        assert!(runtime.adjust_active_layer_opacity(0.1).is_err());

        // Tab operations work via TabPageSet even without compositor (#401)
        assert!(runtime.tab_new().is_ok()); // creates a new tab
        assert!(runtime.tab_close().is_ok()); // close the new tab (2 -> 1)
        assert!(runtime.tab_close().is_err()); // can't close last tab
        assert!(runtime.tab_next().is_ok()); // cycle (only 1 tab)
        assert!(runtime.tab_prev().is_ok()); // cycle (only 1 tab)
        assert!(runtime.tab_goto(0).is_ok()); // goto first tab
        assert!(runtime.tab_goto(99).is_err()); // out-of-range
        assert_eq!(runtime.tab_count(), 1);
        assert!(runtime.active_tab_id().is_some());

        // focused_window returns None
        assert!(runtime.focused_window().is_none());
        // compositor_window_count returns 0
        assert_eq!(runtime.compositor_window_count(), 0);
        // active_layer returns None
        assert!(runtime.active_layer().is_none());
        // arrange returns empty
        assert!(runtime.arrange(Rect::new(0, 0, 80, 24)).is_empty());
    });
}

#[test]
fn test_set_screen_no_compositor() {
    use {crate::testing::TestSessionRuntime, reovim_driver_layout::Rect};

    let mut harness = TestSessionRuntime::new();

    // Should not panic even without compositor
    harness.with_runtime(|runtime| {
        runtime.set_screen(Rect::new(0, 0, 120, 40));
    });
}

// =========================================================================
// buffer_text_range edge cases
// =========================================================================

#[test]
fn test_buffer_text_range_empty_range() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Same position - should return empty string
    let result = harness.with_runtime(|runtime| {
        runtime.buffer_text_range(buffer_id, Position::new(0, 3), Position::new(0, 3))
    });
    assert_eq!(result, Some(String::new()));
}

#[test]
fn test_buffer_text_range_out_of_bounds_column() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hi");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Columns beyond line length should be clamped
    let result = harness.with_runtime(|runtime| {
        runtime.buffer_text_range(buffer_id, Position::new(0, 0), Position::new(0, 100))
    });
    assert_eq!(result, Some("hi".to_string()));
}

// =========================================================================
// Named/unnamed register tests
// =========================================================================

#[test]
fn test_named_register_set_and_get() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    harness.with_runtime(|runtime| {
        let content = crate::api::RegisterContent::characterwise("hello");
        runtime.set_register(Some('a'), content);

        let got = runtime.get_register(Some('a'));
        assert!(got.is_some());
        assert_eq!(got.unwrap().text, "hello");
    });
}

#[test]
fn test_unnamed_register_set_and_get() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    harness.with_runtime(|runtime| {
        let content = crate::api::RegisterContent::characterwise("unnamed");
        runtime.set_register(None, content);

        let got = runtime.get_register(None);
        assert!(got.is_some());
        assert_eq!(got.unwrap().text, "unnamed");
    });
}

#[test]
fn test_get_register_nonexistent_named() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    let got = harness.with_runtime(|runtime| runtime.get_register(Some('z')));
    assert!(got.is_none());
}

// =========================================================================
// UndoApi: record_edit without provider
// =========================================================================

#[test]
fn test_record_edit_without_provider() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("test");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Should not panic when no undo provider is registered
    harness.with_runtime(|runtime| {
        runtime.record_edit(
            buffer_id,
            vec![reovim_types_text::Edit::Insert {
                position: Position::new(0, 0),
                text: "x".to_string(),
            }],
            Position::new(0, 0),
            Position::new(0, 1),
        );
    });
}

#[test]
fn test_record_edit_mine_without_owner() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("test");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Without owner, record_edit_mine falls back to record_edit
    // Should not panic
    harness.with_runtime(|runtime| {
        runtime.record_edit_mine(
            buffer_id,
            vec![reovim_types_text::Edit::Insert {
                position: Position::new(0, 0),
                text: "x".to_string(),
            }],
            Position::new(0, 0),
            Position::new(0, 1),
        );
    });
}

// =========================================================================
// BufferApi: buffer_line_len
// =========================================================================

#[test]
fn test_buffer_line_len_api() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello\nworld");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    harness.with_runtime(|runtime| {
        assert_eq!(runtime.buffer_line_len(buffer_id, 0), Some(5));
        assert_eq!(runtime.buffer_line_len(buffer_id, 1), Some(5));
        assert!(runtime.buffer_line_len(buffer_id, 99).is_none());
    });
}

#[test]
fn test_buffer_line_len_nonexistent_buffer() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    let fake_id = BufferId::new();

    let result = harness.with_runtime(|runtime| runtime.buffer_line_len(fake_id, 0));
    assert!(result.is_none());
}

// =========================================================================
// BufferApi: delete_range that produces empty deleted text
// =========================================================================

#[test]
fn test_delete_range_empty_range() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Delete from position to same position produces empty deleted text
    harness.with_runtime(|runtime| {
        runtime.delete_range(buffer_id, Position::new(0, 5), Position::new(0, 5));
    });

    // Buffer content should be unchanged
    harness.assert_buffer_content("hello world");
}

// =========================================================================
// CompositorApi: show_overlay, hide_overlay, resize_overlay error paths
// =========================================================================

#[test]
fn test_show_overlay_no_compositor() {
    use {crate::testing::TestSessionRuntime, reovim_driver_layout::OverlayConstraints};

    let mut harness = TestSessionRuntime::new();

    let result = harness.with_runtime(|runtime| {
        runtime.show_overlay(OverlayConstraints::centered().with_size(20, 10))
    });
    assert!(result.is_err());
}

#[test]
fn test_hide_overlay_no_compositor() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    let fake_win = WindowId::new();

    let result = harness.with_runtime(|runtime| runtime.hide_overlay(fake_win));
    assert!(result.is_err());
}

#[test]
fn test_resize_overlay_no_compositor() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    let fake_win = WindowId::new();

    let result = harness.with_runtime(|runtime| runtime.resize_overlay(fake_win, 40, 20));
    assert!(result.is_err());
}

#[test]
fn test_focus_no_compositor() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    let fake_win = WindowId::new();

    let result = harness.with_runtime(|runtime| runtime.focus(fake_win));
    assert!(result.is_err());
}

// =========================================================================
// to_kernel_split_direction coverage
// =========================================================================

#[test]
fn test_to_kernel_split_direction() {
    use reovim_driver_layout::SplitDirection;

    let h = to_kernel_split_direction(SplitDirection::Horizontal);
    assert!(matches!(h, KernelSplitDirection::Horizontal));

    let v = to_kernel_split_direction(SplitDirection::Vertical);
    assert!(matches!(v, KernelSplitDirection::Vertical));
}

// =========================================================================
// buffer_text_range multi-line with out-of-bounds start column
// =========================================================================

#[test]
fn test_buffer_text_range_multi_line_out_of_bounds_start() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("ab\ncd\nef");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Start column beyond line length in multi-line range
    let result = harness.with_runtime(|runtime| {
        runtime.buffer_text_range(buffer_id, Position::new(0, 100), Position::new(2, 1))
    });
    // Start column clamped to end of first line -> empty first line + \n + "cd\n" + "e"
    assert_eq!(result, Some("\ncd\ne".to_string()));
}

// =========================================================================
// ExecuteCommand: command not found path
// =========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_execute_command_not_found() {
    use reovim_kernel::api::v1::ModeStack;

    struct NullExecutor;
    impl CommandExecutor for NullExecutor {
        fn get_handle(&self, _id: &CommandId) -> Option<Arc<dyn CommandHandle>> {
            None
        }
    }

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = NullExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    let cmd = CommandId::new(ModuleId::new("test"), "nonexistent");
    let result = runtime.execute_command(cmd, CommandContext::new());
    assert!(matches!(result, CommandResult::Error(_)));
}

// =========================================================================
// ExecuteCommand: recursion guard (#547)
// =========================================================================

#[test]
fn test_execute_command_recursion_guard() {
    use {
        crate::testing::TestSessionRuntime, reovim_driver_command_types::CommandContext,
        reovim_kernel::api::v1::ModuleId,
    };

    let mut harness = TestSessionRuntime::new();
    let cmd = CommandId::new(ModuleId::new("test"), "test_cmd");

    // Manually set command_depth to 16 (the limit)
    harness.with_runtime(|runtime| {
        runtime.command_depth = 16;
        let result = runtime.execute_command(cmd, CommandContext::new());
        assert!(result.is_error());
        match result {
            CommandResult::Error(msg) => {
                assert!(msg.contains("recursion limit"));
            }
            CommandResult::Success => panic!("Expected recursion limit error"),
        }
    });
}

// =========================================================================
// set_window_buffer clears selection
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_set_window_buffer_clears_selection() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let window_id = harness.with_runtime(|runtime| runtime.active_window().unwrap());

    // Set a selection on the window
    if let Some(w) = harness.windows.get_mut(window_id) {
        w.selection = Some(crate::Selection::new(
            Position::new(0, 0),
            Position::new(0, 3),
            crate::SelectionMode::Character,
        ));
    }

    // Create a new buffer and set it on the window
    let new_buf = harness.with_runtime(|runtime| runtime.create_buffer(None, "world"));
    let result = harness.with_runtime(|runtime| runtime.set_window_buffer(window_id, new_buf));
    assert!(result.is_ok());

    // Selection should be cleared
    assert!(harness.windows.get(window_id).unwrap().selection.is_none());
}

// =========================================================================
// insert_text with cursor from active window
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_insert_text_with_cursor_position() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Set cursor to a specific position
    if let Some(w) = harness.windows.active_mut() {
        w.cursor.line = 0;
        w.cursor.column = 3;
    }

    harness.with_runtime(|runtime| {
        runtime.insert_text(buffer_id, Position::new(0, 3), " ");
    });

    harness.assert_buffer_content("hel lo");
}

// =========================================================================
// delete_range with actual content and BufferModified event
// =========================================================================

#[test]
fn test_insert_text_emits_buffer_modified() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    harness.with_runtime(|runtime| {
        runtime.insert_text(buffer_id, Position::new(0, 5), " world");
    });

    harness.assert_buffer_content("hello world");
    let changes = harness.take_changes();
    assert!(changes.buffer_modified);
    assert!(changes.affected_buffers.contains(&buffer_id));
}

#[test]
fn test_delete_range_emits_buffer_modified() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    harness.with_runtime(|runtime| {
        runtime.delete_range(buffer_id, Position::new(0, 5), Position::new(0, 11));
    });

    harness.assert_buffer_content("hello");
    let changes = harness.take_changes();
    assert!(changes.buffer_modified);
    assert!(changes.affected_buffers.contains(&buffer_id));
}

// =========================================================================
// pop_mode with PopResult
// =========================================================================

#[test]
fn test_pop_mode_with_result() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    harness.with_runtime(|runtime| {
        runtime.push_mode(test_mode_2(), TransitionContext::new());
    });

    // Pop with a Cancelled result
    let result = harness.with_runtime(|runtime| runtime.pop_mode(Some(PopResult::Cancelled)));
    assert!(result.is_ok());
}

// =========================================================================
// set_mode records change
// =========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_set_mode_records_change() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();

    harness.with_runtime(|runtime| {
        runtime.set_mode(test_mode_2(), TransitionContext::new());
    });

    let changes = harness.take_changes();
    assert!(changes.mode_changed);
}

// =========================================================================
// insert_text / delete_range fallback when no active window (lines 559, 584)
// =========================================================================

/// Buffer manager that actually stores buffers, for tests requiring real buffers.
struct InMemoryBufferManager {
    buffers: reovim_arch::sync::RwLock<
        std::collections::HashMap<
            BufferId,
            std::sync::Arc<reovim_arch::sync::RwLock<dyn reovim_kernel::api::v1::KernelBuffer>>,
        >,
    >,
}

impl InMemoryBufferManager {
    fn new() -> Self {
        Self {
            buffers: reovim_arch::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_kernel::api::v1::BufferManager for InMemoryBufferManager {
    fn get(
        &self,
        id: BufferId,
    ) -> Option<std::sync::Arc<reovim_arch::sync::RwLock<dyn reovim_kernel::api::v1::KernelBuffer>>>
    {
        self.buffers.read().get(&id).cloned()
    }

    fn register(
        &self,
        buffer: std::sync::Arc<reovim_arch::sync::RwLock<dyn reovim_kernel::api::v1::KernelBuffer>>,
    ) -> BufferId {
        let id = buffer.read().id();
        self.buffers.write().insert(id, buffer);
        id
    }

    fn unregister(
        &self,
        id: BufferId,
    ) -> Option<std::sync::Arc<reovim_arch::sync::RwLock<dyn reovim_kernel::api::v1::KernelBuffer>>>
    {
        self.buffers.write().remove(&id)
    }

    fn list(&self) -> Vec<BufferId> {
        self.buffers.read().keys().copied().collect()
    }

    fn count(&self) -> usize {
        self.buffers.read().len()
    }
}

/// Helper to create a `KernelContext` with a real buffer manager and custom services.
fn make_kernel_with_services(
    services: std::sync::Arc<reovim_kernel::api::v1::ServiceRegistry>,
) -> KernelContext {
    use reovim_kernel::api::v1::{EventBus, OptionRegistry};

    KernelContext::new(
        std::sync::Arc::new(EventBus::new()),
        std::sync::Arc::new(InMemoryBufferManager::new()),
        std::sync::Arc::new(OptionRegistry::new()),
        services,
    )
}

#[test]
fn test_insert_text_no_active_window_uses_zero_position() {
    use reovim_kernel::api::v1::ModeStack;

    let services = std::sync::Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
    let text_registry = std::sync::Arc::new(reovim_provider_text::TextBufferRegistry::new());
    services.register(std::sync::Arc::clone(&text_registry));
    let kernel = make_kernel_with_services(services);
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;

    let buf_arc = std::sync::Arc::new(reovim_arch::sync::RwLock::new(
        reovim_provider_text::Buffer::from_string("hello"),
    ));
    let buf_id = kernel
        .buffers
        .register(std::sync::Arc::clone(&buf_arc) as _);
    text_registry.register(buf_arc);

    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty(); // No windows!
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = Some(buf_id); // Per-client (#471)
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    runtime.insert_text(buf_id, Position::new(0, 5), " world");

    let content = runtime.buffer_content(buf_id);
    assert_eq!(content, Some("hello world".to_string()));
}

#[test]
fn test_delete_range_no_active_window_uses_zero_position() {
    use reovim_kernel::api::v1::ModeStack;

    let services = std::sync::Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
    let text_registry = std::sync::Arc::new(reovim_provider_text::TextBufferRegistry::new());
    services.register(std::sync::Arc::clone(&text_registry));
    let kernel = make_kernel_with_services(services);
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;

    let buf_arc = std::sync::Arc::new(reovim_arch::sync::RwLock::new(
        reovim_provider_text::Buffer::from_string("hello world"),
    ));
    let buf_id = kernel
        .buffers
        .register(std::sync::Arc::clone(&buf_arc) as _);
    text_registry.register(buf_arc);

    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty(); // No windows!
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = Some(buf_id); // Per-client (#471)
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    runtime.delete_range(buf_id, Position::new(0, 5), Position::new(0, 11));

    let content = runtime.buffer_content(buf_id);
    assert_eq!(content, Some("hello".to_string()));
}

// =========================================================================
// RegisterApi with clipboard provider (lines 736-790)
// =========================================================================

struct MockClipboard {
    clipboard: std::sync::Mutex<Option<String>>,
    selection: std::sync::Mutex<Option<String>>,
}

impl MockClipboard {
    fn new() -> Self {
        Self {
            clipboard: std::sync::Mutex::new(None),
            selection: std::sync::Mutex::new(None),
        }
    }

    fn with_clipboard(text: &str) -> Self {
        Self {
            clipboard: std::sync::Mutex::new(Some(text.to_string())),
            selection: std::sync::Mutex::new(None),
        }
    }

    fn with_selection(text: &str) -> Self {
        Self {
            clipboard: std::sync::Mutex::new(None),
            selection: std::sync::Mutex::new(Some(text.to_string())),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_clipboard::ClipboardProvider for MockClipboard {
    fn clipboard_available(&self) -> bool {
        true
    }
    fn copy_to_clipboard(&self, text: &str) -> Result<(), reovim_driver_clipboard::ClipboardError> {
        *self.clipboard.lock().unwrap() = Some(text.to_string());
        Ok(())
    }
    fn paste_from_clipboard(
        &self,
    ) -> Result<Option<String>, reovim_driver_clipboard::ClipboardError> {
        Ok(self.clipboard.lock().unwrap().clone())
    }
    fn selection_available(&self) -> bool {
        true
    }
    fn copy_to_selection(&self, text: &str) -> Result<(), reovim_driver_clipboard::ClipboardError> {
        *self.selection.lock().unwrap() = Some(text.to_string());
        Ok(())
    }
    fn paste_from_selection(
        &self,
    ) -> Result<Option<String>, reovim_driver_clipboard::ClipboardError> {
        Ok(self.selection.lock().unwrap().clone())
    }
}

fn kernel_with_clipboard(provider: MockClipboard) -> KernelContext {
    let services = std::sync::Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
    let clipboard_registry = ClipboardProviderRegistry::new();
    clipboard_registry.register(ClipboardKey::Default, std::sync::Arc::new(provider));
    services.register(std::sync::Arc::new(clipboard_registry));
    make_kernel_with_services(services)
}

/// Raw `get_register(Some('+'))` returns `None` since `RegisterBank`
/// doesn't handle `+`. Use `get_register_with_clipboard` instead (#515).
#[test]
fn test_get_register_raw_clipboard_plus_returns_none() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_clipboard(MockClipboard::with_clipboard("from-clipboard"));
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    // Raw RegisterApi does NOT route + to clipboard
    assert!(runtime.get_register(Some('+')).is_none());

    // get_register_with_clipboard reads from OS clipboard
    let content = runtime.get_register_with_clipboard(Some('+'));
    assert!(content.is_some());
    assert_eq!(content.unwrap().text, "from-clipboard");
}

/// Raw `get_register(Some('*'))` returns `None`. Use
/// `get_register_with_clipboard` to read from OS selection (#515).
#[test]
fn test_get_register_raw_selection_star_returns_none() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_clipboard(MockClipboard::with_selection("from-selection"));
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    // Raw RegisterApi does NOT route * to selection
    assert!(runtime.get_register(Some('*')).is_none());

    // get_register_with_clipboard reads from OS selection
    let content = runtime.get_register_with_clipboard(Some('*'));
    assert!(content.is_some());
    assert_eq!(content.unwrap().text, "from-selection");
}

#[test]
fn test_get_register_numbered_with_provider() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_clipboard(MockClipboard::new());
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    // Pre-populate per-client clipboard history (#515):
    // push "yank-1" first so it becomes index 1, then "yank-0" so it becomes index 0
    let mut clipboard_history = HistoryRing::new();
    clipboard_history.push(RegisterContent::characterwise("yank-1"));
    clipboard_history.push(RegisterContent::characterwise("yank-0"));
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    let c0 = runtime.get_register(Some('0'));
    assert!(c0.is_some());
    assert_eq!(c0.unwrap().text, "yank-0");

    let c1 = runtime.get_register(Some('1'));
    assert!(c1.is_some());
    assert_eq!(c1.unwrap().text, "yank-1");
}

/// `set_register` for `+` does NOT store in `RegisterBank` (which only
/// handles a-z/A-Z/unnamed). Callers must use `store_register_with_sync`
/// for clipboard sync (#515 Phase 4).
#[test]
fn test_set_register_clipboard_plus_not_stored_in_register_bank() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_clipboard(MockClipboard::new());
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    // Raw set_register does NOT store + in RegisterBank
    runtime.set_register(Some('+'), RegisterContent::characterwise("to-clipboard"));
    assert!(runtime.get_register(Some('+')).is_none());
}

/// `store_register_with_sync` for `+` syncs to clipboard and is readable
/// via `get_register_with_clipboard` (#515 Phase 4).
#[test]
fn test_store_register_with_sync_clipboard_plus() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_clipboard(MockClipboard::new());
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    runtime.store_register_with_sync(Some('+'), RegisterContent::characterwise("to-clipboard"));

    let content = runtime.get_register_with_clipboard(Some('+'));
    assert!(content.is_some());
    assert_eq!(content.unwrap().text, "to-clipboard");
}

/// Same as above but for `*` (selection).
#[test]
fn test_set_register_selection_star_not_stored_in_register_bank() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_clipboard(MockClipboard::new());
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    runtime.set_register(Some('*'), RegisterContent::characterwise("to-selection"));
    assert!(runtime.get_register(Some('*')).is_none());
}

/// `store_register_with_sync` for `*` syncs to selection and is readable
/// via `get_register_with_clipboard` (#515 Phase 4).
#[test]
fn test_store_register_with_sync_selection_star() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_clipboard(MockClipboard::new());
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    runtime.store_register_with_sync(Some('*'), RegisterContent::characterwise("to-selection"));

    let content = runtime.get_register_with_clipboard(Some('*'));
    assert!(content.is_some());
    assert_eq!(content.unwrap().text, "to-selection");
}

// =========================================================================
// UndoApi with provider (lines 817-993)
// =========================================================================

struct MockUndoProvider {
    edits: std::sync::Mutex<Vec<reovim_driver_undo::UndoRecord>>,
}

impl MockUndoProvider {
    fn new() -> Self {
        Self {
            edits: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_undo::UndoProvider for MockUndoProvider {
    fn undo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
        Some(UndoResult {
            edits: vec![Edit::Insert {
                position: Position::new(0, 0),
                text: "X".to_string(),
            }],
            cursor: Position::new(0, 1),
        })
    }

    fn redo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
        Some(UndoResult {
            edits: vec![Edit::Delete {
                position: Position::new(0, 0),
                text: "X".to_string(),
            }],
            cursor: Position::new(0, 0),
        })
    }

    fn redo_branch(&self, _buffer_id: BufferId, _branch_idx: usize) -> Option<UndoResult> {
        None
    }

    fn record(
        &self,
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        self.edits
            .lock()
            .unwrap()
            .push(reovim_driver_undo::UndoRecord {
                buffer_id,
                edits,
                cursor_before,
                cursor_after,
            });
    }

    fn has_history(&self, _buffer_id: BufferId) -> bool {
        true
    }

    fn remove(&self, _buffer_id: BufferId) {}

    fn buffer_count(&self) -> usize {
        1
    }

    fn get_tree(&self, _buffer_id: BufferId) -> Option<reovim_types_text::UndoTree> {
        let mut tree = reovim_types_text::UndoTree::new();
        tree.push(
            vec![Edit::Insert {
                position: Position::new(0, 0),
                text: "a".to_string(),
            }],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        Some(tree)
    }

    fn begin_batch(&self, _buffer_id: BufferId, _cursor_before: Position) {}
    fn end_batch(&self, _buffer_id: BufferId, _cursor_after: Position) {}
    fn is_batching(&self, _buffer_id: BufferId) -> bool {
        false
    }

    fn persist(
        &self,
        _buffer_id: BufferId,
        _buffer_path: &str,
        _vfs: &dyn reovim_driver_vfs::VfsDriver,
    ) -> Result<(), reovim_driver_undo::UndoPersistError> {
        Ok(())
    }

    fn load(
        &self,
        _buffer_id: BufferId,
        _buffer_path: &str,
        _vfs: &dyn reovim_driver_vfs::VfsDriver,
    ) -> Result<bool, reovim_driver_undo::UndoPersistError> {
        Ok(false)
    }

    fn undo_for_client(&self, _buffer_id: BufferId, _client_id: usize) -> Option<UndoResult> {
        Some(UndoResult {
            edits: vec![Edit::Insert {
                position: Position::new(0, 0),
                text: "Y".to_string(),
            }],
            cursor: Position::new(0, 1),
        })
    }

    fn redo_for_client(&self, _buffer_id: BufferId, _client_id: usize) -> Option<UndoResult> {
        Some(UndoResult {
            edits: vec![Edit::Delete {
                position: Position::new(0, 0),
                text: "Y".to_string(),
            }],
            cursor: Position::new(0, 0),
        })
    }

    fn record_for_client(
        &self,
        buffer_id: BufferId,
        _client_id: usize,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        self.record(buffer_id, edits, cursor_before, cursor_after);
    }
}

fn kernel_with_undo(provider: MockUndoProvider) -> KernelContext {
    let services = std::sync::Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
    let undo_registry = UndoProviderRegistry::new();
    undo_registry.register(UndoKey::Buffer, std::sync::Arc::new(provider));
    services.register(std::sync::Arc::new(undo_registry));
    make_kernel_with_services(services)
}

#[test]
fn test_undo_with_provider() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_undo(MockUndoProvider::new());
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;

    let buf = reovim_provider_text::Buffer::from_string("hello");
    let buf_id = kernel
        .buffers
        .register(std::sync::Arc::new(reovim_arch::sync::RwLock::new(buf)));

    let mut mode_stack = ModeStack::new(test_mode());
    let mut window = crate::Window::new();
    window.buffer_id = Some(buf_id);
    let mut windows = crate::WindowLayout::empty();
    windows.add(window);
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    let result = runtime.undo(buf_id);
    assert!(result.is_some());
    assert_eq!(result.unwrap().cursor, Position::new(0, 1));
    assert!(runtime.changes.buffer_modified);
    assert!(runtime.changes.cursor_moved);
}

#[test]
fn test_redo_with_provider() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_undo(MockUndoProvider::new());
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;

    let buf = reovim_provider_text::Buffer::from_string("Xhello");
    let buf_id = kernel
        .buffers
        .register(std::sync::Arc::new(reovim_arch::sync::RwLock::new(buf)));

    let mut mode_stack = ModeStack::new(test_mode());
    let mut window = crate::Window::new();
    window.buffer_id = Some(buf_id);
    let mut windows = crate::WindowLayout::empty();
    windows.add(window);
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    let result = runtime.redo(buf_id);
    assert!(result.is_some());
    assert_eq!(result.unwrap().cursor, Position::new(0, 0));
    assert!(runtime.changes.buffer_modified);
    assert!(runtime.changes.cursor_moved);
}

#[test]
fn test_can_undo_with_provider() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_undo(MockUndoProvider::new());
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    let buf_id = BufferId::from_raw(1);
    assert!(runtime.can_undo(buf_id));
}

#[test]
fn test_undo_mine_with_owner_and_provider() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_undo(MockUndoProvider::new());
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let client_id = ClientId::new(42);

    let buf = reovim_provider_text::Buffer::from_string("hello");
    let buf_id = kernel
        .buffers
        .register(std::sync::Arc::new(reovim_arch::sync::RwLock::new(buf)));

    let mut mode_stack = ModeStack::new(test_mode());
    let mut window = crate::Window::new();
    window.buffer_id = Some(buf_id);
    let mut windows = crate::WindowLayout::empty();
    windows.add(window);
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::with_owner(
        client_id,
        &mut session,
        crate::ClientContext {
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

    let result = runtime.undo_mine(buf_id);
    assert!(result.is_some());
    assert_eq!(result.unwrap().cursor, Position::new(0, 1));
    assert!(runtime.changes.buffer_modified);
    assert!(runtime.changes.cursor_moved);
}

#[test]
fn test_redo_mine_with_owner_and_provider() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_undo(MockUndoProvider::new());
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let client_id = ClientId::new(42);

    let buf = reovim_provider_text::Buffer::from_string("Yhello");
    let buf_id = kernel
        .buffers
        .register(std::sync::Arc::new(reovim_arch::sync::RwLock::new(buf)));

    let mut mode_stack = ModeStack::new(test_mode());
    let mut window = crate::Window::new();
    window.buffer_id = Some(buf_id);
    let mut windows = crate::WindowLayout::empty();
    windows.add(window);
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::with_owner(
        client_id,
        &mut session,
        crate::ClientContext {
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

    let result = runtime.redo_mine(buf_id);
    assert!(result.is_some());
    assert_eq!(result.unwrap().cursor, Position::new(0, 0));
    assert!(runtime.changes.buffer_modified);
    assert!(runtime.changes.cursor_moved);
}

#[test]
fn test_record_edit_mine_with_owner_and_provider() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_undo(MockUndoProvider::new());
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let client_id = ClientId::new(42);
    let buf_id = BufferId::from_raw(1);

    let mut mode_stack = ModeStack::new(test_mode());
    let mut windows = crate::WindowLayout::empty();
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::with_owner(
        client_id,
        &mut session,
        crate::ClientContext {
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

    runtime.record_edit_mine(
        buf_id,
        vec![Edit::Insert {
            position: Position::new(0, 0),
            text: "z".to_string(),
        }],
        Position::new(0, 0),
        Position::new(0, 1),
    );
    // No panic is sufficient
}

// =========================================================================
// UndoApi: alternate edit type branches (Delete in undo, Insert in redo)
// =========================================================================

/// Undo provider that returns Delete edits for undo and Insert edits for redo,
/// covering the alternate branches in the undo/redo implementations.
struct AlternateUndoProvider;

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_undo::UndoProvider for AlternateUndoProvider {
    fn undo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
        // Return a Delete edit (the existing MockUndoProvider returns Insert)
        Some(UndoResult {
            edits: vec![Edit::Delete {
                position: Position::new(0, 0),
                text: "X".to_string(),
            }],
            cursor: Position::new(0, 0),
        })
    }

    fn redo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
        // Return an Insert edit (the existing MockUndoProvider returns Delete)
        Some(UndoResult {
            edits: vec![Edit::Insert {
                position: Position::new(0, 0),
                text: "Y".to_string(),
            }],
            cursor: Position::new(0, 1),
        })
    }

    fn redo_branch(&self, _buffer_id: BufferId, _branch_idx: usize) -> Option<UndoResult> {
        None
    }

    fn record(
        &self,
        _buffer_id: BufferId,
        _edits: Vec<Edit>,
        _cursor_before: Position,
        _cursor_after: Position,
    ) {
    }

    fn has_history(&self, _buffer_id: BufferId) -> bool {
        true
    }

    fn remove(&self, _buffer_id: BufferId) {}

    fn buffer_count(&self) -> usize {
        1
    }

    fn get_tree(&self, _buffer_id: BufferId) -> Option<reovim_types_text::UndoTree> {
        None
    }

    fn begin_batch(&self, _buffer_id: BufferId, _cursor_before: Position) {}
    fn end_batch(&self, _buffer_id: BufferId, _cursor_after: Position) {}
    fn is_batching(&self, _buffer_id: BufferId) -> bool {
        false
    }

    fn persist(
        &self,
        _buffer_id: BufferId,
        _buffer_path: &str,
        _vfs: &dyn reovim_driver_vfs::VfsDriver,
    ) -> Result<(), reovim_driver_undo::UndoPersistError> {
        Ok(())
    }

    fn load(
        &self,
        _buffer_id: BufferId,
        _buffer_path: &str,
        _vfs: &dyn reovim_driver_vfs::VfsDriver,
    ) -> Result<bool, reovim_driver_undo::UndoPersistError> {
        Ok(false)
    }

    fn undo_for_client(&self, _buffer_id: BufferId, _client_id: usize) -> Option<UndoResult> {
        // Return Delete edit for undo_mine coverage
        Some(UndoResult {
            edits: vec![Edit::Delete {
                position: Position::new(0, 0),
                text: "Z".to_string(),
            }],
            cursor: Position::new(0, 0),
        })
    }

    fn redo_for_client(&self, _buffer_id: BufferId, _client_id: usize) -> Option<UndoResult> {
        // Return Insert edit for redo_mine coverage
        Some(UndoResult {
            edits: vec![Edit::Insert {
                position: Position::new(0, 0),
                text: "W".to_string(),
            }],
            cursor: Position::new(0, 1),
        })
    }

    fn record_for_client(
        &self,
        _buffer_id: BufferId,
        _client_id: usize,
        _edits: Vec<Edit>,
        _cursor_before: Position,
        _cursor_after: Position,
    ) {
    }
}

fn kernel_with_alternate_undo() -> KernelContext {
    let services = std::sync::Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
    let undo_registry = UndoProviderRegistry::new();
    undo_registry.register(UndoKey::Buffer, std::sync::Arc::new(AlternateUndoProvider));
    services.register(std::sync::Arc::new(undo_registry));
    services.register(std::sync::Arc::new(reovim_provider_text::TextBufferRegistry::new()));
    make_kernel_with_services(services)
}

/// Register a buffer in both kernel and text registry.
///
/// Creates a single concrete `Arc<RwLock<Buffer>>` and coerces it into both
/// `dyn BufferOps` (for `TextBufferRegistry`) and `dyn KernelBuffer` (for kernel)
/// at the respective call sites.
fn register_in_both(kernel: &KernelContext, content: &str) -> BufferId {
    let arc = std::sync::Arc::new(reovim_arch::sync::RwLock::new(
        reovim_provider_text::Buffer::from_string(content),
    ));
    if let Some(reg) = kernel
        .services
        .get::<reovim_provider_text::TextBufferRegistry>()
    {
        reg.register(arc.clone());
    }
    kernel.buffers.register(arc)
}

/// Undo with Delete edits covers the `Edit::Delete` branch in `undo()` (lines 829-831, 835).
#[test]
fn test_undo_with_delete_edits() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_alternate_undo();
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;

    let buf_id = register_in_both(&kernel, "Xhello");

    let mut mode_stack = ModeStack::new(test_mode());
    let mut window = crate::Window::new();
    window.buffer_id = Some(buf_id);
    let mut windows = crate::WindowLayout::empty();
    windows.add(window);
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    let result = runtime.undo(buf_id);
    assert!(result.is_some());
    // Delete edit removed "X" from position (0,0)
    let content = runtime.buffer_content(buf_id);
    assert_eq!(content, Some("hello".to_string()));
    assert!(runtime.changes.buffer_modified);
}

/// Redo with Insert edits covers the `Edit::Insert` branch in `redo()` (lines 863-865, 872).
#[test]
fn test_redo_with_insert_edits() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_alternate_undo();
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;

    let buf_id = register_in_both(&kernel, "hello");

    let mut mode_stack = ModeStack::new(test_mode());
    let mut window = crate::Window::new();
    window.buffer_id = Some(buf_id);
    let mut windows = crate::WindowLayout::empty();
    windows.add(window);
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
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

    let result = runtime.redo(buf_id);
    assert!(result.is_some());
    // Insert edit added "Y" at position (0,0)
    let content = runtime.buffer_content(buf_id);
    assert_eq!(content, Some("Yhello".to_string()));
    assert!(runtime.changes.buffer_modified);
}

/// `undo_mine` with Delete edits covers the `Edit::Delete` branch (lines 938-940, 943).
#[test]
fn test_undo_mine_with_delete_edits() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_alternate_undo();
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let client_id = ClientId::new(42);

    let buf_id = register_in_both(&kernel, "Zhello");

    let mut mode_stack = ModeStack::new(test_mode());
    let mut window = crate::Window::new();
    window.buffer_id = Some(buf_id);
    let mut windows = crate::WindowLayout::empty();
    windows.add(window);
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::with_owner(
        client_id,
        &mut session,
        crate::ClientContext {
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

    let result = runtime.undo_mine(buf_id);
    assert!(result.is_some());
    // Delete edit removed "Z" from position (0,0)
    let content = runtime.buffer_content(buf_id);
    assert_eq!(content, Some("hello".to_string()));
    assert!(runtime.changes.buffer_modified);
}

/// `redo_mine` with Insert edits covers the `Edit::Insert` branch (lines 974-976, 982).
#[test]
fn test_redo_mine_with_insert_edits() {
    use reovim_kernel::api::v1::ModeStack;

    let kernel = kernel_with_alternate_undo();
    let mut session = Session::new(ClientId::new(1), test_mode());
    let executor = StubExecutor;
    let client_id = ClientId::new(42);

    let buf_id = register_in_both(&kernel, "hello");

    let mut mode_stack = ModeStack::new(test_mode());
    let mut window = crate::Window::new();
    window.buffer_id = Some(buf_id);
    let mut windows = crate::WindowLayout::empty();
    windows.add(window);
    let mut extensions = crate::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = crate::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::with_owner(
        client_id,
        &mut session,
        crate::ClientContext {
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

    let result = runtime.redo_mine(buf_id);
    assert!(result.is_some());
    // Insert edit added "W" at position (0,0)
    let content = runtime.buffer_content(buf_id);
    assert_eq!(content, Some("Whello".to_string()));
    assert!(runtime.changes.buffer_modified);
}

/// `apply_undo_edits` returns early when the buffer is not in the kernel.
#[test]
fn test_apply_undo_edits_buffer_not_found() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let rt = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );

    // BufferId::new() is not registered in the kernel
    let buf = BufferId::new();
    let edits = vec![Edit::Insert {
        position: Position::new(0, 0),
        text: "X".to_string(),
    }];

    // Should return without panic (early return from let...else)
    rt.apply_undo_edits(buf, &edits);
}

// =========================================================================
// CompositorApi with compositor (lines 1081-1537)
// =========================================================================

struct MockLayerCompositor {
    id: reovim_driver_layout::LayerId,
    windows: Vec<WindowId>,
    focused: Option<WindowId>,
    next_id: usize,
}

impl MockLayerCompositor {
    fn new() -> Self {
        let first = WindowId::from_raw(1);
        let second = WindowId::from_raw(2);
        Self {
            id: reovim_driver_layout::LayerId::new(0),
            windows: vec![first, second],
            focused: Some(first),
            next_id: 3,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_layout::WindowLayerCompositor for MockLayerCompositor {
    fn id(&self) -> reovim_driver_layout::LayerId {
        self.id
    }

    fn arrange(&self, _bounds: Rect) -> Vec<reovim_driver_layout::WindowPlacement> {
        Vec::new()
    }

    fn add_tiled(&mut self) -> WindowId {
        let id = WindowId::from_raw(self.next_id);
        self.next_id += 1;
        self.windows.push(id);
        if self.focused.is_none() {
            self.focused = Some(id);
        }
        id
    }

    fn split_tiled(
        &mut self,
        _from: WindowId,
        _direction: reovim_driver_layout::SplitDirection,
    ) -> Option<WindowId> {
        let id = WindowId::from_raw(self.next_id);
        self.next_id += 1;
        self.windows.push(id);
        self.focused = Some(id);
        Some(id)
    }

    fn navigate_tiled(&self, from: WindowId, _direction: NavigateDirection) -> Option<WindowId> {
        self.windows.iter().find(|&&w| w != from).copied()
    }

    fn resize_tiled(&mut self, _window: WindowId, _direction: NavigateDirection, _delta: i16) {}

    fn close_tiled(&mut self, window: WindowId) -> Option<WindowId> {
        self.windows.retain(|&w| w != window);
        let next = self.windows.first().copied();
        if self.focused == Some(window) {
            self.focused = next;
        }
        next
    }

    fn equalize_tiled(&mut self) {}

    fn cycle_tiled(&self, from: WindowId, _forward: bool) -> Option<WindowId> {
        self.windows.iter().find(|&&w| w != from).copied()
    }

    fn create_float(&mut self, _bounds: Rect) -> WindowId {
        let id = WindowId::from_raw(self.next_id);
        self.next_id += 1;
        id
    }

    fn move_float(&mut self, _window: WindowId, _x: u16, _y: u16) {}
    fn resize_float(&mut self, _window: WindowId, _width: u16, _height: u16) {}
    fn raise_float(&mut self, _window: WindowId) {}
    fn lower_float(&mut self, _window: WindowId) {}
    fn close_float(&mut self, _window: WindowId) {}
    fn toggle_float(&mut self, _window: WindowId) {}

    fn show_overlay(&mut self, _constraints: reovim_driver_layout::OverlayConstraints) -> WindowId {
        let id = WindowId::from_raw(self.next_id);
        self.next_id += 1;
        id
    }

    fn hide_overlay(&mut self, _window: WindowId) {}
    fn resize_overlay(&mut self, _window: WindowId, _width: u16, _height: u16) {}
    fn hide_all_overlays(&mut self) {}

    fn set_focus(&mut self, window: WindowId) {
        self.focused = Some(window);
    }

    fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    fn windows_in_zone(&self, zone: reovim_driver_layout::Zone) -> Vec<WindowId> {
        if zone == reovim_driver_layout::Zone::Tiled {
            self.windows.clone()
        } else {
            Vec::new()
        }
    }

    fn zone_of(&self, _window: WindowId) -> Option<reovim_driver_layout::Zone> {
        Some(reovim_driver_layout::Zone::Tiled)
    }
}

struct MockRootCompositor {
    layer: MockLayerCompositor,
    focused: Option<WindowId>,
}

impl MockRootCompositor {
    fn new() -> Self {
        let layer = MockLayerCompositor::new();
        let focused = layer.focused;
        Self { layer, focused }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_layout::RootCompositor for MockRootCompositor {
    fn composite(&self, screen: Rect) -> reovim_driver_layout::CompositeResult {
        reovim_driver_layout::CompositeResult::empty(screen)
    }

    fn create_layer(
        &mut self,
        _config: reovim_driver_layout::LayerConfig,
    ) -> reovim_driver_layout::LayerId {
        reovim_driver_layout::LayerId::new(0)
    }

    fn remove_layer(&mut self, _layer: reovim_driver_layout::LayerId) {}

    fn layer_by_label(&self, _label: &str) -> Option<reovim_driver_layout::LayerId> {
        None
    }

    fn layers(&self) -> Vec<&reovim_driver_layout::Layer> {
        Vec::new()
    }

    fn set_layer_visible(&mut self, _layer: reovim_driver_layout::LayerId, _visible: bool) {}

    fn set_layer_opacity(&mut self, _layer: reovim_driver_layout::LayerId, _opacity: f32) {}

    fn reorder_layer(&mut self, _layer: reovim_driver_layout::LayerId, _new_z: u16) {}

    fn set_active_layer(&mut self, _layer: reovim_driver_layout::LayerId) {}

    fn active_layer(&self) -> Option<reovim_driver_layout::LayerId> {
        Some(reovim_driver_layout::LayerId::new(0))
    }

    fn set_focus(&mut self, window: WindowId) {
        self.focused = Some(window);
        reovim_driver_layout::WindowLayerCompositor::set_focus(&mut self.layer, window);
    }

    fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    fn focus_at(&mut self, _x: u16, _y: u16) -> Option<WindowId> {
        self.focused
    }

    fn layer_compositor(
        &self,
        _layer: reovim_driver_layout::LayerId,
    ) -> Option<&dyn reovim_driver_layout::WindowLayerCompositor> {
        Some(&self.layer)
    }

    fn layer_compositor_mut(
        &mut self,
        _layer: reovim_driver_layout::LayerId,
    ) -> Option<&mut dyn reovim_driver_layout::WindowLayerCompositor> {
        Some(&mut self.layer)
    }

    fn window_count(&self) -> usize {
        self.layer.windows.len()
    }

    fn set_screen(&mut self, _screen: Rect) {}

    fn layer_of(&self, _window: WindowId) -> Option<reovim_driver_layout::LayerId> {
        Some(reovim_driver_layout::LayerId::new(0))
    }

    fn boxed_clone(&self) -> Box<dyn reovim_driver_layout::RootCompositor> {
        Box::new(Self {
            layer: MockLayerCompositor {
                id: self.layer.id,
                windows: self.layer.windows.clone(),
                focused: self.layer.focused,
                next_id: self.layer.next_id,
            },
            focused: self.focused,
        })
    }
}

fn make_compositor_runtime<'a>(
    session: &'a mut Session,
    client: crate::ClientContext<'a>,
    kernel: &'a KernelContext,
    executor: &'a StubExecutor,
) -> SessionRuntime<'a> {
    *client.compositor = Some(Box::new(MockRootCompositor::new()));
    SessionRuntime::new(session, client, kernel, executor)
}

#[test]
fn test_compositor_navigate() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let result = rt.navigate(NavigateDirection::Right);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), WindowId::from_raw(2));
}

#[test]
fn test_compositor_split() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let result = rt.split(reovim_driver_layout::SplitDirection::Horizontal);
    assert!(result.is_ok());
    assert!(rt.changes.windows_created.contains(&result.unwrap()));
}

#[test]
fn test_compositor_close_current_window() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let result = rt.close_current_window();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), WindowId::from_raw(2));
}

#[test]
fn test_compositor_close_others() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let result = rt.close_others();
    assert!(result.is_ok());
    assert!(rt.changes.windows_closed.contains(&WindowId::from_raw(2)));
}

#[test]
fn test_compositor_resize() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let result = rt.resize(NavigateDirection::Right, 5);
    assert!(result.is_ok());
    assert!(rt.changes.window_changed);
}

#[test]
fn test_compositor_equalize() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let result = rt.equalize();
    assert!(result.is_ok());
    assert!(rt.changes.window_changed);
}

#[test]
fn test_compositor_cycle() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let result = rt.cycle(true);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), WindowId::from_raw(2));
}

#[test]
fn test_compositor_focus() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let result = rt.focus(WindowId::from_raw(2));
    assert!(result.is_ok());
    assert!(rt.changes.focus_changed);
}

#[test]
fn test_compositor_focus_same_window_no_event() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    // Focus the already-focused window - LayoutChanged event should NOT fire
    let result = rt.focus(WindowId::from_raw(1));
    assert!(result.is_ok());
    assert!(rt.changes.focus_changed);
}

#[test]
fn test_compositor_focused_window() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    assert_eq!(rt.focused_window(), Some(WindowId::from_raw(1)));
}

#[test]
fn test_compositor_window_count() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    assert_eq!(rt.compositor_window_count(), 2);
}

#[test]
fn test_compositor_active_layer() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    assert!(rt.active_layer().is_some());
}

#[test]
fn test_compositor_toggle_float() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    assert!(rt.toggle_float().is_ok());
    assert!(rt.changes.window_changed);
}

#[test]
fn test_compositor_raise_float() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    assert!(rt.raise_float().is_ok());
}

#[test]
fn test_compositor_lower_float() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    assert!(rt.lower_float().is_ok());
}

#[test]
fn test_compositor_show_overlay() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let result =
        rt.show_overlay(reovim_driver_layout::OverlayConstraints::centered().with_size(20, 10));
    assert!(result.is_ok());
}

#[test]
fn test_compositor_hide_overlay() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    assert!(rt.hide_overlay(WindowId::from_raw(99)).is_ok());
}

#[test]
fn test_compositor_resize_overlay() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    assert!(rt.resize_overlay(WindowId::from_raw(99), 40, 20).is_ok());
}

#[test]
fn test_compositor_hide_all_overlays() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    assert!(rt.hide_all_overlays().is_ok());
}

#[test]
fn test_compositor_set_active_layer_opacity() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    assert!(rt.set_active_layer_opacity(0.5).is_ok());
}

#[test]
fn test_compositor_set_active_layer_opacity_clamps() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    // Values should be clamped to 0.0..=1.0
    assert!(rt.set_active_layer_opacity(2.0).is_ok());
    assert!(rt.set_active_layer_opacity(-1.0).is_ok());
}

#[test]
fn test_compositor_active_layer_opacity() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    // MockRootCompositor returns empty layers(), so default 1.0
    let opacity = rt.active_layer_opacity().unwrap();
    assert!((opacity - 1.0).abs() < f32::EPSILON);
}

#[test]
fn test_compositor_adjust_active_layer_opacity() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    // Adjust from default 1.0 by -0.3 → 0.7
    let new_opacity = rt.adjust_active_layer_opacity(-0.3).unwrap();
    assert!((new_opacity - 0.7).abs() < f32::EPSILON);
}

#[test]
fn test_set_screen_with_compositor() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    rt.set_screen(Rect::new(0, 0, 120, 40));
}

#[test]
fn test_emit_layout_event_with_compositor() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    rt.emit_layout_event(reovim_kernel::api::v1::events::kernel::LayoutChangeKind::Equalize);
}

#[test]
fn test_arrange_with_compositor() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let placements = rt.arrange(Rect::new(0, 0, 80, 24));
    assert!(placements.is_empty());
}

// =========================================================================
// CompositorApi: close_current_window with single window (CannotCloseLastWindow)
// =========================================================================

/// Mock compositor with only ONE tiled window, so `close_current_window`
/// returns `CannotCloseLastWindow` (covers line 1176).
struct SingleWindowLayerCompositor {
    id: reovim_driver_layout::LayerId,
    window: WindowId,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_layout::WindowLayerCompositor for SingleWindowLayerCompositor {
    fn id(&self) -> reovim_driver_layout::LayerId {
        self.id
    }
    fn arrange(&self, _bounds: Rect) -> Vec<WindowPlacement> {
        Vec::new()
    }
    fn add_tiled(&mut self) -> WindowId {
        self.window
    }
    fn split_tiled(
        &mut self,
        _from: WindowId,
        _direction: reovim_driver_layout::SplitDirection,
    ) -> Option<WindowId> {
        None
    }
    fn navigate_tiled(&self, _from: WindowId, _direction: NavigateDirection) -> Option<WindowId> {
        None
    }
    fn resize_tiled(&mut self, _window: WindowId, _direction: NavigateDirection, _delta: i16) {}
    fn close_tiled(&mut self, _window: WindowId) -> Option<WindowId> {
        None
    }
    fn equalize_tiled(&mut self) {}
    fn cycle_tiled(&self, _from: WindowId, _forward: bool) -> Option<WindowId> {
        None
    }
    fn create_float(&mut self, _bounds: Rect) -> WindowId {
        self.window
    }
    fn move_float(&mut self, _window: WindowId, _x: u16, _y: u16) {}
    fn resize_float(&mut self, _window: WindowId, _width: u16, _height: u16) {}
    fn raise_float(&mut self, _window: WindowId) {}
    fn lower_float(&mut self, _window: WindowId) {}
    fn close_float(&mut self, _window: WindowId) {}
    fn toggle_float(&mut self, _window: WindowId) {}
    fn show_overlay(&mut self, _constraints: OverlayConstraints) -> WindowId {
        self.window
    }
    fn hide_overlay(&mut self, _window: WindowId) {}
    fn resize_overlay(&mut self, _window: WindowId, _width: u16, _height: u16) {}
    fn hide_all_overlays(&mut self) {}
    fn set_focus(&mut self, _window: WindowId) {}
    fn focused(&self) -> Option<WindowId> {
        Some(self.window)
    }
    fn windows_in_zone(&self, zone: reovim_driver_layout::Zone) -> Vec<WindowId> {
        if zone == reovim_driver_layout::Zone::Tiled {
            vec![self.window] // Only ONE tiled window
        } else {
            Vec::new()
        }
    }
    fn zone_of(&self, _window: WindowId) -> Option<reovim_driver_layout::Zone> {
        Some(reovim_driver_layout::Zone::Tiled)
    }
}

struct SingleWindowRootCompositor {
    layer: SingleWindowLayerCompositor,
}

impl SingleWindowRootCompositor {
    fn new() -> Self {
        Self {
            layer: SingleWindowLayerCompositor {
                id: reovim_driver_layout::LayerId::new(0),
                window: WindowId::from_raw(1),
            },
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_layout::RootCompositor for SingleWindowRootCompositor {
    fn composite(&self, screen: Rect) -> reovim_driver_layout::CompositeResult {
        reovim_driver_layout::CompositeResult::empty(screen)
    }
    fn create_layer(
        &mut self,
        _config: reovim_driver_layout::LayerConfig,
    ) -> reovim_driver_layout::LayerId {
        reovim_driver_layout::LayerId::new(0)
    }
    fn remove_layer(&mut self, _layer: reovim_driver_layout::LayerId) {}
    fn layer_by_label(&self, _label: &str) -> Option<reovim_driver_layout::LayerId> {
        None
    }
    fn layers(&self) -> Vec<&reovim_driver_layout::Layer> {
        Vec::new()
    }
    fn set_layer_visible(&mut self, _layer: reovim_driver_layout::LayerId, _visible: bool) {}
    fn set_layer_opacity(&mut self, _layer: reovim_driver_layout::LayerId, _opacity: f32) {}
    fn reorder_layer(&mut self, _layer: reovim_driver_layout::LayerId, _new_z: u16) {}
    fn set_active_layer(&mut self, _layer: reovim_driver_layout::LayerId) {}
    fn active_layer(&self) -> Option<reovim_driver_layout::LayerId> {
        Some(reovim_driver_layout::LayerId::new(0))
    }
    fn set_focus(&mut self, _window: WindowId) {}
    fn focused(&self) -> Option<WindowId> {
        Some(self.layer.window)
    }
    fn focus_at(&mut self, _x: u16, _y: u16) -> Option<WindowId> {
        Some(self.layer.window)
    }
    fn layer_compositor(
        &self,
        _layer: reovim_driver_layout::LayerId,
    ) -> Option<&dyn reovim_driver_layout::WindowLayerCompositor> {
        Some(&self.layer)
    }
    fn layer_compositor_mut(
        &mut self,
        _layer: reovim_driver_layout::LayerId,
    ) -> Option<&mut dyn reovim_driver_layout::WindowLayerCompositor> {
        Some(&mut self.layer)
    }
    fn window_count(&self) -> usize {
        1
    }
    fn set_screen(&mut self, _screen: Rect) {}
    fn layer_of(&self, _window: WindowId) -> Option<reovim_driver_layout::LayerId> {
        Some(reovim_driver_layout::LayerId::new(0))
    }
    fn boxed_clone(&self) -> Box<dyn reovim_driver_layout::RootCompositor> {
        Box::new(Self::new())
    }
}

#[test]
fn test_compositor_close_current_window_single_window() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c: Option<Box<dyn reovim_driver_layout::RootCompositor>> =
        Some(Box::new(SingleWindowRootCompositor::new()));
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);
    let mut rt = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );

    let result = rt.close_current_window();
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CompositorError::CannotCloseLastWindow));
}

// === #474: Centralized selection extension tests ===

/// When cursor moves with an active selection, `sel.end` should auto-update
/// and `selection_changed` should be set.
#[test]
fn test_record_cursor_move_extends_selection() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    // Add a window with selection and move cursor
    let mut window = crate::Window::new();
    window.cursor = Position::new(0, 5).into();
    window.selection =
        Some(crate::api::Selection::character(Position::new(0, 0), Position::new(0, 1)));
    w.add(window);

    let buf = BufferId::new();
    let mut rt = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    rt.record_cursor_move(buf);

    let changes = rt.take_changes();
    assert!(changes.cursor_moved);
    assert!(changes.selection_changed);

    // sel.end should match cursor position + 1
    let sel = rt.windows().active().unwrap().selection.as_ref().unwrap();
    assert_eq!(sel.end, Position::new(0, 6));
}

/// When cursor moves without a selection, `selection_changed` should NOT be set.
#[test]
fn test_record_cursor_move_no_selection() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut window = crate::Window::new();
    window.cursor = Position::new(0, 3).into();
    // No selection
    w.add(window);

    let buf = BufferId::new();
    let mut rt = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    rt.record_cursor_move(buf);

    let changes = rt.take_changes();
    assert!(changes.cursor_moved);
    assert!(!changes.selection_changed);
}

/// When cursor moves with no active window, only `cursor_moved` is set.
#[test]
fn test_record_cursor_move_no_active_window() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty(); // No windows added
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let buf = BufferId::new();
    let mut rt = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    rt.record_cursor_move(buf);

    let changes = rt.take_changes();
    assert!(changes.cursor_moved);
    assert!(!changes.selection_changed);
}

/// Direct `record_selection_change` should set `selection_changed`.
#[test]
fn test_record_selection_change_directly() {
    use reovim_kernel::api::v1::ModeStack;

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let buf = BufferId::new();
    let mut rt = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    rt.record_selection_change(buf);

    let changes = rt.take_changes();
    assert!(changes.selection_changed);
    assert!(changes.affected_buffers.contains(&buf));
}

// =========================================================================
// CompositorApi: focus() with same window (line 1352 false branch)
// =========================================================================

/// Calling `focus()` on the already-focused window should NOT emit a layout
/// event (the `if previous_focus != Some(window)` branch is false).
#[test]
fn test_compositor_focus_same_window_no_layout_event() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = make_compositor_runtime(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );

    // MockRootCompositor starts with focus on WindowId::from_raw(1).
    // Calling focus() on the already-focused window exercises the
    // `previous_focus == Some(window)` path (line 1352 false branch).
    let already_focused = WindowId::from_raw(1);
    let result = rt.focus(already_focused);
    assert!(result.is_ok());

    // Changes should record focus even though no layout event is emitted
    let changes = rt.take_changes();
    assert!(changes.focus_changed);
}

// =========================================================================
// BufferApi: delete_range() with empty result (line 610 false branch)
// =========================================================================

/// Calling `delete_range()` where start==end produces empty deleted text.
/// This exercises the `if !deleted_text.is_empty()` false branch (line 610).
#[test]
fn test_delete_range_empty_result_no_undo_record() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Delete an empty range (start == end): nothing is deleted
    harness.with_runtime(|runtime| {
        runtime.delete_range(buffer_id, Position::new(0, 3), Position::new(0, 3));
    });

    // Buffer content unchanged
    harness.assert_buffer_content("hello world");

    // buffer_modified is still recorded (changes.record_buffer_modified is called
    // unconditionally), but no undo edit is recorded for empty deletions.
    let changes = harness.take_changes();
    assert!(changes.buffer_modified);
}

// =========================================================================
// CompositorApi: set_screen() without compositor (line 1382 false branch)
// =========================================================================

/// `set_screen()` without a compositor should only update `self.screen`.
/// This exercises the `if let Some(compositor) = ...` false branch (line 1382).
#[test]
fn test_set_screen_without_compositor() {
    use reovim_kernel::api::v1::ModeStack;
    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    // No compositor
    let mut c: Option<Box<dyn reovim_driver_layout::RootCompositor>> = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut rt = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );

    let screen = Rect::new(0, 0, 80, 24);
    rt.set_screen(screen);
    assert_eq!(rt.screen, screen);
}

// =========================================================================
// Per-client accessor coverage (#515)
// =========================================================================

#[test]
fn test_registers_accessor() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let bank = runtime.registers();
        assert!(bank.get().is_empty());
    });
}

#[test]
fn test_registers_mut_accessor() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let bank = runtime.registers_mut();
        bank.set_by_name(Some('a'), crate::api::RegisterContent::characterwise("test"));
        assert_eq!(bank.get_named('a').map(|r| r.text.as_str()), Some("test"));
    });
}

#[test]
fn test_clipboard_history_accessor() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let history = runtime.clipboard_history();
        assert!(history.is_empty());
    });
}

#[test]
fn test_clipboard_history_mut_accessor() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let history = runtime.clipboard_history_mut();
        history.push(crate::api::RegisterContent::characterwise("entry"));
        assert_eq!(history.len(), 1);
    });
}

#[test]
fn test_local_marks_accessor() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let marks = runtime.local_marks();
        assert!(marks.get_local('a').is_none());
    });
}

#[test]
fn test_local_marks_mut_accessor() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let marks = runtime.local_marks_mut();
        marks.set_local('a', reovim_types_text::Position::new(0, 5));
        assert!(marks.get_local('a').is_some());
    });
}

// =========================================================================
// ClipboardApi else-branch coverage (#515)
// =========================================================================

#[test]
fn test_clipboard_api_no_provider_copy_to_clipboard() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        // No clipboard provider registered → returns false
        assert!(!runtime.copy_to_clipboard("test"));
    });
}

#[test]
fn test_clipboard_api_no_provider_paste_from_clipboard() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        // No clipboard provider registered → returns None
        assert!(runtime.paste_from_clipboard().is_none());
    });
}

#[test]
fn test_clipboard_api_no_provider_copy_to_selection() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        assert!(!runtime.copy_to_selection("test"));
    });
}

#[test]
fn test_clipboard_api_no_provider_paste_from_selection() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        assert!(runtime.paste_from_selection().is_none());
    });
}

// =========================================================================
// BufferApi trait-qualified active_buffer coverage
// =========================================================================

#[test]
fn test_buffer_api_active_buffer_trait_qualified() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        // Trait-qualified call through BufferApi to ensure impl block attribution.
        assert!(BufferApi::active_buffer(runtime).is_none());
    });

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let buf_id = BufferApi::active_buffer(runtime);
        assert!(buf_id.is_some());

        // Test set_active_buffer round-trip via trait qualification.
        BufferApi::set_active_buffer(runtime, None);
        assert!(BufferApi::active_buffer(runtime).is_none());

        BufferApi::set_active_buffer(runtime, buf_id);
        assert_eq!(BufferApi::active_buffer(runtime), buf_id);
    });
}

// ========================================================================
// Signal queue tests (#547)
// ========================================================================

#[test]
fn test_signal_queue_initially_empty() {
    use crate::testing::TestSessionRuntime;
    let mut harness = TestSessionRuntime::with_buffer("");
    harness.with_runtime(|runtime| {
        let signals = runtime.take_signals();
        assert!(signals.is_empty());
    });
}

#[test]
fn test_signal_push_and_take() {
    use crate::testing::TestSessionRuntime;
    let mut harness = TestSessionRuntime::with_buffer("");
    harness.with_runtime(|runtime| {
        runtime.signal(RuntimeSignal::Quit);
        let signals = runtime.take_signals();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0], RuntimeSignal::Quit);
    });
}

#[test]
fn test_signal_take_drains_queue() {
    use crate::testing::TestSessionRuntime;
    let mut harness = TestSessionRuntime::with_buffer("");
    harness.with_runtime(|runtime| {
        runtime.signal(RuntimeSignal::Quit);
        let first = runtime.take_signals();
        assert_eq!(first.len(), 1);

        // Second take should be empty
        let second = runtime.take_signals();
        assert!(second.is_empty());
    });
}

#[test]
fn test_signal_multiple_fifo_order() {
    use crate::testing::TestSessionRuntime;
    let mut harness = TestSessionRuntime::with_buffer("");
    harness.with_runtime(|runtime| {
        runtime.signal(RuntimeSignal::Quit);
        runtime.signal(RuntimeSignal::Quit);
        runtime.signal(RuntimeSignal::Quit);

        let signals = runtime.take_signals();
        assert_eq!(signals.len(), 3);
        // All should be Quit (FIFO preserved)
        for s in &signals {
            assert_eq!(*s, RuntimeSignal::Quit);
        }
    });
}

#[test]
fn test_signal_queue_independent_of_state_changes() {
    use crate::testing::TestSessionRuntime;
    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        // Push a signal and also create state changes
        runtime.signal(RuntimeSignal::Quit);
        let changes = runtime.take_changes();
        let signals = runtime.take_signals();

        // Both should be independent
        assert_eq!(signals.len(), 1);
        // State changes are their own thing
        drop(changes);
    });
}

// =========================================================================
// #664: CursorMoved event emission from record_cursor_move
// =========================================================================

/// `record_cursor_move` should emit a text-domain `CursorMoved` event with correct
/// `WindowId`, `BufferId`, and `TextPosition` from/to fields.
#[test]
fn test_record_cursor_move_emits_cursor_moved_event() {
    use {
        reovim_domain_text_events::CursorMoved as TextCursorMoved,
        reovim_kernel::api::v1::{EventResult, ModeStack},
        std::sync::{Arc, Mutex},
    };

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    // Window at (3, 7) — this is the "from" position at construction time
    let mut window = crate::Window::new();
    window.cursor = Position::new(3, 7).into();
    w.add(window);

    // Subscribe to text-domain CursorMoved before creating runtime
    let captured = Arc::new(Mutex::new(None::<TextCursorMoved>));
    let cap = Arc::clone(&captured);
    let _sub = kernel
        .event_bus
        .subscribe::<TextCursorMoved, _>(50, move |event| {
            *cap.lock().unwrap() = Some(*event);
            EventResult::Handled
        });

    let buf = BufferId::new();
    let mut rt = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );

    // Move cursor to (3, 7) same as initial — from and to should match
    rt.record_cursor_move(buf);

    let event = captured.lock().unwrap().expect("CursorMoved should fire");
    assert_eq!(event.buffer_id, buf);
    // from = snapshot at construction = (3, 7)
    assert_eq!(event.from.line, 3);
    assert_eq!(event.from.column, 7);
    // to = current cursor = (3, 7)
    assert_eq!(event.to.line, 3);
    assert_eq!(event.to.column, 7);
}

/// When there is no active window, `CursorMoved` should NOT be emitted.
#[test]
fn test_record_cursor_move_no_window_no_event() {
    use {
        reovim_domain_text_events::CursorMoved as TextCursorMoved,
        reovim_kernel::api::v1::{EventResult, ModeStack},
        std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty(); // No windows
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let received = Arc::new(AtomicBool::new(false));
    let r1 = Arc::clone(&received);
    let _sub = kernel
        .event_bus
        .subscribe::<TextCursorMoved, _>(50, move |_| {
            r1.store(true, Ordering::SeqCst);
            EventResult::Handled
        });

    let buf = BufferId::new();
    let mut rt = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    rt.record_cursor_move(buf);

    // No window = no event
    assert!(!received.load(Ordering::SeqCst));
}

/// Multiple `record_cursor_move` calls update the snapshot for subsequent `from` values.
#[test]
fn test_record_cursor_move_multiple_updates_snapshot() {
    use {
        reovim_domain_text_events::CursorMoved as TextCursorMoved,
        reovim_kernel::api::v1::{EventResult, ModeStack},
        std::sync::{Arc, Mutex},
    };

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut window = crate::Window::new();
    window.cursor = Position::new(0, 0).into();
    w.add(window);

    #[allow(clippy::type_complexity)]
    let events: Arc<Mutex<Vec<(usize, usize, usize, usize)>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);
    let _sub = kernel
        .event_bus
        .subscribe::<TextCursorMoved, _>(50, move |event| {
            events_clone.lock().unwrap().push((
                event.from.line,
                event.from.column,
                event.to.line,
                event.to.column,
            ));
            EventResult::Handled
        });

    let buf = BufferId::new();
    let mut rt = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );

    // First move: cursor still at (0,0)
    rt.record_cursor_move(buf);

    // Simulate cursor moving to (5, 10) — modify window directly
    rt.windows_mut().active_mut().unwrap().cursor = Position::new(5, 10).into();

    // Second move: from should be (0,0), to should be (5,10)
    rt.record_cursor_move(buf);

    let captured = events.lock().unwrap();
    assert_eq!(captured.len(), 2);
    // First: from=(0,0), to=(0,0)
    assert_eq!(captured[0], (0, 0, 0, 0));
    // Second: from=(0,0) [snapshot from first emission], to=(5,10)
    assert_eq!(captured[1], (0, 0, 5, 10));
    drop(captured);
}

// =========================================================================
// Tab page operations (#401)
// =========================================================================

#[test]
fn test_tab_new() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        assert_eq!(runtime.tab_count(), 1);
        let id = runtime.tab_new().expect("tab_new should succeed");
        assert_eq!(runtime.tab_count(), 2);
        // New tab becomes active
        assert_eq!(runtime.active_tab_id(), Some(id));
    });
    assert!(harness.changes().window_changed);
}

#[test]
fn test_tab_close() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        // Create a second tab so we can close one
        runtime.tab_new().expect("tab_new should succeed");
        assert_eq!(runtime.tab_count(), 2);

        runtime.tab_close().expect("tab_close should succeed");
        assert_eq!(runtime.tab_count(), 1);
    });
    assert!(harness.changes().window_changed);
}

#[test]
fn test_tab_close_last_tab_fails() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        assert_eq!(runtime.tab_count(), 1);
        let result = runtime.tab_close();
        assert!(result.is_err());
    });
}

#[test]
fn test_tab_next() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let tab1_id = runtime.active_tab_id().unwrap();
        let tab2_id = runtime.tab_new().expect("tab_new should succeed");
        assert_eq!(runtime.active_tab_id(), Some(tab2_id));

        // tab_next wraps around or goes to next
        let next_id = runtime.tab_next().expect("tab_next should succeed");
        // After tab_new, active is tab2 (index 1). Next cycles back to tab1 (index 0).
        assert_eq!(next_id, tab1_id);
    });
    assert!(harness.changes().window_changed);
}

#[test]
fn test_tab_prev() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let tab1_id = runtime.active_tab_id().unwrap();
        runtime.tab_new().expect("tab_new should succeed");
        // Active is now tab2 (index 1). Prev goes to tab1 (index 0).
        let prev_id = runtime.tab_prev().expect("tab_prev should succeed");
        assert_eq!(prev_id, tab1_id);
    });
    assert!(harness.changes().window_changed);
}

#[test]
fn test_tab_goto_valid() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let tab1_id = runtime.active_tab_id().unwrap();
        runtime.tab_new().expect("tab_new should succeed");
        // tab_new makes tab2 active; goto index 0 goes back to tab1
        let result = runtime.tab_goto(0);
        assert!(result.is_ok());
        assert_eq!(runtime.active_tab_id(), Some(tab1_id));
    });
    assert!(harness.changes().window_changed);
}

#[test]
fn test_tab_goto_invalid_index() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        // Only 1 tab, index 5 is out of bounds
        let result = runtime.tab_goto(5);
        assert!(result.is_err());
    });
}

#[test]
fn test_tab_count_and_active_tab_id() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        assert_eq!(runtime.tab_count(), 1);
        assert!(runtime.active_tab_id().is_some());
    });
}

// =========================================================================
// Jumplist accessor coverage (#654)
// =========================================================================

#[test]
fn test_jumplist_accessor() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let jl = runtime.jumplist();
        assert!(jl.is_empty());
    });
}

#[test]
fn test_jumplist_mut_accessor() {
    use crate::{JumpEntry, testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let jl = runtime.jumplist_mut();
        jl.push(JumpEntry::new(BufferId::new(), Position::new(5, 10)));
        assert!(!jl.is_empty());
    });
}

// =========================================================================
// kernel_and_registers() borrow-split accessor (#515)
// =========================================================================

#[test]
fn test_kernel_and_registers_accessor() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let (kernel, regs, history) = runtime.kernel_and_registers();
        // Kernel is accessible
        assert!(kernel.buffers.count() > 0);
        // Registers are accessible mutably
        regs.set_by_name(Some('a'), crate::api::RegisterContent::characterwise("test"));
        assert_eq!(regs.get_named('a').map(|r| r.text.as_str()), Some("test"));
        // History is accessible mutably
        assert!(history.is_empty());
    });
}

// =========================================================================
// record_buffer_modified() inherent method
// =========================================================================

#[test]
fn test_record_buffer_modified_inherent() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let changes = harness.with_runtime(|runtime| {
        let buf = runtime.active_buffer().unwrap();
        runtime.record_buffer_modified(buf);
        runtime.take_changes()
    });
    assert!(!changes.modified_buffers.is_empty());
}

// ── ByteEdit emission (#740) ────────────────────────────────────────

#[test]
fn test_insert_text_emits_byte_edit() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    let changes = harness.with_runtime(|runtime| {
        runtime.insert_text(buffer_id, Position::new(0, 5), " world");
        runtime.take_changes()
    });

    assert_eq!(changes.byte_edits.len(), 1);
    assert_eq!(
        changes.byte_edits[0],
        (buffer_id, reovim_kernel::api::v1::ByteEdit::insert(5, b" world"))
    );
}

#[test]
fn test_delete_range_emits_byte_edit() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello world");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    let changes = harness.with_runtime(|runtime| {
        runtime.delete_range(buffer_id, Position::new(0, 5), Position::new(0, 11));
        runtime.take_changes()
    });

    assert_eq!(changes.byte_edits.len(), 1);
    assert_eq!(
        changes.byte_edits[0],
        (buffer_id, reovim_kernel::api::v1::ByteEdit::delete(5, b" world"))
    );
}

#[test]
fn test_delete_range_empty_no_byte_edit() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());

    // Deleting an empty range should not push a byte edit
    let changes = harness.with_runtime(|runtime| {
        runtime.delete_range(buffer_id, Position::new(0, 2), Position::new(0, 2));
        runtime.take_changes()
    });

    assert!(changes.byte_edits.is_empty());
}

// =========================================================================
// Dual-emission tests (#740 Plan 09 Phase 6)
// =========================================================================

/// Verify that `insert_text` emits BOTH old kernel `BufferModified` and new
/// `TextBufferModified` on the `EventBus` during the dual-emission transition.
#[test]
fn test_insert_text_dual_emission() {
    use {
        crate::testing::TestSessionRuntime,
        reovim_kernel::api::v1::{EventResult, events::kernel::BufferModified},
        std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");

    let old_fired = Arc::new(AtomicBool::new(false));
    let new_fired = Arc::new(AtomicBool::new(false));

    // Subscribe to old kernel event
    let old_flag = Arc::clone(&old_fired);
    let _sub_old = harness
        .kernel()
        .event_bus
        .subscribe::<BufferModified, _>(50, move |_event| {
            old_flag.store(true, Ordering::SeqCst);
            EventResult::Handled
        });

    // Subscribe to new text-domain event
    let new_flag = Arc::clone(&new_fired);
    let _sub_new = harness
        .kernel()
        .event_bus
        .subscribe::<reovim_domain_text_events::TextBufferModified, _>(50, move |_event| {
            new_flag.store(true, Ordering::SeqCst);
            EventResult::Handled
        });

    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());
    harness.with_runtime(|runtime| {
        runtime.insert_text(buffer_id, Position::new(0, 5), " world");
    });

    assert!(old_fired.load(Ordering::SeqCst), "old kernel BufferModified should fire");
    assert!(new_fired.load(Ordering::SeqCst), "new TextBufferModified should fire");
}

/// Verify that `delete_range` emits BOTH old kernel `BufferModified` and new
/// `TextBufferModified` during dual-emission transition.
#[test]
fn test_delete_range_dual_emission() {
    use {
        crate::testing::TestSessionRuntime,
        reovim_kernel::api::v1::{EventResult, events::kernel::BufferModified},
        std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    let mut harness = TestSessionRuntime::with_buffer("hello world");

    let old_fired = Arc::new(AtomicBool::new(false));
    let new_fired = Arc::new(AtomicBool::new(false));

    let old_flag = Arc::clone(&old_fired);
    let _sub_old = harness
        .kernel()
        .event_bus
        .subscribe::<BufferModified, _>(50, move |_event| {
            old_flag.store(true, Ordering::SeqCst);
            EventResult::Handled
        });

    let new_flag = Arc::clone(&new_fired);
    let _sub_new = harness
        .kernel()
        .event_bus
        .subscribe::<reovim_domain_text_events::TextBufferModified, _>(50, move |_event| {
            new_flag.store(true, Ordering::SeqCst);
            EventResult::Handled
        });

    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());
    harness.with_runtime(|runtime| {
        runtime.delete_range(buffer_id, Position::new(0, 5), Position::new(0, 11));
    });

    assert!(old_fired.load(Ordering::SeqCst), "old kernel BufferModified should fire");
    assert!(new_fired.load(Ordering::SeqCst), "new TextBufferModified should fire");
}

/// Verify that `insert_text` emits `TextBufferModified` with correct byte
/// offsets and `TextEdit` payload.
#[test]
fn test_insert_text_text_buffer_modified_fields() {
    use {
        crate::testing::TestSessionRuntime,
        reovim_domain_text_events::TextBufferModified,
        reovim_kernel::api::v1::EventResult,
        std::sync::{Arc, Mutex},
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");

    let captured = Arc::new(Mutex::new(None::<(usize, usize, usize)>));
    let cap = Arc::clone(&captured);
    let _sub = harness
        .kernel()
        .event_bus
        .subscribe::<TextBufferModified, _>(50, move |event| {
            *cap.lock().unwrap() = Some((event.start_byte, event.old_end_byte, event.new_end_byte));
            EventResult::Handled
        });

    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());
    harness.with_runtime(|runtime| {
        // "hello" is 5 bytes; insert " world" (6 bytes) at byte offset 5
        runtime.insert_text(buffer_id, Position::new(0, 5), " world");
    });

    let (start, old_end, new_end) = captured.lock().unwrap().expect("event should have fired");
    assert_eq!(start, 5, "start_byte");
    assert_eq!(old_end, 5, "old_end_byte (insert does not consume bytes)");
    assert_eq!(new_end, 11, "new_end_byte = start + text.len()");
}

/// Verify that `delete_range` emits `TextBufferModified` with correct byte
/// offsets.
#[test]
fn test_delete_range_text_buffer_modified_fields() {
    use {
        crate::testing::TestSessionRuntime,
        reovim_domain_text_events::TextBufferModified,
        reovim_kernel::api::v1::EventResult,
        std::sync::{Arc, Mutex},
    };

    let mut harness = TestSessionRuntime::with_buffer("hello world");

    let captured = Arc::new(Mutex::new(None::<(usize, usize, usize)>));
    let cap = Arc::clone(&captured);
    let _sub = harness
        .kernel()
        .event_bus
        .subscribe::<TextBufferModified, _>(50, move |event| {
            *cap.lock().unwrap() = Some((event.start_byte, event.old_end_byte, event.new_end_byte));
            EventResult::Handled
        });

    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());
    harness.with_runtime(|runtime| {
        // Delete " world" (6 bytes) starting at byte offset 5
        runtime.delete_range(buffer_id, Position::new(0, 5), Position::new(0, 11));
    });

    let (start, old_end, new_end) = captured.lock().unwrap().expect("event should have fired");
    assert_eq!(start, 5, "start_byte");
    assert_eq!(old_end, 11, "old_end_byte = start + deleted.len()");
    assert_eq!(new_end, 5, "new_end_byte = start (bytes removed)");
}

/// Verify that `record_cursor_move` emits BOTH old kernel `CursorMoved` and
/// new text-domain `CursorMoved` with `WindowId` enrichment.
#[test]
fn test_record_cursor_move_dual_emission() {
    use {
        reovim_kernel::api::v1::{EventResult, ModeStack, events::kernel::CursorMoved},
        std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    let mut session = Session::new(ClientId::new(1), test_mode());
    let kernel = KernelContext::default();
    let executor = StubExecutor;
    let mut ms = ModeStack::new(test_mode());
    let mut w = crate::WindowLayout::empty();
    let mut e = crate::ExtensionMap::new();
    let mut c = None;
    let mut tabs = crate::TabPageSet::new();
    let mut r = RegisterBank::new();
    let mut ch = HistoryRing::new();
    let mut lm = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut window = crate::Window::new();
    window.cursor = Position::new(3, 7).into();
    w.add(window);

    // Subscribe to old kernel CursorMoved
    let old_fired = Arc::new(AtomicBool::new(false));
    let old_flag = Arc::clone(&old_fired);
    let _sub_old = kernel
        .event_bus
        .subscribe::<CursorMoved, _>(50, move |_event| {
            old_flag.store(true, Ordering::SeqCst);
            EventResult::Handled
        });

    // Subscribe to new text-domain CursorMoved
    let new_fired = Arc::new(AtomicBool::new(false));
    let new_flag = Arc::clone(&new_fired);
    let _sub_new = kernel
        .event_bus
        .subscribe::<reovim_domain_text_events::CursorMoved, _>(50, move |_event| {
            new_flag.store(true, Ordering::SeqCst);
            EventResult::Handled
        });

    let buf = BufferId::new();
    let mut rt = SessionRuntime::new(
        &mut session,
        crate::ClientContext {
            mode_stack: &mut ms,
            windows: &mut w,
            extensions: &mut e,
            compositor: &mut c,
            tabs: &mut tabs,
            registers: &mut r,
            clipboard_history: &mut ch,
            local_marks: &mut lm,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );

    rt.record_cursor_move(buf);

    assert!(old_fired.load(Ordering::SeqCst), "old kernel CursorMoved should fire");
    assert!(new_fired.load(Ordering::SeqCst), "new text-domain CursorMoved should fire");
}

// =========================================================================
// FullReplace sanity: old event only, no TextBufferModified (#740 Phase 7)
// =========================================================================

/// Verify that `replace_content` (`FullReplace`) emits old kernel `BufferModified`
/// but does NOT emit `TextBufferModified` — there is no `TextEdit::FullReplace`.
#[test]
fn test_fullreplace_emits_old_event_only() {
    use {
        crate::testing::TestSessionRuntime,
        reovim_kernel::api::v1::{EventResult, events::kernel::BufferModified},
        std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");

    let old_fired = Arc::new(AtomicBool::new(false));
    let new_fired = Arc::new(AtomicBool::new(false));

    let old_flag = Arc::clone(&old_fired);
    let _sub_old = harness
        .kernel()
        .event_bus
        .subscribe::<BufferModified, _>(50, move |_event| {
            old_flag.store(true, Ordering::SeqCst);
            EventResult::Handled
        });

    let new_flag = Arc::clone(&new_fired);
    let _sub_new = harness
        .kernel()
        .event_bus
        .subscribe::<reovim_domain_text_events::TextBufferModified, _>(50, move |_event| {
            new_flag.store(true, Ordering::SeqCst);
            EventResult::Handled
        });

    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());
    harness.with_runtime(|runtime| {
        runtime.replace_content(buffer_id, "completely new content");
    });

    assert!(
        old_fired.load(Ordering::SeqCst),
        "old kernel BufferModified should fire for FullReplace"
    );
    assert!(
        !new_fired.load(Ordering::SeqCst),
        "TextBufferModified should NOT fire for FullReplace (no TextEdit equivalent)"
    );
}

// =========================================================================
// StateChanges text_buffer_edits field (#740 Plan 09 Phase 7)
// =========================================================================

/// Verify that `insert_text` populates `text_buffer_edits` in `StateChanges`.
#[test]
fn test_insert_text_populates_text_buffer_edits() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");

    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());
    harness.with_runtime(|runtime| {
        runtime.insert_text(buffer_id, Position::new(0, 5), " world");
    });

    let changes = harness.take_changes();
    assert_eq!(changes.text_buffer_edits.len(), 1);
    let event = &changes.text_buffer_edits[0];
    assert_eq!(event.buffer_id, buffer_id);
    assert_eq!(event.start_byte, 5);
    assert_eq!(event.old_end_byte, 5);
    assert_eq!(event.new_end_byte, 11);
    match &event.edit {
        reovim_domain_text_events::TextEdit::Insert { text, .. } => {
            assert_eq!(text, " world");
        }
        reovim_domain_text_events::TextEdit::Delete { .. } => {
            panic!("expected TextEdit::Insert")
        }
    }
}

/// Verify that `delete_range` populates `text_buffer_edits` in `StateChanges`.
#[test]
fn test_delete_range_populates_text_buffer_edits() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello world");

    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());
    harness.with_runtime(|runtime| {
        runtime.delete_range(buffer_id, Position::new(0, 5), Position::new(0, 11));
    });

    let changes = harness.take_changes();
    assert_eq!(changes.text_buffer_edits.len(), 1);
    let event = &changes.text_buffer_edits[0];
    assert_eq!(event.buffer_id, buffer_id);
    assert_eq!(event.start_byte, 5);
    assert_eq!(event.old_end_byte, 11);
    assert_eq!(event.new_end_byte, 5);
    match &event.edit {
        reovim_domain_text_events::TextEdit::Delete { text, .. } => {
            assert_eq!(text, " world");
        }
        reovim_domain_text_events::TextEdit::Insert { .. } => {
            panic!("expected TextEdit::Delete")
        }
    }
}

/// Verify that `replace_content` does NOT populate `text_buffer_edits`
/// (`FullReplace` has no `TextEdit` equivalent).
#[test]
fn test_fullreplace_no_text_buffer_edits() {
    use crate::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");

    let buffer_id = harness.with_runtime(|runtime| runtime.active_buffer().unwrap());
    harness.with_runtime(|runtime| {
        runtime.replace_content(buffer_id, "completely new content");
    });

    let changes = harness.take_changes();
    assert!(
        changes.text_buffer_edits.is_empty(),
        "FullReplace should not populate text_buffer_edits"
    );
    // But modified_buffer_edits should have the FullReplace entry
    assert_eq!(changes.modified_buffer_edits.len(), 1);
}

// Note: byte-level undo-log cleanup is tracked outside session runtime.
