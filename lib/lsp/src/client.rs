//! LSP client implementation.
//!
//! Handles spawning the language server process and managing the
//! request/response lifecycle.

use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use {
    lsp_types::{
        ClientCapabilities, DiagnosticClientCapabilities, DocumentDiagnosticParams,
        DocumentDiagnosticReport, GotoDefinitionParams, GotoDefinitionResponse, Hover,
        HoverClientCapabilities, HoverParams, InitializeParams, InitializeResult,
        InitializedParams, PartialResultParams, Position, ReferenceClientCapabilities,
        ReferenceContext, ReferenceParams, ServerCapabilities, TextDocumentClientCapabilities,
        TextDocumentIdentifier, TextDocumentPositionParams, TextDocumentSyncClientCapabilities,
        Uri, WorkDoneProgressParams, WorkspaceFolder,
    },
    serde_json::Value,
    tokio::{
        io::{BufReader, BufWriter},
        process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
        sync::{Mutex, mpsc, oneshot},
    },
    tracing::{debug, error, info, warn},
};

use crate::{
    jsonrpc::{Id, Message, Notification, Request, Response},
    transport::{Transport, TransportError},
};

/// Error type for client operations.
#[derive(Debug)]
pub enum ClientError {
    /// Failed to spawn the server process.
    SpawnFailed(std::io::Error),
    /// Transport error.
    Transport(TransportError),
    /// Server returned an error response.
    ServerError(crate::jsonrpc::Error),
    /// Request timed out.
    Timeout,
    /// Channel closed (server died).
    ChannelClosed,
    /// Serialization error.
    Serialization(serde_json::Error),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpawnFailed(e) => write!(f, "Failed to spawn server: {e}"),
            Self::Transport(e) => write!(f, "Transport error: {e}"),
            Self::ServerError(e) => write!(f, "Server error: {e}"),
            Self::Timeout => write!(f, "Request timed out"),
            Self::ChannelClosed => write!(f, "Channel closed"),
            Self::Serialization(e) => write!(f, "Serialization error: {e}"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<TransportError> for ClientError {
    fn from(e: TransportError) -> Self {
        Self::Transport(e)
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e)
    }
}

/// A pending request waiting for a response.
type PendingRequest = oneshot::Sender<Result<Value, ClientError>>;

/// Server capabilities and state.
#[derive(Debug, Clone)]
pub struct ServerState {
    /// Server capabilities received from initialize response.
    pub capabilities: ServerCapabilities,
    /// Whether the server has been initialized.
    pub initialized: bool,
}

/// LSP client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Command to spawn the language server.
    pub command: String,
    /// Arguments to pass to the server.
    pub args: Vec<String>,
    /// Working directory for the server.
    pub root_path: PathBuf,
    /// Workspace folders to register.
    pub workspace_folders: Vec<WorkspaceFolder>,
}

impl ClientConfig {
    /// Create a new client config for rust-analyzer.
    #[must_use]
    pub fn rust_analyzer(root_path: &std::path::Path) -> Self {
        let root_uri = uri_from_path(root_path);

        Self {
            command: "rust-analyzer".to_string(),
            args: vec![],
            root_path: root_path.to_path_buf(),
            workspace_folders: vec![WorkspaceFolder {
                uri: root_uri,
                name: root_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace")
                    .to_string(),
            }],
        }
    }
}

/// Convert a file path to a `lsp_types::Uri`.
///
/// Uses proper file:// URI encoding.
///
/// # Panics
///
/// Panics if the fallback URI cannot be parsed (should never happen).
#[must_use]
pub fn uri_from_path(path: &std::path::Path) -> Uri {
    // Canonicalize to absolute path if possible
    let abs_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path_str = abs_path.to_string_lossy();

    // Handle Windows paths (convert backslashes, add leading slash)
    #[cfg(windows)]
    let uri_str = format!("file:///{}", path_str.replace('\\', "/"));

    // Unix paths: file:///absolute/path (3 slashes)
    #[cfg(not(windows))]
    let uri_str = format!("file://{path_str}");

    uri_str.parse().unwrap_or_else(|_| {
        // Fallback to root
        "file:///".parse().expect("fallback URI should parse")
    })
}

/// LSP client.
///
/// Manages communication with a language server process.
pub struct Client {
    /// Configuration.
    config: ClientConfig,

    /// Next request ID.
    next_id: AtomicU64,

    /// Pending requests waiting for responses.
    pending: Arc<Mutex<HashMap<Id, PendingRequest>>>,

    /// Channel for sending messages to the writer task.
    writer_tx: mpsc::UnboundedSender<Message>,

    /// Server state (capabilities, initialized flag).
    state: Arc<Mutex<Option<ServerState>>>,

    /// Server process handle.
    #[allow(dead_code)]
    process: Child,
}

