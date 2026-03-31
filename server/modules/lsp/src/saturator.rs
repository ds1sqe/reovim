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
        NumberOrString, ProgressParams, PublishDiagnosticsParams, RegistrationParams,
        ShowMessageParams, TextDocumentContentChangeEvent, TextDocumentIdentifier,
        TextDocumentItem, UnregistrationParams, Uri, VersionedTextDocumentIdentifier,
        WorkDoneProgress,
    },
    reovim_driver_lsp::{
        CapabilityStore, DiagnosticCache, LspError, LspLogger, LspProvider, LspRequest,
        LspServerConfig,
        client::Client,
        jsonrpc::{Message, Response},
        transport::Transport,
    },
    reovim_driver_session::{PendingLevel, PendingNotificationQueue, PendingOp},
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
        queue: Option<Arc<PendingNotificationQueue>>,
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
        let queue_clone = queue.clone();
        let lang_id_clone = language_id.clone();
        let run_handle = tokio::spawn(Self::run(
            client_clone,
            stdout_reader,
            request_rx,
            cache_clone,
            active_clone,
            caps_clone,
            logger_clone,
            queue_clone,
            lang_id_clone,
        ));

        // Monitor the run task: if it panics, clear active and notify user.
        let active_watcher = Arc::clone(&active);
        let queue_watcher = queue.clone();
        tokio::spawn(async move {
            if let Err(e) = run_handle.await {
                active_watcher.store(false, Ordering::Relaxed);
                tracing::error!("LSP saturator task panicked: {e}");
                if let Some(ref q) = queue_watcher {
                    q.push_op(
                        None,
                        PendingOp::Push {
                            level: PendingLevel::Error,
                            title: format!("LSP server crashed: {e}"),
                        },
                    );
                }
            }
        });

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
    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn run(
        client: Arc<Client>,
        mut stdout_reader: tokio::io::BufReader<tokio::process::ChildStdout>,
        mut request_rx: mpsc::Receiver<LspRequest>,
        cache: Arc<DiagnosticCache>,
        active: Arc<AtomicBool>,
        capabilities: Arc<CapabilityStore>,
        logger: Option<Arc<LspLogger>>,
        queue: Option<Arc<PendingNotificationQueue>>,
        server_name: String,
    ) {
        info!("LSP saturator started");

        loop {
            tokio::select! {
                // Handle incoming messages from server
                result = Transport::recv(&mut stdout_reader) => {
                    match result {
                        Ok(message) => {
                            Self::log_incoming(logger.as_ref(), &message);
                            Self::handle_server_message(
                                &client, &cache, &capabilities, message,
                                queue.as_deref(), &server_name,
                            ).await;
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn handle_server_message(
        client: &Arc<Client>,
        cache: &Arc<DiagnosticCache>,
        capabilities: &CapabilityStore,
        message: Message,
        queue: Option<&PendingNotificationQueue>,
        server_name: &str,
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
            Message::Notification(notification) => match notification.method.as_str() {
                "textDocument/publishDiagnostics" => {
                    if let Some(params) = notification.params
                        && let Ok(diag_params) =
                            serde_json::from_value::<PublishDiagnosticsParams>(params)
                    {
                        Self::handle_diagnostics(cache, diag_params);
                    }
                }
                "$/progress" => {
                    if let Some(queue) = queue
                        && let Some(params) = notification.params
                    {
                        handle_progress_notification(queue, server_name, params);
                    }
                }
                "window/showMessage" => {
                    if let Some(queue) = queue
                        && let Some(params) = notification.params
                    {
                        handle_show_message(queue, server_name, params);
                    }
                }
                "window/logMessage" => {
                    if let Some(queue) = queue
                        && let Some(params) = notification.params
                    {
                        handle_log_message(queue, server_name, params);
                    }
                }
                _ => {
                    debug!(method = %notification.method, "Unhandled notification");
                }
            },
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
    #[allow(clippy::too_many_lines)]
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
            LspRequest::DidSave { uri, text } => {
                debug!(uri = %uri.as_str(), "Sending didSave notification");
                client.did_save(uri, text);
            }
            LspRequest::SignatureHelp {
                uri,
                position,
                response_tx,
            } => {
                debug!(uri = %uri.as_str(), position = ?position, "Spawning signature_help task");
                let client = Arc::clone(client);
                tokio::spawn(async move {
                    let result = client.signature_help(uri, position).await;
                    debug!(success = result.is_ok(), "signature_help completed");
                    let _ = response_tx.send(result);
                });
            }
            LspRequest::DocumentHighlight {
                uri,
                position,
                response_tx,
            } => {
                debug!(uri = %uri.as_str(), position = ?position, "Spawning document_highlight task");
                let client = Arc::clone(client);
                tokio::spawn(async move {
                    let result = client.document_highlight(uri, position).await;
                    debug!(success = result.is_ok(), "document_highlight completed");
                    let _ = response_tx.send(result);
                });
            }
            LspRequest::Formatting {
                uri,
                options,
                response_tx,
            } => {
                debug!(uri = %uri.as_str(), "Spawning formatting task");
                let client = Arc::clone(client);
                tokio::spawn(async move {
                    let result = client.formatting(uri, options).await;
                    debug!(success = result.is_ok(), "formatting completed");
                    let _ = response_tx.send(result);
                });
            }
            LspRequest::RangeFormatting {
                uri,
                range,
                options,
                response_tx,
            } => {
                debug!(uri = %uri.as_str(), range = ?range, "Spawning range_formatting task");
                let client = Arc::clone(client);
                tokio::spawn(async move {
                    let result = client.range_formatting(uri, range, options).await;
                    debug!(success = result.is_ok(), "range_formatting completed");
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
    // Requires LspLogger construction (file I/O + env setup).
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    // Requires LspLogger construction (file I/O + env setup).
    #[cfg_attr(coverage_nightly, coverage(off))]
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
            LspRequest::DidSave { uri, .. } => {
                log.log_sent("textDocument/didSave", uri.as_str());
            }
            LspRequest::SignatureHelp { uri, position, .. } => {
                log.log_sent(
                    "textDocument/signatureHelp",
                    &format!("{} {}:{}", uri.as_str(), position.line, position.character),
                );
            }
            LspRequest::DocumentHighlight { uri, position, .. } => {
                log.log_sent(
                    "textDocument/documentHighlight",
                    &format!("{} {}:{}", uri.as_str(), position.line, position.character),
                );
            }
            LspRequest::Formatting { uri, .. } => {
                log.log_sent("textDocument/formatting", uri.as_str());
            }
            LspRequest::RangeFormatting { uri, range, .. } => {
                log.log_sent(
                    "textDocument/rangeFormatting",
                    &format!(
                        "{} {}:{}-{}:{}",
                        uri.as_str(),
                        range.start.line,
                        range.start.character,
                        range.end.line,
                        range.end.character
                    ),
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

// ============================================================================
// Notification handlers (#691) — extracted as standalone functions for testability
// ============================================================================

/// Handle `$/progress` notification from the language server.
///
/// Parses `ProgressParams` and pushes the appropriate `PendingOp` to the queue.
fn handle_progress_notification(
    queue: &PendingNotificationQueue,
    server_name: &str,
    params: serde_json::Value,
) {
    let Ok(progress_params) = serde_json::from_value::<ProgressParams>(params) else {
        warn!("Failed to parse $/progress params");
        return;
    };

    let token = match &progress_params.token {
        NumberOrString::String(s) => s.clone(),
        NumberOrString::Number(n) => format!("lsp_{n}"),
    };

    let source = Some(server_name.to_owned());

    let lsp_types::ProgressParamsValue::WorkDone(progress) = progress_params.value;

    match progress {
        WorkDoneProgress::Begin(begin) => {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let percentage = begin.percentage.map_or(0, |p| p.clamp(0, 100) as u8);
            queue.push_op(
                source,
                PendingOp::ProgressBegin {
                    token,
                    title: begin.title,
                    message: begin.message.unwrap_or_default(),
                    percentage,
                },
            );
        }
        WorkDoneProgress::Report(report) => {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let percentage = report.percentage.map(|p| p.clamp(0, 100) as u8);
            queue.push_op(
                source,
                PendingOp::ProgressReport {
                    token,
                    message: report.message,
                    percentage,
                },
            );
        }
        WorkDoneProgress::End(end) => {
            queue.push_op(
                source,
                PendingOp::ProgressEnd {
                    token,
                    message: end.message,
                },
            );
        }
    }
}

/// Handle `window/showMessage` notification.
///
/// Maps LSP `MessageType` to `PendingLevel` and pushes to the queue.
fn handle_show_message(
    queue: &PendingNotificationQueue,
    server_name: &str,
    params: serde_json::Value,
) {
    let Ok(msg_params) = serde_json::from_value::<ShowMessageParams>(params) else {
        warn!("Failed to parse window/showMessage params");
        return;
    };

    let level = match msg_params.typ {
        lsp_types::MessageType::ERROR => PendingLevel::Error,
        lsp_types::MessageType::WARNING => PendingLevel::Warning,
        _ => PendingLevel::Info, // INFO, LOG, and others → Info
    };

    queue.push_op(
        Some(server_name.to_owned()),
        PendingOp::Push {
            level,
            title: msg_params.message,
        },
    );
}

/// Handle `window/logMessage` notification.
///
/// Only Error and Warning log messages are surfaced as toasts.
/// Info and lower are routed to `tracing::info!` only (FD recommendation).
fn handle_log_message(
    queue: &PendingNotificationQueue,
    server_name: &str,
    params: serde_json::Value,
) {
    let Ok(msg_params) = serde_json::from_value::<ShowMessageParams>(params) else {
        warn!("Failed to parse window/logMessage params");
        return;
    };

    match msg_params.typ {
        lsp_types::MessageType::ERROR => {
            queue.push_op(
                Some(server_name.to_owned()),
                PendingOp::Push {
                    level: PendingLevel::Error,
                    title: msg_params.message,
                },
            );
        }
        lsp_types::MessageType::WARNING => {
            queue.push_op(
                Some(server_name.to_owned()),
                PendingOp::Push {
                    level: PendingLevel::Warning,
                    title: msg_params.message,
                },
            );
        }
        _ => {
            // Info/Log level — trace only, no toast
            info!(
                server = %server_name,
                message = %msg_params.message,
                "LSP log message"
            );
        }
    }
}

#[cfg(test)]
#[path = "saturator_tests.rs"]
mod tests;
