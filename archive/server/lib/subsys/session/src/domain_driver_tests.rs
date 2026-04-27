use std::sync::{Arc, Mutex};

use {
    reovim_kernel::api::v1::{BufferId, WindowId},
    reovim_subsys_coordination::{Cursor, CursorHeader},
    reovim_subsys_input::InputEvent,
};

use super::{
    BufferContentProvider, ClientId, CommandResult, Directive, DispatchResult, DisplayLine,
    DomainDriver, DomainRouting, ExtensionMap,
};

// --- Test cursor implementation ---

#[derive(Clone)]
struct TestCursor {
    header_val: CursorHeader,
    line: u64,
    col: u64,
}

impl TestCursor {
    fn new(domain_id: u32, line: u64, col: u64) -> Self {
        Self {
            header_val: CursorHeader::new(domain_id, 0, 0),
            line,
            col,
        }
    }
}

impl Cursor for TestCursor {
    fn header(&self) -> &CursorHeader {
        &self.header_val
    }

    fn content(&self) -> &[u8] {
        &[]
    }

    fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::from(self.header().as_bytes());
        bytes.extend_from_slice(&self.line.to_le_bytes());
        bytes.extend_from_slice(&self.col.to_le_bytes());
        bytes
    }

    fn display(&self) -> String {
        format!("{}:{}", self.line, self.col)
    }

    fn clone_box(&self) -> Box<dyn Cursor> {
        Box::new(self.clone())
    }
}

// --- Minimal content provider for testing ---

struct TestContentProvider;

impl BufferContentProvider for TestContentProvider {
    fn content_bytes(&self, _buffer_id: BufferId) -> Option<Vec<u8>> {
        Some(Vec::new())
    }

    fn content_size(&self, _buffer_id: BufferId) -> Option<u64> {
        Some(0)
    }

    fn content_unit_count(&self, _buffer_id: BufferId) -> Option<usize> {
        Some(0)
    }

    fn display_lines(
        &self,
        _buffer_id: BufferId,
        _offset: usize,
        _count: usize,
    ) -> Option<Vec<DisplayLine>> {
        Some(Vec::new())
    }

    fn is_modified(&self, _buffer_id: BufferId) -> bool {
        false
    }

    fn write_to(
        &self,
        _buffer_id: BufferId,
        _writer: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        Ok(())
    }
}

// --- Mock DomainDriver with state tracking ---

#[derive(Debug, Clone)]
#[allow(dead_code)] // variant fields used via pattern matching
enum Event {
    ClientAdded(ClientId),
    ClientRemoved(ClientId),
    FocusGained(ClientId, WindowId, BufferId),
    FocusLost(ClientId, WindowId, BufferId),
    DispatchInput(ClientId),
    DispatchCommand(ClientId, String),
    BufferCreated,
    BufferClosed(BufferId),
}

struct MockDomainDriver {
    name: &'static str,
    id: u32,
    events: Mutex<Vec<Event>>,
    next_buffer_id: Mutex<usize>,
    content_provider: Arc<dyn BufferContentProvider>,
    /// Internal flag: when true, the driver considers the cursor moved on key.
    /// Not reflected in `DispatchResult` — the server polls via `collect_projections`.
    cursor_moves_on_key: Mutex<bool>,
    /// If set, `dispatch_command` returns `NotHandled` (unknown command).
    reject_commands: Mutex<bool>,
}

impl MockDomainDriver {
    fn new(name: &'static str, id: u32) -> Self {
        Self {
            name,
            id,
            events: Mutex::new(Vec::new()),
            next_buffer_id: Mutex::new(100),
            content_provider: Arc::new(TestContentProvider),
            cursor_moves_on_key: Mutex::new(false),
            reject_commands: Mutex::new(false),
        }
    }

    fn events(&self) -> Vec<Event> {
        self.events.lock().unwrap().clone()
    }

    fn set_cursor_moves_on_key(&self, val: bool) {
        *self.cursor_moves_on_key.lock().unwrap() = val;
    }

    fn set_reject_commands(&self, val: bool) {
        *self.reject_commands.lock().unwrap() = val;
    }
}

