//! LSP saturator - background task following reovim's saturator pattern.
//!
//! The saturator pattern decouples LSP I/O from the main render thread:
//!
//! 1. **Main thread**: Sends non-blocking requests via `try_send()`
//! 2. **Saturator task**: Owns the LSP client, processes requests/responses
//! 3. **`DiagnosticCache`**: Uses `ArcSwap` for lock-free reads
//!
//! This ensures the render thread never blocks on LSP operations.

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use {
    lsp_types::{
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        DocumentDiagnosticReport, GotoDefinitionResponse, Hover, Location, Position,
        PublishDiagnosticsParams, TextDocumentContentChangeEvent, TextDocumentIdentifier,
        TextDocumentItem, Uri, VersionedTextDocumentIdentifier,
    },
    parking_lot::Mutex,
    tokio::{
        io::BufReader,
        process::ChildStdout,
        sync::{mpsc, oneshot},
    },
    tracing::{debug, error, info, warn},
};

use crate::{
    cache::DiagnosticCache,
    client::{Client, ClientConfig, ClientError},
    jsonrpc::{Message, Request, Response},
    transport::Transport,
};

/// Response type for navigation requests.
pub type NavigationResult<T> = Result<T, ClientError>;

/// Request types that can be sent to the saturator.
pub enum LspRequest {
    /// Open a document.
    DidOpen {
        /// Document URI.
        uri: Uri,
        /// Language ID (e.g., "rust").
        language_id: String,
        /// Document version.
        version: i32,
        /// Document content.
        content: String,
    },
    /// Document content changed.
    DidChange {
        /// Document URI.
        uri: Uri,
        /// Document version.
        version: i32,
        /// New content (FULL sync mode).
        content: String,
    },
    /// Document closed.
    DidClose {
        /// Document URI.
        uri: Uri,
    },
    /// Go to definition request.
    GotoDefinition {
        /// Document URI.
        uri: Uri,
        /// Cursor position.
        position: Position,
        /// Response channel.
        response_tx: oneshot::Sender<NavigationResult<Option<GotoDefinitionResponse>>>,
    },
    /// Find references request.
    References {
        /// Document URI.
        uri: Uri,
        /// Cursor position.
        position: Position,
        /// Include the declaration in results.
        include_declaration: bool,
        /// Response channel.
        response_tx: oneshot::Sender<NavigationResult<Option<Vec<Location>>>>,
    },
    /// Hover request.
    Hover {
        /// Document URI.
        uri: Uri,
        /// Cursor position.
        position: Position,
        /// Response channel.
        response_tx: oneshot::Sender<NavigationResult<Option<Hover>>>,
    },
    /// Shutdown the server.
    Shutdown,
}

impl std::fmt::Debug for LspRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DidOpen {
                uri,
                language_id,
                version,
                ..
            } => f
                .debug_struct("DidOpen")
                .field("uri", uri)
                .field("language_id", language_id)
                .field("version", version)
                .finish(),
            Self::DidChange { uri, version, .. } => f
                .debug_struct("DidChange")
                .field("uri", uri)
                .field("version", version)
                .finish(),
            Self::DidClose { uri } => f.debug_struct("DidClose").field("uri", uri).finish(),
            Self::GotoDefinition { uri, position, .. } => f
                .debug_struct("GotoDefinition")
                .field("uri", uri)
                .field("position", position)
                .finish(),
            Self::References {
                uri,
                position,
                include_declaration,
                ..
            } => f
                .debug_struct("References")
                .field("uri", uri)
                .field("position", position)
                .field("include_declaration", include_declaration)
                .finish(),
            Self::Hover { uri, position, .. } => f
                .debug_struct("Hover")
                .field("uri", uri)
                .field("position", position)
                .finish(),
            Self::Shutdown => write!(f, "Shutdown"),
        }
    }
}

/// Handle for sending requests to the saturator.
///
/// Stored in plugin state and used by the main thread to send
/// non-blocking requests.
#[derive(Debug, Clone)]
pub struct LspSaturatorHandle {
    /// Channel for sending requests (buffer=1 for backpressure).
    tx: mpsc::Sender<LspRequest>,
}

