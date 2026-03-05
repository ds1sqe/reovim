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
    reovim_driver_syntax::{HighlightGroup, SyntaxHighlight},
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

/// Convert a `HighlightGroup` to its category string.
///
/// Uses the `SyntaxHighlight::category()` method from the syntax driver crate.
/// This ensures consistency between server and client token categorization.
///
/// # Example Categories
///
/// - `keyword`, `keyword.control`, `keyword.function`
/// - `function`, `function.builtin`, `function.macro`
/// - `variable`, `variable.builtin`, `variable.parameter`
/// - `string`, `string.escape`
/// - `comment`, `comment.doc`
///
/// # Note
///
/// Used by `GetTokens` and `StreamTokens` to convert highlight groups to client-facing categories.
#[must_use]
pub fn highlight_group_to_category(group: HighlightGroup) -> &'static str {
    group.category()
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

                // Get tokens from driver
                let tokens = if syntax_state.has_driver(buffer_id) {
                    syntax_state.get(buffer_id).map_or_else(Vec::new, |driver| {
                        // Get highlights from driver and convert to TokenSpan
                        driver
                            .highlights(byte_range)
                            .into_iter()
                            .map(|span| TokenSpan {
                                start_byte: span.start_byte as u32,
                                end_byte: span.end_byte as u32,
                                category: highlight_group_to_category(span.group).to_string(),
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
                    driver
                        .highlights(0..content.len())
                        .into_iter()
                        .map(|span| TokenSpan {
                            start_byte: span.start_byte as u32,
                            end_byte: span.end_byte as u32,
                            category: highlight_group_to_category(span.group).to_string(),
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
mod tests {
    use super::*;

    #[test]
    fn test_highlight_group_to_category() {
        // Keywords
        assert_eq!(highlight_group_to_category(HighlightGroup::Keyword), "keyword");
        assert_eq!(highlight_group_to_category(HighlightGroup::KeywordControl), "keyword.control");
        assert_eq!(
            highlight_group_to_category(HighlightGroup::KeywordFunction),
            "keyword.function"
        );

        // Functions
        assert_eq!(highlight_group_to_category(HighlightGroup::Function), "function");
        assert_eq!(
            highlight_group_to_category(HighlightGroup::FunctionBuiltin),
            "function.builtin"
        );
        assert_eq!(highlight_group_to_category(HighlightGroup::Method), "function.method");

        // Variables
        assert_eq!(highlight_group_to_category(HighlightGroup::Variable), "variable");
        assert_eq!(
            highlight_group_to_category(HighlightGroup::VariableBuiltin),
            "variable.builtin"
        );
        assert_eq!(highlight_group_to_category(HighlightGroup::Parameter), "variable.parameter");

        // Literals
        assert_eq!(highlight_group_to_category(HighlightGroup::String), "string");
        assert_eq!(highlight_group_to_category(HighlightGroup::StringEscape), "string.escape");
        assert_eq!(highlight_group_to_category(HighlightGroup::Number), "number");
        assert_eq!(highlight_group_to_category(HighlightGroup::Boolean), "boolean");

        // Comments
        assert_eq!(highlight_group_to_category(HighlightGroup::Comment), "comment");
        assert_eq!(highlight_group_to_category(HighlightGroup::CommentDoc), "comment.doc");

        // Types
        assert_eq!(highlight_group_to_category(HighlightGroup::Type), "type");
        assert_eq!(highlight_group_to_category(HighlightGroup::TypeBuiltin), "type.builtin");

        // Punctuation
        assert_eq!(highlight_group_to_category(HighlightGroup::Punctuation), "punctuation");
        assert_eq!(
            highlight_group_to_category(HighlightGroup::PunctuationBracket),
            "punctuation.bracket"
        );

        // Operators
        assert_eq!(highlight_group_to_category(HighlightGroup::Operator), "operator");

        // Diagnostics
        assert_eq!(highlight_group_to_category(HighlightGroup::Error), "diagnostic.error");
        assert_eq!(highlight_group_to_category(HighlightGroup::Warning), "diagnostic.warning");

        // Markup
        assert_eq!(highlight_group_to_category(HighlightGroup::MarkupHeading), "markup.heading");
        assert_eq!(highlight_group_to_category(HighlightGroup::MarkupBold), "markup.bold");

        // Special
        assert_eq!(highlight_group_to_category(HighlightGroup::Namespace), "namespace");
        assert_eq!(highlight_group_to_category(HighlightGroup::Attribute), "attribute");
        assert_eq!(highlight_group_to_category(HighlightGroup::Embedded), "embedded");
    }

    #[test]
    fn test_detect_language_from_path() {
        // Rust
        assert_eq!(detect_language_from_path(Some("src/main.rs")), ("rust", "Rust"));
        assert_eq!(detect_language_from_path(Some("lib.rs")), ("rust", "Rust"));

        // Python
        assert_eq!(detect_language_from_path(Some("script.py")), ("python", "Python"));
        assert_eq!(detect_language_from_path(Some("types.pyi")), ("python", "Python"));

        // JavaScript/TypeScript
        assert_eq!(detect_language_from_path(Some("app.js")), ("javascript", "JavaScript"));
        assert_eq!(detect_language_from_path(Some("app.ts")), ("typescript", "TypeScript"));
        assert_eq!(
            detect_language_from_path(Some("App.tsx")),
            ("typescriptreact", "TypeScript React")
        );

        // Config files
        assert_eq!(detect_language_from_path(Some("config.json")), ("json", "JSON"));
        assert_eq!(detect_language_from_path(Some("config.yaml")), ("yaml", "YAML"));
        assert_eq!(detect_language_from_path(Some("Cargo.toml")), ("toml", "TOML"));

        // Unknown
        assert_eq!(detect_language_from_path(Some("readme")), ("text", "Plain Text"));
        assert_eq!(detect_language_from_path(None), ("text", "Plain Text"));
    }

    #[test]
    fn test_extensions_for_language() {
        assert_eq!(extensions_for_language("rust"), vec![".rs"]);
        assert_eq!(extensions_for_language("python"), vec![".py", ".pyi"]);
        assert_eq!(extensions_for_language("typescript"), vec![".ts", ".mts", ".cts"]);
        assert!(extensions_for_language("unknown").is_empty());
    }

    #[tokio::test]
    async fn test_get_tokens_no_session() {
        let registry = Arc::new(SessionRegistry::new());
        let service = SyntaxServiceImpl::new(registry, SessionId::new("nonexistent"));

        let request = Request::new(GetTokensRequest {
            buffer_id: 0,
            start_line: None,
            end_line: None,
        });
        let response = service.get_tokens(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_get_language_info_no_session() {
        let registry = Arc::new(SessionRegistry::new());
        let service = SyntaxServiceImpl::new(registry, SessionId::new("nonexistent"));

        let request = Request::new(GetLanguageInfoRequest { buffer_id: 0 });
        let response = service.get_language_info(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    // ============================================================================
    // Comprehensive language detection tests
    // ============================================================================

    #[test]
    fn test_detect_language_c_family() {
        assert_eq!(detect_language_from_path(Some("main.c")), ("c", "C"));
        assert_eq!(detect_language_from_path(Some("main.cpp")), ("cpp", "C++"));
        assert_eq!(detect_language_from_path(Some("main.cc")), ("cpp", "C++"));
        assert_eq!(detect_language_from_path(Some("main.cxx")), ("cpp", "C++"));
        assert_eq!(detect_language_from_path(Some("main.hpp")), ("cpp", "C++"));
        assert_eq!(detect_language_from_path(Some("main.hxx")), ("cpp", "C++"));
        assert_eq!(detect_language_from_path(Some("header.h")), ("c", "C Header"));
    }

    #[test]
    fn test_detect_language_javascript_variants() {
        assert_eq!(detect_language_from_path(Some("app.mjs")), ("javascript", "JavaScript"));
        assert_eq!(detect_language_from_path(Some("app.cjs")), ("javascript", "JavaScript"));
        assert_eq!(
            detect_language_from_path(Some("App.jsx")),
            ("javascriptreact", "JavaScript React")
        );
    }

    #[test]
    fn test_detect_language_typescript_variants() {
        assert_eq!(detect_language_from_path(Some("app.mts")), ("typescript", "TypeScript"));
        assert_eq!(detect_language_from_path(Some("app.cts")), ("typescript", "TypeScript"));
    }

    #[test]
    fn test_detect_language_general_languages() {
        assert_eq!(detect_language_from_path(Some("main.go")), ("go", "Go"));
        assert_eq!(detect_language_from_path(Some("Main.java")), ("java", "Java"));
        assert_eq!(detect_language_from_path(Some("app.rb")), ("ruby", "Ruby"));
        assert_eq!(detect_language_from_path(Some("index.php")), ("php", "PHP"));
        assert_eq!(detect_language_from_path(Some("app.swift")), ("swift", "Swift"));
        assert_eq!(detect_language_from_path(Some("app.kt")), ("kotlin", "Kotlin"));
        assert_eq!(detect_language_from_path(Some("app.kts")), ("kotlin", "Kotlin"));
        assert_eq!(detect_language_from_path(Some("app.scala")), ("scala", "Scala"));
        assert_eq!(detect_language_from_path(Some("app.hs")), ("haskell", "Haskell"));
        assert_eq!(detect_language_from_path(Some("app.ml")), ("ocaml", "OCaml"));
        assert_eq!(detect_language_from_path(Some("app.mli")), ("ocaml", "OCaml"));
        assert_eq!(detect_language_from_path(Some("app.lua")), ("lua", "Lua"));
    }

    #[test]
    fn test_detect_language_shell_variants() {
        assert_eq!(detect_language_from_path(Some("script.sh")), ("shellscript", "Shell Script"));
        assert_eq!(detect_language_from_path(Some("script.bash")), ("shellscript", "Shell Script"));
        assert_eq!(detect_language_from_path(Some("config.zsh")), ("zsh", "Zsh"));
        assert_eq!(detect_language_from_path(Some("config.fish")), ("fish", "Fish"));
    }

    #[test]
    fn test_detect_language_web_styles() {
        assert_eq!(detect_language_from_path(Some("style.css")), ("css", "CSS"));
        assert_eq!(detect_language_from_path(Some("style.scss")), ("scss", "SCSS"));
        assert_eq!(detect_language_from_path(Some("style.less")), ("less", "Less"));
    }

    #[test]
    fn test_detect_language_markup_and_config() {
        assert_eq!(detect_language_from_path(Some("page.html")), ("html", "HTML"));
        assert_eq!(detect_language_from_path(Some("page.htm")), ("html", "HTML"));
        assert_eq!(detect_language_from_path(Some("data.xml")), ("xml", "XML"));
        assert_eq!(detect_language_from_path(Some("config.yml")), ("yaml", "YAML"));
        assert_eq!(detect_language_from_path(Some("doc.md")), ("markdown", "Markdown"));
        assert_eq!(detect_language_from_path(Some("doc.markdown")), ("markdown", "Markdown"));
        assert_eq!(detect_language_from_path(Some("query.sql")), ("sql", "SQL"));
        assert_eq!(detect_language_from_path(Some("init.vim")), ("vim", "Vimscript"));
    }

    #[test]
    fn test_detect_language_functional_languages() {
        assert_eq!(detect_language_from_path(Some("init.el")), ("lisp", "Lisp"));
        assert_eq!(detect_language_from_path(Some("app.lisp")), ("lisp", "Lisp"));
        assert_eq!(detect_language_from_path(Some("core.clj")), ("clojure", "Clojure"));
        assert_eq!(detect_language_from_path(Some("core.cljs")), ("clojure", "Clojure"));
        assert_eq!(detect_language_from_path(Some("app.ex")), ("elixir", "Elixir"));
        assert_eq!(detect_language_from_path(Some("app.exs")), ("elixir", "Elixir"));
        assert_eq!(detect_language_from_path(Some("app.erl")), ("erlang", "Erlang"));
    }

    #[test]
    fn test_detect_language_modern_languages() {
        assert_eq!(detect_language_from_path(Some("main.zig")), ("zig", "Zig"));
        assert_eq!(detect_language_from_path(Some("main.nim")), ("nim", "Nim"));
        assert_eq!(detect_language_from_path(Some("main.cr")), ("crystal", "Crystal"));
        assert_eq!(detect_language_from_path(Some("main.dart")), ("dart", "Dart"));
        assert_eq!(detect_language_from_path(Some("analysis.r")), ("r", "R"));
        assert_eq!(detect_language_from_path(Some("analysis.jl")), ("julia", "Julia"));
    }

    #[test]
    fn test_detect_language_data_and_build() {
        assert_eq!(detect_language_from_path(Some("api.proto")), ("protobuf", "Protocol Buffers"));
        assert_eq!(detect_language_from_path(Some("schema.graphql")), ("graphql", "GraphQL"));
        assert_eq!(detect_language_from_path(Some("schema.gql")), ("graphql", "GraphQL"));
        assert_eq!(detect_language_from_path(Some("dockerfile")), ("dockerfile", "Dockerfile"));
        assert_eq!(detect_language_from_path(Some("build.make")), ("makefile", "Makefile"));
        assert_eq!(detect_language_from_path(Some("build.makefile")), ("makefile", "Makefile"));
        assert_eq!(detect_language_from_path(Some("build.cmake")), ("cmake", "CMake"));
        assert_eq!(detect_language_from_path(Some("readme.txt")), ("plaintext", "Plain Text"));
    }

    // ============================================================================
    // Comprehensive extensions_for_language tests
    // ============================================================================

    #[test]
    fn test_extensions_for_all_languages() {
        // Test all supported languages have non-empty extensions
        let languages_with_expected_extensions = [
            ("rust", vec![".rs"]),
            ("c", vec![".c", ".h"]),
            ("cpp", vec![".cpp", ".cc", ".cxx", ".hpp"]),
            ("go", vec![".go"]),
            ("java", vec![".java"]),
            ("ruby", vec![".rb"]),
            ("php", vec![".php"]),
            ("swift", vec![".swift"]),
            ("kotlin", vec![".kt", ".kts"]),
            ("scala", vec![".scala"]),
            ("haskell", vec![".hs"]),
            ("ocaml", vec![".ml", ".mli"]),
            ("lua", vec![".lua"]),
            ("shellscript", vec![".sh", ".bash"]),
            ("zsh", vec![".zsh"]),
            ("fish", vec![".fish"]),
            ("css", vec![".css"]),
            ("scss", vec![".scss"]),
            ("less", vec![".less"]),
            ("html", vec![".html", ".htm"]),
            ("xml", vec![".xml"]),
            ("json", vec![".json"]),
            ("yaml", vec![".yaml", ".yml"]),
            ("toml", vec![".toml"]),
            ("markdown", vec![".md", ".markdown"]),
            ("sql", vec![".sql"]),
            ("vim", vec![".vim"]),
            ("lisp", vec![".el", ".lisp"]),
            ("clojure", vec![".clj", ".cljs"]),
            ("elixir", vec![".ex", ".exs"]),
            ("erlang", vec![".erl"]),
            ("zig", vec![".zig"]),
            ("nim", vec![".nim"]),
            ("crystal", vec![".cr"]),
            ("dart", vec![".dart"]),
            ("r", vec![".r"]),
            ("julia", vec![".jl"]),
            ("protobuf", vec![".proto"]),
            ("graphql", vec![".graphql", ".gql"]),
            ("typescriptreact", vec![".tsx"]),
            ("javascriptreact", vec![".jsx"]),
        ];

        for (lang_id, expected) in &languages_with_expected_extensions {
            let exts = extensions_for_language(lang_id);
            let expected_strings: Vec<String> = expected.iter().map(|s| (*s).to_string()).collect();
            assert_eq!(exts, expected_strings, "Extensions mismatch for language {lang_id}");
        }
    }

    #[test]
    fn test_extensions_for_build_languages() {
        let docker_exts = extensions_for_language("dockerfile");
        assert!(docker_exts.contains(&"Dockerfile".to_string()));

        let make_exts = extensions_for_language("makefile");
        assert!(make_exts.contains(&"Makefile".to_string()));

        let cmake_exts = extensions_for_language("cmake");
        assert!(cmake_exts.contains(&"CMakeLists.txt".to_string()));
    }

    #[test]
    fn test_highlight_group_additional_groups() {
        // Test remaining highlight groups for completeness
        assert_eq!(highlight_group_to_category(HighlightGroup::FunctionMacro), "function.macro");
        assert_eq!(highlight_group_to_category(HighlightGroup::MarkupItalic), "markup.italic");
        assert_eq!(highlight_group_to_category(HighlightGroup::MarkupLink), "markup.link");
        assert_eq!(
            highlight_group_to_category(HighlightGroup::PunctuationDelimiter),
            "punctuation.delimiter"
        );
        assert_eq!(highlight_group_to_category(HighlightGroup::Character), "character");
        assert_eq!(highlight_group_to_category(HighlightGroup::Constant), "constant");
    }

    #[tokio::test]
    async fn test_stream_tokens_no_session() {
        let registry = Arc::new(SessionRegistry::new());
        let service = SyntaxServiceImpl::new(registry, SessionId::new("nonexistent"));

        let request = Request::new(StreamTokensRequest { buffer_id: 1 });
        let response = service.stream_tokens(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[test]
    fn test_syntax_service_new() {
        let registry = Arc::new(SessionRegistry::new());
        let service = SyntaxServiceImpl::new(Arc::clone(&registry), SessionId::new("test"));
        // Verify it was constructed without panic
        let _ = service;
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_syntax_service_get_session_not_found() {
        let registry = Arc::new(SessionRegistry::new());
        let service = SyntaxServiceImpl::new(registry, SessionId::new("nonexistent"));

        let result = service.get_session();
        match result {
            Err(status) => assert_eq!(status.code(), tonic::Code::NotFound),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    #[test]
    fn test_syntax_service_get_session_found() {
        let registry = Arc::new(SessionRegistry::new());
        let session = std::sync::Arc::new(crate::session::Session::new(SessionId::new("test")));
        registry.insert(&session);
        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        let result = service.get_session();
        assert!(result.is_ok());
    }

    /// Create a registry with a session that has a real buffer manager.
    fn test_registry_with_buffer_manager() -> (Arc<SessionRegistry>, Arc<crate::session::Session>) {
        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
                TextObjectEngine,
            },
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let state = crate::session::SessionState::with_kernel(kernel);
        let session = Arc::new(crate::session::Session::from_state(SessionId::new("test"), state));

        let registry = Arc::new(SessionRegistry::new());
        registry.insert(&session);

        (registry, session)
    }

    #[tokio::test]
    async fn test_get_tokens_with_buffer() {
        let (registry, session) = test_registry_with_buffer_manager();

        session
            .with_state_mut(|state| {
                state.create_buffer("fn main() {\n    println!(\"hello\");\n}");
            })
            .await;

        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetTokensRequest {
            buffer_id: 0, // active buffer
            start_line: None,
            end_line: None,
        });
        let response = service.get_tokens(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.total_lines, 3);
        // Language detected as "text" since no file path set
        assert_eq!(resp.language_id, "text");
    }

    #[tokio::test]
    async fn test_get_tokens_with_specific_buffer_id() {
        let (registry, session) = test_registry_with_buffer_manager();

        let buffer_id = session
            .with_state_mut(|state| state.create_buffer("line1\nline2"))
            .await;

        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        #[allow(clippy::cast_possible_truncation)]
        let request = Request::new(GetTokensRequest {
            buffer_id: buffer_id.as_usize() as u64,
            start_line: Some(0),
            end_line: Some(1),
        });
        let response = service.get_tokens(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.total_lines, 2);
    }

    #[tokio::test]
    async fn test_get_tokens_no_buffer() {
        let (registry, _session) = test_registry_with_buffer_manager();
        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetTokensRequest {
            buffer_id: 0,
            start_line: None,
            end_line: None,
        });
        let response = service.get_tokens(request).await;

        // No buffer exists -> NotFound
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_get_tokens_nonexistent_buffer_id() {
        let (registry, session) = test_registry_with_buffer_manager();

        // Create a buffer so there's an active buffer
        session
            .with_state_mut(|state| {
                state.create_buffer("test");
            })
            .await;

        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetTokensRequest {
            buffer_id: 999, // nonexistent specific buffer
            start_line: None,
            end_line: None,
        });
        let response = service.get_tokens(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_get_language_info_with_buffer() {
        let (registry, session) = test_registry_with_buffer_manager();

        let buffer_id = session
            .with_state_mut(|state| state.create_buffer("hello"))
            .await;

        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        #[allow(clippy::cast_possible_truncation)]
        let request = Request::new(GetLanguageInfoRequest {
            buffer_id: buffer_id.as_usize() as u64,
        });
        let response = service.get_language_info(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        // No file path, so plain text
        assert_eq!(resp.language_id, "text");
        assert_eq!(resp.language_name, "Plain Text");
        assert!(!resp.has_parser);
    }

    #[tokio::test]
    async fn test_get_language_info_nonexistent_buffer() {
        let (registry, _session) = test_registry_with_buffer_manager();
        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetLanguageInfoRequest { buffer_id: 999 });
        let response = service.get_language_info(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_stream_tokens_with_buffer() {
        let (registry, session) = test_registry_with_buffer_manager();

        let buffer_id = session
            .with_state_mut(|state| state.create_buffer("fn main() {}"))
            .await;

        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        #[allow(clippy::cast_possible_truncation)]
        let request = Request::new(StreamTokensRequest {
            buffer_id: buffer_id.as_usize() as u64,
        });
        let response = service.stream_tokens(request).await;

        assert!(response.is_ok());
        // The response is a stream - we just verify it was created successfully
    }

    // =========================================================================
    // Coverage: Syntax driver paths with factory (#497)
    // =========================================================================

    /// Create a test `SyntaxDriverFactory` and `SyntaxDriver` for testing
    /// the paths where drivers produce actual highlights.
    mod test_syntax_driver {
        use std::{ops::Range, sync::Arc};

        use reovim_driver_syntax::{
            HighlightGroup, HighlightSpan, SyntaxDriver, SyntaxDriverFactory, SyntaxEdit,
        };

        /// A minimal test driver for unit tests.
        pub struct TestDriver {
            language: String,
            parsed: bool,
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        impl TestDriver {
            pub fn new(language: &str) -> Self {
                Self {
                    language: language.to_string(),
                    parsed: false,
                }
            }
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        impl SyntaxDriver for TestDriver {
            fn language(&self) -> &str {
                &self.language
            }

            fn parse(&mut self, _content: &str) {
                self.parsed = true;
            }

            fn update(&mut self, _content: &str, _edit: &SyntaxEdit) {
                // No-op for tests
            }

            fn highlights(&self, byte_range: Range<usize>) -> Vec<HighlightSpan> {
                if self.parsed {
                    vec![HighlightSpan::new(
                        byte_range.start,
                        byte_range.end.min(100),
                        HighlightGroup::Keyword,
                    )]
                } else {
                    Vec::new()
                }
            }

            fn is_parsed(&self) -> bool {
                self.parsed
            }
        }

        /// A minimal test factory.
        pub struct TestFactory;

        #[cfg_attr(coverage_nightly, coverage(off))]
        impl SyntaxDriverFactory for TestFactory {
            fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
                // Only support "text" (the default for buffers without a file path)
                if language_id == "text" || language_id == "rust" {
                    Some(Box::new(TestDriver::new(language_id)))
                } else {
                    None
                }
            }

            fn supported_languages(&self) -> Vec<&str> {
                vec!["text", "rust"]
            }

            fn supports(&self, language_id: &str) -> bool {
                language_id == "text" || language_id == "rust"
            }
        }

        /// Create an Arc factory.
        pub fn test_factory() -> Arc<dyn SyntaxDriverFactory> {
            Arc::new(TestFactory)
        }
    }

    /// Create a registry with buffer manager and syntax factory pre-installed.
    fn test_registry_with_syntax_factory() -> (Arc<SessionRegistry>, Arc<crate::session::Session>) {
        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
                TextObjectEngine,
            },
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let state = crate::session::SessionState::with_kernel(kernel);
        let session = Arc::new(crate::session::Session::from_state(SessionId::new("test"), state));

        // Pre-install the syntax factory into the session's SyntaxSessionState
        session.with_state_mut_sync(|state| {
            let syntax_state = state
                .app
                .extensions
                .get_or_insert::<crate::session::SyntaxSessionState>();
            syntax_state.set_factory(test_syntax_driver::test_factory());
        });

        let registry = Arc::new(SessionRegistry::new());
        registry.insert(&session);

        (registry, session)
    }

    #[tokio::test]
    async fn test_get_tokens_with_syntax_driver_producing_highlights() {
        // This covers lines 281-293: the path where ensure_driver returns true
        // and the driver produces actual highlights.
        let (registry, session) = test_registry_with_syntax_factory();

        session
            .with_state_mut(|state| {
                state.create_buffer("fn main() {}");
            })
            .await;

        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetTokensRequest {
            buffer_id: 0, // active buffer
            start_line: None,
            end_line: None,
        });
        let response = service.get_tokens(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.total_lines, 1);
        // The TestDriver returns highlights when parsed
        assert!(!resp.tokens.is_empty());
        assert_eq!(resp.tokens[0].category, "keyword");
    }

    #[tokio::test]
    async fn test_get_tokens_end_line_beyond_total_lines() {
        // This covers line 269: the path where end_line >= total_lines,
        // causing `content.len()` to be used as end_byte.
        let (registry, session) = test_registry_with_syntax_factory();

        session
            .with_state_mut(|state| {
                state.create_buffer("line one\nline two");
            })
            .await;

        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetTokensRequest {
            buffer_id: 0,
            start_line: Some(0),
            end_line: Some(999), // Way beyond total lines (2)
        });
        let response = service.get_tokens(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.total_lines, 2);
    }

    #[tokio::test]
    async fn test_stream_tokens_with_syntax_factory_and_initial_tokens() {
        // This covers lines 354-363: the stream_tokens path where the driver
        // produces initial highlights for the stream.
        use tokio_stream::StreamExt;

        let (registry, session) = test_registry_with_syntax_factory();

        let buffer_id = session
            .with_state_mut(|state| state.create_buffer("fn main() {}"))
            .await;

        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        #[allow(clippy::cast_possible_truncation)]
        let request = Request::new(StreamTokensRequest {
            buffer_id: buffer_id.as_usize() as u64,
        });
        let response = service.stream_tokens(request).await;

        assert!(response.is_ok());
        let mut stream = response.unwrap().into_inner();

        // Read the initial full refresh from the stream
        let first_update = stream.next().await;
        assert!(first_update.is_some());
        let update = first_update.unwrap().unwrap();
        assert!(update.full_refresh);
        assert!(!update.tokens.is_empty());
        assert_eq!(update.tokens[0].category, "keyword");
    }

    #[tokio::test]
    async fn test_stream_tokens_spawned_task_initial_send() {
        // This covers lines 381-385: the spawned task sending the initial update
        // and lines 398-400: the debug log and session_clone drop.
        use tokio_stream::StreamExt;

        let (registry, session) = test_registry_with_syntax_factory();

        let buffer_id = session
            .with_state_mut(|state| state.create_buffer("test"))
            .await;

        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        #[allow(clippy::cast_possible_truncation)]
        let request = Request::new(StreamTokensRequest {
            buffer_id: buffer_id.as_usize() as u64,
        });
        let response = service.stream_tokens(request).await.unwrap();
        let mut stream = response.into_inner();

        // Receive initial update
        let update = stream.next().await.unwrap().unwrap();
        assert!(update.full_refresh);

        // Drop the stream to trigger the "client disconnected" path (lines 383-384, 392-394)
        drop(stream);
        // Give the spawned task time to detect the disconnect
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_get_tokens_empty_buffer_hits_content_len() {
        // This covers line 269: the else branch where end_line >= total_lines.
        // With an empty buffer (0 lines), end_line = 0 and total_lines = 0,
        // so end_line < total_lines is false, and content.len() is used.
        let (registry, session) = test_registry_with_syntax_factory();

        session
            .with_state_mut(|state| {
                state.create_buffer(""); // Empty content -> 0 lines
            })
            .await;

        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetTokensRequest {
            buffer_id: 0,
            start_line: None,
            end_line: None,
        });
        let response = service.get_tokens(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.total_lines, 0);
    }

    #[tokio::test]
    async fn test_stream_tokens_forwarding_loop() {
        // This covers lines 388-395: the forwarding loop in the spawned task
        // that receives TokenUpdate messages from syntax_rx and forwards them
        // through the gRPC stream, filtering by buffer_id.
        use {reovim_driver_syntax::SyntaxEdit, tokio_stream::StreamExt};

        let (registry, session) = test_registry_with_syntax_factory();

        let buffer_id = session
            .with_state_mut(|state| state.create_buffer("fn main() {}"))
            .await;

        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        #[allow(clippy::cast_possible_truncation)]
        let request = Request::new(StreamTokensRequest {
            buffer_id: buffer_id.as_usize() as u64,
        });
        let response = service.stream_tokens(request).await.unwrap();
        let mut stream = response.into_inner();

        // Consume the initial full refresh
        let first = stream.next().await.unwrap().unwrap();
        assert!(first.full_refresh);

        // Now trigger a notify_edit on the syntax state, which sends an update
        // through the subscriber channel that the spawned task is listening on.
        session
            .with_state_mut(|state| {
                let edit = SyntaxEdit::insert(0, 0, 0, 3, 0, 3);
                let syntax_state = state
                    .app
                    .extensions
                    .get_or_insert::<crate::session::SyntaxSessionState>();
                let content = "fn main() { let x = 1; }";
                syntax_state
                    .get_mut(buffer_id)
                    .unwrap()
                    .update(content, &edit);
                // Collect tokens while we still have the syntax state borrow
                let highlights = syntax_state
                    .get(buffer_id)
                    .unwrap()
                    .highlights(0..content.len());
                #[allow(clippy::cast_possible_truncation)]
                let tokens: Vec<reovim_protocol::v2::TokenSpan> = highlights
                    .into_iter()
                    .map(|span| reovim_protocol::v2::TokenSpan {
                        start_byte: span.start_byte as u32,
                        end_byte: span.end_byte as u32,
                        category: span.group.category().to_string(),
                    })
                    .collect();
                let update = reovim_protocol::v2::TokenUpdate {
                    buffer_id: buffer_id.as_usize() as u64,
                    tokens,
                    start_line: 0,
                    end_line: 0,
                    full_refresh: false,
                };
                // Broadcast via stream state
                let stream_state = state
                    .app
                    .extensions
                    .get_or_insert::<crate::session::SyntaxStreamState>();
                stream_state.broadcast(&update);
            })
            .await;

        // The forwarding loop should pick up the update and forward it
        let update = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
            .await
            .expect("Timed out waiting for forwarded update")
            .unwrap()
            .unwrap();
        assert!(!update.full_refresh);
        #[allow(clippy::cast_possible_truncation)]
        {
            assert_eq!(update.buffer_id, buffer_id.as_usize() as u64);
        }

        // Drop stream to end the task
        drop(stream);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_stream_tokens_nonexistent_buffer() {
        let (registry, _session) = test_registry_with_buffer_manager();
        let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(StreamTokensRequest { buffer_id: 999 });
        let response = service.stream_tokens(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }
}
