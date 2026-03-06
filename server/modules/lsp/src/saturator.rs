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
        PublishDiagnosticsParams, RegistrationParams, TextDocumentContentChangeEvent,
        TextDocumentIdentifier, TextDocumentItem, UnregistrationParams, Uri,
        VersionedTextDocumentIdentifier,
    },
    reovim_driver_lsp::{
        CapabilityStore, DiagnosticCache, LspError, LspLogger, LspProvider, LspRequest,
        LspServerConfig,
        client::Client,
        jsonrpc::{Message, Response},
        transport::Transport,
    },
    tokio::sync::mpsc,
    tracing::{debug, error, info, warn},
};

/// Handle for sending requests to the saturator (non-blocking).
///
/// Implements [`LspProvider`] so it can be registered in a service registry.
/// Carries generic server state (capabilities, root, language) so that any
/// consumer can inspect the server without going through the saturator (#521, #530).
#[derive(Clone)]
pub struct LspSaturatorHandle {
    /// Channel for sending requests (buffered for backpressure).
    tx: mpsc::Sender<LspRequest>,
    /// Shared diagnostic cache for lock-free reads.
    cache: Arc<DiagnosticCache>,
    /// Whether the server is running and initialized.
    active: Arc<AtomicBool>,
    /// Server capabilities, updated dynamically via `client/registerCapability` (#533).
    capabilities: Arc<CapabilityStore>,
    /// Project root path this server covers.
    root_path: std::path::PathBuf,
    /// Language ID this server handles (e.g., "rust").
    language_id: String,
    /// Server info (name, version) from the initialize response.
    server_info: Option<lsp_types::ServerInfo>,
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

    fn capabilities(&self) -> Option<Arc<lsp_types::ServerCapabilities>> {
        if self.is_active() {
            Some(self.capabilities.load_full())
        } else {
            None
        }
    }

    fn root_path(&self) -> &std::path::Path {
        &self.root_path
    }

    fn language_id(&self) -> &str {
        &self.language_id
    }

