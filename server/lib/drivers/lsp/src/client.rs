//! LSP client implementation.
//!
//! Handles spawning the language server process and managing the
//! request/response lifecycle over JSON-RPC.

use std::{
    collections::HashMap,
    path::Path,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use {
    lsp_types::{
        ClientCapabilities, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams,
        InitializeParams, InitializeResult, InitializedParams, Location, PartialResultParams,
        Position, ReferenceContext, ReferenceParams, ServerCapabilities,
        TextDocumentClientCapabilities, TextDocumentIdentifier, TextDocumentPositionParams,
        TextDocumentSyncClientCapabilities, Uri, WorkDoneProgressParams,
    },
    serde_json::Value,
    tokio::{
        io::{BufReader, BufWriter},
        process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
        sync::{Mutex, oneshot},
    },
    tracing::{debug, error, info, warn},
};

use crate::{
    LspError, LspServerConfig,
    jsonrpc::{Id, Message, Notification, Request, Response},
    transport::Transport,
};

/// A pending request waiting for a response.
type PendingRequest = oneshot::Sender<Result<Value, LspError>>;

/// Server capabilities and initialization state.
#[derive(Debug, Clone)]
pub struct ServerState {
    /// Server capabilities received from initialize response.
    pub capabilities: ServerCapabilities,
    /// Whether the server has been initialized.
    pub initialized: bool,
}

/// LSP client.
///
/// Manages communication with a language server process via JSON-RPC
/// over stdio. The client handles:
/// - Process spawning with stdio pipes
/// - Request/response correlation via ID matching
/// - Notification sending (fire-and-forget)
/// - Initialize/shutdown lifecycle
pub struct Client {
    /// Server configuration.
    config: LspServerConfig,

    /// Next request ID (atomic counter).
    next_id: AtomicU64,

    /// Pending requests waiting for responses, keyed by request ID.
    pending: Arc<Mutex<HashMap<Id, PendingRequest>>>,

    /// Channel for sending messages to the writer task.
    writer_tx: tokio::sync::mpsc::UnboundedSender<Message>,

    /// Server state (capabilities, initialized flag).
    state: Arc<Mutex<Option<ServerState>>>,

    /// Server process handle (kept alive for `kill_on_drop`).
    _process: Child,
}

impl Client {
    /// Spawn a new language server and create a client.
    ///
    /// Returns the client, a buffered reader for stdout (for the caller to
    /// drive the message receive loop), and optionally stderr.
    ///
    /// # Errors
    ///
    /// Returns an error if the server process cannot be spawned.
    ///
    /// # Panics
    ///
    /// Panics if stdin/stdout are not captured from the process.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn spawn(
        config: LspServerConfig,
    ) -> Result<(Self, BufReader<ChildStdout>, Option<ChildStderr>), LspError> {
        info!(
            command = %config.command,
            args = ?config.args,
            root = %config.root_path.display(),
            "Spawning language server"
        );

        let mut process = Command::new(&config.command)
            .args(&config.args)
            .current_dir(&config.root_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| LspError::SpawnFailed(e.to_string()))?;

        let stdin = process.stdin.take().expect("stdin not captured");
        let stdout = process.stdout.take().expect("stdout not captured");
        let stderr = process.stderr.take();

        let stdout_reader = BufReader::new(stdout);
        let stdin_writer = BufWriter::new(stdin);

        // Create channel for outgoing messages
        let (writer_tx, writer_rx) = tokio::sync::mpsc::unbounded_channel();

        // Spawn writer task
        tokio::spawn(Self::writer_task(stdin_writer, writer_rx));

        let client = Self {
            config,
            next_id: AtomicU64::new(1),
            pending: Arc::new(Mutex::new(HashMap::new())),
            writer_tx,
            state: Arc::new(Mutex::new(None)),
            _process: process,
        };

        Ok((client, stdout_reader, stderr))
    }

    /// Writer task that sends messages to the server's stdin.
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn writer_task(
        mut writer: BufWriter<ChildStdin>,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<Message>,
    ) {
        while let Some(message) = rx.recv().await {
            if let Err(e) = Transport::send(&mut writer, &message).await {
                error!("Failed to send message: {e}");
                break;
            }
        }
        debug!("Writer task exited");
    }

    /// Generate the next request ID.
    fn next_request_id(&self) -> Id {
        #[allow(clippy::cast_possible_wrap)]
        Id::Number(self.next_id.fetch_add(1, Ordering::Relaxed) as i64)
    }

    /// Send a request and wait for a response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The request cannot be serialized
    /// - The writer channel is closed
    /// - The response contains an error
    /// - The response cannot be deserialized
    pub async fn request<P: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R, LspError> {
        let id = self.next_request_id();
        let params_value = serde_json::to_value(params)?;

        let request = Request::new(id.clone(), method, Some(params_value));

        // Create oneshot channel for response
        let (tx, rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id.clone(), tx);
        }

        // Send request
        debug!(id = ?id, method = %method, "Sending request");
        if self.writer_tx.send(Message::Request(request)).is_err() {
            return Err(LspError::ChannelClosed);
        }

        // Wait for response
        let result = rx.await.map_err(|_| LspError::ChannelClosed)??;

        // Deserialize result
        serde_json::from_value(result).map_err(LspError::from)
    }

    /// Send a notification (no response expected).
    ///
    /// # Errors
    ///
    /// Returns an error if the notification cannot be serialized or sent.
    pub fn notify<P: serde::Serialize>(&self, method: &str, params: P) -> Result<(), LspError> {
        let params_value = serde_json::to_value(params)?;
        let notification = Notification::new(method, Some(params_value));

        debug!(method = %method, "Sending notification");
        self.writer_tx
            .send(Message::Notification(notification))
            .map_err(|_| LspError::ChannelClosed)
    }

    /// Send a response to a server-to-client request.
    ///
    /// Used for requests like `client/registerCapability` or
    /// `window/workDoneProgress/create` that require acknowledgment.
    pub fn send_response(&self, response: Response) {
        debug!(id = ?response.id, "Sending response");
        if self.writer_tx.send(Message::Response(response)).is_err() {
            warn!("Failed to send response - channel closed");
        }
    }

    /// Handle a response from the server.
    ///
    /// Matches the response to a pending request by ID and sends the result
    /// through the corresponding oneshot channel.
    pub async fn handle_response(&self, response: Response) {
        let mut pending = self.pending.lock().await;

        if let Some(tx) = pending.remove(&response.id) {
            let result = if let Some(error) = response.error {
                Err(LspError::ServerError {
                    code: error.code,
                    message: error.message,
                })
            } else {
                Ok(response.result.unwrap_or(Value::Null))
            };

            if tx.send(result).is_err() {
                warn!(
                    id = ?response.id,
                    "Response receiver dropped - request handler may have timed out"
                );
            } else {
                debug!(id = ?response.id, "Response delivered");
            }
        } else {
            warn!(id = ?response.id, "No pending request for response");
        }
    }

    /// Initialize the language server.
    ///
    /// Sends the `initialize` request followed by the `initialized` notification.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    ///
    /// # Panics
    ///
    /// Panics if internal server state is unexpectedly missing after being set.
    #[allow(deprecated)] // root_path and root_uri are deprecated in LSP but still widely used
    pub async fn initialize(&self) -> Result<InitializeResult, LspError> {
        let root_uri = crate::uri_from_path(&self.config.root_path);

        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_path: Some(self.config.root_path.to_string_lossy().to_string()),
            root_uri: Some(root_uri),
            capabilities: Self::client_capabilities(),
            workspace_folders: Some(self.config.workspace_folders.clone()),
            ..Default::default()
        };

        info!("Sending initialize request");
        let result: InitializeResult = self.request("initialize", params).await?;
        info!(capabilities = ?result.capabilities, "Server initialized");

        // Store server state
        {
            let mut state = self.state.lock().await;
            *state = Some(ServerState {
                capabilities: result.capabilities.clone(),
                initialized: false,
            });
        }

        // Send initialized notification
        self.notify("initialized", InitializedParams {})?;

        // Mark as initialized
        {
            let mut state = self.state.lock().await;
            state.as_mut().expect("state was set above").initialized = true;
        }

        info!("Language server ready");
        Ok(result)
    }

    /// Get server capabilities.
    #[must_use]
    pub async fn capabilities(&self) -> Option<ServerCapabilities> {
        let state = self.state.lock().await;
        state.as_ref().map(|s| s.capabilities.clone())
    }

    /// Check if the server is initialized.
    #[must_use]
    pub async fn is_initialized(&self) -> bool {
        let state = self.state.lock().await;
        state.as_ref().is_some_and(|s| s.initialized)
    }

    /// Get the root path.
    #[must_use]
    pub fn root_path(&self) -> &Path {
        &self.config.root_path
    }

    /// Build client capabilities advertised to the server.
    fn client_capabilities() -> ClientCapabilities {
        ClientCapabilities {
            text_document: Some(TextDocumentClientCapabilities {
                synchronization: Some(TextDocumentSyncClientCapabilities {
                    dynamic_registration: Some(false),
                    will_save: Some(false),
                    will_save_wait_until: Some(false),
                    did_save: Some(true),
                }),
                // Enable push diagnostics (publishDiagnostics)
                publish_diagnostics: Some(
                    lsp_types::PublishDiagnosticsClientCapabilities::default(),
                ),
                // Enable hover support
                hover: Some(lsp_types::HoverClientCapabilities {
                    dynamic_registration: Some(false),
                    content_format: Some(vec![
                        lsp_types::MarkupKind::Markdown,
                        lsp_types::MarkupKind::PlainText,
                    ]),
                }),
                // Enable references support
                references: Some(lsp_types::ReferenceClientCapabilities {
                    dynamic_registration: Some(false),
                }),
                // Enable definition support
                definition: Some(lsp_types::GotoCapability {
                    dynamic_registration: Some(false),
                    link_support: Some(true),
                }),
                ..Default::default()
            }),
            // Enable work done progress support
            window: Some(lsp_types::WindowClientCapabilities {
                work_done_progress: Some(true),
                show_message: Some(lsp_types::ShowMessageRequestClientCapabilities {
                    message_action_item: None,
                }),
                show_document: Some(lsp_types::ShowDocumentClientCapabilities { support: true }),
            }),
            ..Default::default()
        }
    }

    /// Go to definition of the symbol at the given position.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn goto_definition(
        &self,
        uri: Uri,
        position: Position,
    ) -> Result<Option<GotoDefinitionResponse>, LspError> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.request("textDocument/definition", params).await
    }

    /// Find all references to the symbol at the given position.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn references(
        &self,
        uri: Uri,
        position: Position,
        include_declaration: bool,
    ) -> Result<Option<Vec<Location>>, LspError> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration,
            },
        };

        self.request("textDocument/references", params).await
    }

    /// Get hover information for the symbol at the given position.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn hover(&self, uri: Uri, position: Position) -> Result<Option<Hover>, LspError> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        self.request("textDocument/hover", params).await
    }

    /// Shutdown the language server gracefully.
    ///
    /// Sends shutdown request followed by exit notification.
    ///
    /// # Errors
    ///
    /// Returns an error if shutdown fails.
    pub async fn shutdown(&self) -> Result<(), LspError> {
        info!("Shutting down language server");

        // Send shutdown request
        let _: Value = self.request("shutdown", Value::Null).await?;

        // Send exit notification
        self.notify("exit", Value::Null)?;

        info!("Language server shutdown complete");
        Ok(())
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("command", &self.config.command)
            .field("root_path", &self.config.root_path)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
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
            .handle_response(Response::success(
                request_id,
                serde_json::to_value(init_result).unwrap(),
            ))
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
}
