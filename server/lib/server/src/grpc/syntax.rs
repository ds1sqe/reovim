//! `SyntaxService` gRPC implementation.
//!
//! Exposes syntax tokens via gRPC for client-side syntax highlighting.
//!
//! # Philosophy
//!
//! Server provides token categories (mechanism), client applies colors (policy).
//! No color data in the protocol - only semantic categories like "keyword",
//! "function", "string", etc.
//!
//! # Integration Status
//!
//! The syntax driver infrastructure (`SyntaxDriver`, `SyntaxDriverFactory`) exists
//! in `reovim-driver-syntax` but isn't yet integrated into sessions. This service
//! provides the gRPC API surface; full functionality will be enabled when syntax
//! drivers are connected to the session model.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v2::{
        GetLanguageInfoRequest, GetLanguageInfoResponse, GetTokensRequest, GetTokensResponse,
        StreamTokensRequest, TokenSpan, TokenUpdate, syntax_service_server::SyntaxService,
    },
    tokio::sync::mpsc,
    tokio_stream::wrappers::ReceiverStream,
    tonic::{Request, Response, Status},
};

use crate::session::{Session, SessionId, SessionRegistry, SyntaxSessionState, SyntaxStreamState};

/// Forward syntax token updates from session to gRPC stream.
///
/// Runs inside a `tokio::spawn` task. Sends the initial full refresh, then
/// forwards incremental updates filtered by buffer ID. Exits when the
/// client disconnects (send fails) or the syntax channel closes.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn forward_token_updates(
    tx: mpsc::Sender<Result<TokenUpdate, Status>>,
    initial_update: TokenUpdate,
    mut syntax_rx: mpsc::Receiver<TokenUpdate>,
    buffer_id: BufferId,
    _session: Arc<Session>,
) {
    // Send initial full refresh
    if tx.send(Ok(initial_update)).await.is_err() {
        return; // Client disconnected
    }

    // Forward updates from syntax state
    while let Some(update) = syntax_rx.recv().await {
        // Only forward updates for the requested buffer
        if update.buffer_id == buffer_id.as_usize() as u64 && tx.send(Ok(update)).await.is_err() {
            break; // Client disconnected
        }
    }

    tracing::debug!(buffer_id = buffer_id.as_usize(), "StreamTokens stream ended");
}

/// gRPC `SyntaxService` implementation.
///
/// Bridges the syntax driver layer to gRPC clients. Provides token data
/// for syntax highlighting without color information.
pub struct SyntaxServiceImpl {
    /// Shared session registry.
    sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
    default_session_id: SessionId,
}

impl SyntaxServiceImpl {
    /// Create a new `SyntaxService` with access to the session registry.
    #[must_use]
    pub const fn new(sessions: Arc<SessionRegistry>, default_session_id: SessionId) -> Self {
        Self {
            sessions,
            default_session_id,
        }
    }

    /// Get the default session.
    fn get_session(&self) -> Result<Arc<crate::session::Session>, Status> {
        self.sessions
            .get(&self.default_session_id)
            .ok_or_else(|| Status::not_found("No active session"))
    }
}

