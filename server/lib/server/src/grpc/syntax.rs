//! `SyntaxService` gRPC implementation.
//!
//! # Architecture (#753 E6)
//!
//! Syntax token management is domain-owned. The server provides the gRPC
//! transport layer; the domain driver manages syntax drivers and produces
//! token updates. Methods that previously accessed `SyntaxSessionState`
//! and `state.buffer()` directly are stubbed pending domain driver wiring.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v2::{
        GetLanguageInfoRequest, GetLanguageInfoResponse, GetTokensRequest, GetTokensResponse,
        StreamTokensRequest, TokenUpdate, syntax_service_server::SyntaxService,
    },
    tokio::sync::mpsc,
    tokio_stream::wrappers::ReceiverStream,
    tonic::{Request, Response, Status},
};

use crate::session::{Session, SessionId, SessionRegistry, SyntaxStreamState};

/// Forward syntax token updates from session to gRPC stream.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn forward_token_updates(
    tx: mpsc::Sender<Result<TokenUpdate, Status>>,
    initial_update: TokenUpdate,
    mut syntax_rx: mpsc::Receiver<TokenUpdate>,
    buffer_id: BufferId,
    _session: Arc<Session>,
) {
    if tx.send(Ok(initial_update)).await.is_err() {
        return;
    }

    while let Some(update) = syntax_rx.recv().await {
        if update.buffer_id == buffer_id.as_usize() as u64 && tx.send(Ok(update)).await.is_err() {
            break;
        }
    }

    tracing::debug!(buffer_id = buffer_id.as_usize(), "StreamTokens stream ended");
}

/// gRPC `SyntaxService` implementation.
pub struct SyntaxServiceImpl {
    sessions: Arc<SessionRegistry>,
    default_session_id: SessionId,
}

impl SyntaxServiceImpl {
    #[must_use]
    pub const fn new(sessions: Arc<SessionRegistry>, default_session_id: SessionId) -> Self {
        Self {
            sessions,
            default_session_id,
        }
    }

    fn get_session(&self) -> Result<Arc<crate::session::Session>, Status> {
        self.sessions
            .get(&self.default_session_id)
            .ok_or_else(|| Status::not_found("No active session"))
    }
}

/// Detect language from file extension.
///
/// Retained for domain driver wiring (#753 E6).
#[allow(dead_code)]
fn detect_language_from_path(path: Option<&str>) -> (&'static str, &'static str) {
    let Some(path) = path else {
        return ("text", "Plain Text");
    };

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
///
/// Retained for domain driver wiring (#753 E6).
#[allow(dead_code)]
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
        "toml" => vec![".toml".to_string()],
        "markdown" => vec![".md".to_string(), ".markdown".to_string()],
        "json" => vec![".json".to_string()],
        "yaml" => vec![".yaml".to_string(), ".yml".to_string()],
        "html" => vec![".html".to_string(), ".htm".to_string()],
        "css" => vec![".css".to_string()],
        "shellscript" => vec![".sh".to_string(), ".bash".to_string()],
        _ => vec![],
    }
}

#[tonic::async_trait]
impl SyntaxService for SyntaxServiceImpl {
    /// Get syntax tokens for a buffer range.
    ///
    /// TODO(#753 E6): Route through DomainDriver.
    #[allow(clippy::cast_possible_truncation)]
    async fn get_tokens(
        &self,
        _request: Request<GetTokensRequest>,
    ) -> Result<Response<GetTokensResponse>, Status> {
        Err(Status::unimplemented(
            "GetTokens: pending domain driver wiring (#753 E6)",
        ))
    }

    /// Stream type for `StreamTokens`.
    type StreamTokensStream = ReceiverStream<Result<TokenUpdate, Status>>;

    /// Stream token updates in real-time.
    ///
    /// TODO(#753 E6): Route through DomainDriver for initial tokens.
    async fn stream_tokens(
        &self,
        request: Request<StreamTokensRequest>,
    ) -> Result<Response<Self::StreamTokensStream>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;
        let session_clone = Arc::clone(&session);

        let buffer_id = BufferId::from_raw(req.buffer_id as usize);

        // Subscribe to token updates (SyntaxStreamState is server-side, no driver dep)
        let (initial_update, syntax_rx) = session
            .with_state_mut(|state| {
                let stream_state = state.app.extensions.get_or_insert::<SyntaxStreamState>();
                let rx = stream_state.subscribe();

                // Empty initial update — domain driver will push real tokens
                let initial = TokenUpdate {
                    buffer_id: buffer_id.as_usize() as u64,
                    tokens: vec![],
                    start_line: 0,
                    end_line: 0,
                    full_refresh: true,
                    layer: "syntax".into(),
                    priority: 0,
                };

                Ok::<_, Status>((initial, rx))
            })
            .await?;

        let (tx, rx) = mpsc::channel(32);
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
    /// TODO(#753 E6): Route through DomainDriver for buffer file path.
    async fn get_language_info(
        &self,
        _request: Request<GetLanguageInfoRequest>,
    ) -> Result<Response<GetLanguageInfoResponse>, Status> {
        Err(Status::unimplemented(
            "GetLanguageInfo: pending domain driver wiring (#753 E6)",
        ))
    }
}

#[cfg(test)]
#[path = "syntax_tests.rs"]
mod tests;
