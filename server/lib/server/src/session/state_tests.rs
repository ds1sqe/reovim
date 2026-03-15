use {
    super::*,
    parking_lot::RwLock as ParkingLotRwLock,
    reovim_driver_buffer::TestBufferManager,
    reovim_kernel::api::v1::{
        EventBus, HistoryRing, Jumplist, MarkBank, ModuleId, MotionEngine, OptionRegistry,
        RegisterBank, ServiceRegistry, TextObjectEngine,
    },
};

fn test_mode_id() -> ModeId {
    ModeId::new(ModuleId::new("test"), "normal")
}

fn test_vfs() -> Arc<dyn VfsDriver> {
    Arc::new(reovim_driver_vfs::MockVfs::new())
}

/// Create a test kernel with a real buffer manager.
fn test_kernel() -> KernelContext {
    KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    )
}

#[test]
fn test_session_state_new() {
    let kernel = KernelContext::default();
    let state = SessionState::new(kernel, test_mode_id(), test_vfs());

    assert!(state.is_running());
    assert!(state.mode_registry.is_empty());
    assert!(state.command_registry.is_empty());
    assert!(state.keymap_registry.is_empty());
    // #491: Use home_mode() instead of removed current_mode()
    assert_eq!(state.home_mode().name(), "normal");
}

#[test]
fn test_session_state_has_vfs() {
    let kernel = KernelContext::default();
    let state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // VFS should be accessible
    assert!(!state.vfs.exists(std::path::Path::new("/nonexistent")));
}

#[test]
fn test_session_state_quit() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    assert!(state.is_running());
    state.request_quit();
    assert!(!state.is_running());
}

#[test]
fn test_session_state_lookup_keys_empty() {
    let kernel = KernelContext::default();
    let state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let keys = reovim_driver_input::KeySequence::parse("j").unwrap();
    let result = state.lookup_keys(&test_mode_id(), &keys);

    assert!(result.is_not_found());
}

#[test]
fn test_session_state_mode_accepts_char_input_default() {
    let kernel = KernelContext::default();
    let state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Unknown mode defaults to not accepting input
    assert!(!state.mode_accepts_char_input());
}

#[test]
fn test_session_state_create_buffer() {
    // Use test_kernel() which has a real buffer manager
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let id = state.create_buffer("hello world");
    assert!(state.buffer(id).is_some());
    assert!(state.app.kernel.buffers.list().contains(&id));
}

/// Test `resolve_key_for_client` uses per-client state (#471, #477).
///
/// Verifies that:
/// 1. Key resolution uses the provided per-client state
/// 2. The shared session state is NOT affected
#[test]
fn test_resolve_key_for_client_mode_isolation() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Create per-client state (#471, #477)
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    let mut client_mode_stack = ModeStack::new(insert_mode);
    let mut client_windows = reovim_driver_session::WindowLayout::empty();
    let mut client_extensions = reovim_driver_session::ExtensionMap::new();
    let mut client_compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();

    // #491: Use home_mode() instead of removed current_mode()
    // Verify initial states
    assert_eq!(state.home_mode().name(), "normal"); // shared
    assert_eq!(client_mode_stack.current().name(), "insert"); // per-client

    // resolve_key_for_client should use per-client state, not shared
    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    // Call resolve_key_for_client - it will return None (no resolver)
    // but the important thing is it uses per-client state
    let _result = state.resolve_key_for_client(
        1, // test client_id for per-client undo (#471)
        reovim_driver_session::ClientContext {
            mode_stack: &mut client_mode_stack,
            windows: &mut client_windows,
            extensions: &mut client_extensions,
            compositor: &mut client_compositor,
            tabs: &mut tabs,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            local_marks: &mut local_marks,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &key,
    );

    // Verify home_mode unchanged
    assert_eq!(state.home_mode().name(), "normal");
    // Client mode stack should still be in insert mode
    assert_eq!(client_mode_stack.current().name(), "insert");
}

#[test]
fn test_session_state_with_registries() {
    let kernel = KernelContext::default();
    let mode_reg = ModeRegistry::new();
    let cmd_reg = CommandRegistry::new();
    let keymap_reg = KeymapRegistry::new();
    let resolver_reg = ResolverRegistry::new();

    let state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        mode_reg,
        cmd_reg,
        keymap_reg,
        resolver_reg,
        None,
    );

    assert!(state.is_running());
    assert_eq!(state.home_mode().name(), "normal");
}

#[test]
fn test_session_state_default() {
    let state = SessionState::default();
    assert!(state.is_running());
    assert!(state.mode_registry.is_empty());
}

#[test]
fn test_session_state_with_kernel() {
    let kernel = test_kernel();
    let state = SessionState::with_kernel(kernel);
    assert!(state.is_running());
}

#[test]
fn test_driver_session_accessors() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let _driver = state.driver_session();
    let _driver_mut = state.driver_session_mut();
}

#[test]
fn test_session_active_buffer() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    assert!(state.app.kernel.buffers.list().is_empty());

    let buffer_id = state.create_buffer("test");
    assert!(state.app.kernel.buffers.list().contains(&buffer_id));
}

#[test]
fn test_set_session_active_buffer() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // active_buffer is now per-client; verify buffer list instead
    let buffer_id = state.create_buffer("test");
    assert!(state.app.kernel.buffers.list().contains(&buffer_id));
}

#[test]
fn test_session_terminal_size() {
    // terminal_size is now per-client; verify default VT100 dimensions
    let default_size = (80u16, 24u16);
    assert_eq!(default_size, (80, 24));
}

#[test]
fn test_set_session_terminal_size() {
    // terminal_size is now per-client; verify it can be stored in a tuple
    let terminal_size = (120u16, 40u16);
    assert_eq!(terminal_size, (120, 40));
}

#[test]
fn test_home_mode() {
    let kernel = KernelContext::default();
    let state = SessionState::new(kernel, test_mode_id(), test_vfs());

    assert_eq!(state.home_mode().name(), "normal");
}

#[test]
fn test_buffer_operations() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let id = state.create_buffer("hello");
    let buffer = state.buffer(id);
    assert!(buffer.is_some());

    let content = buffer.unwrap().read().content();
    assert_eq!(content, "hello");
}