/// Detect language from file extension.
///
/// Returns `(language_id, language_name)` tuple.
/// Returns `("text", "Plain Text")` for unknown extensions.
fn detect_language_from_path(path: Option<&str>) -> (&'static str, &'static str) {
    let Some(path) = path else {
        return ("text", "Plain Text");
    };

    // Extract extension
    let ext = path.rsplit('.').next().unwrap_or("");

    match ext.to_lowercase().as_str() {
        "rs" => ("rust", "Rust"),
        "py" | "pyi" => ("python", "Python"),
        "js" | "mjs" | "cjs" => ("javascript", "JavaScript"),
        "ts" | "mts" | "cts" => ("typescript", "TypeScript"),
        "tsx" => ("typescriptreact", "TypeScript React"),
        "jsx" => ("javascriptreact", "JavaScript React"),
        "c" => ("c", "C"),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => ("cpp", "C++"),
        "h" => ("c", "C Header"),
        "go" => ("go", "Go"),
        "java" => ("java", "Java"),
        "rb" => ("ruby", "Ruby"),
        "php" => ("php", "PHP"),
        "swift" => ("swift", "Swift"),
        "kt" | "kts" => ("kotlin", "Kotlin"),
        "scala" => ("scala", "Scala"),
        "hs" => ("haskell", "Haskell"),
        "ml" | "mli" => ("ocaml", "OCaml"),
        "lua" => ("lua", "Lua"),
        "sh" | "bash" => ("shellscript", "Shell Script"),
        "zsh" => ("zsh", "Zsh"),
        "fish" => ("fish", "Fish"),
        "css" => ("css", "CSS"),
        "scss" => ("scss", "SCSS"),
        "less" => ("less", "Less"),
        "html" | "htm" => ("html", "HTML"),
        "xml" => ("xml", "XML"),
        "json" => ("json", "JSON"),
        "yaml" | "yml" => ("yaml", "YAML"),
        "toml" => ("toml", "TOML"),
        "md" | "markdown" => ("markdown", "Markdown"),
        "sql" => ("sql", "SQL"),
        "vim" => ("vim", "Vimscript"),
        "el" | "lisp" => ("lisp", "Lisp"),
        "clj" | "cljs" => ("clojure", "Clojure"),
        "ex" | "exs" => ("elixir", "Elixir"),
        "erl" => ("erlang", "Erlang"),
        "zig" => ("zig", "Zig"),
        "nim" => ("nim", "Nim"),
        "cr" => ("crystal", "Crystal"),
        "dart" => ("dart", "Dart"),
        "r" => ("r", "R"),
        "jl" => ("julia", "Julia"),
        "proto" => ("protobuf", "Protocol Buffers"),
        "graphql" | "gql" => ("graphql", "GraphQL"),
        "dockerfile" => ("dockerfile", "Dockerfile"),
        "make" | "makefile" => ("makefile", "Makefile"),
        "cmake" => ("cmake", "CMake"),
        "txt" => ("plaintext", "Plain Text"),
        _ => ("text", "Plain Text"),
    }
}

/// Get file extensions for a language ID.
fn extensions_for_language(language_id: &str) -> Vec<String> {
    match language_id {
        "rust" => vec![".rs".to_string()],
        "python" => vec![".py".to_string(), ".pyi".to_string()],
        "javascript" => vec![".js".to_string(), ".mjs".to_string(), ".cjs".to_string()],
        "typescript" => vec![".ts".to_string(), ".mts".to_string(), ".cts".to_string()],
        "typescriptreact" => vec![".tsx".to_string()],
        "javascriptreact" => vec![".jsx".to_string()],
        "c" => vec![".c".to_string(), ".h".to_string()],
        "cpp" => vec![
            ".cpp".to_string(),
            ".cc".to_string(),
            ".cxx".to_string(),
            ".hpp".to_string(),
        ],
        "go" => vec![".go".to_string()],
        "java" => vec![".java".to_string()],
        "ruby" => vec![".rb".to_string()],
        "php" => vec![".php".to_string()],
        "swift" => vec![".swift".to_string()],
        "kotlin" => vec![".kt".to_string(), ".kts".to_string()],
        "scala" => vec![".scala".to_string()],
        "haskell" => vec![".hs".to_string()],
        "ocaml" => vec![".ml".to_string(), ".mli".to_string()],
        "lua" => vec![".lua".to_string()],
        "shellscript" => vec![".sh".to_string(), ".bash".to_string()],
        "zsh" => vec![".zsh".to_string()],
        "fish" => vec![".fish".to_string()],
        "css" => vec![".css".to_string()],
        "scss" => vec![".scss".to_string()],
        "less" => vec![".less".to_string()],
        "html" => vec![".html".to_string(), ".htm".to_string()],
        "xml" => vec![".xml".to_string()],
        "json" => vec![".json".to_string()],
        "yaml" => vec![".yaml".to_string(), ".yml".to_string()],
        "toml" => vec![".toml".to_string()],
        "markdown" => vec![".md".to_string(), ".markdown".to_string()],
        "sql" => vec![".sql".to_string()],
        "vim" => vec![".vim".to_string()],
        "lisp" => vec![".el".to_string(), ".lisp".to_string()],
        "clojure" => vec![".clj".to_string(), ".cljs".to_string()],
        "elixir" => vec![".ex".to_string(), ".exs".to_string()],
        "erlang" => vec![".erl".to_string()],
        "zig" => vec![".zig".to_string()],
        "nim" => vec![".nim".to_string()],
        "crystal" => vec![".cr".to_string()],
        "dart" => vec![".dart".to_string()],
        "r" => vec![".r".to_string()],
        "julia" => vec![".jl".to_string()],
        "protobuf" => vec![".proto".to_string()],
        "graphql" => vec![".graphql".to_string(), ".gql".to_string()],
        "dockerfile" => vec!["Dockerfile".to_string()],
        "makefile" => vec!["Makefile".to_string(), ".make".to_string()],
        "cmake" => vec!["CMakeLists.txt".to_string(), ".cmake".to_string()],
        _ => vec![],
    }
}

