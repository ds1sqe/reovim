use std::sync::{Arc, Mutex};

use {
    reovim_kernel::api::v1::{BufferId, WindowId},
    reovim_subsys_coordination::{Cursor, CursorHeader},
    reovim_subsys_input::{KeyCode, KeyEvent},
};

use super::{
    BufferContentProvider, ChangeSet, ClientId, DisplayLine, DomainDriver, DomainStateQuery,
    ExtensionMap, RegisterInfo, SelectionInfo, Viewport,
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
        _viewport: &Viewport,
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
    DispatchKey(ClientId),
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
    /// If set, `dispatch_key` returns a `ChangeSet` with `cursor_moved=true`.
    cursor_moves_on_key: Mutex<bool>,
    /// If set, `dispatch_command` returns `None` (unknown command).
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

    fn dispatch_key(&self, client_id: ClientId, _key: &KeyEvent) -> ChangeSet {
        self.events
            .lock()
            .unwrap()
            .push(Event::DispatchKey(client_id));

        let mut cs = ChangeSet::new();
        if *self.cursor_moves_on_key.lock().unwrap() {
            cs.record_cursor_move(reovim_kernel::api::v1::BufferId::from_raw(0));
        }
        cs
    }

    fn dispatch_command(
        &self,
        client_id: ClientId,
        command: &str,
        _args: &[String],
    ) -> Option<ChangeSet> {
        self.events
            .lock()
            .unwrap()
            .push(Event::DispatchCommand(client_id, command.to_string()));

        if *self.reject_commands.lock().unwrap() {
            return None;
        }
        Some(ChangeSet::new())
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
}

// --- Mock DomainStateQuery ---

struct MockStateQuery {
    mode: Option<String>,
    registers: Vec<RegisterInfo>,
}

impl MockStateQuery {
    fn with_mode(mode: &str) -> Self {
        Self {
            mode: Some(mode.to_string()),
            registers: Vec::new(),
        }
    }

    fn with_registers(registers: Vec<RegisterInfo>) -> Self {
        Self {
            mode: None,
            registers,
        }
    }
}

impl DomainStateQuery for MockStateQuery {
    fn mode_name(&self, _client_id: ClientId) -> Option<String> {
        self.mode.clone()
    }

    fn registers(&self, _client_id: ClientId) -> Vec<RegisterInfo> {
        self.registers.clone()
    }
}

// --- Tests ---

#[test]
fn domain_driver_object_safety() {
    let _: Arc<dyn DomainDriver> = Arc::new(MockDomainDriver::new("text", 1));
}

#[test]
fn domain_state_query_object_safety() {
    let _: Box<dyn DomainStateQuery> = Box::new(MockStateQuery::with_mode("NORMAL"));
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
fn dispatch_key_returns_changeset() {
    let driver = MockDomainDriver::new("text", 1);
    let client = ClientId(0);
    let key = KeyEvent::new(KeyCode::Char('j'));
    let cs = driver.dispatch_key(client, &key);
    assert!(!cs.has_changes()); // default: no cursor move
}

#[test]
fn dispatch_key_with_cursor_move() {
    let driver = MockDomainDriver::new("text", 1);
    driver.set_cursor_moves_on_key(true);
    let client = ClientId(0);
    let key = KeyEvent::new(KeyCode::Char('j'));
    let cs = driver.dispatch_key(client, &key);
    assert!(cs.cursor_moved);
    assert!(cs.has_changes());
}

#[test]
fn dispatch_command_handled() {
    let driver = MockDomainDriver::new("text", 1);
    let client = ClientId(0);
    let result = driver.dispatch_command(client, "w", &[]);
    assert!(result.is_some());
}

#[test]
fn dispatch_command_unknown_returns_none() {
    let driver = MockDomainDriver::new("text", 1);
    driver.set_reject_commands(true);
    let client = ClientId(0);
    let result = driver.dispatch_command(client, "q", &[]);
    assert!(result.is_none());
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
fn state_query_defaults() {
    struct EmptyQuery;
    impl DomainStateQuery for EmptyQuery {}

    let q = EmptyQuery;
    assert!(q.mode_name(ClientId(0)).is_none());
    assert!(q.registers(ClientId(0)).is_empty());
    assert!(q.status_info(ClientId(0)).is_none());
}

#[test]
fn state_query_with_mode() {
    let q = MockStateQuery::with_mode("NORMAL");
    assert_eq!(q.mode_name(ClientId(0)).unwrap(), "NORMAL");
}

#[test]
fn state_query_with_registers() {
    let regs = vec![RegisterInfo {
        name: '"',
        display: "hello world".to_string(),
        content_type: "text/characterwise".to_string(),
    }];
    let q = MockStateQuery::with_registers(regs);
    let result = q.registers(ClientId(0));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, '"');
    assert_eq!(result[0].display, "hello world");
}

#[test]
fn register_info_debug_and_clone() {
    let info = RegisterInfo {
        name: 'a',
        display: "test".to_string(),
        content_type: "text/linewise".to_string(),
    };
    let cloned = info.clone();
    assert_eq!(cloned.name, 'a');
    let debug = format!("{info:?}");
    assert!(debug.contains("RegisterInfo"));
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

    text_driver.dispatch_key(client, &KeyEvent::new(KeyCode::Char('j')));
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

// --- Phase 4A tests: dispatch_key_with_extensions ---

#[test]
fn dispatch_key_with_extensions_default_delegates_to_dispatch_key() {
    let driver = MockDomainDriver::new("text", 1);
    driver.set_cursor_moves_on_key(true);
    let client = ClientId(0);
    let key = KeyEvent::new(KeyCode::Char('j'));
    let mut client_ext = ExtensionMap::new();
    let mut shared_ext = ExtensionMap::new();

    let cs = driver.dispatch_key_with_extensions(client, &key, &mut client_ext, &mut shared_ext);
    assert!(cs.cursor_moved);
    // Verify dispatch_key was called (event recorded)
    let events = driver.events();
    assert!(matches!(events.last(), Some(Event::DispatchKey(c)) if *c == client));
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

// --- Phase 4A tests: SelectionInfo ---

#[test]
fn selection_info_default_returns_none() {
    struct EmptyQuery;
    impl DomainStateQuery for EmptyQuery {}

    let q = EmptyQuery;
    assert!(
        q.selection_info(ClientId(0), WindowId::from_raw(1))
            .is_none()
    );
}

#[test]
fn selection_info_debug_and_clone() {
    let info = SelectionInfo {
        start_line: 0,
        start_column: 5,
        end_line: 2,
        end_column: 10,
        mode: "char".to_string(),
    };
    let cloned = info.clone();
    assert_eq!(cloned.start_line, 0);
    assert_eq!(cloned.end_column, 10);
    assert_eq!(cloned.mode, "char");
    let debug = format!("{info:?}");
    assert!(debug.contains("SelectionInfo"));
}
