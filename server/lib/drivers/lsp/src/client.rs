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
        ClientCapabilities, CompletionParams, CompletionResponse, DocumentHighlight,
        DocumentHighlightParams, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams,
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
                // Enable hover support (dynamic registration: #533)
                hover: Some(lsp_types::HoverClientCapabilities {
                    dynamic_registration: Some(true),
                    content_format: Some(vec![
                        lsp_types::MarkupKind::Markdown,
                        lsp_types::MarkupKind::PlainText,
                    ]),
                }),
                // Enable references support (dynamic registration: #533)
                references: Some(lsp_types::ReferenceClientCapabilities {
                    dynamic_registration: Some(true),
                }),
                // Enable definition support (dynamic registration: #533)
                definition: Some(lsp_types::GotoCapability {
                    dynamic_registration: Some(true),
                    link_support: Some(true),
                }),
                // Enable completion support (dynamic registration: #533)
                completion: Some(lsp_types::CompletionClientCapabilities {
                    dynamic_registration: Some(true),
                    completion_item: Some(lsp_types::CompletionItemCapability {
                        snippet_support: Some(true),
                        documentation_format: Some(vec![
                            lsp_types::MarkupKind::Markdown,
                            lsp_types::MarkupKind::PlainText,
                        ]),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                // Enable document highlight support (dynamic registration: #664)
                document_highlight: Some(lsp_types::DynamicRegistrationClientCapabilities {
                    dynamic_registration: Some(true),
                }),
                // Enable signature help support
                signature_help: Some(lsp_types::SignatureHelpClientCapabilities {
                    dynamic_registration: Some(true),
                    signature_information: Some(lsp_types::SignatureInformationSettings {
                        documentation_format: Some(vec![
                            lsp_types::MarkupKind::Markdown,
                            lsp_types::MarkupKind::PlainText,
                        ]),
                        parameter_information: Some(lsp_types::ParameterInformationSettings {
                            label_offset_support: Some(true),
                        }),
                        active_parameter_support: Some(true),
                    }),
                    context_support: Some(true),
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

    /// Get completion items at the given position.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn completion(
        &self,
        uri: Uri,
        position: Position,
    ) -> Result<Option<CompletionResponse>, LspError> {
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        };

        self.request("textDocument/completion", params).await
    }

    /// Notify the server that a document was saved.
    // coverage(off): thin wrapper sending notification over live LSP channel
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn did_save(&self, uri: Uri, text: Option<String>) {
        let params = lsp_types::DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
            text,
        };
        if let Err(e) = self.notify("textDocument/didSave", params) {
            warn!("Failed to send didSave notification: {e}");
        }
    }

    /// Get signature help at the given position.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    // coverage(off): async I/O — sends JSON-RPC request over live LSP channel
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn signature_help(
        &self,
        uri: Uri,
        position: Position,
    ) -> Result<Option<lsp_types::SignatureHelp>, LspError> {
        let params = lsp_types::SignatureHelpParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            context: None,
        };

        self.request("textDocument/signatureHelp", params).await
    }

    /// Get document highlights for the symbol at the given position.
    ///
    /// Returns a list of ranges where the symbol appears, with kind
    /// information (text, read, write).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn document_highlight(
        &self,
        uri: Uri,
        position: Position,
    ) -> Result<Option<Vec<DocumentHighlight>>, LspError> {
        let params = DocumentHighlightParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.request("textDocument/documentHighlight", params).await
    }

    /// Get formatting edits for the entire document.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    // coverage(off): async I/O — sends JSON-RPC request over live LSP channel
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn formatting(
        &self,
        uri: Uri,
        options: lsp_types::FormattingOptions,
    ) -> Result<Option<Vec<lsp_types::TextEdit>>, LspError> {
        let params = lsp_types::DocumentFormattingParams {
            text_document: TextDocumentIdentifier { uri },
            options,
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        self.request("textDocument/formatting", params).await
    }

    /// Get formatting edits for a range in the document.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    // coverage(off): async I/O — sends JSON-RPC request over live LSP channel
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn range_formatting(
        &self,
        uri: Uri,
        range: lsp_types::Range,
        options: lsp_types::FormattingOptions,
    ) -> Result<Option<Vec<lsp_types::TextEdit>>, LspError> {
        let params = lsp_types::DocumentRangeFormattingParams {
            text_document: TextDocumentIdentifier { uri },
            range,
            options,
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        self.request("textDocument/rangeFormatting", params).await
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
#[path = "client_tests.rs"]
mod tests;
