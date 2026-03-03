//! LSP saturator - background task following reovim's saturator pattern.
//!
//! The saturator pattern decouples LSP I/O from the main render thread:
//!
//! 1. **Main thread**: Sends non-blocking requests via `send_request()`
//! 2. **Saturator task**: Owns the LSP client, processes requests/responses
//! 3. **`DiagnosticCache`**: Uses `ArcSwap` for lock-free reads
//!
//! This ensures the render thread never blocks on LSP operations.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use {
    lsp_types::{
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        PublishDiagnosticsParams, TextDocumentContentChangeEvent, TextDocumentIdentifier,
        TextDocumentItem, Uri, VersionedTextDocumentIdentifier,
    },
    reovim_driver_lsp::{
        DiagnosticCache, LspError, LspProvider, LspRequest, LspServerConfig,
        client::Client,
        jsonrpc::{Message, Response},
        transport::Transport,
    },
    tokio::sync::mpsc,
    tracing::{debug, error, info, warn},
};

/// Handle for sending requests to the saturator (non-blocking).
///
/// Implements [`LspProvider`] so it can be registered in [`ServiceRegistry`].
#[derive(Clone)]
pub struct LspSaturatorHandle {
    /// Channel for sending requests (buffered for backpressure).
    tx: mpsc::Sender<LspRequest>,
    /// Shared diagnostic cache for lock-free reads.
    cache: Arc<DiagnosticCache>,
    /// Whether the server is running and initialized.
    active: Arc<AtomicBool>,
}

