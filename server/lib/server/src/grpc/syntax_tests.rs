use super::*;

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

/// Create a registry with a session that has a real buffer manager and
/// `TextBufferRegistry` so that `state.buffer()` can resolve buffers.
fn test_registry_with_buffer_manager() -> (Arc<SessionRegistry>, Arc<crate::session::Session>) {
    use {
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry},
    };

    let services = Arc::new(ServiceRegistry::new());
    services.register(Arc::new(reovim_driver_buffer::TextBufferRegistry::new()));
    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(OptionRegistry::new()),
        services,
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
        Annotation, HighlightCategory, SyntaxDriver, SyntaxDriverFactory, SyntaxEdit,
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

        fn highlights(&self, byte_range: Range<usize>) -> Vec<Annotation> {
            if self.parsed {
                vec![Annotation::new(
                    byte_range.start,
                    byte_range.end.min(100),
                    HighlightCategory::new("keyword"),
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

/// Create a registry with buffer manager, `TextBufferRegistry`, and syntax
/// factory pre-installed.
fn test_registry_with_syntax_factory() -> (Arc<SessionRegistry>, Arc<crate::session::Session>) {
    use {
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry},
    };

    let services = Arc::new(ServiceRegistry::new());
    services.register(Arc::new(reovim_driver_buffer::TextBufferRegistry::new()));
    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(OptionRegistry::new()),
        services,
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
                    category: span.category.to_string(),
                })
                .collect();
            let update = reovim_protocol::v2::TokenUpdate {
                buffer_id: buffer_id.as_usize() as u64,
                tokens,
                start_line: 0,
                end_line: 0,
                full_refresh: false,
                layer: "syntax".into(),
                priority: 0,
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

// =========================================================================
// Coverage for file-path-gated branches (lines 287, 362)
// =========================================================================

/// Cover line 287: `syntax_state.ensure_driver_from_path(...)` inside `get_tokens`.
///
/// This branch only runs when `buffer.file_path()` is `Some(...)`.  We create a
/// buffer, set its file path to a Rust source file, then call `get_tokens`.  The
/// factory in the test-with-syntax-factory session supports "text" and "rust",
/// so the path-based detection may load a driver even if the no-op factory
/// doesn't recognise the language — what matters is the branch is entered.
#[tokio::test]
async fn test_get_tokens_with_file_path_covers_ensure_driver_from_path() {
    let (registry, session) = test_registry_with_syntax_factory();

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("fn main() {}"))
        .await;

    // Set a file path on the buffer so the `if let Some(path) = &file_path` branch
    // is taken inside get_tokens (line 287).
    session
        .with_state_mut(|state| {
            if let Some(buf_arc) = state.buffer(buffer_id) {
                buf_arc.write().set_file_path(Some("main.rs".to_string()));
            }
        })
        .await;

    let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(GetTokensRequest {
        buffer_id: buffer_id.as_usize() as u64,
        start_line: None,
        end_line: None,
    });
    let response = service.get_tokens(request).await;
    assert!(response.is_ok());
    // Language should now be "rust" (detected from .rs path).
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.language_id, "rust");
}

/// Cover line 362: `syntax_state.ensure_driver_from_path(...)` inside `stream_tokens`.
///
/// Same idea as above but for the `stream_tokens` handler path.
#[tokio::test]
async fn test_stream_tokens_with_file_path_covers_ensure_driver_from_path() {
    let (registry, session) = test_registry_with_syntax_factory();

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("fn main() {}"))
        .await;

    // Attach a file path so the inner `if let Some(path) = &file_path` guard fires.
    session
        .with_state_mut(|state| {
            if let Some(buf_arc) = state.buffer(buffer_id) {
                buf_arc.write().set_file_path(Some("lib.rs".to_string()));
            }
        })
        .await;

    let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(StreamTokensRequest {
        buffer_id: buffer_id.as_usize() as u64,
    });
    let response = service.stream_tokens(request).await;
    assert!(response.is_ok());
}

// =========================================================================
// Coverage for registry-based language detection in get_language_info
// (lines 442-453)
// =========================================================================