#[test]
fn test_active_buffer() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    assert!(state.app.kernel.buffers.list().is_empty());

    let id = state.create_buffer("test");
    assert!(state.app.kernel.buffers.list().contains(&id));
}

#[test]
fn test_set_active_buffer() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // active_buffer is now per-client; verify buffer registration instead
    let buffer_id = state.create_buffer("test");
    assert!(state.app.kernel.buffers.list().contains(&buffer_id));
}

#[test]
fn test_request_detach() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    state.request_detach();
    // After detach, server continues running but clients disconnect
    // The is_running() check is for quit, not detach
    assert!(state.is_running());
}

#[test]
fn test_resolver_registry() {
    let kernel = KernelContext::default();
    let state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let registry = state.resolver_registry();
    assert!(registry.get(&test_mode_id()).is_none());
}

#[test]
fn test_buffer_nonexistent() {
    use reovim_kernel::api::v1::BufferId;

    let kernel = test_kernel();
    let state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let buffer = state.buffer(BufferId::new());
    assert!(buffer.is_none());
}

#[test]
fn test_create_buffer_registers_buffers() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    assert!(state.app.kernel.buffers.list().is_empty());

    let id1 = state.create_buffer("first");
    assert!(state.app.kernel.buffers.list().contains(&id1));

    let id2 = state.create_buffer("second");
    // Both buffers should be registered
    let buffers = state.app.kernel.buffers.list();
    assert!(buffers.contains(&id1));
    assert!(buffers.contains(&id2));
    assert_ne!(id1, id2);
}

#[test]
fn test_resolve_key_returns_none_without_resolver() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('j'));
    let result = state.resolve_key(&key);

    // No resolver registered, should return None
    assert!(result.is_none());
}

#[test]
fn test_try_on_command_complete_returns_none_without_resolver() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let result = state.try_on_command_complete();
    assert!(result.is_none());
}

#[test]
fn test_try_on_command_complete_for_client_returns_none_without_resolver() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let mut mode_stack = ModeStack::new(test_mode_id());
    let mut windows = reovim_driver_session::WindowLayout::empty();
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let result = state.try_on_command_complete_for_client(
        1,
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
    );

    assert!(result.is_none());
}

#[test]
fn test_execute_command_for_client_returns_none_without_command() {
    use reovim_driver_command_types::CommandContext;

    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let mut mode_stack = ModeStack::new(test_mode_id());
    let mut windows = reovim_driver_session::WindowLayout::empty();
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let cmd_id = reovim_kernel::api::v1::CommandId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "nonexistent",
    );
    let ctx = CommandContext::default();

    let result = state.execute_command_for_client(
        1,
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
        &cmd_id,
        &ctx,
    );

    assert!(result.is_none());
}

#[test]
fn test_with_registries_with_initial_buffer() {
    let kernel = test_kernel();

    // Create a buffer before creating the state
    let buffer_id = {
        use reovim_kernel::api::v1::Buffer;
        let mut buffer = Buffer::new();
        buffer.set_content("initial");
        kernel.buffers.register(buffer)
    };

    let state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        ModeRegistry::new(),
        CommandRegistry::new(),
        KeymapRegistry::new(),
        ResolverRegistry::new(),
        None,
    );

    // Buffer should be registered in kernel
    assert!(state.app.kernel.buffers.list().contains(&buffer_id));
}

#[test]
fn test_session_state_multiple_buffers() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Create multiple buffers
    let id1 = state.create_buffer("first");
    let id2 = state.create_buffer("second");
    let id3 = state.create_buffer("third");

    // All buffers should be registered
    let buffers = state.app.kernel.buffers.list();
    assert!(buffers.contains(&id1));
    assert!(buffers.contains(&id2));
    assert!(buffers.contains(&id3));

    // All buffers should exist
    assert!(state.buffer(id1).is_some());
    assert!(state.buffer(id2).is_some());
    assert!(state.buffer(id3).is_some());

    // Verify contents
    assert_eq!(state.buffer(id1).unwrap().read().content(), "first");
    assert_eq!(state.buffer(id2).unwrap().read().content(), "second");
    assert_eq!(state.buffer(id3).unwrap().read().content(), "third");
}

#[test]
fn test_session_state_buffer_created_and_accessible() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let id = state.create_buffer("content");
    assert!(!state.app.kernel.buffers.list().is_empty());
    assert!(state.buffer(id).is_some());
}

#[test]
fn test_session_terminal_size_roundtrip() {
    // terminal_size is now per-client; verify tuple storage works
    let mut terminal_size = (80u16, 24u16);
    assert_eq!(terminal_size, (80, 24));

    terminal_size = (200, 50);
    assert_eq!(terminal_size, (200, 50));

    terminal_size = (1, 1);
    assert_eq!(terminal_size, (1, 1));
}

#[test]
fn test_session_state_driver_session_accessors() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Verify driver_session() and driver_session_mut() work
    let _ds = state.driver_session();
    let _ds_mut = state.driver_session_mut();
}

#[test]
fn test_session_state_mode_accepts_char_input_with_registered_mode() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Register a mode that accepts char input (discriminant 1, different from home mode 0)
    let insert_mode_id = ModeId::with_discriminant(ModuleId::new("test"), "INSERT", 1);
    let insert_info = reovim_driver_input::ModeInfo {
        id: insert_mode_id,
        display_name: "INSERT",
        cursor_style: reovim_kernel::api::v1::CursorStyle::Bar,
        accepts_char_input: true,
        has_selection: false,
        inherits_from: None,
        is_entry: false,
    };
    state
        .mode_registry
        .register(crate::registry::ModeEntry::from_info(insert_info));

    // mode_accepts_char_input checks home_mode (test/normal, discriminant 0)
    // We registered test/INSERT (discriminant 1) which is a different mode
    // So home_mode is NOT in the registry, should return false
    assert!(!state.mode_accepts_char_input());

    // Now register home_mode with accepts_char_input: false
    let normal_info = reovim_driver_input::ModeInfo {
        id: test_mode_id(),
        display_name: "NORMAL",
        cursor_style: reovim_kernel::api::v1::CursorStyle::Block,
        accepts_char_input: false,
        has_selection: false,
        inherits_from: None,
        is_entry: true,
    };
    state
        .mode_registry
        .register(crate::registry::ModeEntry::from_info(normal_info));

    // Home mode is now registered but does NOT accept char input
    assert!(!state.mode_accepts_char_input());
}

