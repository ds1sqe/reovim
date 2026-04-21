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
    reovim_subsys_input::INPUT_HEADER_SIZE,
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

/// Helper: build a SendInputRequest from a raw payload (must be a valid
/// InputEvent payload, i.e. >= 8 bytes).
fn send_input_request(payload: Vec<u8>) -> SendInputRequest {
    SendInputRequest {
        payload,
        window_id: None,
        timestamp_ns: 0,
    }
}

/// Helper: build an arbitrary opaque payload suitable for `InputEvent::new`.
///
/// The server is platform-agnostic — it only requires that the payload is at
/// least `INPUT_HEADER_SIZE` bytes and forwards the bytes to the domain
/// driver unchanged.  These tests exercise the forwarding invariant by
/// sending distinct byte sequences and asserting the driver received them
/// verbatim.  The payload contents have no codec meaning.
fn opaque_payload(tag: &[u8]) -> Vec<u8> {
    let mut payload = vec![0u8; INPUT_HEADER_SIZE];
    payload.extend_from_slice(tag);
    payload
}

/// Helper: build a deliberately short (invalid) payload for error-path tests.
fn short_payload(bytes: Vec<u8>) -> SendInputRequest {
    SendInputRequest {
        payload: bytes,
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
    payloads: Mutex<Vec<Vec<u8>>>,
    content_provider: Arc<dyn BufferContentProvider>,
}

impl CapturedKeyDriver {
    fn new() -> Self {
        Self {
            payloads: Mutex::new(Vec::new()),
            content_provider: Arc::new(TestContentProvider),
        }
    }

    fn captured_payloads(&self) -> Vec<Vec<u8>> {
        self.payloads.lock().unwrap().clone()
    }
}

impl reovim_subsys_session::DomainRouting for CapturedKeyDriver {
    fn current_mode(&self, _client_id: SubsysClientId) -> Option<reovim_kernel::api::v1::ModeId> {
        Some(reovim_kernel::api::v1::ModeId::new(
            reovim_kernel::api::v1::ModuleId::new("test"),
            "normal",
        ))
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

    fn dispatch_input(
        &self,
        _client_id: SubsysClientId,
        event: &reovim_subsys_input::InputEvent,
        _client_ext: &mut reovim_subsys_session::ExtensionMap,
        _shared_ext: &mut reovim_subsys_session::ExtensionMap,
    ) -> DispatchResult {
        self.payloads.lock().unwrap().push(event.payload().to_vec());
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
async fn test_send_input_short_payload_rejected() {
    // Payloads shorter than INPUT_HEADER_SIZE (8 bytes) are rejected.
    let registry = test_registry();
    registry
        .get(&SessionId::new("test"))
        .unwrap()
        .add_client(ClientId::new(1));
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    // 6 bytes — shorter than the 8-byte INPUT_HEADER_SIZE
    let request =
        authed_request(short_payload(vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00]), ClientId::new(1));
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

    // Auth check happens before session lookup; short payload is fine here.
    let request = authed_request(short_payload(b"a".to_vec()), ClientId::new(1));
    let response = service.send_input(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_send_input_rejects_unauthenticated() {
    let registry = test_registry();
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    // No ClientId in extensions — auth check happens before payload validation.
    let request = Request::new(short_payload(b"a".to_vec()));
    let response = service.send_input(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn test_send_input_client_not_found() {
    let registry = test_registry();
    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    // Client existence check happens before payload validation.
    let request = authed_request(short_payload(b"a".to_vec()), ClientId::new(999));
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

    // Following check happens before payload validation.
    let request = authed_request(short_payload(b"a".to_vec()), client_id);
    let response = service.send_input(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert!(!resp.ok); // Input ignored for following clients
}

#[tokio::test]
async fn test_send_input_empty_payload_rejected() {
    // An empty payload is too short (< 8 bytes) and must be rejected.
    let registry = test_registry();
    let client_id = ClientId::new(1);
    registry
        .get(&SessionId::new("test"))
        .unwrap()
        .add_client(client_id);

    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let request = authed_request(short_payload(vec![]), client_id);
    let response = service.send_input(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_send_input_forwards_payload_unchanged() {
    // The server forwards the opaque input payload to the domain driver
    // byte-for-byte.  Content has no codec meaning at this layer.
    let registry = test_registry();
    let session = registry.get(&SessionId::new("test")).unwrap();
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let driver = Arc::new(CapturedKeyDriver::new());
    session.set_domain_driver(driver.clone());

    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let payload = opaque_payload(&[0xAA, 0xBB, 0xCC]);
    let response = service
        .send_input(authed_request(send_input_request(payload.clone()), client_id))
        .await;

    assert!(response.is_ok());
    assert_eq!(driver.captured_payloads(), vec![payload]);
}

#[tokio::test]
async fn test_send_input_forwards_distinct_payloads_distinctly() {
    // Two distinct payloads arrive at the driver as two distinct byte
    // sequences — the server does not merge, reinterpret, or reorder.
    let registry = test_registry();
    let session = registry.get(&SessionId::new("test")).unwrap();
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let driver = Arc::new(CapturedKeyDriver::new());
    session.set_domain_driver(driver.clone());

    let service =
        InputServiceImpl::new(registry, SessionId::new("test"), Arc::new(BridgeRegistry::new()));

    let first = opaque_payload(&[0xDE, 0xAD]);
    let second = opaque_payload(&[0xBE, 0xEF, 0x01]);
    for payload in [&first, &second] {
        let r = service
            .send_input(authed_request(send_input_request(payload.clone()), client_id))
            .await;
        assert!(r.is_ok());
    }

    assert_eq!(driver.captured_payloads(), vec![first, second]);
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