impl LspSaturatorHandle {
    /// Send a request to the saturator.
    ///
    /// Uses `try_send()` for non-blocking operation.
    /// If the channel is full, the request is dropped (backpressure).
    pub fn send(&self, request: LspRequest) -> bool {
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

    /// Send a didOpen notification.
    pub fn did_open(&self, uri: Uri, language_id: String, version: i32, content: String) -> bool {
        self.send(LspRequest::DidOpen {
            uri,
            language_id,
            version,
            content,
        })
    }

    /// Send a didChange notification.
    pub fn did_change(&self, uri: Uri, version: i32, content: String) -> bool {
        self.send(LspRequest::DidChange {
            uri,
            version,
            content,
        })
    }

    /// Send a didClose notification.
    pub fn did_close(&self, uri: Uri) -> bool {
        self.send(LspRequest::DidClose { uri })
    }

    /// Request shutdown.
    pub fn shutdown(&self) -> bool {
        self.send(LspRequest::Shutdown)
    }

    /// Request goto definition.
    ///
    /// Returns a receiver for the response.
    pub fn goto_definition(
        &self,
        uri: Uri,
        position: Position,
    ) -> Option<oneshot::Receiver<NavigationResult<Option<GotoDefinitionResponse>>>> {
        let (tx, rx) = oneshot::channel();
        if self.send(LspRequest::GotoDefinition {
            uri,
            position,
            response_tx: tx,
        }) {
            Some(rx)
        } else {
            None
        }
    }

    /// Request references.
    ///
    /// Returns a receiver for the response.
    pub fn references(
        &self,
        uri: Uri,
        position: Position,
        include_declaration: bool,
    ) -> Option<oneshot::Receiver<NavigationResult<Option<Vec<Location>>>>> {
        let (tx, rx) = oneshot::channel();
        if self.send(LspRequest::References {
            uri,
            position,
            include_declaration,
            response_tx: tx,
        }) {
            Some(rx)
        } else {
            None
        }
    }

    /// Request hover.
    ///
    /// Returns a receiver for the response.
    pub fn hover(
        &self,
        uri: Uri,
        position: Position,
    ) -> Option<oneshot::Receiver<NavigationResult<Option<Hover>>>> {
        let (tx, rx) = oneshot::channel();
        if self.send(LspRequest::Hover {
            uri,
            position,
            response_tx: tx,
        }) {
            Some(rx)
        } else {
            None
        }
    }
}

/// LSP saturator state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SaturatorState {
    /// Saturator is starting up.
    Starting,
    /// Saturator is running.
    Running,
    /// Saturator is shutting down.
    ShuttingDown,
    /// Saturator has stopped.
    Stopped,
    /// Saturator failed to start.
    Failed,
}

/// Tracks open documents for pull diagnostics.
///
/// Used to know which documents to request diagnostics for when
/// receiving `workspace/diagnostic/refresh`.
#[derive(Debug, Default)]
struct OpenDocuments {
    /// Set of currently open document URIs.
    uris: HashSet<Uri>,
}

impl OpenDocuments {
    /// Add a document to the open set.
    fn add(&mut self, uri: Uri) {
        self.uris.insert(uri);
    }

    /// Remove a document from the open set.
    fn remove(&mut self, uri: &Uri) {
        self.uris.remove(uri);
    }

    /// Get all open document URIs.
    fn all(&self) -> Vec<Uri> {
        self.uris.iter().cloned().collect()
    }
}

/// LSP saturator - background task that owns the LSP client.
pub struct LspSaturator;