/// Cover lines 442-453: the registry-based detection branch in `get_language_info`.
///
/// This branch requires:
///   1. `file_path` is `Some`
///   2. `syntax_state.detect_language(path)` returns `Some(lang_id)`
///   3. `syntax_state.registry()` returns `Some(registry)`
///   4. `registry.get_info(&lang_id)` returns `Some(info)`
///
/// When the test factory is installed but no language registry is configured,
/// conditions 2-4 are not all met, so the fall-through path is taken instead.
/// Without a real tree-sitter registry it is difficult to satisfy all four
/// conditions in a unit test.  We therefore verify the fall-through (plain-text)
/// path works correctly when a file path is set but no registry entry exists,
/// ensuring the surrounding code (lines 440-441, 455-462) is exercised, and we
/// accept that the `if let` chain on 442-453 evaluates its conditions (lines
/// 442-444 are entered even if the overall guard is false).
#[tokio::test]
async fn test_get_language_info_with_file_path_fallback() {
    let (registry, session) = test_registry_with_syntax_factory();

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("fn main() {}"))
        .await;

    // Set a .rs file path.  The test factory supports "rust" but there is no
    // language registry, so `detect_language` returns None and the fallback
    // (lines 455-462) is taken.  The file_path branch entry (line 441) is
    // still covered.
    session
        .with_state_mut(|state| {
            if let Some(buf_arc) = state.buffer(buffer_id) {
                buf_arc.write().set_file_path(Some("main.rs".to_string()));
            }
        })
        .await;

    let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(GetLanguageInfoRequest {
        buffer_id: buffer_id.as_usize() as u64,
    });
    let response = service.get_language_info(request).await;
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    // Hardcoded fallback: detected as "rust" from .rs extension.
    assert_eq!(resp.language_id, "rust");
    assert!(resp.has_parser, "test factory supports rust");
}

// =========================================================================
// Coverage: registry-based detection let-chain body (lines 443-453)
// =========================================================================

/// Cover lines 443-453: the `if let` chain body in `get_language_info` that
/// returns early with registry-derived language info.
///
/// Requires: `file_path` set, `detect_language` returns Some, `registry()` returns
/// Some, and `registry.get_info(lang_id)` returns Some(info).
#[tokio::test]
async fn test_get_language_info_via_registry_based_detection() {
    use {
        reovim_driver_syntax::{DefaultLanguageRegistry, LanguageInfo},
        std::sync::Arc,
    };

    let (registry, session) = test_registry_with_syntax_factory();

    let buffer_id = session
        .with_state_mut(|state| state.create_buffer("fn main() {}"))
        .await;

    // Set file path so detect_language can match it.
    session
        .with_state_mut(|state| {
            if let Some(buf_arc) = state.buffer(buffer_id) {
                buf_arc.write().set_file_path(Some("main.rs".to_string()));
            }
        })
        .await;

    // Install a DefaultLanguageRegistry that recognises "rust" from ".rs".
    session.with_state_mut_sync(|state| {
        let syntax_state = state
            .app
            .extensions
            .get_or_insert::<crate::session::SyntaxSessionState>();
        let lang_registry = Arc::new(DefaultLanguageRegistry::new(vec![
            LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
        ]));
        syntax_state.set_registry(lang_registry);
    });

    let service = SyntaxServiceImpl::new(registry, SessionId::new("test"));

    #[allow(clippy::cast_possible_truncation)]
    let request = Request::new(GetLanguageInfoRequest {
        buffer_id: buffer_id.as_usize() as u64,
    });
    let response = service.get_language_info(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();

    // Registry-based detection path: language_id and name from LanguageInfo.
    assert_eq!(resp.language_id, "rust");
    assert_eq!(resp.language_name, "Rust");
    // extensions from LanguageInfo: ".rs"
    assert!(
        resp.extensions.contains(&".rs".to_string()),
        "expected .rs in extensions, got {:?}",
        resp.extensions
    );
    // test factory supports rust
    assert!(resp.has_parser);
}