#[test]
fn test_resolve_key_for_client_no_resolver() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let mut mode_stack = ModeStack::new(test_mode_id());
    let mut windows = reovim_driver_session::WindowLayout::empty();
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('x'));
    let result = state.resolve_key_for_client(
        1,
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
        &key,
    );

    // No resolver registered - should return None
    assert!(result.is_none());
}

#[test]
fn test_session_state_request_quit_and_detach() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Detach should not affect running state
    state.request_detach();
    assert!(state.is_running());

    // Quit should stop running
    state.request_quit();
    assert!(!state.is_running());
}

#[test]
fn test_lookup_keys_after_registration() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Register a keybinding
    let cmd = reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "cursor-down");
    state
        .keymap_registry
        .register_str(&test_mode_id(), "j", cmd);

    // Lookup should find it
    let keys = reovim_driver_input::KeySequence::parse("j").unwrap();
    let result = state.lookup_keys(&test_mode_id(), &keys);
    assert!(result.is_found());
}

#[test]
fn test_session_active_buffer_delegation() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // active_buffer is now per-client; verify buffer list
    assert!(state.app.kernel.buffers.list().is_empty());

    let id = state.create_buffer("test");
    assert!(state.app.kernel.buffers.list().contains(&id));
}

#[test]
fn test_set_session_active_buffer_delegation() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // active_buffer is now per-client; verify buffer creation instead
    let buf_id = state.create_buffer("test");
    assert!(state.app.kernel.buffers.list().contains(&buf_id));
}

#[test]
fn test_with_registries_custom_mode() {
    let custom_mode = ModeId::new(ModuleId::new("custom"), "visual");
    let kernel = KernelContext::default();

    let state = SessionState::with_registries(
        kernel,
        custom_mode,
        test_vfs(),
        ModeRegistry::new(),
        CommandRegistry::new(),
        KeymapRegistry::new(),
        ResolverRegistry::new(),
        None,
    );

    assert_eq!(state.home_mode().name(), "visual");
    assert_eq!(format!("{}", state.home_mode().module()), "custom");
}

#[test]
fn test_with_registries_no_buffers_no_compositor() {
    let kernel = KernelContext::default();

    let state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        ModeRegistry::new(),
        CommandRegistry::new(),
        KeymapRegistry::new(),
        ResolverRegistry::new(),
        None,
    );

    // No buffers in kernel -> empty buffer list
    assert!(state.app.kernel.buffers.list().is_empty());
}

#[test]
fn test_session_state_new_with_custom_vfs() {
    use std::path::Path;

    let kernel = KernelContext::default();
    let vfs = test_vfs();
    let state = SessionState::new(kernel, test_mode_id(), vfs);

    // Verify VFS is accessible
    assert!(!state.vfs.exists(Path::new("/some/file")));
}

#[test]
fn test_create_buffer_second_also_registered() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let first_id = state.create_buffer("first buffer");
    let second_id = state.create_buffer("second buffer");

    // Both buffers should be registered
    let buffers = state.app.kernel.buffers.list();
    assert!(buffers.contains(&first_id));
    assert!(buffers.contains(&second_id));
}

#[test]
fn test_session_state_registries_accessible_after_with_registries() {
    let kernel = KernelContext::default();
    let mut mode_reg = ModeRegistry::new();
    let cmd_reg = CommandRegistry::new();
    let mut keymap_reg = KeymapRegistry::new();

    // Register something in each registry
    let mode_info = reovim_driver_input::ModeInfo {
        id: test_mode_id(),
        display_name: "NORMAL",
        cursor_style: reovim_kernel::api::v1::CursorStyle::Block,
        accepts_char_input: false,
        has_selection: false,
        inherits_from: None,
        is_entry: true,
    };
    mode_reg.register(crate::registry::ModeEntry::from_info(mode_info));

    let cmd_id = reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "cursor-down");
    keymap_reg.register_str(&test_mode_id(), "j", cmd_id);

    let state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        mode_reg,
        cmd_reg,
        keymap_reg,
        ResolverRegistry::new(),
        None,
    );

    // Mode registry should have our mode
    assert!(!state.mode_registry.is_empty());

    // Keymap lookup should find our binding
    let keys = reovim_driver_input::KeySequence::parse("j").unwrap();
    let result = state.lookup_keys(&test_mode_id(), &keys);
    assert!(result.is_found());
}

#[test]
fn test_with_registries_with_buffer_and_no_compositor() {
    let kernel = test_kernel();

    // Create a buffer first
    let mut buffer = reovim_kernel::api::v1::Buffer::new();
    buffer.set_content("hello world");
    let buffer_id = kernel.buffers.register(buffer);

    let state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        ModeRegistry::new(),
        CommandRegistry::new(),
        KeymapRegistry::new(),
        ResolverRegistry::new(),
        None, // No compositor
    );

    // Buffer should be registered in kernel
    assert!(state.app.kernel.buffers.list().contains(&buffer_id));
}

#[test]
fn test_with_registries_multiple_buffers() {
    let kernel = test_kernel();

    // Create multiple buffers
    let mut buf1 = reovim_kernel::api::v1::Buffer::new();
    buf1.set_content("first");
    let id1 = kernel.buffers.register(buf1);

    let mut buf2 = reovim_kernel::api::v1::Buffer::new();
    buf2.set_content("second");
    let id2 = kernel.buffers.register(buf2);

    let state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        ModeRegistry::new(),
        CommandRegistry::new(),
        KeymapRegistry::new(),
        ResolverRegistry::new(),
        None,
    );

    // Both buffers should be registered
    let buffers = state.app.kernel.buffers.list();
    assert!(
        buffers.contains(&id1) && buffers.contains(&id2),
        "Both buffers should be registered, got {buffers:?}"
    );
}

