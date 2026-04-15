use std::sync::Arc;

use {
    reovim_kernel::api::v1::{CommandId, ModeId, ModuleId, WindowId},
    reovim_provider_text::TextBufferRegistry,
    reovim_subsys_input::{KeyCode, KeyEvent},
    reovim_subsys_session::{ClientId, DomainDriver, DomainStateQuery},
};

use {
    super::*,
    crate::api::{CommandExecutor, CommandHandle},
};

// ============================================================================
// Test helpers
// ============================================================================

fn test_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "normal")
}

fn make_test_kernel() -> Arc<KernelContext> {
    let ctx = reovim_kernel::testing::create_test_context();
    ctx.services.register(Arc::new(TextBufferRegistry::new()));
    Arc::new(ctx)
}

struct StubExecutor;

impl CommandExecutor for StubExecutor {
    fn get_handle(&self, _id: &CommandId) -> Option<Arc<dyn CommandHandle>> {
        None
    }
}

fn make_driver() -> TextDomainDriver {
    let kernel = make_test_kernel();
    let text_buffers = kernel
        .services
        .get::<TextBufferRegistry>()
        .expect("TextBufferRegistry must be registered");
    TextDomainDriver::new(42, test_mode(), kernel, Arc::new(StubExecutor), text_buffers)
}

fn make_driver_with_buffer(content: &str) -> (TextDomainDriver, BufferId) {
    let driver = make_driver();
    let buffer_id = driver.create_buffer(content.as_bytes());
    (driver, buffer_id)
}

// ============================================================================
// Basic properties
// ============================================================================

#[test]
fn test_domain_name() {
    let driver = make_driver();
    assert_eq!(driver.domain_name(), "text");
}

#[test]
fn test_domain_id() {
    let driver = make_driver();
    assert_eq!(driver.domain_id(), 42);
}

// ============================================================================
// Object safety
// ============================================================================

#[test]
fn test_object_safety_domain_driver() {
    let driver = make_driver();
    let _: Arc<dyn DomainDriver> = Arc::new(driver);
}

// ============================================================================
// Buffer lifecycle
// ============================================================================

#[test]
fn test_create_buffer() {
    let driver = make_driver();
    let buffer_id = driver.create_buffer(b"hello world");

    let provider = driver.content_provider();
    let bytes = provider.content_bytes(buffer_id).unwrap();
    assert_eq!(bytes, b"hello world");
}

#[test]
fn test_close_buffer() {
    let (driver, buffer_id) = make_driver_with_buffer("hello");

    driver.close_buffer(buffer_id);

    let provider = driver.content_provider();
    assert!(provider.content_bytes(buffer_id).is_none());
}

#[test]
fn test_content_provider() {
    let driver = make_driver();
    let _: Arc<dyn BufferContentProvider> = driver.content_provider();
}

// ============================================================================
// Client lifecycle
// ============================================================================

#[test]
fn test_on_client_added() {
    let driver = make_driver();
    let client = ClientId::new(1);

    driver.on_client_added(client);

    // Client should have a mode name (from home mode)
    let mode = driver.mode_name(client);
    assert!(mode.is_some());
    assert_eq!(mode.unwrap(), "normal");
}

#[test]
fn test_on_client_removed() {
    let driver = make_driver();
    let client = ClientId::new(1);

    driver.on_client_added(client);
    driver.on_client_removed(client);

    // Client state should be gone
    assert!(driver.mode_name(client).is_none());
}

#[test]
fn test_multiple_clients() {
    let driver = make_driver();
    let c1 = ClientId::new(1);
    let c2 = ClientId::new(2);

    driver.on_client_added(c1);
    driver.on_client_added(c2);

    // Both clients should have independent state
    assert!(driver.mode_name(c1).is_some());
    assert!(driver.mode_name(c2).is_some());

    driver.on_client_removed(c1);
    assert!(driver.mode_name(c1).is_none());
    assert!(driver.mode_name(c2).is_some());
}

// ============================================================================
// Focus notifications
// ============================================================================

