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

use crate::session::{SessionId, SessionRegistry, SyntaxSessionState};

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
                    .or_else(|| state.active_buffer())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                let buffer_arc = state.buffer(buffer_id).ok_or_else(|| {
                    Status::not_found(format!("Buffer {} not found", buffer_id.as_usize()))
                })?;

                let buffer = buffer_arc.read();
                let total_lines = buffer.line_count();

                // Detect language from file path
                let (language_id, _language_name) = detect_language_from_path(buffer.file_path());

                // Get buffer content for byte range calculation
                let content = buffer.content();

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

                // Get or create driver for this buffer
                let tokens = if syntax_state.ensure_driver(buffer_id, language_id, &content) {
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
                let (language_id, _) = detect_language_from_path(buffer.file_path());

                // Drop buffer lock before accessing extensions
                drop(buffer);
                drop(buffer_arc);

                // Get syntax state and ensure driver exists (#491)
                let syntax_state = state.app.extensions.get_or_insert::<SyntaxSessionState>();

                // Ensure driver is created for this buffer
                syntax_state.ensure_driver(buffer_id, language_id, &content);

                // Subscribe to updates
                let rx = syntax_state.subscribe();

                // Get initial tokens
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
        tokio::spawn(async move {
            // Send initial full refresh
            if tx.send(Ok(initial_update)).await.is_err() {
                return; // Client disconnected
            }

            // Forward updates from syntax state
            let mut syntax_rx = syntax_rx;
            while let Some(update) = syntax_rx.recv().await {
                // Only forward updates for the requested buffer
                if update.buffer_id == buffer_id.as_usize() as u64
                    && tx.send(Ok(update)).await.is_err()
                {
                    break; // Client disconnected
                }
            }

            tracing::debug!(buffer_id = buffer_id.as_usize(), "StreamTokens stream ended");
            let _ = session_clone;
        });

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
                let (language_id, language_name) = detect_language_from_path(buffer.file_path());
                let extensions = extensions_for_language(language_id);

                // Drop buffer lock before accessing extensions
                drop(buffer);
                drop(buffer_arc);

                // Check if a parser is available via the factory (#491)
                let syntax_state = state.app.extensions.get_or_insert::<SyntaxSessionState>();
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
}