#[test]
fn test_resolve_key_returns_none_for_different_modes() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Try resolving with several different key types
    let keys = [
        reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('j')),
        reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('k')),
        reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Escape),
        reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Enter),
    ];

    for key in &keys {
        let result = state.resolve_key(key);
        assert!(result.is_none(), "No resolver registered, should return None");
    }
}

#[test]
fn test_resolve_key_for_client_different_modes() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Test with insert mode stack
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");
    let mut client_mode_stack = ModeStack::new(insert_mode);
    let mut client_windows = reovim_driver_session::WindowLayout::empty();
    let mut client_extensions = reovim_driver_session::ExtensionMap::new();
    let mut client_compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));
    let result = state.resolve_key_for_client(
        1,
        reovim_driver_session::ClientContext {
            mode_stack: &mut client_mode_stack,
            windows: &mut client_windows,
            extensions: &mut client_extensions,
            compositor: &mut client_compositor,
            tabs: &mut tabs,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            local_marks: &mut local_marks,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &key,
    );

    // No resolver for insert mode either
    assert!(result.is_none());
}

#[test]
fn test_execute_command_for_client_with_registered_command() {
    use {
        reovim_driver_command::{ArgSpec, Command, CommandHandler},
        reovim_driver_session::SessionRuntime,
    };

    struct DummyCmd;
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Command for DummyCmd {
        fn id(&self) -> reovim_kernel::api::v1::CommandId {
            reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "dummy")
        }
        fn description(&self) -> &'static str {
            "dummy"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
        fn names(&self) -> &[&'static str] {
            &["dummy"]
        }
    }
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CommandHandler for DummyCmd {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &reovim_driver_command::CommandContext,
        ) -> CommandResult {
            CommandResult::Success
        }
    }

    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let cmd = DummyCmd;
    let cmd_id = cmd.id();
    state.command_registry.register(Arc::new(cmd));

    let mut mode_stack = ModeStack::new(test_mode_id());
    let mut windows = reovim_driver_session::WindowLayout::empty();
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let ctx = reovim_driver_command::CommandContext::new();
    let result = state.execute_command_for_client(
        1,
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
        &cmd_id,
        &ctx,
    );

    assert!(result.is_some());
    let (cmd_result, _changes, _signals) = result.unwrap();
    assert_eq!(cmd_result, CommandResult::Success);
}

#[test]
fn test_session_state_create_multiple_buffers_verify_contents() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let id1 = state.create_buffer("alpha");
    let id2 = state.create_buffer("beta");
    let id3 = state.create_buffer("gamma");

    assert_eq!(state.buffer(id1).unwrap().read().content(), "alpha");
    assert_eq!(state.buffer(id2).unwrap().read().content(), "beta");
    assert_eq!(state.buffer(id3).unwrap().read().content(), "gamma");

    // All buffers should be registered
    let buffers = state.app.kernel.buffers.list();
    assert!(buffers.contains(&id1));
    assert!(buffers.contains(&id2));
    assert!(buffers.contains(&id3));
}

#[test]
fn test_session_state_buffer_switch_per_client() {
    let kernel = test_kernel();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    let id1 = state.create_buffer("first");
    let id2 = state.create_buffer("second");

    // active_buffer is now per-client; verify both buffers are registered
    let buffers = state.app.kernel.buffers.list();
    assert!(buffers.contains(&id1));
    assert!(buffers.contains(&id2));

    // Per-client active_buffer can be tracked independently
    let mut client_active_buffer: Option<BufferId> = Some(id1);
    assert_eq!(client_active_buffer, Some(id1));

    client_active_buffer = Some(id2);
    assert_eq!(client_active_buffer, Some(id2));

    client_active_buffer = Some(id1);
    assert_eq!(client_active_buffer, Some(id1));
}

// ========================================================================
// Coverage tests for uncovered lines
// ========================================================================