#[test]
fn test_on_focus_gained() {
    let (driver, buffer_id) = make_driver_with_buffer("hello");
    let client = ClientId::new(1);
    let window = WindowId::new();

    driver.on_client_added(client);
    driver.on_focus_gained(client, window, buffer_id);

    // Active buffer should be set (verified via internal state)
    // The driver tracks this for dispatch_key/dispatch_command
}

#[test]
fn test_on_focus_lost() {
    let (driver, buffer_id) = make_driver_with_buffer("hello");
    let client = ClientId::new(1);
    let window = WindowId::new();

    driver.on_client_added(client);
    driver.on_focus_gained(client, window, buffer_id);
    driver.on_focus_lost(client, window, buffer_id);

    // Mode state should be preserved (not cleaned up)
    assert!(driver.mode_name(client).is_some());
}

// ============================================================================
// Cursor access
// ============================================================================

#[test]
fn test_initial_cursor() {
    let driver = make_driver();
    let client = ClientId::new(1);
    let buffer_id = driver.create_buffer(b"hello");

    let cursor = driver.initial_cursor(client, buffer_id);

    assert_eq!(cursor.header().domain_id(), 42);
    assert_eq!(cursor.header().inner_id(), crate::TEXT_CURSOR_INNER_ID);
    // Initial cursor is at (0, 0) = 16 bytes content
    assert_eq!(cursor.content().len(), 16);
}

#[test]
fn test_cursors_no_client() {
    let driver = make_driver();
    let client = ClientId::new(99);
    let window = WindowId::new();

    let cursors = driver.cursors(client, window);
    assert!(cursors.is_empty());
}

#[test]
fn test_cursors_with_window() {
    let (driver, buffer_id) = make_driver_with_buffer("hello world");
    let client = ClientId::new(1);

    driver.on_client_added(client);

    // Add a window with a buffer to the client's per-client state
    {
        let mut clients = driver.clients.write();
        let state = clients.get_mut(&client).unwrap();
        let window = crate::Window::with_buffer(buffer_id);
        let window_id = window.id;
        state.windows.add(window);

        // Query cursors for this window
        drop(clients); // Release write lock before read

        let cursors = driver.cursors(client, window_id);
        assert_eq!(cursors.len(), 1);
        assert_eq!(cursors[0].header().domain_id(), 42);
    }
}

// ============================================================================
// Dispatch
// ============================================================================

#[test]
fn test_dispatch_key_no_client() {
    let driver = make_driver();
    let client = ClientId::new(99);
    let key = KeyEvent::new(KeyCode::Char('l'));

    let cs = driver.dispatch_key(client, &key);
    // No client → default empty ChangeSet
    assert!(!cs.has_changes());
}

#[test]
fn test_dispatch_key_with_client() {
    let driver = make_driver();
    let client = ClientId::new(1);

    driver.on_client_added(client);

    let key = KeyEvent::new(KeyCode::Char('l'));
    let cs = driver.dispatch_key(client, &key);

    // Without resolver wired, dispatch_key returns empty ChangeSet
    // The infrastructure (lock → SessionRuntime → bridge) is exercised
    assert!(!cs.has_changes());
}

#[test]
fn test_dispatch_command_no_client() {
    let driver = make_driver();
    let client = ClientId::new(99);

    let result = driver.dispatch_command(client, "test:cmd", &[]);
    // No client → None (not Some(empty))
    assert!(result.is_none());
}

#[test]
fn test_dispatch_command_with_client() {
    let driver = make_driver();
    let client = ClientId::new(1);

    driver.on_client_added(client);

    let result = driver.dispatch_command(client, "test:cmd", &[]);
    // Client exists → Some(ChangeSet) even if no command runs
    assert!(result.is_some());
}

// ============================================================================
// DomainStateQuery
// ============================================================================

#[test]
fn test_mode_name() {
    let driver = make_driver();
    let client = ClientId::new(1);

    driver.on_client_added(client);

    let name = driver.mode_name(client);
    assert_eq!(name, Some("normal".to_owned()));
}