impl LspSaturator {
    /// Start the saturator with the given configuration.
    ///
    /// Returns a handle for sending requests and a shared diagnostic cache.
    ///
    /// # Arguments
    ///
    /// * `config` - Client configuration
    /// * `render_signal` - Optional channel to signal render thread when diagnostics update
    ///
    /// # Errors
    ///
    /// Returns an error if the language server cannot be spawned.
    pub async fn start(
        config: ClientConfig,
        render_signal: Option<mpsc::Sender<()>>,
    ) -> Result<(LspSaturatorHandle, Arc<DiagnosticCache>), ClientError> {
        // Create diagnostic cache
        let cache = Arc::new(DiagnosticCache::new());

        // Create open documents tracker (shared for pull diagnostics)
        let open_docs = Arc::new(Mutex::new(OpenDocuments::default()));

        // Create request channel with reasonable buffer to prevent request drops
        // Buffer of 32 allows for request bursts while still providing backpressure
        let (request_tx, request_rx) = mpsc::channel::<LspRequest>(32);

        // Spawn the client
        let (client, _incoming_rx, stdout_reader, stderr) = Client::spawn(config)?;
        let client = Arc::new(client);

        // Spawn the saturator task
        let cache_clone = Arc::clone(&cache);
        let client_clone = Arc::clone(&client);
        let open_docs_clone = Arc::clone(&open_docs);
        tokio::spawn(Self::run(
            client_clone,
            stdout_reader,
            request_rx,
            cache_clone,
            open_docs_clone,
            render_signal,
        ));

        // Spawn stderr reader if available
        if let Some(stderr) = stderr {
            tokio::spawn(Self::stderr_reader(stderr));
        }

        // Initialize the client
        client.initialize().await?;

        let handle = LspSaturatorHandle { tx: request_tx };

        Ok((handle, cache))
    }

    /// Main saturator loop.
    async fn run(
        client: Arc<Client>,
        mut stdout_reader: BufReader<ChildStdout>,
        mut request_rx: mpsc::Receiver<LspRequest>,
        cache: Arc<DiagnosticCache>,
        open_docs: Arc<Mutex<OpenDocuments>>,
        render_signal: Option<mpsc::Sender<()>>,
    ) {
        info!("LSP saturator started");

        loop {
            tokio::select! {
                // Handle incoming messages from server
                result = Transport::recv(&mut stdout_reader) => {
                    match result {
                        Ok(message) => {
                            Self::handle_server_message(
                                &client,
                                &cache,
                                &open_docs,
                                render_signal.as_ref(),
                                message
                            ).await;
                        }
                        Err(e) => {
                            error!("Failed to receive message: {}", e);
                            break;
                        }
                    }
                }

                // Handle outgoing requests from main thread
                Some(request) = request_rx.recv() => {
                    match request {
                        LspRequest::DidOpen { uri, language_id, version, content } => {
                            Self::handle_did_open(&client, &open_docs, uri, language_id, version, content);
                        }
                        LspRequest::DidChange { uri, version, content } => {
                            Self::handle_did_change(&client, uri, version, content);
                        }
                        LspRequest::DidClose { uri } => {
                            Self::handle_did_close(&client, &cache, &open_docs, &uri);
                        }
                        LspRequest::GotoDefinition { uri, position, response_tx } => {
                            let result = client.goto_definition(uri, position).await;
                            let _ = response_tx.send(result);
                        }
                        LspRequest::References { uri, position, include_declaration, response_tx } => {
                            let result = client.references(uri, position, include_declaration).await;
                            let _ = response_tx.send(result);
                        }
                        LspRequest::Hover { uri, position, response_tx } => {
                            let result = client.hover(uri, position).await;
                            let _ = response_tx.send(result);
                        }
                        LspRequest::Shutdown => {
                            info!("Shutdown requested");
                            if let Err(e) = client.shutdown().await {
                                error!("Shutdown error: {}", e);
                            }
                            break;
                        }
                    }
                }

                else => break,
            }
        }

        info!("LSP saturator stopped");
    }

    /// Handle a message from the server.
    async fn handle_server_message(
        client: &Arc<Client>,
        cache: &Arc<DiagnosticCache>,
        open_docs: &Arc<Mutex<OpenDocuments>>,
        render_signal: Option<&mpsc::Sender<()>>,
        message: Message,
    ) {
        match message {
            Message::Response(response) => {
                client.handle_response(response).await;
            }
            Message::Notification(notification) => {
                if notification.method == "textDocument/publishDiagnostics" {
                    if let Some(params) = notification.params
                        && let Ok(diag_params) =
                            serde_json::from_value::<PublishDiagnosticsParams>(params)
                    {
                        Self::handle_diagnostics(cache, render_signal, diag_params);
                    }
                } else {
                    debug!(method = %notification.method, "Unhandled notification");
                }
            }
            Message::Request(request) => {
                // Server-to-client requests - must respond to avoid blocking rust-analyzer
                Self::handle_server_request(client, cache, open_docs, render_signal, request).await;
            }
        }
    }