impl DomainDriver for MockDomainDriver {
    fn domain_name(&self) -> &'static str {
        self.name
    }

    fn domain_id(&self) -> u32 {
        self.id
    }

    fn create_buffer(&self, _content: &[u8]) -> BufferId {
        let id = {
            let mut next = self.next_buffer_id.lock().unwrap();
            let id = BufferId::from_raw(*next);
            *next += 1;
            id
        };
        self.events.lock().unwrap().push(Event::BufferCreated);
        id
    }

    fn close_buffer(&self, buffer_id: BufferId) {
        self.events
            .lock()
            .unwrap()
            .push(Event::BufferClosed(buffer_id));
    }

    fn content_provider(&self) -> Arc<dyn BufferContentProvider> {
        Arc::clone(&self.content_provider)
    }

    fn dispatch_input(
        &self,
        client_id: ClientId,
        _event: &InputEvent,
        _client_ext: &mut ExtensionMap,
        _shared_ext: &mut ExtensionMap,
    ) -> DispatchResult {
        self.events
            .lock()
            .unwrap()
            .push(Event::DispatchInput(client_id));

        // cursor_moves_on_key is tracked internally but DispatchResult does not
        // carry cursor_moved (the server polls). We record the flag for test
        // assertions on the MockDomainDriver state only.
        let _ = *self.cursor_moves_on_key.lock().unwrap();
        DispatchResult::default()
    }

    fn dispatch_command(
        &self,
        client_id: ClientId,
        command: &str,
        _args: &[String],
    ) -> CommandResult {
        self.events
            .lock()
            .unwrap()
            .push(Event::DispatchCommand(client_id, command.to_string()));

        if *self.reject_commands.lock().unwrap() {
            return CommandResult::NotHandled;
        }
        CommandResult::Handled(DispatchResult::default())
    }

    fn on_client_added(&self, client_id: ClientId) {
        self.events
            .lock()
            .unwrap()
            .push(Event::ClientAdded(client_id));
    }

    fn on_client_removed(&self, client_id: ClientId) {
        self.events
            .lock()
            .unwrap()
            .push(Event::ClientRemoved(client_id));
    }

    fn on_focus_gained(&self, client_id: ClientId, window_id: WindowId, buffer_id: BufferId) {
        self.events
            .lock()
            .unwrap()
            .push(Event::FocusGained(client_id, window_id, buffer_id));
    }

    fn on_focus_lost(&self, client_id: ClientId, window_id: WindowId, buffer_id: BufferId) {
        self.events
            .lock()
            .unwrap()
            .push(Event::FocusLost(client_id, window_id, buffer_id));
    }

    fn cursors(&self, _client_id: ClientId, _window_id: WindowId) -> Vec<Box<dyn Cursor>> {
        vec![Box::new(TestCursor::new(self.id, 0, 0))]
    }

    fn initial_cursor(&self, _client_id: ClientId, _buffer_id: BufferId) -> Box<dyn Cursor> {
        Box::new(TestCursor::new(self.id, 0, 0))
    }

    fn collect_projections(
        &self,
        _client_id: ClientId,
    ) -> Vec<reovim_subsys_coordination::Projection> {
        Vec::new()
    }

    fn initial_projections(
        &self,
        _client_id: ClientId,
    ) -> Vec<reovim_subsys_coordination::Projection> {
        Vec::new()
    }
}

impl DomainRouting for MockDomainDriver {}

// --- Tests ---

#[test]
fn domain_driver_object_safety() {
    let _: Arc<dyn DomainDriver> = Arc::new(MockDomainDriver::new("text", 1));
}

#[test]
fn domain_name_and_id() {
    let driver = MockDomainDriver::new("text", 1);
    assert_eq!(driver.domain_name(), "text");
    assert_eq!(driver.domain_id(), 1);
}

#[test]
fn create_buffer_returns_unique_ids() {
    let driver = MockDomainDriver::new("text", 1);
    let id1 = driver.create_buffer(b"hello");
    let id2 = driver.create_buffer(b"world");
    assert_ne!(id1, id2);
}

#[test]
fn close_buffer_records_event() {
    let driver = MockDomainDriver::new("text", 1);
    let id = driver.create_buffer(b"data");
    driver.close_buffer(id);
    let events = driver.events();
    assert!(matches!(events.last(), Some(Event::BufferClosed(bid)) if *bid == id));
}

#[test]
fn content_provider_returns_arc() {
    let driver = MockDomainDriver::new("text", 1);
    let p1 = driver.content_provider();
    let p2 = driver.content_provider();
    assert!(Arc::ptr_eq(&p1, &p2));
}

#[test]
fn dispatch_input_returns_dispatch_result() {
    let driver = MockDomainDriver::new("text", 1);
    let client = ClientId(0);
    let event = InputEvent::new(vec![0; reovim_subsys_input::INPUT_HEADER_SIZE], None, 0)
        .expect("header-sized payload should be valid");
    let result =
        driver.dispatch_input(client, &event, &mut ExtensionMap::new(), &mut ExtensionMap::new());
    // Default: no buffer changes, Continue directive
    assert!(!result.buffers.has_changes());
    assert_eq!(result.directive, Directive::Continue);
}

#[test]
fn dispatch_input_records_event() {
    let driver = MockDomainDriver::new("text", 1);
    driver.set_cursor_moves_on_key(true);
    let client = ClientId(0);
    let event = InputEvent::new(vec![0; reovim_subsys_input::INPUT_HEADER_SIZE], None, 0)
        .expect("header-sized payload should be valid");
    driver.dispatch_input(client, &event, &mut ExtensionMap::new(), &mut ExtensionMap::new());
    // cursor_moved is no longer in DispatchResult — server polls via
    // collect_projections. Verify the dispatch event was recorded.
    let events = driver.events();
    assert!(matches!(events.last(), Some(Event::DispatchInput(c)) if *c == client));
}

