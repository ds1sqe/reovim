//! Tests for `InputService::send_input` gRPC endpoint.
//!
//! Tests for removed internal helpers (resolve_to_command_context,
//! handle_resolve_result, apply_mode_transition_for_client,
//! handle_pop_result_for_client, emit_syntax_updates, notify_codec_indices)
//! have been deleted — those methods were removed in #753 E6.
//! Domain dispatch testing lives in driver-text-session.

use {
    super::*,
    std::sync::{Arc, Mutex},
};

use {
    reovim_kernel::api::v1::{BufferId, WindowId},
    reovim_subsys_coordination::{Cursor, CursorHeader, Projection},
    reovim_subsys_session::{
        BufferContentProvider, ClientId as SubsysClientId, CommandResult, DispatchResult,
        DisplayLine, DomainDriver,
    },
};

fn test_registry() -> Arc<SessionRegistry> {
    let registry = Arc::new(SessionRegistry::new());
    let session = Arc::new(Session::new(SessionId::new("test")));
    registry.insert(&session);
    registry
}

/// Helper: build a request with token-authenticated `ClientId` in extensions.
fn authed_request<T>(body: T, client_id: ClientId) -> Request<T> {
    let mut request = Request::new(body);
    request.extensions_mut().insert(client_id);
    request
}

/// Helper: build a SendInputRequest from a key string.
fn send_input_request(keys: &str) -> SendInputRequest {
    SendInputRequest {
        payload: keys.as_bytes().to_vec(),
        window_id: None,
        timestamp_ns: 0,
    }
}

#[derive(Clone)]
struct TestCursor {
    header: CursorHeader,
}

impl TestCursor {
    fn new() -> Self {
        Self {
            header: CursorHeader::new(1, 0, 0),
        }
    }
}

impl Cursor for TestCursor {
    fn header(&self) -> &CursorHeader {
        &self.header
    }

    fn content(&self) -> &[u8] {
        &[]
    }

    fn encode(&self) -> Vec<u8> {
        self.header.as_bytes().to_vec()
    }

    fn display(&self) -> String {
        String::new()
    }

    fn clone_box(&self) -> Box<dyn Cursor> {
        Box::new(self.clone())
    }
}

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

struct CapturedKeyDriver {
    keys: Mutex<Vec<reovim_subsys_input::KeyEvent>>,
    content_provider: Arc<dyn BufferContentProvider>,
}

impl CapturedKeyDriver {
    fn new() -> Self {
        Self {
            keys: Mutex::new(Vec::new()),
            content_provider: Arc::new(TestContentProvider),
        }
    }

    fn captured_keys(&self) -> Vec<reovim_subsys_input::KeyEvent> {
        self.keys.lock().unwrap().clone()
    }
}

impl DomainDriver for CapturedKeyDriver {
    fn domain_name(&self) -> &'static str {
        "test"
    }

    fn domain_id(&self) -> u32 {
        1
    }

    fn create_buffer(&self, _content: &[u8]) -> BufferId {
        BufferId::new()
    }

    fn close_buffer(&self, _buffer_id: BufferId) {}

    fn content_provider(&self) -> Arc<dyn BufferContentProvider> {
        Arc::clone(&self.content_provider)
    }

    fn dispatch_key(
        &self,
        _client_id: SubsysClientId,
        key: &reovim_subsys_input::KeyEvent,
    ) -> DispatchResult {
        self.keys.lock().unwrap().push(*key);
        DispatchResult::default()
    }

    fn dispatch_command(
        &self,
        _client_id: SubsysClientId,
        _command: &str,
        _args: &[String],
    ) -> CommandResult {
        CommandResult::NotHandled
    }

    fn on_client_added(&self, _client_id: SubsysClientId) {}

    fn on_client_removed(&self, _client_id: SubsysClientId) {}

    fn on_focus_gained(
        &self,
        _client_id: SubsysClientId,
        _window_id: WindowId,
        _buffer_id: BufferId,
    ) {
    }

    fn on_focus_lost(
        &self,
        _client_id: SubsysClientId,
        _window_id: WindowId,
        _buffer_id: BufferId,
    ) {
    }

    fn cursors(&self, _client_id: SubsysClientId, _window_id: WindowId) -> Vec<Box<dyn Cursor>> {
        vec![Box::new(TestCursor::new())]
    }

    fn initial_cursor(&self, _client_id: SubsysClientId, _buffer_id: BufferId) -> Box<dyn Cursor> {
        Box::new(TestCursor::new())
    }

    fn collect_projections(&self, _client_id: SubsysClientId) -> Vec<Projection> {
        Vec::new()
    }

    fn initial_projections(&self, _client_id: SubsysClientId) -> Vec<Projection> {
        Vec::new()
    }
}

// =========================================================================
// send_input: error paths
// =========================================================================