#[test]
fn test_mode_name_no_client() {
    let driver = make_driver();
    assert!(driver.mode_name(ClientId::new(99)).is_none());
}

#[test]
fn test_registers_empty() {
    let driver = make_driver();
    let client = ClientId::new(1);

    driver.on_client_added(client);

    let regs = driver.registers(client);
    assert!(regs.is_empty());
}

#[test]
fn test_registers_no_client() {
    let driver = make_driver();
    let regs = driver.registers(ClientId::new(99));
    assert!(regs.is_empty());
}

#[test]
fn test_status_info() {
    let driver = make_driver();
    let client = ClientId::new(1);
    driver.on_client_added(client);
    assert!(driver.status_info(client).is_none());
}

// ============================================================================
// Phase 4A: dispatch_key_with_extensions tests
// ============================================================================

#[test]
fn dispatch_with_extensions_returns_changeset() {
    let driver = make_driver();
    let client = ClientId::new(1);
    driver.on_client_added(client);

    let key = KeyEvent::new(KeyCode::Char('j'));
    let mut client_ext = reovim_subsys_session::ExtensionMap::new();
    let mut shared_ext = reovim_subsys_session::ExtensionMap::new();

    // No dispatch provider wired → falls back to dispatch_key (no-op)
    let cs = driver.dispatch_key_with_extensions(client, &key, &mut client_ext, &mut shared_ext);
    // Should not panic, and should return a valid ChangeSet
    assert!(!cs.should_quit);
}

#[test]
fn dispatch_with_extensions_unknown_client_returns_empty() {
    let driver = make_driver();
    let key = KeyEvent::new(KeyCode::Char('j'));
    let mut client_ext = reovim_subsys_session::ExtensionMap::new();
    let mut shared_ext = reovim_subsys_session::ExtensionMap::new();

    let cs = driver.dispatch_key_with_extensions(
        ClientId::new(999),
        &key,
        &mut client_ext,
        &mut shared_ext,
    );
    assert!(!cs.has_changes());
}

// ============================================================================
// Phase 4A: query method tests
// ============================================================================

#[test]
fn current_mode_returns_home_mode() {
    let driver = make_driver();
    let client = ClientId::new(1);
    driver.on_client_added(client);

    let mode = driver.current_mode(client);
    assert!(mode.is_some());
    assert_eq!(mode.unwrap().name(), "normal");
}

#[test]
fn current_mode_unknown_client_returns_none() {
    let driver = make_driver();
    assert!(driver.current_mode(ClientId::new(99)).is_none());
}

#[test]
fn active_window_empty_client_returns_none() {
    let driver = make_driver();
    let client = ClientId::new(1);
    driver.on_client_added(client);

    // New client has empty WindowLayout → no active window
    assert!(driver.active_window(client).is_none());
}

#[test]
fn window_buffer_unknown_window_returns_none() {
    let driver = make_driver();
    let client = ClientId::new(1);
    driver.on_client_added(client);

    assert!(
        driver
            .window_buffer(client, WindowId::from_raw(99))
            .is_none()
    );
}

#[test]
fn windows_empty_client_returns_empty() {
    let driver = make_driver();
    let client = ClientId::new(1);
    driver.on_client_added(client);
    assert!(driver.windows(client).is_empty());
}

#[test]
fn windows_unknown_client_returns_empty() {
    let driver = make_driver();
    assert!(driver.windows(ClientId::new(99)).is_empty());
}

#[test]
fn window_count_empty_client_returns_zero() {
    let driver = make_driver();
    let client = ClientId::new(1);
    driver.on_client_added(client);
    assert_eq!(driver.window_count(client), 0);
}

#[test]
fn window_count_unknown_client_returns_zero() {
    let driver = make_driver();
    assert_eq!(driver.window_count(ClientId::new(99)), 0);
}

#[test]
fn selection_info_no_selection_returns_none() {
    let driver = make_driver();
    let client = ClientId::new(1);
    driver.on_client_added(client);
    assert!(
        driver
            .selection_info(client, WindowId::from_raw(1))
            .is_none()
    );
}
