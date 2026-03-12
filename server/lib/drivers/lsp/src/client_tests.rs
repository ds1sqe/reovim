use super::*;

use std::path::PathBuf;

/// Create a test client without spawning a real language server.
///
/// Uses `sleep 60` as a dummy process to keep the Child handle alive.
/// Returns the client and a receiver for messages sent by the client.
fn test_client_with_rx() -> (Client, tokio::sync::mpsc::UnboundedReceiver<Message>) {
    let process = Command::new("sleep")
        .arg("60")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("sleep should be available");

    let (writer_tx, writer_rx) = tokio::sync::mpsc::unbounded_channel();

    let config = LspServerConfig {
        command: "sleep".to_string(),
        args: vec!["60".to_string()],
        root_path: PathBuf::from("/tmp/test-root"),
        workspace_folders: vec![],
    };

    let client = Client {
        config,
        next_id: AtomicU64::new(1),
        pending: Arc::new(Mutex::new(HashMap::new())),
        writer_tx,
        state: Arc::new(Mutex::new(None)),
        _process: process,
    };

    (client, writer_rx)
}

#[test]
fn test_server_state_debug() {
    let state = ServerState {
        capabilities: ServerCapabilities::default(),
        initialized: true,
    };
    let debug = format!("{state:?}");
    assert!(debug.contains("initialized: true"));
}

#[test]
fn test_server_state_clone() {
    let state = ServerState {
        capabilities: ServerCapabilities::default(),
        initialized: true,
    };
    let cloned = state.clone();
    assert_eq!(cloned.initialized, state.initialized);
}

#[test]
fn test_client_capabilities_has_text_document() {
    let caps = Client::client_capabilities();
    assert!(caps.text_document.is_some());
    let td = caps.text_document.unwrap();
    assert!(td.synchronization.is_some());
    assert!(td.publish_diagnostics.is_some());
    assert!(td.hover.is_some());
    assert!(td.references.is_some());
    assert!(td.definition.is_some());
}

#[test]
fn test_client_capabilities_has_window() {
    let caps = Client::client_capabilities();
    assert!(caps.window.is_some());
    let window = caps.window.unwrap();
    assert_eq!(window.work_done_progress, Some(true));
}

#[tokio::test]
async fn test_next_request_id_increments() {
    let (client, _rx) = test_client_with_rx();
    let id1 = client.next_request_id();
    let id2 = client.next_request_id();
    assert_eq!(id1, Id::Number(1));
    assert_eq!(id2, Id::Number(2));
}

#[tokio::test]
async fn test_notify_sends_message() {
    let (client, mut rx) = test_client_with_rx();
    client.notify("test/notification", Value::Null).unwrap();
    let msg = rx.recv().await.expect("should receive message");
    assert!(matches!(msg, Message::Notification(_)));
}