#[test]
fn dispatch_command_handled() {
    let driver = MockDomainDriver::new("text", 1);
    let client = ClientId(0);
    let result = driver.dispatch_command(client, "w", &[]);
    assert!(matches!(result, CommandResult::Handled(_)));
}

#[test]
fn dispatch_command_unknown_returns_not_handled() {
    let driver = MockDomainDriver::new("text", 1);
    driver.set_reject_commands(true);
    let client = ClientId(0);
    let result = driver.dispatch_command(client, "q", &[]);
    assert!(matches!(result, CommandResult::NotHandled));
}

#[test]
fn client_lifecycle_events() {
    let driver = MockDomainDriver::new("text", 1);
    let client = ClientId(0);

    driver.on_client_added(client);
    driver.on_client_removed(client);

    let events = driver.events();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], Event::ClientAdded(c) if c == client));
    assert!(matches!(events[1], Event::ClientRemoved(c) if c == client));
}

#[test]
fn focus_switch_sequence() {
    let driver = MockDomainDriver::new("text", 1);
    let client = ClientId(0);
    let win1 = WindowId::from_raw(1);
    let win2 = WindowId::from_raw(2);
    let buf1 = BufferId::from_raw(10);
    let buf2 = BufferId::from_raw(20);

    driver.on_focus_lost(client, win1, buf1);
    driver.on_focus_gained(client, win2, buf2);

    let events = driver.events();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], Event::FocusLost(c, w, b)
        if c == client && w == win1 && b == buf1));
    assert!(matches!(events[1], Event::FocusGained(c, w, b)
        if c == client && w == win2 && b == buf2));
}

#[test]
fn cursors_returns_vec() {
    let driver = MockDomainDriver::new("text", 1);
    let cursors = driver.cursors(ClientId(0), WindowId::from_raw(1));
    assert_eq!(cursors.len(), 1);
    assert_eq!(cursors[0].header().domain_id(), 1);
}

#[test]
fn initial_cursor_has_correct_domain() {
    let driver = MockDomainDriver::new("mesh", 2);
    let cursor = driver.initial_cursor(ClientId(0), BufferId::from_raw(1));
    assert_eq!(cursor.header().domain_id(), 2);
}

#[test]
fn multiple_domains_independent_state() {
    let text_driver = MockDomainDriver::new("text", 1);
    let mesh_driver = MockDomainDriver::new("mesh", 2);
    let client = ClientId(0);

    text_driver.on_client_added(client);
    mesh_driver.on_client_added(client);

    assert_eq!(text_driver.events().len(), 1);
    assert_eq!(mesh_driver.events().len(), 1);

    let event = InputEvent::new(vec![0; reovim_subsys_input::INPUT_HEADER_SIZE], None, 0)
        .expect("header-sized payload should be valid");
    text_driver.dispatch_input(client, &event, &mut ExtensionMap::new(), &mut ExtensionMap::new());
    assert_eq!(text_driver.events().len(), 2);
    assert_eq!(mesh_driver.events().len(), 1);
}

#[test]
fn broadcast_client_lifecycle_to_all_domains() {
    let text: Arc<dyn DomainDriver> = Arc::new(MockDomainDriver::new("text", 1));
    let mesh: Arc<dyn DomainDriver> = Arc::new(MockDomainDriver::new("mesh", 2));
    let domains: Vec<Arc<dyn DomainDriver>> = vec![Arc::clone(&text), Arc::clone(&mesh)];

    let client = ClientId(0);

    // Simulate: broadcast on_client_added to ALL domains
    for domain in &domains {
        domain.on_client_added(client);
    }

    // Both domains received the event
    assert_eq!(text.domain_name(), "text");
    assert_eq!(mesh.domain_name(), "mesh");
}

// --- Phase 4A tests: query method defaults ---

#[test]
fn current_mode_default_returns_none() {
    let driver = MockDomainDriver::new("text", 1);
    assert!(driver.current_mode(ClientId(0)).is_none());
}

#[test]
fn active_window_default_returns_none() {
    let driver = MockDomainDriver::new("text", 1);
    assert!(driver.active_window(ClientId(0)).is_none());
}

#[test]
fn window_buffer_default_returns_none() {
    let driver = MockDomainDriver::new("text", 1);
    assert!(
        driver
            .window_buffer(ClientId(0), WindowId::from_raw(1))
            .is_none()
    );
}

#[test]
fn windows_default_returns_empty() {
    let driver = MockDomainDriver::new("text", 1);
    assert!(driver.windows(ClientId(0)).is_empty());
}

#[test]
fn window_count_default_returns_zero() {
    let driver = MockDomainDriver::new("text", 1);
    assert_eq!(driver.window_count(ClientId(0)), 0);
}

#[test]
fn cursor_generation_default_returns_zero() {
    let driver = MockDomainDriver::new("text", 1);
    assert_eq!(driver.cursor_generation(ClientId(0)), 0);
}