impl std::fmt::Debug for LspSaturatorHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspSaturatorHandle")
            .field("active", &self.active.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl LspProvider for LspSaturatorHandle {
    fn send_request(&self, request: LspRequest) -> bool {
        match self.tx.try_send(request) {
            Ok(()) => {
                debug!("LSP request sent successfully");
                true
            }
            Err(mpsc::error::TrySendError::Full(req)) => {
                info!(?req, "LSP request channel full, dropping request");
                false
            }
            Err(mpsc::error::TrySendError::Closed(req)) => {
                warn!(?req, "LSP request channel closed");
                false
            }
        }
    }

    fn diagnostics(&self) -> &DiagnosticCache {
        &self.cache
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }
}

/// LSP saturator - background task that owns the LSP Client.
///
/// Runs a `tokio::select!` loop between:
/// - Incoming messages from the language server (stdout)
/// - Outgoing requests from the main thread (channel)
pub struct LspSaturator;

impl LspSaturator {
    /// Start the saturator with the given configuration.
    ///
    /// Spawns the language server process, initializes it, and returns
    /// a handle for sending requests.
    ///
    /// # Errors
    ///
    /// Returns an error if the language server cannot be spawned or initialized.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn start(config: LspServerConfig) -> Result<LspSaturatorHandle, LspError> {
        let cache = Arc::new(DiagnosticCache::new());
        let active = Arc::new(AtomicBool::new(false));
        let (request_tx, request_rx) = mpsc::channel::<LspRequest>(32);

        let (client, stdout_reader, stderr) = Client::spawn(config)?;
        let client = Arc::new(client);

        // Spawn saturator main loop
        let cache_clone = Arc::clone(&cache);
        let active_clone = Arc::clone(&active);
        let client_clone = Arc::clone(&client);
        tokio::spawn(Self::run(client_clone, stdout_reader, request_rx, cache_clone, active_clone));

        // Spawn stderr reader if available
        if let Some(stderr) = stderr {
            tokio::spawn(Self::stderr_reader(stderr));
        }

        // Initialize the server
        client.initialize().await?;
        active.store(true, Ordering::Relaxed);

        Ok(LspSaturatorHandle {
            tx: request_tx,
            cache,
            active,
        })
    }

    /// Main saturator loop.
    ///
    /// Uses `tokio::select!` to handle both incoming server messages and
    /// outgoing requests concurrently.
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn run(
        client: Arc<Client>,
        mut stdout_reader: tokio::io::BufReader<tokio::process::ChildStdout>,
        mut request_rx: mpsc::Receiver<LspRequest>,
        cache: Arc<DiagnosticCache>,
        active: Arc<AtomicBool>,
    ) {
        info!("LSP saturator started");

        loop {
            tokio::select! {
                // Handle incoming messages from server
                result = Transport::recv(&mut stdout_reader) => {
                    match result {
                        Ok(message) => {
                            Self::handle_server_message(&client, &cache, message).await;
                        }
                        Err(e) => {
                            error!("Failed to receive message: {e}");
                            break;
                        }
                    }
                }

                // Handle outgoing requests from main thread
                Some(request) = request_rx.recv() => {
                    Self::handle_request(&client, &cache, request).await;
                }

                else => break,
            }
        }

        active.store(false, Ordering::Relaxed);
        info!("LSP saturator stopped");
    }

    /// Handle an incoming message from the language server.
    async fn handle_server_message(
        client: &Arc<Client>,
        cache: &Arc<DiagnosticCache>,
        message: Message,
    ) {
        match message {
            Message::Response(response) => {
                debug!(
                    id = ?response.id,
                    has_error = response.error.is_some(),
                    "Received response from server"
                );
                client.handle_response(response).await;
            }
            Message::Notification(notification) => {
                if notification.method == "textDocument/publishDiagnostics" {
                    if let Some(params) = notification.params
                        && let Ok(diag_params) =
                            serde_json::from_value::<PublishDiagnosticsParams>(params)
                    {
                        Self::handle_diagnostics(cache, diag_params);
                    }
                } else {
                    debug!(method = %notification.method, "Unhandled notification");
                }
            }
            Message::Request(request) => {
                Self::handle_server_request(client, request);
            }
        }
    }

    /// Handle publishDiagnostics notification.
    fn handle_diagnostics(cache: &Arc<DiagnosticCache>, params: PublishDiagnosticsParams) {
        let count = params.diagnostics.len();
        info!(
            uri = ?params.uri,
            count = count,
            version = ?params.version,
            "Received publishDiagnostics"
        );

        // Store in cache (atomic swap)
        cache.store(&params.uri, params.version, params.diagnostics);
    }

    /// Handle server-to-client requests that require a response.
    fn handle_server_request(client: &Arc<Client>, request: reovim_driver_lsp::jsonrpc::Request) {
        debug!(method = %request.method, id = ?request.id, "Server request");

        let response = match request.method.as_str() {
            "client/registerCapability" => {
                debug!("Responding to client/registerCapability");
                Response::success(request.id, serde_json::Value::Null)
            }
            "window/workDoneProgress/create" => {
                debug!("Responding to window/workDoneProgress/create");
                Response::success(request.id, serde_json::Value::Null)
            }
            _ => {
                debug!(method = %request.method, "Unknown server request");
                Response::error(
                    request.id,
                    reovim_driver_lsp::jsonrpc::Error::new(
                        reovim_driver_lsp::jsonrpc::error_codes::METHOD_NOT_FOUND,
                        format!("Method not found: {}", request.method),
                    ),
                )
            }
        };

        client.send_response(response);
    }

    /// Handle an outgoing request from the main thread.
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn handle_request(
        client: &Arc<Client>,
        cache: &Arc<DiagnosticCache>,
        request: LspRequest,
    ) {
        match request {
            LspRequest::DidOpen {
                uri,
                language_id,
                version,
                content,
            } => {
                Self::handle_did_open(client, uri, language_id, version, content);
            }
            LspRequest::DidChange {
                uri,
                version,
                content,
            } => {
                Self::handle_did_change(client, uri, version, content);
            }
            LspRequest::DidClose { uri } => {
                Self::handle_did_close(client, cache, &uri);
            }
            LspRequest::GotoDefinition {
                uri,
                position,
                response_tx,
            } => {
                debug!(uri = %uri.as_str(), position = ?position, "Spawning goto_definition task");
                let client = Arc::clone(client);
                tokio::spawn(async move {
                    let result = client.goto_definition(uri, position).await;
                    debug!(success = result.is_ok(), "goto_definition completed");
                    let _ = response_tx.send(result);
                });
            }
            LspRequest::References {
                uri,
                position,
                include_declaration,
                response_tx,
            } => {
                debug!(uri = %uri.as_str(), position = ?position, "Spawning references task");
                let client = Arc::clone(client);
                tokio::spawn(async move {
                    let result = client.references(uri, position, include_declaration).await;
                    debug!(success = result.is_ok(), "references completed");
                    let _ = response_tx.send(result);
                });
            }
            LspRequest::Hover {
                uri,
                position,
                response_tx,
            } => {
                debug!(uri = %uri.as_str(), position = ?position, "Spawning hover task");
                let client = Arc::clone(client);
                tokio::spawn(async move {
                    let result = client.hover(uri, position).await;
                    debug!(success = result.is_ok(), "hover completed");
                    let _ = response_tx.send(result);
                });
            }
            LspRequest::Shutdown => {
                info!("Shutdown requested");
                if let Err(e) = client.shutdown().await {
                    error!("Shutdown error: {e}");
                }
            }
        }
    }

    /// Handle didOpen notification.
    fn handle_did_open(
        client: &Arc<Client>,
        uri: Uri,
        language_id: String,
        version: i32,
        content: String,
    ) {
        debug!(uri = ?uri, language_id = %language_id, version = version, "didOpen");

        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id,
                version,
                text: content,
            },
        };

        if let Err(e) = client.notify("textDocument/didOpen", params) {
            error!("Failed to send didOpen: {e}");
        }
    }

    /// Handle didChange notification.
    fn handle_did_change(client: &Arc<Client>, uri: Uri, version: i32, content: String) {
        debug!(uri = ?uri, version = version, "didChange");

        // FULL sync mode - send entire content
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: content,
            }],
        };

        if let Err(e) = client.notify("textDocument/didChange", params) {
            error!("Failed to send didChange: {e}");
        }
    }

    /// Handle didClose notification.
    fn handle_did_close(client: &Arc<Client>, cache: &Arc<DiagnosticCache>, uri: &Uri) {
        debug!(uri = ?uri, "didClose");

        let params = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
        };

        if let Err(e) = client.notify("textDocument/didClose", params) {
            error!("Failed to send didClose: {e}");
        }

        // Clear diagnostics for closed document
        cache.remove(uri);
    }

    /// Read and log stderr from the server.
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn stderr_reader(stderr: tokio::process::ChildStderr) {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let mut reader = BufReader::new(stderr);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        debug!(target: "lsp_server", "{}", trimmed);
                    }
                }
                Err(e) => {
                    error!("Failed to read stderr: {e}");
                    break;
                }
            }
        }

        debug!("Stderr reader exited");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use reovim_driver_lsp::{LspServerConfig, client::Client, jsonrpc};

    fn make_uri(path: &str) -> Uri {
        path.parse().expect("test URI should parse")
    }

    /// Create a test client using `cat` to keep stdio pipes alive.
    fn make_test_client() -> Arc<Client> {
        let config = LspServerConfig {
            command: "cat".to_string(),
            args: vec![],
            root_path: PathBuf::from("/tmp"),
            workspace_folders: vec![],
        };
        let (client, _reader, _stderr) = Client::spawn(config).expect("cat should be available");
        Arc::new(client)
    }

    #[test]
    fn test_handle_send_request_success() {
        let (tx, _rx) = mpsc::channel::<LspRequest>(10);
        let handle = LspSaturatorHandle {
            tx,
            cache: Arc::new(DiagnosticCache::new()),
            active: Arc::new(AtomicBool::new(true)),
        };
        assert!(handle.send_request(LspRequest::Shutdown));
    }

    #[test]
    fn test_handle_send_request_channel_full() {
        let (tx, _rx) = mpsc::channel::<LspRequest>(1);
        // Fill the channel
        tx.try_send(LspRequest::Shutdown).unwrap();

        let handle = LspSaturatorHandle {
            tx,
            cache: Arc::new(DiagnosticCache::new()),
            active: Arc::new(AtomicBool::new(true)),
        };
        assert!(!handle.send_request(LspRequest::Shutdown));
    }

    #[test]
    fn test_handle_send_when_channel_closed() {
        let (tx, rx) = mpsc::channel::<LspRequest>(1);
        drop(rx);

        let handle = LspSaturatorHandle {
            tx,
            cache: Arc::new(DiagnosticCache::new()),
            active: Arc::new(AtomicBool::new(false)),
        };
        assert!(!handle.send_request(LspRequest::Shutdown));
    }

    #[test]
    fn test_handle_diagnostics_returns_cache() {
        let handle = LspSaturatorHandle {
            tx: mpsc::channel::<LspRequest>(1).0,
            cache: Arc::new(DiagnosticCache::new()),
            active: Arc::new(AtomicBool::new(false)),
        };
        assert!(handle.diagnostics().is_empty());
    }

    #[test]
    fn test_handle_is_active_initially_false() {
        let handle = LspSaturatorHandle {
            tx: mpsc::channel::<LspRequest>(1).0,
            cache: Arc::new(DiagnosticCache::new()),
            active: Arc::new(AtomicBool::new(false)),
        };
        assert!(!handle.is_active());
    }

    #[test]
    fn test_handle_is_active_when_set() {
        let active = Arc::new(AtomicBool::new(true));
        let handle = LspSaturatorHandle {
            tx: mpsc::channel::<LspRequest>(1).0,
            cache: Arc::new(DiagnosticCache::new()),
            active,
        };
        assert!(handle.is_active());
    }

    #[test]
    fn test_handle_debug() {
        let handle = LspSaturatorHandle {
            tx: mpsc::channel::<LspRequest>(1).0,
            cache: Arc::new(DiagnosticCache::new()),
            active: Arc::new(AtomicBool::new(true)),
        };
        let debug = format!("{handle:?}");
        assert!(debug.contains("LspSaturatorHandle"));
        assert!(debug.contains("active: true"));
    }

    #[test]
    fn test_handle_clone() {
        let handle = LspSaturatorHandle {
            tx: mpsc::channel::<LspRequest>(1).0,
            cache: Arc::new(DiagnosticCache::new()),
            active: Arc::new(AtomicBool::new(true)),
        };
        let cloned = handle.clone();
        assert_eq!(cloned.is_active(), handle.is_active());
    }

    #[test]
    fn test_handle_diagnostics() {
        let uri = make_uri("file:///test.rs");
        let cache = Arc::new(DiagnosticCache::new());
        cache.store(&uri, Some(1), vec![]);

        let handle = LspSaturatorHandle {
            tx: mpsc::channel::<LspRequest>(1).0,
            cache,
            active: Arc::new(AtomicBool::new(true)),
        };

        assert!(handle.diagnostics().has(&uri));
    }

    #[test]
    fn test_handle_diagnostics_stores_push_diagnostics() {
        let uri = make_uri("file:///src/main.rs");
        let cache = Arc::new(DiagnosticCache::new());

        let params = PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics: vec![],
            version: Some(1),
        };

        LspSaturator::handle_diagnostics(&cache, params);
        assert!(cache.has(&uri));
    }

    // --- Tests calling actual functions (coverage) ---

    #[tokio::test]
    async fn test_handle_server_message_with_response() {
        // Enable debug-level tracing so debug!() field expressions are evaluated
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();

        let client = make_test_client();
        let cache = Arc::new(DiagnosticCache::new());

        let response = jsonrpc::Response::success(jsonrpc::Id::Number(99), serde_json::Value::Null);
        let message = jsonrpc::Message::Response(response);

        // No pending request for ID 99 — logs warning, no panic
        LspSaturator::handle_server_message(&client, &cache, message).await;
    }

    #[tokio::test]
    async fn test_handle_server_message_with_diagnostics_notification() {
        let client = make_test_client();
        let cache = Arc::new(DiagnosticCache::new());

        let uri: Uri = "file:///test.rs".parse().unwrap();
        let params = PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics: vec![],
            version: Some(1),
        };

        let notification = jsonrpc::Notification::new(
            "textDocument/publishDiagnostics",
            Some(serde_json::to_value(params).unwrap()),
        );
        let message = jsonrpc::Message::Notification(notification);

        LspSaturator::handle_server_message(&client, &cache, message).await;
        assert!(cache.has(&uri));
    }

    #[tokio::test]
    async fn test_handle_server_message_diagnostics_no_params() {
        let client = make_test_client();
        let cache = Arc::new(DiagnosticCache::new());

        let notification = jsonrpc::Notification::new("textDocument/publishDiagnostics", None);
        let message = jsonrpc::Message::Notification(notification);

        LspSaturator::handle_server_message(&client, &cache, message).await;
        assert!(cache.is_empty());
    }

    #[tokio::test]
    async fn test_handle_server_message_diagnostics_invalid_params() {
        let client = make_test_client();
        let cache = Arc::new(DiagnosticCache::new());

        let notification = jsonrpc::Notification::new(
            "textDocument/publishDiagnostics",
            Some(serde_json::json!({"invalid": "data"})),
        );
        let message = jsonrpc::Message::Notification(notification);

        LspSaturator::handle_server_message(&client, &cache, message).await;
        assert!(cache.is_empty());
    }

    #[tokio::test]
    async fn test_handle_server_message_unhandled_notification() {
        let client = make_test_client();
        let cache = Arc::new(DiagnosticCache::new());

        let notification = jsonrpc::Notification::new(
            "window/logMessage",
            Some(serde_json::json!({"type": 3, "message": "test"})),
        );
        let message = jsonrpc::Message::Notification(notification);

        LspSaturator::handle_server_message(&client, &cache, message).await;
    }

    #[tokio::test]
    async fn test_handle_server_message_with_request() {
        let client = make_test_client();
        let cache = Arc::new(DiagnosticCache::new());

        let request = jsonrpc::Request::new(1_i64, "client/registerCapability", None);
        let message = jsonrpc::Message::Request(request);

        LspSaturator::handle_server_message(&client, &cache, message).await;
    }

    #[tokio::test]
    async fn test_handle_server_request_register_capability_direct() {
        let client = make_test_client();
        let request = jsonrpc::Request::new(1_i64, "client/registerCapability", None);
        LspSaturator::handle_server_request(&client, request);
    }

    #[tokio::test]
    async fn test_handle_server_request_work_done_progress() {
        let client = make_test_client();
        let request = jsonrpc::Request::new(2_i64, "window/workDoneProgress/create", None);
        LspSaturator::handle_server_request(&client, request);
    }

    #[tokio::test]
    async fn test_handle_server_request_unknown_direct() {
        let client = make_test_client();
        let request = jsonrpc::Request::new(3_i64, "custom/unknown", None);
        LspSaturator::handle_server_request(&client, request);
    }

    #[tokio::test]
    async fn test_handle_did_open_sends_notification() {
        let client = make_test_client();
        let uri: Uri = "file:///test.rs".parse().unwrap();
        LspSaturator::handle_did_open(
            &client,
            uri,
            "rust".to_string(),
            1,
            "fn main() {}".to_string(),
        );
    }

    #[tokio::test]
    async fn test_handle_did_change_sends_notification() {
        let client = make_test_client();
        let uri: Uri = "file:///test.rs".parse().unwrap();
        LspSaturator::handle_did_change(&client, uri, 2, "fn main() { println!(); }".to_string());
    }

    /// Create a client whose writer channel is already dead.
    ///
    /// Spawns `true` (exits immediately), sends a dummy message to trigger
    /// the broken pipe, then waits for the writer task to exit.
    async fn make_dead_client() -> Arc<Client> {
        let config = LspServerConfig {
            command: "true".to_string(),
            args: vec![],
            root_path: PathBuf::from("/tmp"),
            workspace_folders: vec![],
        };
        let (client, _reader, _stderr) = Client::spawn(config).expect("true should be available");
        let client = Arc::new(client);

        // Force the writer task to detect the broken pipe
        for _ in 0..20 {
            if client.notify("dummy", serde_json::Value::Null).is_err() {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        client
    }

    #[tokio::test]
    async fn test_handle_did_open_with_closed_channel() {
        let client = make_dead_client().await;
        let uri: Uri = "file:///test.rs".parse().unwrap();
        // Should log error but not panic
        LspSaturator::handle_did_open(
            &client,
            uri,
            "rust".to_string(),
            1,
            "fn main() {}".to_string(),
        );
    }

    #[tokio::test]
    async fn test_handle_did_change_with_closed_channel() {
        let client = make_dead_client().await;
        let uri: Uri = "file:///test.rs".parse().unwrap();
        LspSaturator::handle_did_change(&client, uri, 2, "fn main() { println!(); }".to_string());
    }

    #[tokio::test]
    async fn test_handle_did_close_with_closed_channel() {
        let client = make_dead_client().await;
        let cache = Arc::new(DiagnosticCache::new());
        let uri: Uri = "file:///test.rs".parse().unwrap();

        cache.store(&uri, Some(1), vec![]);
        LspSaturator::handle_did_close(&client, &cache, &uri);
        // Cache should still be cleared even if notify fails
        assert!(!cache.has(&uri));
    }

    #[tokio::test]
    async fn test_handle_did_close_clears_cache() {
        let client = make_test_client();
        let cache = Arc::new(DiagnosticCache::new());
        let uri: Uri = "file:///test.rs".parse().unwrap();

        // Pre-populate cache
        cache.store(&uri, Some(1), vec![]);
        assert!(cache.has(&uri));

        LspSaturator::handle_did_close(&client, &cache, &uri);
        assert!(!cache.has(&uri));
    }
}