#[tokio::test]
async fn test_notify_channel_closed_returns_error() {
    let (client, rx) = test_client_with_rx();
    drop(rx);
    let result = client.notify("test/notification", Value::Null);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_send_response_sends_on_channel() {
    let (client, mut rx) = test_client_with_rx();
    let response = Response::success(Id::Number(1), Value::Null);
    client.send_response(response);
    let msg = rx.recv().await.expect("should receive");
    assert!(matches!(msg, Message::Response(_)));
}

#[tokio::test]
async fn test_send_response_channel_closed_no_panic() {
    let (client, rx) = test_client_with_rx();
    drop(rx);
    let response = Response::success(Id::Number(1), Value::Null);
    client.send_response(response);
}

#[tokio::test]
async fn test_handle_response_delivers_result() {
    let (client, _rx) = test_client_with_rx();
    let (tx, rx_oneshot) = oneshot::channel();
    {
        let mut pending = client.pending.lock().await;
        pending.insert(Id::Number(42), tx);
    }

    let response = Response::success(Id::Number(42), serde_json::json!("hello"));
    client.handle_response(response).await;

    let result = rx_oneshot.await.unwrap().unwrap();
    assert_eq!(result, serde_json::json!("hello"));
}

#[tokio::test]
async fn test_handle_response_delivers_error() {
    let (client, _rx) = test_client_with_rx();
    let (tx, rx_oneshot) = oneshot::channel();
    {
        let mut pending = client.pending.lock().await;
        pending.insert(Id::Number(42), tx);
    }

    let error = crate::jsonrpc::Error::new(-32600, "invalid request");
    let response = Response::error(Id::Number(42), error);
    client.handle_response(response).await;

    let result = rx_oneshot.await.unwrap();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_handle_response_unknown_id_no_panic() {
    let (client, _rx) = test_client_with_rx();
    let response = Response::success(Id::Number(999), Value::Null);
    client.handle_response(response).await;
}

#[tokio::test]
async fn test_handle_response_receiver_dropped_no_panic() {
    let (client, _rx) = test_client_with_rx();
    let (tx, rx_oneshot) = oneshot::channel();
    {
        let mut pending = client.pending.lock().await;
        pending.insert(Id::Number(42), tx);
    }
    drop(rx_oneshot);

    let response = Response::success(Id::Number(42), Value::Null);
    client.handle_response(response).await;
}

#[tokio::test]
async fn test_handle_response_null_result() {
    let (client, _rx) = test_client_with_rx();
    let (tx, rx_oneshot) = oneshot::channel();
    {
        let mut pending = client.pending.lock().await;
        pending.insert(Id::Number(42), tx);
    }

    let response = Response {
        jsonrpc: "2.0".to_string(),
        id: Id::Number(42),
        result: None,
        error: None,
    };
    client.handle_response(response).await;

    let result = rx_oneshot.await.unwrap().unwrap();
    assert_eq!(result, Value::Null);
}

#[tokio::test]
async fn test_capabilities_initially_none() {
    let (client, _rx) = test_client_with_rx();
    assert!(client.capabilities().await.is_none());
}

#[tokio::test]
async fn test_is_initialized_initially_false() {
    let (client, _rx) = test_client_with_rx();
    assert!(!client.is_initialized().await);
}

#[tokio::test]
async fn test_root_path_returns_config_path() {
    let (client, _rx) = test_client_with_rx();
    assert_eq!(client.root_path(), Path::new("/tmp/test-root"));
}

#[tokio::test]
async fn test_client_debug_format() {
    let (client, _rx) = test_client_with_rx();
    let debug = format!("{client:?}");
    assert!(debug.contains("Client"));
    assert!(debug.contains("sleep"));
    assert!(debug.contains("/tmp/test-root"));
}

#[tokio::test]
async fn test_request_sends_and_receives() {
    let (client, mut rx) = test_client_with_rx();
    let client = Arc::new(client);
    let client_clone = Arc::clone(&client);

    let task = tokio::spawn(async move {
        client_clone
            .request::<Value, Value>("test/method", serde_json::json!({"key": "value"}))
            .await
    });

    let msg = rx.recv().await.unwrap();
    let request_id = match msg {
        Message::Request(req) => {
            assert_eq!(req.method, "test/method");
            req.id
        }
        _ => panic!("expected request message"),
    };

    client
        .handle_response(Response::success(request_id, serde_json::json!("result")))
        .await;

    let result = task.await.unwrap().unwrap();
    assert_eq!(result, serde_json::json!("result"));
}

#[tokio::test]
async fn test_request_channel_closed() {
    let (client, rx) = test_client_with_rx();
    drop(rx);

    let result = client
        .request::<Value, Value>("test/method", Value::Null)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_initialize_lifecycle() {
    let (client, mut rx) = test_client_with_rx();
    let client = Arc::new(client);
    let client_clone = Arc::clone(&client);

    let task = tokio::spawn(async move { client_clone.initialize().await });

    // Read initialize request
    let msg = rx.recv().await.unwrap();
    let request_id = match msg {
        Message::Request(req) => {
            assert_eq!(req.method, "initialize");
            req.id
        }
        _ => panic!("expected initialize request"),
    };

    // Send InitializeResult
    let init_result = InitializeResult {
        capabilities: ServerCapabilities::default(),
        server_info: None,
    };
    client
        .handle_response(Response::success(request_id, serde_json::to_value(init_result).unwrap()))
        .await;

    // Read initialized notification
    let msg = rx.recv().await.unwrap();
    match msg {
        Message::Notification(n) => assert_eq!(n.method, "initialized"),
        _ => panic!("expected initialized notification"),
    }

    let result = task.await.unwrap().unwrap();
    assert!(result.server_info.is_none());

    // Verify state was stored
    assert!(client.is_initialized().await);
    assert!(client.capabilities().await.is_some());
}

#[tokio::test]
async fn test_goto_definition_sends_request() {
    let (client, mut rx) = test_client_with_rx();
    let client = Arc::new(client);
    let client_clone = Arc::clone(&client);

    let uri: Uri = "file:///test.rs".parse().unwrap();
    let position = Position::new(0, 0);

    let task = tokio::spawn(async move { client_clone.goto_definition(uri, position).await });

    let msg = rx.recv().await.unwrap();
    let request_id = match msg {
        Message::Request(req) => {
            assert_eq!(req.method, "textDocument/definition");
            req.id
        }
        _ => panic!("expected request"),
    };

    client
        .handle_response(Response::success(request_id, Value::Null))
        .await;

    let result = task.await.unwrap().unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_references_sends_request() {
    let (client, mut rx) = test_client_with_rx();
    let client = Arc::new(client);
    let client_clone = Arc::clone(&client);

    let uri: Uri = "file:///test.rs".parse().unwrap();
    let position = Position::new(1, 5);

    let task = tokio::spawn(async move { client_clone.references(uri, position, true).await });

    let msg = rx.recv().await.unwrap();
    let request_id = match msg {
        Message::Request(req) => {
            assert_eq!(req.method, "textDocument/references");
            req.id
        }
        _ => panic!("expected request"),
    };

    client
        .handle_response(Response::success(request_id, Value::Null))
        .await;

    let result = task.await.unwrap().unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_hover_sends_request() {
    let (client, mut rx) = test_client_with_rx();
    let client = Arc::new(client);
    let client_clone = Arc::clone(&client);

    let uri: Uri = "file:///test.rs".parse().unwrap();
    let position = Position::new(2, 10);

    let task = tokio::spawn(async move { client_clone.hover(uri, position).await });

    let msg = rx.recv().await.unwrap();
    let request_id = match msg {
        Message::Request(req) => {
            assert_eq!(req.method, "textDocument/hover");
            req.id
        }
        _ => panic!("expected request"),
    };

    client
        .handle_response(Response::success(request_id, Value::Null))
        .await;

    let result = task.await.unwrap().unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_shutdown_sends_request_and_notification() {
    let (client, mut rx) = test_client_with_rx();
    let client = Arc::new(client);
    let client_clone = Arc::clone(&client);

    let task = tokio::spawn(async move { client_clone.shutdown().await });

    // Read shutdown request
    let msg = rx.recv().await.unwrap();
    let request_id = match msg {
        Message::Request(req) => {
            assert_eq!(req.method, "shutdown");
            req.id
        }
        _ => panic!("expected shutdown request"),
    };

    client
        .handle_response(Response::success(request_id, Value::Null))
        .await;

    // Read exit notification
    let msg = rx.recv().await.unwrap();
    match msg {
        Message::Notification(n) => assert_eq!(n.method, "exit"),
        _ => panic!("expected exit notification"),
    }

    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn test_completion_sends_request() {
    let (client, mut rx) = test_client_with_rx();
    let client = Arc::new(client);
    let client_clone = Arc::clone(&client);

    let uri: Uri = "file:///test.rs".parse().unwrap();
    let position = Position::new(5, 10);

    let task = tokio::spawn(async move { client_clone.completion(uri, position).await });

    let msg = rx.recv().await.unwrap();
    let request_id = match msg {
        Message::Request(req) => {
            assert_eq!(req.method, "textDocument/completion");
            req.id
        }
        _ => panic!("expected request"),
    };

    client
        .handle_response(Response::success(request_id, Value::Null))
        .await;

    let result = task.await.unwrap().unwrap();
    assert!(result.is_none());
}

#[test]
fn test_client_capabilities_has_completion() {
    let caps = Client::client_capabilities();
    let text_doc = caps.text_document.unwrap();
    let completion = text_doc.completion.unwrap();
    assert_eq!(completion.dynamic_registration, Some(true));
    let item = completion.completion_item.unwrap();
    assert_eq!(item.snippet_support, Some(true));
    let formats = item.documentation_format.unwrap();
    assert!(formats.contains(&lsp_types::MarkupKind::Markdown));
    assert!(formats.contains(&lsp_types::MarkupKind::PlainText));
}

#[test]
fn test_client_capabilities_dynamic_registration_hover() {
    let caps = Client::client_capabilities();
    let text_doc = caps.text_document.unwrap();
    let hover = text_doc.hover.unwrap();
    assert_eq!(hover.dynamic_registration, Some(true));
}

#[test]
fn test_client_capabilities_dynamic_registration_definition() {
    let caps = Client::client_capabilities();
    let text_doc = caps.text_document.unwrap();
    let definition = text_doc.definition.unwrap();
    assert_eq!(definition.dynamic_registration, Some(true));
}

#[test]
fn test_client_capabilities_dynamic_registration_references() {
    let caps = Client::client_capabilities();
    let text_doc = caps.text_document.unwrap();
    let references = text_doc.references.unwrap();
    assert_eq!(references.dynamic_registration, Some(true));
}

#[test]
fn test_client_capabilities_synchronization_not_dynamic() {
    let caps = Client::client_capabilities();
    let text_doc = caps.text_document.unwrap();
    let sync = text_doc.synchronization.unwrap();
    assert_eq!(sync.dynamic_registration, Some(false));
}