#[tokio::test]
async fn test_send_input_invalid_notation() {
    let registry = test_registry();
    registry
        .get(&SessionId::new("test"))
        .unwrap()
        .add_client(ClientId::new(1));
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let request = authed_request(send_input_request("<Ctrl"), ClientId::new(1));
    let response = service.send_input(request).await;

    assert!(response.is_err());
    let status = response.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_send_input_no_session() {
    let registry = Arc::new(SessionRegistry::new());
    let service = InputServiceImpl::new(
        registry,
        SessionId::new("nonexistent"),
        Arc::new(BridgeRegistry::new()),
    );

    let request = authed_request(send_input_request("a"), ClientId::new(1));
    let response = service.send_input(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_send_input_rejects_unauthenticated() {
    let registry = test_registry();
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    // No ClientId in extensions
    let request = Request::new(send_input_request("a"));
    let response = service.send_input(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn test_send_input_client_not_found() {
    let registry = test_registry();
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    // Client ID 999 doesn't exist
    let request = authed_request(send_input_request("a"), ClientId::new(999));
    let response = service.send_input(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::FailedPrecondition);
}

#[tokio::test]
async fn test_send_input_following_client_ignored() {
    let registry = test_registry();
    let session = registry.get(&SessionId::new("test")).unwrap();
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Set client to Following mode — need a target client
    let target_id = ClientId::new(2);
    session.add_client(target_id);
    session.clients().set_client_relation_unchecked(
        client_id,
        Some(crate::session::ClientRelation::Following { target: target_id }),
    );

    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let request = authed_request(send_input_request("a"), client_id);
    let response = service.send_input(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(!resp.ok); // Input ignored for following clients
}

#[tokio::test]
async fn test_send_input_invalid_utf8() {
    let registry = test_registry();
    let client_id = ClientId::new(1);
    registry
        .get(&SessionId::new("test"))
        .unwrap()
        .add_client(client_id);

    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    // Invalid UTF-8 bytes
    let request = authed_request(
        SendInputRequest {
            payload: vec![0xFF, 0xFE],
            window_id: None,
            timestamp_ns: 0,
        },
        client_id,
    );
    let response = service.send_input(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_send_input_empty_key_sequence() {
    let registry = test_registry();
    let client_id = ClientId::new(1);
    registry
        .get(&SessionId::new("test"))
        .unwrap()
        .add_client(client_id);

    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    // Empty key string — KeySequence::parse returns None for empty input
    let request = authed_request(send_input_request(""), client_id);
    let response = service.send_input(request).await;

    // Empty string is invalid key notation
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_send_input_adapts_single_key_notation_before_dispatch() {
    let registry = test_registry();
    let session = registry.get(&SessionId::new("test")).unwrap();
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let driver = Arc::new(CapturedKeyDriver::new());
    session.set_domain_driver(driver.clone());

    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let response = service
        .send_input(authed_request(send_input_request("j"), client_id))
        .await;

    assert!(response.is_ok());
    assert_eq!(
        driver.captured_keys(),
        vec![reovim_subsys_input::KeyEvent::new(
            reovim_subsys_input::KeyCode::Char('j')
        )]
    );
}

#[tokio::test]
async fn test_send_input_adapts_multi_token_special_notation_before_dispatch() {
    let registry = test_registry();
    let session = registry.get(&SessionId::new("test")).unwrap();
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let driver = Arc::new(CapturedKeyDriver::new());
    session.set_domain_driver(driver.clone());

    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let response = service
        .send_input(authed_request(send_input_request("<C-w>h"), client_id))
        .await;

    assert!(response.is_ok());
    assert_eq!(
        driver.captured_keys(),
        vec![
            reovim_subsys_input::KeyEvent::with_modifiers(
                reovim_subsys_input::KeyCode::Char('w'),
                reovim_subsys_input::Modifiers::CTRL,
            ),
            reovim_subsys_input::KeyEvent::new(reovim_subsys_input::KeyCode::Char('h')),
        ]
    );
}

// =========================================================================
// InputServiceImpl construction
// =========================================================================

#[test]
fn test_input_service_new() {
    let registry = Arc::new(SessionRegistry::new());
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));
    let _ = service;
}

// resolve_to_command_context: REMOVED (#753 E6) — driver types
// handle_resolve_result: REMOVED (#753 E6) — driver types
// apply_mode_transition_for_client: REMOVED (#753 E6) — driver types
// handle_pop_result_for_client: REMOVED (#753 E6) — driver types
// emit_syntax_updates: REMOVED (#753 E6) — polling architecture
// notify_codec_indices: REMOVED (#753 E6) — polling architecture
// emit_notifications_with_*: REMOVED (#753 E6) — StateChanges deleted
// Codec routing tests: REMOVED (#753 E6) — codec routes through domain driver