    fn server_info(&self) -> Option<&lsp_types::ServerInfo> {
        self.server_info.as_ref()
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
    /// a handle for sending requests. The handle carries server capabilities
    /// and metadata from the initialize response (#521, #530).
    ///
    /// # Errors
    ///
    /// Returns an error if the language server cannot be spawned or initialized.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn start(
        config: LspServerConfig,
        language_id: String,
    ) -> Result<LspSaturatorHandle, LspError> {
        let cache = Arc::new(DiagnosticCache::new());
        let active = Arc::new(AtomicBool::new(false));
        let (request_tx, request_rx) = mpsc::channel::<LspRequest>(32);

        let root_path = config.root_path.clone();
        let (client, stdout_reader, stderr) = Client::spawn(config)?;
        let client = Arc::new(client);

        // Create capability store with empty caps; updated after initialize().
        let capability_store =
            Arc::new(CapabilityStore::new(lsp_types::ServerCapabilities::default()));

        // Create dedicated LSP logger if REOVIM_LSP_LOG is set.
        let logger = LspLogger::from_env(&language_id).map(Arc::new);

        if let Some(ref log) = logger {
            log.log_event(&format!("starting LSP server for {language_id}"));
        }

        // Spawn saturator main loop (must be before initialize() to read response).
        let cache_clone = Arc::clone(&cache);
        let active_clone = Arc::clone(&active);
        let client_clone = Arc::clone(&client);
        let caps_clone = Arc::clone(&capability_store);
        let logger_clone = logger.clone();
        tokio::spawn(Self::run(
            client_clone,
            stdout_reader,
            request_rx,
            cache_clone,
            active_clone,
            caps_clone,
            logger_clone,
        ));

        // Spawn stderr reader if available
        if let Some(stderr) = stderr {
            tokio::spawn(Self::stderr_reader(stderr, logger.clone()));
        }

        // Initialize the server — extract capabilities and server info (#521).
        let init_result = client.initialize().await?;
        capability_store.store(init_result.capabilities);
        active.store(true, Ordering::Relaxed);

        if let Some(ref log) = logger {
            let server_name = init_result
                .server_info
                .as_ref()
                .map_or("unknown", |s| &s.name);
            log.log_event(&format!("initialized ({server_name})"));
        }

        Ok(LspSaturatorHandle {
            tx: request_tx,
            cache,
            active,
            capabilities: capability_store,
            root_path,
            language_id,
            server_info: init_result.server_info,
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
        capabilities: Arc<CapabilityStore>,
        logger: Option<Arc<LspLogger>>,
    ) {
        info!("LSP saturator started");

        loop {
            tokio::select! {
                // Handle incoming messages from server
                result = Transport::recv(&mut stdout_reader) => {
                    match result {
                        Ok(message) => {
                            Self::log_incoming(logger.as_ref(), &message);
                            Self::handle_server_message(&client, &cache, &capabilities, message).await;
                        }
                        Err(e) => {
                            error!("Failed to receive message: {e}");
                            if let Some(ref log) = logger {
                                log.log_event(&format!("transport error: {e}"));
                            }
                            break;
                        }
                    }
                }

                // Handle outgoing requests from main thread
                Some(request) = request_rx.recv() => {
                    Self::log_outgoing(logger.as_ref(), &request);
                    Self::handle_request(&client, &cache, request).await;
                }

                else => break,
            }
        }

        active.store(false, Ordering::Relaxed);
        if let Some(ref log) = logger {
            log.log_event("saturator stopped");
        }
        info!("LSP saturator stopped");
    }

    /// Handle an incoming message from the language server.
    async fn handle_server_message(
        client: &Arc<Client>,
        cache: &Arc<DiagnosticCache>,
        capabilities: &CapabilityStore,
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
                Self::handle_server_request(client, capabilities, request);
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
    fn handle_server_request(
        client: &Arc<Client>,
        capabilities: &CapabilityStore,
        request: reovim_driver_lsp::jsonrpc::Request,
    ) {
        debug!(method = %request.method, id = ?request.id, "Server request");

        let response = match request.method.as_str() {
            "client/registerCapability" => {
                if let Some(params) = request.params {
                    match serde_json::from_value::<RegistrationParams>(params) {
                        Ok(reg_params) => {
                            capabilities.apply_registrations(&reg_params.registrations);
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to parse registerCapability params");
                        }
                    }
                }
                Response::success(request.id, serde_json::Value::Null)
            }
            "client/unregisterCapability" => {
                if let Some(params) = request.params {
                    match serde_json::from_value::<UnregistrationParams>(params) {
                        Ok(unreg_params) => {
                            capabilities.apply_unregistrations(&unreg_params.unregisterations);
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to parse unregisterCapability params");
                        }
                    }
                }
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
            LspRequest::Completion {
                uri,
                position,
                response_tx,
            } => {
                debug!(uri = %uri.as_str(), position = ?position, "Spawning completion task");
                let client = Arc::clone(client);
                tokio::spawn(async move {
                    let result = client.completion(uri, position).await;
                    debug!(success = result.is_ok(), "completion completed");
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

    /// Log an incoming message from the server.
    fn log_incoming(logger: Option<&Arc<LspLogger>>, message: &Message) {
        let Some(log) = logger else { return };
        match message {
            Message::Response(resp) => {
                let id = format!("{:?}", resp.id);
                let status = if resp.error.is_some() { "error" } else { "ok" };
                log.log_response(&id, "response", status);
            }
            Message::Notification(notif) => {
                log.log_server_notification(&notif.method, "");
            }
            Message::Request(req) => {
                log.log_server_request(&req.method, &format!("id={:?}", req.id));
            }
        }
    }

    /// Log an outgoing request to the server.
    fn log_outgoing(logger: Option<&Arc<LspLogger>>, request: &LspRequest) {
        let Some(log) = logger else { return };
        match request {
            LspRequest::DidOpen {
                uri, language_id, ..
            } => {
                log.log_sent(
                    "textDocument/didOpen",
                    &format!("{} lang={language_id}", uri.as_str()),
                );
            }
            LspRequest::DidChange { uri, version, .. } => {
                log.log_sent("textDocument/didChange", &format!("{} v={version}", uri.as_str()));
            }
            LspRequest::DidClose { uri } => {
                log.log_sent("textDocument/didClose", uri.as_str());
            }
            LspRequest::GotoDefinition { uri, position, .. } => {
                log.log_sent(
                    "textDocument/definition",
                    &format!("{} {}:{}", uri.as_str(), position.line, position.character),
                );
            }
            LspRequest::References {
                uri,
                position,
                include_declaration,
                ..
            } => {
                log.log_sent(
                    "textDocument/references",
                    &format!(
                        "{} {}:{} decl={include_declaration}",
                        uri.as_str(),
                        position.line,
                        position.character
                    ),
                );
            }
            LspRequest::Hover { uri, position, .. } => {
                log.log_sent(
                    "textDocument/hover",
                    &format!("{} {}:{}", uri.as_str(), position.line, position.character),
                );
            }
            LspRequest::Completion { uri, position, .. } => {
                log.log_sent(
                    "textDocument/completion",
                    &format!("{} {}:{}", uri.as_str(), position.line, position.character),
                );
            }
            LspRequest::Shutdown => {
                log.log_sent("shutdown", "");
            }
        }
    }

    /// Read and log stderr from the server.
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn stderr_reader(stderr: tokio::process::ChildStderr, logger: Option<Arc<LspLogger>>) {
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
                        if let Some(ref log) = logger {
                            log.log_stderr(trimmed);
                        }
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

    /// Create a test handle with default server state.
    fn make_test_handle(tx: mpsc::Sender<LspRequest>, active: bool) -> LspSaturatorHandle {
        LspSaturatorHandle {
            tx,
            cache: Arc::new(DiagnosticCache::new()),
            active: Arc::new(AtomicBool::new(active)),
            capabilities: Arc::new(CapabilityStore::new(lsp_types::ServerCapabilities::default())),
            root_path: PathBuf::from("/tmp/test"),
            language_id: "rust".to_string(),
            server_info: None,
        }
    }

    /// Create a test `CapabilityStore` for request handler tests.
    fn make_test_capability_store() -> CapabilityStore {
        CapabilityStore::new(lsp_types::ServerCapabilities::default())
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
        let handle = make_test_handle(tx, true);
        assert!(handle.send_request(LspRequest::Shutdown));
    }

    #[test]
    fn test_handle_send_request_channel_full() {
        let (tx, _rx) = mpsc::channel::<LspRequest>(1);
        // Fill the channel
        tx.try_send(LspRequest::Shutdown).unwrap();
        let handle = make_test_handle(tx, true);
        assert!(!handle.send_request(LspRequest::Shutdown));
    }

    #[test]
    fn test_handle_send_when_channel_closed() {
        let (tx, rx) = mpsc::channel::<LspRequest>(1);
        drop(rx);
        let handle = make_test_handle(tx, false);
        assert!(!handle.send_request(LspRequest::Shutdown));
    }

    #[test]
    fn test_handle_diagnostics_returns_cache() {
        let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, false);
        assert!(handle.diagnostics().is_empty());
    }

    #[test]
    fn test_handle_is_active_initially_false() {
        let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, false);
        assert!(!handle.is_active());
    }

    #[test]
    fn test_handle_is_active_when_set() {
        let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
        assert!(handle.is_active());
    }

    #[test]
    fn test_handle_debug() {
        let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
        let debug = format!("{handle:?}");
        assert!(debug.contains("LspSaturatorHandle"));
        assert!(debug.contains("active: true"));
    }

    #[test]
    fn test_handle_clone() {
        let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
        let cloned = handle.clone();
        assert_eq!(cloned.is_active(), handle.is_active());
    }

    #[test]
    fn test_handle_diagnostics() {
        let uri = make_uri("file:///test.rs");
        let mut handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
        handle.cache = Arc::new(DiagnosticCache::new());
        handle.cache.store(&uri, Some(1), vec![]);
        assert!(handle.diagnostics().has(&uri));
    }

    #[test]
    fn test_handle_capabilities_when_active() {
        let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
        assert!(handle.capabilities().is_some());
    }

    #[test]
    fn test_handle_capabilities_when_inactive() {
        let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, false);
        assert!(handle.capabilities().is_none());
    }

    #[test]
    fn test_handle_root_path() {
        let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
        assert_eq!(handle.root_path(), std::path::Path::new("/tmp/test"));
    }

    #[test]
    fn test_handle_language_id() {
        let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
        assert_eq!(handle.language_id(), "rust");
    }

    #[test]
    fn test_handle_server_info_absent() {
        let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
        assert!(handle.server_info().is_none());
    }

    #[test]
    fn test_handle_server_info_present() {
        let mut handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
        handle.server_info = Some(lsp_types::ServerInfo {
            name: "test-server".to_string(),
            version: Some("1.0.0".to_string()),
        });
        let info = handle.server_info().unwrap();
        assert_eq!(info.name, "test-server");
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
        LspSaturator::handle_server_message(
            &client,
            &cache,
            &make_test_capability_store(),
            message,
        )
        .await;
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

        LspSaturator::handle_server_message(
            &client,
            &cache,
            &make_test_capability_store(),
            message,
        )
        .await;
        assert!(cache.has(&uri));
    }

    #[tokio::test]
    async fn test_handle_server_message_diagnostics_no_params() {
        let client = make_test_client();
        let cache = Arc::new(DiagnosticCache::new());

        let notification = jsonrpc::Notification::new("textDocument/publishDiagnostics", None);
        let message = jsonrpc::Message::Notification(notification);

        LspSaturator::handle_server_message(
            &client,
            &cache,
            &make_test_capability_store(),
            message,
        )
        .await;
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

        LspSaturator::handle_server_message(
            &client,
            &cache,
            &make_test_capability_store(),
            message,
        )
        .await;
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

        LspSaturator::handle_server_message(
            &client,
            &cache,
            &make_test_capability_store(),
            message,
        )
        .await;
    }

    #[tokio::test]
    async fn test_handle_server_message_with_request() {
        let client = make_test_client();
        let cache = Arc::new(DiagnosticCache::new());

        let request = jsonrpc::Request::new(1_i64, "client/registerCapability", None);
        let message = jsonrpc::Message::Request(request);

        LspSaturator::handle_server_message(
            &client,
            &cache,
            &make_test_capability_store(),
            message,
        )
        .await;
    }

    #[tokio::test]
    async fn test_handle_server_request_register_capability_direct() {
        let client = make_test_client();
        let request = jsonrpc::Request::new(1_i64, "client/registerCapability", None);
        LspSaturator::handle_server_request(&client, &make_test_capability_store(), request);
    }

    #[tokio::test]
    async fn test_handle_server_request_work_done_progress() {
        let client = make_test_client();
        let request = jsonrpc::Request::new(2_i64, "window/workDoneProgress/create", None);
        LspSaturator::handle_server_request(&client, &make_test_capability_store(), request);
    }

    #[tokio::test]
    async fn test_handle_server_request_unknown_direct() {
        let client = make_test_client();
        let request = jsonrpc::Request::new(3_i64, "custom/unknown", None);
        LspSaturator::handle_server_request(&client, &make_test_capability_store(), request);
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

    // --- Dynamic registration tests (#533) ---

    #[tokio::test]
    async fn test_handle_server_request_register_capability_with_params() {
        let client = make_test_client();
        let caps = make_test_capability_store();
        assert!(caps.load_full().hover_provider.is_none());

        let params = serde_json::json!({
            "registrations": [{
                "id": "r1",
                "method": "textDocument/hover",
                "registerOptions": null
            }]
        });
        let request = jsonrpc::Request::new(1_i64, "client/registerCapability", Some(params));
        LspSaturator::handle_server_request(&client, &caps, request);

        assert!(caps.load_full().hover_provider.is_some());
    }

    #[tokio::test]
    async fn test_handle_server_request_register_capability_no_params() {
        let client = make_test_client();
        let caps = make_test_capability_store();
        let request = jsonrpc::Request::new(1_i64, "client/registerCapability", None);
        // Should not panic, just ACK with null
        LspSaturator::handle_server_request(&client, &caps, request);
    }

    #[tokio::test]
    async fn test_handle_server_request_register_capability_invalid_params() {
        let client = make_test_client();
        let caps = make_test_capability_store();
        let params = serde_json::json!({"invalid": "data"});
        let request = jsonrpc::Request::new(1_i64, "client/registerCapability", Some(params));
        // Should log warning but not panic
        LspSaturator::handle_server_request(&client, &caps, request);
    }

    #[tokio::test]
    async fn test_handle_server_request_unregister_capability_with_params() {
        let client = make_test_client();
        let caps = make_test_capability_store();

        // Register first
        let reg_params = serde_json::json!({
            "registrations": [{
                "id": "r1",
                "method": "textDocument/definition",
                "registerOptions": null
            }]
        });
        let reg_request =
            jsonrpc::Request::new(1_i64, "client/registerCapability", Some(reg_params));
        LspSaturator::handle_server_request(&client, &caps, reg_request);
        assert!(caps.load_full().definition_provider.is_some());

        // Unregister
        let unreg_params = serde_json::json!({
            "unregisterations": [{
                "id": "r1",
                "method": "textDocument/definition"
            }]
        });
        let unreg_request =
            jsonrpc::Request::new(2_i64, "client/unregisterCapability", Some(unreg_params));
        LspSaturator::handle_server_request(&client, &caps, unreg_request);
        assert!(caps.load_full().definition_provider.is_none());
    }

    #[tokio::test]
    async fn test_handle_server_request_unregister_capability_no_params() {
        let client = make_test_client();
        let caps = make_test_capability_store();
        let request = jsonrpc::Request::new(1_i64, "client/unregisterCapability", None);
        LspSaturator::handle_server_request(&client, &caps, request);
    }

    #[tokio::test]
    async fn test_handle_server_request_unregister_capability_invalid_params() {
        let client = make_test_client();
        let caps = make_test_capability_store();
        let params = serde_json::json!({"bad": "format"});
        let request = jsonrpc::Request::new(1_i64, "client/unregisterCapability", Some(params));
        LspSaturator::handle_server_request(&client, &caps, request);
    }

    #[test]
    fn test_handle_capabilities_reflects_registration() {
        let (tx, _rx) = mpsc::channel::<LspRequest>(1);
        let handle = make_test_handle(tx, true);

        // Initially no hover
        let caps = handle.capabilities().unwrap();
        assert!(caps.hover_provider.is_none());

        // Dynamically register via the shared CapabilityStore
        let reg = lsp_types::Registration {
            id: "r1".to_string(),
            method: "textDocument/hover".to_string(),
            register_options: None,
        };
        handle.capabilities.apply_registration(&reg);

        // Now capabilities() reflects the change
        let caps = handle.capabilities().unwrap();
        assert!(caps.hover_provider.is_some());
    }
}