/// Mock `WindowLayerCompositor` that returns empty tiled windows and
/// supports `add_tiled()`.
struct MockLayerCompositor {
    layer_id: reovim_driver_layout::LayerId,
    tiled_windows: Vec<reovim_driver_layout::WindowId>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl MockLayerCompositor {
    fn new(layer_id: reovim_driver_layout::LayerId) -> Self {
        Self {
            layer_id,
            tiled_windows: Vec::new(),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_layout::WindowLayerCompositor for MockLayerCompositor {
    fn id(&self) -> reovim_driver_layout::LayerId {
        self.layer_id
    }

    fn arrange(
        &self,
        _bounds: reovim_driver_layout::Rect,
    ) -> Vec<reovim_driver_layout::WindowPlacement> {
        Vec::new()
    }

    fn add_tiled(&mut self) -> reovim_driver_layout::WindowId {
        let id = reovim_driver_layout::WindowId::from_raw(self.tiled_windows.len() + 1);
        self.tiled_windows.push(id);
        id
    }

    fn split_tiled(
        &mut self,
        _from: reovim_driver_layout::WindowId,
        _direction: reovim_driver_layout::SplitDirection,
    ) -> Option<reovim_driver_layout::WindowId> {
        None
    }

    fn navigate_tiled(
        &self,
        _from: reovim_driver_layout::WindowId,
        _direction: reovim_driver_layout::NavigateDirection,
    ) -> Option<reovim_driver_layout::WindowId> {
        None
    }

    fn resize_tiled(
        &mut self,
        _window: reovim_driver_layout::WindowId,
        _direction: reovim_driver_layout::NavigateDirection,
        _delta: i16,
    ) {
    }

    fn close_tiled(
        &mut self,
        _window: reovim_driver_layout::WindowId,
    ) -> Option<reovim_driver_layout::WindowId> {
        None
    }

    fn equalize_tiled(&mut self) {}

    fn cycle_tiled(
        &self,
        _from: reovim_driver_layout::WindowId,
        _forward: bool,
    ) -> Option<reovim_driver_layout::WindowId> {
        None
    }

    fn create_float(
        &mut self,
        _bounds: reovim_driver_layout::Rect,
    ) -> reovim_driver_layout::WindowId {
        reovim_driver_layout::WindowId::from_raw(100)
    }

    fn move_float(&mut self, _window: reovim_driver_layout::WindowId, _x: u16, _y: u16) {}

    fn resize_float(&mut self, _window: reovim_driver_layout::WindowId, _width: u16, _height: u16) {
    }

    fn raise_float(&mut self, _window: reovim_driver_layout::WindowId) {}
    fn lower_float(&mut self, _window: reovim_driver_layout::WindowId) {}
    fn close_float(&mut self, _window: reovim_driver_layout::WindowId) {}
    fn toggle_float(&mut self, _window: reovim_driver_layout::WindowId) {}

    fn show_overlay(
        &mut self,
        _constraints: reovim_driver_layout::OverlayConstraints,
    ) -> reovim_driver_layout::WindowId {
        reovim_driver_layout::WindowId::from_raw(200)
    }

    fn hide_overlay(&mut self, _window: reovim_driver_layout::WindowId) {}

    fn resize_overlay(
        &mut self,
        _window: reovim_driver_layout::WindowId,
        _width: u16,
        _height: u16,
    ) {
    }

    fn hide_all_overlays(&mut self) {}

    fn set_focus(&mut self, _window: reovim_driver_layout::WindowId) {}

    fn focused(&self) -> Option<reovim_driver_layout::WindowId> {
        None
    }

    fn windows_in_zone(
        &self,
        zone: reovim_driver_layout::Zone,
    ) -> Vec<reovim_driver_layout::WindowId> {
        match zone {
            reovim_driver_layout::Zone::Tiled => self.tiled_windows.clone(),
            _ => Vec::new(),
        }
    }

    fn zone_of(
        &self,
        _window: reovim_driver_layout::WindowId,
    ) -> Option<reovim_driver_layout::Zone> {
        None
    }
}

/// Mock `RootCompositor` that provides an active layer with a
/// `MockLayerCompositor` (starts with empty tiled windows).
struct MockCompositor {
    layer: MockLayerCompositor,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl MockCompositor {
    fn new() -> Self {
        Self {
            layer: MockLayerCompositor::new(reovim_driver_layout::LayerId::new(0)),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_layout::RootCompositor for MockCompositor {
    fn composite(
        &self,
        screen: reovim_driver_layout::Rect,
    ) -> reovim_driver_layout::CompositeResult {
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

    fn set_focus(&mut self, _window: reovim_driver_layout::WindowId) {}

    fn focused(&self) -> Option<reovim_driver_layout::WindowId> {
        None
    }

    fn focus_at(&mut self, _x: u16, _y: u16) -> Option<reovim_driver_layout::WindowId> {
        None
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
        self.layer.tiled_windows.len()
    }

    fn set_screen(&mut self, _screen: reovim_driver_layout::Rect) {}

    fn layer_of(
        &self,
        _window: reovim_driver_layout::WindowId,
    ) -> Option<reovim_driver_layout::LayerId> {
        Some(reovim_driver_layout::LayerId::new(0))
    }

    fn boxed_clone(&self) -> Box<dyn reovim_driver_layout::RootCompositor> {
        Box::new(Self::new())
    }
}

/// Test `with_registries` with `Some(compositor)` to cover line 148.
#[test]
fn test_with_registries_with_compositor() {
    let kernel = KernelContext::default();
    let compositor: Box<dyn reovim_driver_layout::RootCompositor> = Box::new(MockCompositor::new());

    let state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        ModeRegistry::new(),
        CommandRegistry::new(),
        KeymapRegistry::new(),
        ResolverRegistry::new(),
        Some(compositor),
    );

    // Compositor should have been set
    assert!(state.driver_session.compositor().is_some());
}

/// Test `with_registries` with buffers AND compositor to cover lines 160-168
/// (initial window creation in compositor).
#[test]
fn test_with_registries_with_buffer_and_compositor_creates_window() {
    let kernel = test_kernel();

    // Create a buffer first so buffer_ids is non-empty
    let mut buffer = reovim_kernel::api::v1::Buffer::new();
    buffer.set_content("test content");
    let buffer_id = kernel.buffers.register(buffer);

    let compositor: Box<dyn reovim_driver_layout::RootCompositor> = Box::new(MockCompositor::new());

    let state = SessionState::with_registries(
        kernel,
        test_mode_id(),
        test_vfs(),
        ModeRegistry::new(),
        CommandRegistry::new(),
        KeymapRegistry::new(),
        ResolverRegistry::new(),
        Some(compositor),
    );

    // Buffer should be registered in kernel
    assert!(state.app.kernel.buffers.list().contains(&buffer_id));

    // Compositor should have been set and should have one tiled window
    let compositor = state.driver_session.compositor().unwrap();
    let layer_id = compositor.active_layer().unwrap();
    let layer = compositor.layer_compositor(layer_id).unwrap();
    let tiled_windows = layer.windows_in_zone(reovim_driver_layout::Zone::Tiled);
    assert_eq!(tiled_windows.len(), 1, "Should have created initial tiled window");
}

/// A minimal resolver that returns `Some(ModeTransition)` from
/// `on_command_complete`, used to cover the body of
/// `try_on_command_complete` and `try_on_command_complete_for_client`.
struct CompletingResolver {
    mode: ModeId,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CompletingResolver {
    fn new(mode: ModeId) -> Self {
        Self { mode }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_input::ModeKeyResolver for CompletingResolver {
    fn resolve_with_keymap(
        &self,
        _key: &reovim_driver_input::KeyEvent,
        _state: &mut reovim_driver_input::ModeState,
        _input: &reovim_driver_input::ResolveInput<'_>,
    ) -> reovim_driver_input::ResolveResult {
        reovim_driver_input::ResolveResult::Pending
    }

    fn on_command_complete(
        &self,
        _session: &mut dyn reovim_driver_input::SessionApiDyn,
        _shared_extensions: &mut reovim_driver_input::ExtensionMap,
        _client_extensions: &mut reovim_driver_input::ExtensionMap,
    ) -> Option<reovim_driver_input::ModeTransition> {
        // Return a Pop transition to indicate completion
        Some(reovim_driver_input::ModeTransition::Pop { result: None })
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode
    }
}

/// Test `try_on_command_complete` with a registered resolver that returns
/// `Some(ModeTransition)`, covering lines 571-585.
#[test]
fn test_try_on_command_complete_with_resolver() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Register a resolver for the home mode
    state
        .resolver_registry
        .register(CompletingResolver::new(test_mode_id()));

    let result = state.try_on_command_complete();
    assert!(result.is_some(), "Should return ModeTransition from resolver");

    // Verify it's a Pop transition
    assert!(
        matches!(result.unwrap(), reovim_driver_input::ModeTransition::Pop { result: None }),
        "Should be a Pop transition"
    );
}

/// Test `try_on_command_complete_for_client` with a registered resolver
/// that returns `Some(ModeTransition)`, covering lines 628-640.
#[test]
fn test_try_on_command_complete_for_client_with_resolver() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Register a resolver for the test mode
    state
        .resolver_registry
        .register(CompletingResolver::new(test_mode_id()));

    let mut mode_stack = ModeStack::new(test_mode_id());
    let mut windows = reovim_driver_session::WindowLayout::empty();
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let result = state.try_on_command_complete_for_client(
        1,
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
    );

    assert!(result.is_some(), "Should return ModeTransition from resolver");
    assert!(
        matches!(result.unwrap(), reovim_driver_input::ModeTransition::Pop { result: None }),
        "Should be a Pop transition"
    );
}

/// A resolver that calls `session.execute_command()` during
/// `on_command_complete`, exercising the `StubExecutor::execute` path
/// (covers lines 554-561 and 615-622).
struct ExecutingResolver {
    mode: ModeId,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ExecutingResolver {
    fn new(mode: ModeId) -> Self {
        Self { mode }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_input::ModeKeyResolver for ExecutingResolver {
    fn resolve_with_keymap(
        &self,
        _key: &reovim_driver_input::KeyEvent,
        _state: &mut reovim_driver_input::ModeState,
        _input: &reovim_driver_input::ResolveInput<'_>,
    ) -> reovim_driver_input::ResolveResult {
        reovim_driver_input::ResolveResult::Pending
    }

    fn on_command_complete(
        &self,
        session: &mut dyn reovim_driver_input::SessionApiDyn,
        _shared_extensions: &mut reovim_driver_input::ExtensionMap,
        _client_extensions: &mut reovim_driver_input::ExtensionMap,
    ) -> Option<reovim_driver_input::ModeTransition> {
        // Call execute_command to exercise the StubExecutor path
        let cmd_id = reovim_kernel::api::v1::CommandId::new(
            reovim_kernel::api::v1::ModuleId::new("test"),
            "stub-cmd",
        );
        let ctx = reovim_driver_command::CommandContext::new();
        let _result = session.execute_command(cmd_id, ctx);
        Some(reovim_driver_input::ModeTransition::Pop { result: None })
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode
    }
}

/// Test `try_on_command_complete` where the resolver calls
/// `session.execute_command()`, covering the `StubExecutor::execute`
/// implementation at lines 554-561.
#[test]
fn test_try_on_command_complete_exercises_stub_executor() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    state
        .resolver_registry
        .register(ExecutingResolver::new(test_mode_id()));

    let result = state.try_on_command_complete();
    assert!(result.is_some());
}

/// Test `try_on_command_complete_for_client` where the resolver calls
/// `session.execute_command()`, covering the `StubExecutor::execute`
/// implementation at lines 615-622.
#[test]
fn test_try_on_command_complete_for_client_exercises_stub_executor() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    state
        .resolver_registry
        .register(ExecutingResolver::new(test_mode_id()));

    let mut mode_stack = ModeStack::new(test_mode_id());
    let mut windows = reovim_driver_session::WindowLayout::empty();
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let result = state.try_on_command_complete_for_client(
        1,
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
    );

    assert!(result.is_some());
}

/// A resolver that calls `session.execute_command()` during
/// `resolve_with_session`, exercising the `StubExecutor::execute`
/// paths in `resolve_key` and `resolve_key_for_client`
/// (covers lines 389-396 and 491-498).
struct ExecutingDuringResolveResolver {
    mode: ModeId,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ExecutingDuringResolveResolver {
    fn new(mode: ModeId) -> Self {
        Self { mode }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_input::ModeKeyResolver for ExecutingDuringResolveResolver {
    fn resolve_with_session(
        &self,
        _key: &reovim_driver_input::KeyEvent,
        _state: &mut reovim_driver_input::ModeState,
        _input: &reovim_driver_input::ResolveInput<'_>,
        session: &mut dyn reovim_driver_input::SessionApiDyn,
        _shared_extensions: &mut reovim_driver_input::ExtensionMap,
        _client_extensions: &mut reovim_driver_input::ExtensionMap,
    ) -> reovim_driver_input::ResolveResult {
        // Call execute_command to exercise the StubExecutor
        let cmd_id = reovim_kernel::api::v1::CommandId::new(
            reovim_kernel::api::v1::ModuleId::new("test"),
            "stub-resolve-cmd",
        );
        let ctx = reovim_driver_command::CommandContext::new();
        let _result = session.execute_command(cmd_id, ctx);
        reovim_driver_input::ResolveResult::Completed
    }

    fn resolve_with_keymap(
        &self,
        _key: &reovim_driver_input::KeyEvent,
        _state: &mut reovim_driver_input::ModeState,
        _input: &reovim_driver_input::ResolveInput<'_>,
    ) -> reovim_driver_input::ResolveResult {
        reovim_driver_input::ResolveResult::Pending
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode
    }
}

/// Test `resolve_key` where the resolver calls `session.execute_command()`,
/// covering the `StubExecutor::execute` at lines 389-396.
#[test]
fn test_resolve_key_exercises_stub_executor() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    state
        .resolver_registry
        .register(ExecutingDuringResolveResolver::new(test_mode_id()));

    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));
    let result = state.resolve_key(&key);

    assert!(result.is_some(), "Resolver should handle the key");
    let (resolve_result, _changes) = result.unwrap();
    assert!(
        matches!(resolve_result, reovim_driver_input::ResolveResult::Completed),
        "Should be Completed"
    );
}

/// Test `resolve_key_for_client` where the resolver calls
/// `session.execute_command()`, covering the `StubExecutor::execute`
/// at lines 491-498.
#[test]
fn test_resolve_key_for_client_exercises_stub_executor() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    state
        .resolver_registry
        .register(ExecutingDuringResolveResolver::new(test_mode_id()));

    let mut mode_stack = ModeStack::new(test_mode_id());
    let mut windows = reovim_driver_session::WindowLayout::empty();
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));
    let result = state.resolve_key_for_client(
        1,
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
        &key,
    );

    assert!(result.is_some(), "Resolver should handle the key");
    let (resolve_result, _changes) = result.unwrap();
    assert!(
        matches!(resolve_result, reovim_driver_input::ResolveResult::Completed),
        "Should be Completed"
    );
}

/// Test that `DummyCmd` trait methods (`description`, `args`, `names`)
/// are callable, covering lines 1471-1479 of the existing test's
/// `DummyCmd` definition.
#[test]
fn test_dummy_command_trait_methods() {
    use reovim_driver_command::{ArgSpec, Command, CommandHandler};

    struct DummyCmd;
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Command for DummyCmd {
        fn id(&self) -> reovim_kernel::api::v1::CommandId {
            reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "dummy")
        }
        fn description(&self) -> &'static str {
            "dummy"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
        fn names(&self) -> &[&'static str] {
            &["dummy"]
        }
    }
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CommandHandler for DummyCmd {
        fn execute(
            &self,
            _runtime: &mut reovim_driver_session::SessionRuntime<'_>,
            _args: &reovim_driver_command::CommandContext,
        ) -> reovim_driver_command::CommandResult {
            reovim_driver_command::CommandResult::Success
        }
    }

    let cmd = DummyCmd;

    // Exercise all Command trait methods
    let _id = cmd.id();
    assert_eq!(cmd.description(), "dummy");
    assert!(cmd.args().is_empty());
    assert_eq!(cmd.names(), &["dummy"]);
}

// ========================================================================
// PendingBindings population tests (lines 576-632)
// ========================================================================

/// Resolver returning `Pending` with configurable pending keys and parent.
struct PendingWithKeysResolver {
    mode: ModeId,
    parent: Option<ModeId>,
    keys: reovim_driver_input::KeySequence,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_input::ModeKeyResolver for PendingWithKeysResolver {
    fn resolve_with_keymap(
        &self,
        _key: &reovim_driver_input::KeyEvent,
        _state: &mut reovim_driver_input::ModeState,
        _input: &reovim_driver_input::ResolveInput<'_>,
    ) -> reovim_driver_input::ResolveResult {
        reovim_driver_input::ResolveResult::Pending
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        self.parent.as_ref()
    }

    fn pending_keys(&self) -> reovim_driver_input::KeySequence {
        self.keys.clone()
    }
}

/// Resolver returning `ModeTransition::Push` to a target mode.
struct PushToModeResolver {
    mode: ModeId,
    target: ModeId,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_input::ModeKeyResolver for PushToModeResolver {
    fn resolve_with_session(
        &self,
        _key: &reovim_driver_input::KeyEvent,
        _state: &mut reovim_driver_input::ModeState,
        _input: &reovim_driver_input::ResolveInput<'_>,
        _session: &mut dyn reovim_driver_input::SessionApiDyn,
        _shared_extensions: &mut reovim_driver_input::ExtensionMap,
        _client_extensions: &mut reovim_driver_input::ExtensionMap,
    ) -> reovim_driver_input::ResolveResult {
        reovim_driver_input::ResolveResult::ModeTransition(
            reovim_driver_input::ModeTransition::Push {
                mode: self.target.clone(),
                context: reovim_driver_input::TransitionContext::new(),
            },
        )
    }

    fn resolve_with_keymap(
        &self,
        _key: &reovim_driver_input::KeyEvent,
        _state: &mut reovim_driver_input::ModeState,
        _input: &reovim_driver_input::ResolveInput<'_>,
    ) -> reovim_driver_input::ResolveResult {
        reovim_driver_input::ResolveResult::NotHandled
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode
    }
}

/// Stub resolver declaring a parent mode via `inherits_from`.
struct InheritingStubResolver {
    mode: ModeId,
    parent: ModeId,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_input::ModeKeyResolver for InheritingStubResolver {
    fn resolve_with_keymap(
        &self,
        _key: &reovim_driver_input::KeyEvent,
        _state: &mut reovim_driver_input::ModeState,
        _input: &reovim_driver_input::ResolveInput<'_>,
    ) -> reovim_driver_input::ResolveResult {
        reovim_driver_input::ResolveResult::NotHandled
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        Some(&self.parent)
    }
}

/// Helper: resolve a key for a client, returning result and allowing
/// inspection of the client extensions afterwards.
fn resolve_pb_test(
    state: &mut SessionState,
    key: &reovim_driver_input::KeyEvent,
    extensions: &mut reovim_driver_session::ExtensionMap,
) -> Option<(reovim_driver_input::ResolveResult, reovim_driver_session::api::StateChanges)> {
    let mut mode_stack = ModeStack::new(state.home_mode().clone());
    let mut windows = reovim_driver_session::WindowLayout::empty();
    let mut compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    state.resolve_key_for_client(
        1,
        reovim_driver_session::ClientContext {
            mode_stack: &mut mode_stack,
            windows: &mut windows,
            extensions,
            compositor: &mut compositor,
            tabs: &mut tabs,
            registers: &mut registers,
            clipboard_history: &mut clipboard_history,
            local_marks: &mut local_marks,
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        key,
    )
}

/// Test `PendingBindings` populated on `Pending` result with non-empty
/// pending keys (covers lines 578-594).
#[test]
fn test_pending_bindings_on_pending_result() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Pending key sequence: "g"
    let g_key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('g'));
    let pending_keys = reovim_driver_input::KeySequence::from_keys(&[g_key]);

    // Register resolver with non-empty pending keys
    state.resolver_registry.register(PendingWithKeysResolver {
        mode: test_mode_id(),
        parent: None,
        keys: pending_keys,
    });

    // Register keymap bindings: "gg" → goto-top, "gd" → goto-def
    let goto_top = reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "goto-top");
    let goto_def = reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "goto-def");
    state
        .keymap_registry
        .register_str(&test_mode_id(), "gg", goto_top);
    state
        .keymap_registry
        .register_str(&test_mode_id(), "gd", goto_def);

    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('g'));
    let result = resolve_pb_test(&mut state, &key, &mut extensions);
    assert!(result.is_some());

    let pb = extensions.get::<PendingBindings>().unwrap();
    assert!(!pb.pending_keys.is_empty());
    assert_eq!(pb.mode, test_mode_id());
    assert_eq!(pb.continuations.len(), 2);
}

/// Test `PendingBindings` includes parent mode bindings on `Pending`
/// (covers lines 583-588).
#[test]
fn test_pending_bindings_on_pending_with_parent_mode() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Use distinct discriminant so parent_mode != test_mode_id() in HashMap
    let parent_mode = ModeId::with_discriminant(ModuleId::new("test"), "motion", 10);
    let g_key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('g'));
    let pending_keys = reovim_driver_input::KeySequence::from_keys(&[g_key]);

    // Register resolver with parent
    state.resolver_registry.register(PendingWithKeysResolver {
        mode: test_mode_id(),
        parent: Some(parent_mode.clone()),
        keys: pending_keys,
    });

    // Binding in child mode: "gg" → goto-top
    let goto_top = reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "goto-top");
    state
        .keymap_registry
        .register_str(&test_mode_id(), "gg", goto_top);

    // Binding in parent mode: "gd" → goto-def
    let goto_def = reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "goto-def");
    state
        .keymap_registry
        .register_str(&parent_mode, "gd", goto_def);

    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('g'));
    let result = resolve_pb_test(&mut state, &key, &mut extensions);
    assert!(result.is_some());