#[tonic::async_trait]
impl SyntaxService for SyntaxServiceImpl {
    /// Get syntax tokens for a buffer range.
    ///
    /// # Current Implementation
    ///
    /// Returns empty tokens with language detection from file path.
    /// Full token extraction will be enabled when syntax drivers are
    /// integrated into the session model.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::significant_drop_tightening)]
    async fn get_tokens(
        &self,
        request: Request<GetTokensRequest>,
    ) -> Result<Response<GetTokensResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        // buffer_id 0 means "active buffer"
        let requested_buffer_id = if req.buffer_id == 0 {
            None
        } else {
            Some(BufferId::from_raw(req.buffer_id as usize))
        };

        session
            .with_state_mut(|state| {
                let buffer_id = requested_buffer_id
                    .or_else(|| state.app.kernel.buffers.list().first().copied())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                let buffer_arc = state.buffer(buffer_id).ok_or_else(|| {
                    Status::not_found(format!("Buffer {} not found", buffer_id.as_usize()))
                })?;

                let buffer = buffer_arc.read();
                let total_lines = buffer.line_count();

                // Get buffer content for byte range calculation
                let content = buffer.content();
                let file_path = buffer.file_path().map(String::from);

                // Detect language from file path (hardcoded fallback for response)
                let (language_id, _language_name) = detect_language_from_path(file_path.as_deref());

                // Calculate byte range for requested lines
                let start_line = req.start_line.unwrap_or(0) as usize;
                let end_line = match req.end_line {
                    Some(e) if (e as usize) < total_lines => e as usize,
                    _ => total_lines.saturating_sub(1),
                };

                // Convert line range to byte range
                let start_byte =
                    buffer.position_to_byte(reovim_kernel::api::v1::Position::new(start_line, 0));
                let end_byte = if end_line < total_lines {
                    buffer.position_to_byte(reovim_kernel::api::v1::Position::new(end_line + 1, 0))
                } else {
                    content.len()
                };
                let byte_range = start_byte..end_byte;

                // Drop buffer read lock before accessing extensions
                drop(buffer);
                drop(buffer_arc);

                // Get syntax session state from session-wide extensions (#491)
                let syntax_state = state.app.extensions.get_or_insert::<SyntaxSessionState>();

                // Try registry-based detection first, fall back to hardcoded
                if let Some(path) = &file_path {
                    syntax_state.ensure_driver_from_path(buffer_id, path, &content);
                }
                // Fall back to hardcoded language detection if registry didn't work
                if !syntax_state.has_driver(buffer_id) {
                    syntax_state.ensure_driver(buffer_id, language_id, &content);
                }

                // Get tokens from driver (highlights + decorations)
                let tokens = if syntax_state.has_driver(buffer_id) {
                    syntax_state.get(buffer_id).map_or_else(Vec::new, |driver| {
                        let mut annotations = driver.highlights(byte_range.clone());
                        annotations.extend(driver.decorations(byte_range));
                        annotations
                            .into_iter()
                            .map(|span| TokenSpan {
                                start_byte: span.start_byte as u32,
                                end_byte: span.end_byte as u32,
                                category: span.category.to_string(),
                            })
                            .collect()
                    })
                } else {
                    // No driver available for this language
                    Vec::new()
                };

                Ok(Response::new(GetTokensResponse {
                    buffer_id: buffer_id.as_usize() as u64,
                    tokens,
                    language_id: language_id.to_string(),
                    total_lines: total_lines as u64,
                }))
            })
            .await
    }

    /// Stream type for `StreamTokens`.
    type StreamTokensStream = ReceiverStream<Result<TokenUpdate, Status>>;

    /// Stream token updates in real-time.
    ///
    /// Subscribes to the `SyntaxSessionState` to receive token updates when
    /// buffers are modified. Sends an initial full refresh with current tokens.
    #[allow(clippy::cast_possible_truncation)]
    async fn stream_tokens(
        &self,
        request: Request<StreamTokensRequest>,
    ) -> Result<Response<Self::StreamTokensStream>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;
        let session_clone = Arc::clone(&session);

        // Verify buffer exists and get initial tokens
        let buffer_id = BufferId::from_raw(req.buffer_id as usize);
        let (initial_update, syntax_rx) = session
            .with_state_mut(|state| {
                // Verify buffer exists
                let buffer_arc = state.buffer(buffer_id).ok_or_else(|| {
                    Status::not_found(format!("Buffer {} not found", buffer_id.as_usize()))
                })?;

                let buffer = buffer_arc.read();
                let total_lines = buffer.line_count() as u64;
                let content = buffer.content();
                let file_path = buffer.file_path().map(String::from);
                let (language_id, _) = detect_language_from_path(file_path.as_deref());

                // Drop buffer lock before accessing extensions
                drop(buffer);
                drop(buffer_arc);

                // Get syntax state and ensure driver exists (#491)
                let syntax_state = state.app.extensions.get_or_insert::<SyntaxSessionState>();

                // Try registry-based detection first, fall back to hardcoded
                if let Some(path) = &file_path {
                    syntax_state.ensure_driver_from_path(buffer_id, path, &content);
                }
                if !syntax_state.has_driver(buffer_id) {
                    syntax_state.ensure_driver(buffer_id, language_id, &content);
                }

                // Get initial tokens (must finish borrow before accessing stream state)
                let tokens = syntax_state.get(buffer_id).map_or_else(Vec::new, |driver| {
                    let len = content.len();
                    let mut annotations = driver.highlights(0..len);
                    annotations.extend(driver.decorations(0..len));
                    annotations
                        .into_iter()
                        .map(|span| TokenSpan {
                            start_byte: span.start_byte as u32,
                            end_byte: span.end_byte as u32,
                            category: span.category.to_string(),
                        })
                        .collect()
                });

                // Subscribe to updates via stream state
                let stream_state = state.app.extensions.get_or_insert::<SyntaxStreamState>();
                let rx = stream_state.subscribe();

                let initial = TokenUpdate {
                    buffer_id: buffer_id.as_usize() as u64,
                    tokens,
                    start_line: 0,
                    end_line: total_lines.saturating_sub(1),
                    full_refresh: true,
                    layer: "syntax".into(),
                    priority: 0,
                };

                Ok::<_, Status>((initial, rx))
            })
            .await?;

        // Create output channel for the gRPC stream
        let (tx, rx) = mpsc::channel(32);

        // Spawn a task to forward updates
        tokio::spawn(forward_token_updates(
            tx,
            initial_update,
            syntax_rx,
            buffer_id,
            session_clone,
        ));

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    /// Get language info for a buffer.
    ///
    /// Returns language metadata including whether a tree-sitter parser
    /// is available for the detected language.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::significant_drop_tightening)]
    async fn get_language_info(
        &self,
        request: Request<GetLanguageInfoRequest>,
    ) -> Result<Response<GetLanguageInfoResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        session
            .with_state_mut(|state| {
                let buffer_id = BufferId::from_raw(req.buffer_id as usize);

                let buffer_arc = state.buffer(buffer_id).ok_or_else(|| {
                    Status::not_found(format!("Buffer {} not found", buffer_id.as_usize()))
                })?;

                let buffer = buffer_arc.read();
                let file_path = buffer.file_path().map(String::from);

                // Drop buffer lock before accessing extensions
                drop(buffer);
                drop(buffer_arc);

                let syntax_state = state.app.extensions.get_or_insert::<SyntaxSessionState>();

                // Try registry-based detection first
                if let Some(ref path) = file_path
                    && let Some(lang_id) = syntax_state.detect_language(path)
                    && let Some(registry) = syntax_state.registry()
                    && let Some(info) = registry.get_info(&lang_id)
                {
                    let has_parser = syntax_state.factory().is_some_and(|f| f.supports(&lang_id));
                    let extensions = info.extensions.iter().map(|e| format!(".{e}")).collect();
                    return Ok(Response::new(GetLanguageInfoResponse {
                        language_id: lang_id,
                        language_name: info.name.clone(),
                        extensions,
                        has_parser,
                    }));
                }

                // Fall back to hardcoded detection
                let (language_id, language_name) = detect_language_from_path(file_path.as_deref());
                let extensions = extensions_for_language(language_id);
                let has_parser = syntax_state
                    .factory()
                    .is_some_and(|f| f.supports(language_id));

                Ok(Response::new(GetLanguageInfoResponse {
                    language_id: language_id.to_string(),
                    language_name: language_name.to_string(),
                    extensions,
                    has_parser,
                }))
            })
            .await
    }
}

#[cfg(test)]
#[path = "syntax_tests.rs"]
mod tests;