    /// Handle publishDiagnostics notification.
    fn handle_diagnostics(
        cache: &Arc<DiagnosticCache>,
        render_signal: Option<&mpsc::Sender<()>>,
        params: PublishDiagnosticsParams,
    ) {
        let count = params.diagnostics.len();
        info!(
            uri = ?params.uri,
            count = count,
            version = ?params.version,
            "Received publishDiagnostics"
        );

        // Store in cache (atomic swap)
        cache.store(&params.uri, params.version, params.diagnostics);

        // Signal render thread if channel provided
        if let Some(tx) = render_signal {
            let _ = tx.try_send(());
        }
    }

    /// Handle server-to-client requests.
    ///
    /// Some LSP methods like `workspace/diagnostic/refresh` are server-to-client
    /// requests that require a response. We must respond to avoid blocking the server.
    async fn handle_server_request(
        client: &Arc<Client>,
        cache: &Arc<DiagnosticCache>,
        open_docs: &Arc<Mutex<OpenDocuments>>,
        render_signal: Option<&mpsc::Sender<()>>,
        request: Request,
    ) {
        debug!(method = %request.method, id = ?request.id, "Server request");

        let response = match request.method.as_str() {
            "workspace/diagnostic/refresh" => {
                // rust-analyzer uses this to request diagnostic refresh
                // Respond with success first, then request diagnostics for open documents
                debug!("Responding to workspace/diagnostic/refresh");

                // Request diagnostics for all open documents
                Self::request_all_diagnostics(client, cache, open_docs, render_signal).await;

                Response::success(request.id, serde_json::Value::Null)
            }
            "client/registerCapability" => {
                // Dynamic capability registration - acknowledge
                debug!("Responding to client/registerCapability");
                Response::success(request.id, serde_json::Value::Null)
            }
            "window/workDoneProgress/create" => {
                // Progress reporting - acknowledge
                debug!("Responding to window/workDoneProgress/create");
                Response::success(request.id, serde_json::Value::Null)
            }
            _ => {
                // Unknown request - respond with method not found
                debug!(method = %request.method, "Unknown server request");
                Response::error(
                    request.id,
                    crate::jsonrpc::Error::new(
                        crate::jsonrpc::error_codes::METHOD_NOT_FOUND,
                        format!("Method not found: {}", request.method),
                    ),
                )
            }
        };

        // Send the response
        client.send_response(response);
    }

    /// Request diagnostics for all open documents (pull diagnostics - LSP 3.17).
    async fn request_all_diagnostics(
        client: &Arc<Client>,
        cache: &Arc<DiagnosticCache>,
        open_docs: &Arc<Mutex<OpenDocuments>>,
        render_signal: Option<&mpsc::Sender<()>>,
    ) {
        // Get list of open document URIs
        let uris = {
            let docs = open_docs.lock();
            docs.all()
        };

        if uris.is_empty() {
            debug!("No open documents for diagnostic refresh");
            return;
        }

        debug!(count = uris.len(), "Requesting diagnostics for open documents");

        // Request diagnostics for each open document
        for uri in uris {
            match client.request_diagnostics(uri.clone(), None, None).await {
                Ok(report) => {
                    Self::handle_diagnostic_report(cache, render_signal, &uri, report);
                }
                Err(e) => {
                    debug!(uri = ?uri, error = %e, "Failed to request diagnostics");
                }
            }
        }
    }