impl Client {
    /// Spawn a new language server and create a client.
    ///
    /// Returns the client and channels for receiving messages from the server.
    ///
    /// # Errors
    ///
    /// Returns an error if the server process cannot be spawned.
    ///
    /// # Panics
    ///
    /// Panics if stdin/stdout are not captured from the process (should never happen).
    #[allow(clippy::type_complexity)]
    pub fn spawn(
        config: ClientConfig,
    ) -> Result<
        (
            Self,
            mpsc::UnboundedReceiver<Message>,
            BufReader<ChildStdout>,
            Option<ChildStderr>,
        ),
        ClientError,
    > {
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
            .map_err(ClientError::SpawnFailed)?;

        let stdin = process.stdin.take().expect("stdin not captured");
        let stdout = process.stdout.take().expect("stdout not captured");
        let stderr = process.stderr.take();

        let stdout_reader = BufReader::new(stdout);
        let stdin_writer = BufWriter::new(stdin);

        // Create channel for outgoing messages
        let (writer_tx, writer_rx) = mpsc::unbounded_channel();

        // Spawn writer task
        tokio::spawn(Self::writer_task(stdin_writer, writer_rx));

        let client = Self {
            config,
            next_id: AtomicU64::new(1),
            pending: Arc::new(Mutex::new(HashMap::new())),
            writer_tx,
            state: Arc::new(Mutex::new(None)),
            process,
        };

        // Create channel for incoming messages (to be processed by saturator)
        let (_incoming_tx, incoming_rx) = mpsc::unbounded_channel();

        Ok((client, incoming_rx, stdout_reader, stderr))
    }

    /// Writer task that sends messages to the server.
    async fn writer_task(
        mut writer: BufWriter<ChildStdin>,
        mut rx: mpsc::UnboundedReceiver<Message>,
    ) {
        while let Some(message) = rx.recv().await {
            if let Err(e) = Transport::send(&mut writer, &message).await {
                error!("Failed to send message: {}", e);
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
    /// - The request cannot be sent
    /// - The response contains an error
    /// - The channel is closed
    pub async fn request<P: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R, ClientError> {
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
            return Err(ClientError::ChannelClosed);
        }

        // Wait for response
        let result = rx.await.map_err(|_| ClientError::ChannelClosed)??;

        // Deserialize result
        serde_json::from_value(result).map_err(ClientError::from)
    }

    /// Send a notification (no response expected).
    ///
    /// # Errors
    ///
    /// Returns an error if the notification cannot be sent.
    pub fn notify<P: serde::Serialize>(&self, method: &str, params: P) -> Result<(), ClientError> {
        let params_value = serde_json::to_value(params)?;
        let notification = Notification::new(method, Some(params_value));

        debug!(method = %method, "Sending notification");
        self.writer_tx
            .send(Message::Notification(notification))
            .map_err(|_| ClientError::ChannelClosed)
    }

    /// Send a response to a server request.
    ///
    /// Used for server-to-client requests that require a response.
    pub fn send_response(&self, response: Response) {
        debug!(id = ?response.id, "Sending response");
        if self.writer_tx.send(Message::Response(response)).is_err() {
            warn!("Failed to send response - channel closed");
        }
    }

    /// Handle a response from the server.
    ///
    /// Matches the response to a pending request and sends the result.
    pub async fn handle_response(&self, response: Response) {
        let mut pending = self.pending.lock().await;

        if let Some(tx) = pending.remove(&response.id) {
            let result = if let Some(error) = response.error {
                Err(ClientError::ServerError(error))
            } else {
                Ok(response.result.unwrap_or(Value::Null))
            };

            if tx.send(result).is_err() {
                warn!(id = ?response.id, "Response receiver dropped - request handler may have timed out");
            } else {
                debug!(id = ?response.id, "Response sent to receiver");
            }
        } else {
            warn!(id = ?response.id, "No pending request for response");
        }
    }

    /// Initialize the language server.
    ///
    /// Sends the `initialize` request and `initialized` notification.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    #[allow(deprecated)] // root_path and root_uri are deprecated but still widely used
    pub async fn initialize(&self) -> Result<InitializeResult, ClientError> {
        let root_uri = uri_from_path(&self.config.root_path);

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
            if let Some(ref mut s) = *state {
                s.initialized = true;
            }
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

    /// Build client capabilities.
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
                // Enable pull diagnostics (textDocument/diagnostic) - LSP 3.17
                diagnostic: Some(DiagnosticClientCapabilities {
                    dynamic_registration: Some(false),
                    related_document_support: Some(true),
                }),
                // Enable hover support
                hover: Some(HoverClientCapabilities {
                    dynamic_registration: Some(false),
                    content_format: Some(vec![
                        lsp_types::MarkupKind::Markdown,
                        lsp_types::MarkupKind::PlainText,
                    ]),
                }),
                // Enable references support
                references: Some(ReferenceClientCapabilities {
                    dynamic_registration: Some(false),
                }),
                // Definition support uses default capabilities
                definition: Some(lsp_types::GotoCapability {
                    dynamic_registration: Some(false),
                    link_support: Some(true),
                }),
                ..Default::default()
            }),
            // Enable work done progress support (required for progress notifications)
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

    /// Request diagnostics for a document (pull diagnostics - LSP 3.17).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn request_diagnostics(
        &self,
        uri: Uri,
        identifier: Option<String>,
        previous_result_id: Option<String>,
    ) -> Result<DocumentDiagnosticReport, ClientError> {
        let params = DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri },
            identifier,
            previous_result_id,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.request("textDocument/diagnostic", params).await
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
    ) -> Result<Option<GotoDefinitionResponse>, ClientError> {
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
    ) -> Result<Option<Vec<lsp_types::Location>>, ClientError> {
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
    pub async fn hover(&self, uri: Uri, position: Position) -> Result<Option<Hover>, ClientError> {
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
    pub async fn shutdown(&self) -> Result<(), ClientError> {
        info!("Shutting down language server");

        // Send shutdown request
        let _: Value = self.request("shutdown", Value::Null).await?;

        // Send exit notification
        self.notify("exit", Value::Null)?;

        info!("Language server shutdown complete");
        Ok(())
    }

    /// Get the root path.
    #[must_use]
    pub const fn root_path(&self) -> &PathBuf {
        &self.config.root_path
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
