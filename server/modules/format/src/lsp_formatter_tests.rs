use lsp_types::{Position, Range, TextEdit};

use super::*;

// ============================================================================
// language_from_path tests
// ============================================================================

#[test]
fn test_language_from_path_rust() {
    assert_eq!(language_from_path("main.rs"), "rust");
}

#[test]
fn test_language_from_path_python() {
    assert_eq!(language_from_path("script.py"), "python");
}

#[test]
fn test_language_from_path_javascript() {
    assert_eq!(language_from_path("app.js"), "javascript");
    assert_eq!(language_from_path("component.jsx"), "javascript");
}

#[test]
fn test_language_from_path_typescript() {
    assert_eq!(language_from_path("app.ts"), "typescript");
    assert_eq!(language_from_path("component.tsx"), "typescript");
}

#[test]
fn test_language_from_path_c() {
    assert_eq!(language_from_path("main.c"), "c");
    assert_eq!(language_from_path("header.h"), "c");
}

#[test]
fn test_language_from_path_cpp() {
    assert_eq!(language_from_path("main.cpp"), "cpp");
    assert_eq!(language_from_path("header.hpp"), "cpp");
    assert_eq!(language_from_path("main.cc"), "cpp");
    assert_eq!(language_from_path("main.cxx"), "cpp");
}

#[test]
fn test_language_from_path_go() {
    assert_eq!(language_from_path("main.go"), "go");
}

#[test]
fn test_language_from_path_lua() {
    assert_eq!(language_from_path("init.lua"), "lua");
}

#[test]
fn test_language_from_path_json() {
    assert_eq!(language_from_path("config.json"), "json");
}

#[test]
fn test_language_from_path_toml() {
    assert_eq!(language_from_path("Cargo.toml"), "toml");
}

#[test]
fn test_language_from_path_markdown() {
    assert_eq!(language_from_path("README.md"), "markdown");
}

#[test]
fn test_language_from_path_unknown() {
    assert_eq!(language_from_path("Makefile"), "unknown");
    assert_eq!(language_from_path("no_extension"), "unknown");
}

// ============================================================================
// find_provider tests
// ============================================================================

#[test]
fn test_find_provider_no_registry() {
    let services = Arc::new(ServiceRegistry::new());
    assert!(find_provider(&services, "main.rs").is_none());
}

// ============================================================================
// has_formatting_capability tests
// ============================================================================

#[test]
fn test_has_formatting_no_capabilities() {
    use reovim_driver_text_lsp::{DiagnosticCache, LspRequest};

    struct NoCapProvider;
    impl LspProvider for NoCapProvider {
        fn send_request(&self, _: LspRequest) -> bool {
            false
        }
        fn diagnostics(&self) -> &DiagnosticCache {
            unimplemented!()
        }
        fn is_active(&self) -> bool {
            true
        }
        fn capabilities(&self) -> Option<Arc<lsp_types::ServerCapabilities>> {
            None
        }
        fn root_path(&self) -> &Path {
            Path::new("/")
        }
        #[allow(clippy::unnecessary_literal_bound)]
        fn language_id(&self) -> &str {
            "test"
        }
        fn server_info(&self) -> Option<&lsp_types::ServerInfo> {
            None
        }
    }

    assert!(!has_formatting_capability(&NoCapProvider));
}

#[test]
fn test_has_formatting_with_capability() {
    use reovim_driver_text_lsp::{DiagnosticCache, LspRequest};

    struct FmtProvider;
    impl LspProvider for FmtProvider {
        fn send_request(&self, _: LspRequest) -> bool {
            false
        }
        fn diagnostics(&self) -> &DiagnosticCache {
            unimplemented!()
        }
        fn is_active(&self) -> bool {
            true
        }
        fn capabilities(&self) -> Option<Arc<lsp_types::ServerCapabilities>> {
            Some(Arc::new(lsp_types::ServerCapabilities {
                document_formatting_provider: Some(lsp_types::OneOf::Left(true)),
                ..Default::default()
            }))
        }
        fn root_path(&self) -> &Path {
            Path::new("/")
        }
        #[allow(clippy::unnecessary_literal_bound)]
        fn language_id(&self) -> &str {
            "test"
        }
        fn server_info(&self) -> Option<&lsp_types::ServerInfo> {
            None
        }
    }

    assert!(has_formatting_capability(&FmtProvider));
}