    /// Handle a diagnostic report from the textDocument/diagnostic response.
    fn handle_diagnostic_report(
        cache: &Arc<DiagnosticCache>,
        render_signal: Option<&mpsc::Sender<()>>,
        uri: &Uri,
        report: DocumentDiagnosticReport,
    ) {
        match report {
            DocumentDiagnosticReport::Full(full_report) => {
                let diagnostics = full_report.full_document_diagnostic_report.items;
                let count = diagnostics.len();
                debug!(uri = ?uri, count = count, "Received full diagnostic report");

                // Store in cache
                cache.store(uri, None, diagnostics);

                // Signal render thread if channel provided
                if let Some(tx) = render_signal {
                    let _ = tx.try_send(());
                }
            }
            DocumentDiagnosticReport::Unchanged(unchanged_report) => {
                debug!(
                    uri = ?uri,
                    result_id = ?unchanged_report.unchanged_document_diagnostic_report.result_id,
                    "Diagnostics unchanged"
                );
                // No update needed - diagnostics haven't changed
            }
        }
    }

    /// Handle didOpen request.
    fn handle_did_open(
        client: &Arc<Client>,
        open_docs: &Arc<Mutex<OpenDocuments>>,
        uri: Uri,
        language_id: String,
        version: i32,
        content: String,
    ) {
        debug!(uri = ?uri, language_id = %language_id, version = version, "didOpen");

        // Track this document as open for pull diagnostics
        {
            let mut docs = open_docs.lock();
            docs.add(uri.clone());
        }

        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id,
                version,
                text: content,
            },
        };

        if let Err(e) = client.notify("textDocument/didOpen", params) {
            error!("Failed to send didOpen: {}", e);
        }
    }

    /// Handle didChange request.
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
            error!("Failed to send didChange: {}", e);
        }
    }

    /// Handle didClose request.
    fn handle_did_close(
        client: &Arc<Client>,
        cache: &Arc<DiagnosticCache>,
        open_docs: &Arc<Mutex<OpenDocuments>>,
        uri: &Uri,
    ) {
        debug!(uri = ?uri, "didClose");

        // Remove from open documents tracking
        {
            let mut docs = open_docs.lock();
            docs.remove(uri);
        }

        let params = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
        };

        if let Err(e) = client.notify("textDocument/didClose", params) {
            error!("Failed to send didClose: {}", e);
        }

        // Clear diagnostics for closed document
        cache.remove(uri);
    }

    /// Read and log stderr from the server.
    async fn stderr_reader(stderr: tokio::process::ChildStderr) {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let mut reader = BufReader::new(stderr);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let line = line.trim();
                    if !line.is_empty() {
                        debug!(target: "lsp_server", "{}", line);
                    }
                }
                Err(e) => {
                    error!("Failed to read stderr: {}", e);
                    break;
                }
            }
        }

        debug!("Stderr reader exited");
    }
}

/// Find the project root by looking for Cargo.toml.
///
/// Walks up from the given path until finding a Cargo.toml or reaching the root.
#[must_use]
#[allow(dead_code)]
pub fn find_rust_project_root(path: &std::path::Path) -> Option<PathBuf> {
    let mut current = if path.is_file() {
        path.parent()?.to_path_buf()
    } else {
        path.to_path_buf()
    };

    loop {
        if current.join("Cargo.toml").exists() {
            return Some(current);
        }

        if !current.pop() {
            return None;
        }
    }
}

/// Check if rust-analyzer is available in PATH.
#[must_use]
#[allow(dead_code)]
pub fn rust_analyzer_available() -> bool {
    std::process::Command::new("rust-analyzer")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_uri(path: &str) -> Uri {
        path.parse().expect("test URI should parse")
    }

    #[test]
    fn test_find_rust_project_root() {
        // Test with a path that doesn't have Cargo.toml
        let _result = find_rust_project_root(std::path::Path::new("/tmp"));
        // May or may not find a Cargo.toml depending on the system

        // Test that it returns None for root
        let result = find_rust_project_root(std::path::Path::new("/"));
        assert!(result.is_none());
    }

    #[test]
    fn test_handle_send_when_channel_closed() {
        let (tx, rx) = mpsc::channel::<LspRequest>(1);
        drop(rx); // Close the channel

        let handle = LspSaturatorHandle { tx };
        assert!(!handle.did_open(
            make_uri("file:///test.rs"),
            "rust".to_string(),
            1,
            String::new(),
        ));
    }
}