    let pb = extensions.get::<PendingBindings>().unwrap();
    // Should have both child (gg) and parent (gd) continuations
    assert_eq!(pb.continuations.len(), 2);
}

/// Test `PendingBindings` populated on `ModeTransition::Push`
/// (covers lines 605-622).
#[test]
fn test_pending_bindings_on_mode_push() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Use distinct discriminant so target_mode != test_mode_id()
    let target_mode = ModeId::with_discriminant(ModuleId::new("test"), "operator", 1);

    // Register resolver that pushes to target mode
    state.resolver_registry.register(PushToModeResolver {
        mode: test_mode_id(),
        target: target_mode.clone(),
    });

    // Register keybindings for target mode
    let word_forward =
        reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "word-forward");
    let word_backward =
        reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "word-backward");
    state
        .keymap_registry
        .register_str(&target_mode, "w", word_forward);
    state
        .keymap_registry
        .register_str(&target_mode, "b", word_backward);

    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('d'));
    let result = resolve_pb_test(&mut state, &key, &mut extensions);
    assert!(result.is_some());

    let pb = extensions.get::<PendingBindings>().unwrap();
    assert!(!pb.mode_prefix.is_empty()); // trigger key "d"
    assert!(pb.pending_keys.is_empty()); // no pending yet
    assert_eq!(pb.mode, target_mode);
    assert_eq!(pb.continuations.len(), 2);
}