// ============================================================================
// default_formatting_options tests
// ============================================================================

#[test]
fn test_default_formatting_options() {
    let opts = default_formatting_options();
    assert_eq!(opts.tab_size, 4);
    assert!(opts.insert_spaces);
}

// ============================================================================
// apply_text_edits tests
// ============================================================================

#[test]
fn test_apply_text_edits_empty() {
    let content = "hello world";
    let result = apply_text_edits(content, &[]);
    assert_eq!(result, "hello world");
}

#[test]
fn test_apply_text_edits_single_replace() {
    let content = "hello world";
    let edits = vec![TextEdit {
        range: Range::new(Position::new(0, 0), Position::new(0, 5)),
        new_text: "goodbye".to_string(),
    }];
    let result = apply_text_edits(content, &edits);
    assert_eq!(result, "goodbye world");
}

#[test]
fn test_apply_text_edits_insert() {
    let content = "hello world";
    let edits = vec![TextEdit {
        range: Range::new(Position::new(0, 5), Position::new(0, 5)),
        new_text: " beautiful".to_string(),
    }];
    let result = apply_text_edits(content, &edits);
    assert_eq!(result, "hello beautiful world");
}

#[test]
fn test_apply_text_edits_delete() {
    let content = "hello beautiful world";
    let edits = vec![TextEdit {
        range: Range::new(Position::new(0, 5), Position::new(0, 15)),
        new_text: String::new(),
    }];
    let result = apply_text_edits(content, &edits);
    assert_eq!(result, "hello world");
}

#[test]
fn test_apply_text_edits_multiline() {
    let content = "line1\nline2\nline3";
    let edits = vec![TextEdit {
        range: Range::new(Position::new(1, 0), Position::new(1, 5)),
        new_text: "replaced".to_string(),
    }];
    let result = apply_text_edits(content, &edits);
    assert_eq!(result, "line1\nreplaced\nline3");
}

#[test]
fn test_apply_text_edits_multiple_sorted() {
    let content = "aaa bbb ccc";
    let edits = vec![
        TextEdit {
            range: Range::new(Position::new(0, 0), Position::new(0, 3)),
            new_text: "AAA".to_string(),
        },
        TextEdit {
            range: Range::new(Position::new(0, 8), Position::new(0, 11)),
            new_text: "CCC".to_string(),
        },
    ];
    let result = apply_text_edits(content, &edits);
    assert_eq!(result, "AAA bbb CCC");
}

#[test]
fn test_apply_text_edits_cross_line_delete() {
    let content = "line1\nline2\nline3";
    let edits = vec![TextEdit {
        range: Range::new(Position::new(0, 5), Position::new(2, 0)),
        new_text: "\n".to_string(),
    }];
    let result = apply_text_edits(content, &edits);
    assert_eq!(result, "line1\nline3");
}

// ============================================================================
// offset_from_position tests
// ============================================================================

#[test]
fn test_offset_from_position_start() {
    let offset = offset_from_position("hello\nworld", Position::new(0, 0));
    assert_eq!(offset, 0);
}

#[test]
fn test_offset_from_position_mid_line() {
    let offset = offset_from_position("hello\nworld", Position::new(0, 3));
    assert_eq!(offset, 3);
}

#[test]
fn test_offset_from_position_second_line() {
    let offset = offset_from_position("hello\nworld", Position::new(1, 0));
    assert_eq!(offset, 6);
}

#[test]
fn test_offset_from_position_past_end() {
    let offset = offset_from_position("hello", Position::new(5, 0));
    assert_eq!(offset, 5);
}

#[test]
fn test_offset_from_position_char_past_line_end() {
    let offset = offset_from_position("hi\nworld", Position::new(0, 100));
    assert_eq!(offset, 2); // clamped to line length
}

// ============================================================================
// LspFormatter tests
// ============================================================================

#[test]
fn test_lsp_formatter_name() {
    let services = Arc::new(ServiceRegistry::new());
    let fmt = LspFormatter::new(services);
    assert_eq!(fmt.name(), "lsp");
}

#[test]
fn test_lsp_formatter_supports_range() {
    let services = Arc::new(ServiceRegistry::new());
    let fmt = LspFormatter::new(services);
    assert!(fmt.supports_range());
}