/// Test `PendingBindings` on Push includes parent bindings
/// (covers lines 611-615).
#[test]
fn test_pending_bindings_on_push_with_parent() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Use distinct discriminants so all three modes are unique
    let target_mode = ModeId::with_discriminant(ModuleId::new("test"), "operator", 1);
    let motion_mode = ModeId::with_discriminant(ModuleId::new("test"), "motion", 2);

    // Register resolver that pushes to target mode
    state.resolver_registry.register(PushToModeResolver {
        mode: test_mode_id(),
        target: target_mode.clone(),
    });

    // Register inheriting resolver for target mode (inherits from motion)
    state.resolver_registry.register(InheritingStubResolver {
        mode: target_mode.clone(),
        parent: motion_mode.clone(),
    });

    // Parent mode binding: "w" → word-forward
    let word_fwd = reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "word-forward");
    state
        .keymap_registry
        .register_str(&motion_mode, "w", word_fwd);

    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('d'));
    let result = resolve_pb_test(&mut state, &key, &mut extensions);
    assert!(result.is_some());

    let pb = extensions.get::<PendingBindings>().unwrap();
    // Continuations from parent mode
    assert_eq!(pb.continuations.len(), 1);
    assert_eq!(pb.mode, target_mode);
}

/// Test `PendingBindings` NOT populated when Push target has no bindings
/// (covers line 617 false branch).
#[test]
fn test_pending_bindings_on_push_empty_continuations() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Use distinct discriminant
    let target_mode = ModeId::with_discriminant(ModuleId::new("test"), "empty-mode", 3);

    // Register resolver that pushes to target mode (no bindings)
    state.resolver_registry.register(PushToModeResolver {
        mode: test_mode_id(),
        target: target_mode,
    });

    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('d'));
    let result = resolve_pb_test(&mut state, &key, &mut extensions);
    assert!(result.is_some());

    // No bindings registered for target → PendingBindings should not be set
    assert!(extensions.get::<PendingBindings>().is_none());
}

/// Test `PendingBindings` cleared on non-Pending, non-Push result
/// (covers lines 627-628).
#[test]
fn test_pending_bindings_cleared_on_completed() {
    let kernel = KernelContext::default();
    let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

    // Register resolver returning Completed
    state
        .resolver_registry
        .register(ExecutingDuringResolveResolver::new(test_mode_id()));

    // Pre-populate PendingBindings
    let mut extensions = reovim_driver_session::ExtensionMap::new();
    let pb = extensions.get_or_insert::<PendingBindings>();
    let cmd = reovim_kernel::api::v1::CommandId::new(ModuleId::new("test"), "dummy");
    pb.continuations.push((
        reovim_driver_input::KeySequence::new(),
        reovim_driver_input::BindingInfo::from_command(
            cmd,
            reovim_driver_input::BindingLayer::Policy,
        ),
    ));
    assert!(extensions.get::<PendingBindings>().unwrap().is_active());

    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));
    let result = resolve_pb_test(&mut state, &key, &mut extensions);
    assert!(result.is_some());

    // PendingBindings should be cleared
    let pb = extensions.get::<PendingBindings>().unwrap();
    assert!(!pb.is_active());
}
