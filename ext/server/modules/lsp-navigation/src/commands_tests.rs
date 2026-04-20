use std::sync::Arc;

use {lsp_types::LocationLink, reovim_driver_text_lsp::DiagnosticCache};

use super::*;

// ========================================================================
// Mock LspProvider for pure function tests
// ========================================================================

struct MockProvider {
    active: bool,
    capabilities: Option<lsp_types::ServerCapabilities>,
    accept_request: bool,
}

impl MockProvider {
    const fn active_with_caps(caps: lsp_types::ServerCapabilities) -> Self {
        Self {
            active: true,
            capabilities: Some(caps),
            accept_request: true,
        }
    }

    const fn inactive() -> Self {
        Self {
            active: false,
            capabilities: None,
            accept_request: false,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::unnecessary_literal_bound)]
impl LspProvider for MockProvider {
    fn send_request(&self, _request: LspRequest) -> bool {
        self.accept_request
    }

    fn diagnostics(&self) -> &DiagnosticCache {
        static CACHE: std::sync::OnceLock<DiagnosticCache> = std::sync::OnceLock::new();
        CACHE.get_or_init(DiagnosticCache::new)
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn capabilities(&self) -> Option<std::sync::Arc<lsp_types::ServerCapabilities>> {
        self.capabilities
            .as_ref()
            .map(|c| std::sync::Arc::new(c.clone()))
    }

    fn root_path(&self) -> &Path {
        Path::new("/mock")
    }

    fn language_id(&self) -> &str {
        "mock"
    }

    fn server_info(&self) -> Option<&lsp_types::ServerInfo> {
        None
    }
}

// ========================================================================
// Command metadata tests
// ========================================================================

#[test]
fn goto_definition_metadata() {
    use reovim_driver_command::Command;
    let cmd = GotoDefinition;
    assert_eq!(cmd.id(), ids::GOTO_DEFINITION);
    assert!(!cmd.description().is_empty());
}

#[test]
fn references_metadata() {
    use reovim_driver_command::Command;
    let cmd = References;
    assert_eq!(cmd.id(), ids::REFERENCES);
    assert!(!cmd.description().is_empty());
}

#[test]
fn command_handlers_unique_ids() {
    let handlers = command_handlers();
    let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
    // All IDs must be unique
    for (i, id) in ids.iter().enumerate() {
        for (j, other) in ids.iter().enumerate() {
            if i != j {
                assert_ne!(id, other, "duplicate command ID at {i} and {j}");
            }
        }
    }
}

// ========================================================================
// definition_to_locations tests
// ========================================================================

fn make_uri(path: &str) -> lsp_types::Uri {
    path.parse().expect("test URI should parse")
}

fn make_location(uri: &str, line: u32, col: u32) -> lsp_types::Location {
    lsp_types::Location {
        uri: make_uri(uri),
        range: lsp_types::Range {
            start: lsp_types::Position::new(line, col),
            end: lsp_types::Position::new(line, col + 1),
        },
    }
}

#[test]
fn definition_scalar() {
    let loc = make_location("file:///test.rs", 10, 5);
    let response = GotoDefinitionResponse::Scalar(loc.clone());
    let result = definition_to_locations(response);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].uri, loc.uri);
}

#[test]
fn definition_array() {
    let locs = vec![
        make_location("file:///a.rs", 1, 0),
        make_location("file:///b.rs", 2, 0),
    ];
    let response = GotoDefinitionResponse::Array(locs.clone());
    let result = definition_to_locations(response);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].uri, locs[0].uri);
    assert_eq!(result[1].uri, locs[1].uri);
}

#[test]
fn definition_link() {
    let link = LocationLink {
        origin_selection_range: None,
        target_uri: make_uri("file:///target.rs"),
        target_range: lsp_types::Range {
            start: lsp_types::Position::new(0, 0),
            end: lsp_types::Position::new(10, 0),
        },
        target_selection_range: lsp_types::Range {
            start: lsp_types::Position::new(5, 3),
            end: lsp_types::Position::new(5, 10),
        },
    };
    let response = GotoDefinitionResponse::Link(vec![link]);
    let result = definition_to_locations(response);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].range.start.line, 5);
    assert_eq!(result[0].range.start.character, 3);
}

#[test]
fn definition_empty_array() {
    let response = GotoDefinitionResponse::Array(vec![]);
    let result = definition_to_locations(response);
    assert!(result.is_empty());
}

#[test]
fn definition_empty_link() {
    let response = GotoDefinitionResponse::Link(vec![]);
    let result = definition_to_locations(response);
    assert!(result.is_empty());
}

// ========================================================================
// path_from_uri tests
// ========================================================================

#[test]
fn path_from_file_uri() {
    let uri = make_uri("file:///project/src/main.rs");
    let path = path_from_uri(&uri);
    assert_eq!(path, std::path::Path::new("/project/src/main.rs"));
}

#[test]
fn path_from_non_file_uri() {
    let uri = make_uri("https://example.com/file.rs");
    let path = path_from_uri(&uri);
    assert_eq!(path, std::path::PathBuf::new());
}

// ========================================================================
// location_to_picker_item tests
// ========================================================================

#[test]
fn location_to_item_basic() {
    let loc = make_location("file:///project/src/main.rs", 9, 4);
    let item = location_to_picker_item(&loc);
    assert_eq!(item.display, "main.rs:10:5");
    assert!(item.detail.is_some());
    assert!(matches!(
        &item.data,
        PickerData::GotoLocation {
            line: 10,
            col: 5,
            ..
        }
    ));
}

#[test]
fn location_to_item_zero_position() {
    let loc = make_location("file:///test.rs", 0, 0);
    let item = location_to_picker_item(&loc);
    assert_eq!(item.display, "test.rs:1:1");
}

// ========================================================================
// language_from_path tests
// ========================================================================

#[test]
fn language_rust() {
    assert_eq!(language_from_path("src/main.rs"), "rust");
}

#[test]
fn language_python() {
    assert_eq!(language_from_path("script.py"), "python");
}

#[test]
fn language_javascript() {
    assert_eq!(language_from_path("app.js"), "javascript");
    assert_eq!(language_from_path("component.jsx"), "javascript");
}

#[test]
fn language_typescript() {
    assert_eq!(language_from_path("app.ts"), "typescript");
    assert_eq!(language_from_path("component.tsx"), "typescript");
}

#[test]
fn language_c() {
    assert_eq!(language_from_path("main.c"), "c");
    assert_eq!(language_from_path("header.h"), "c");
}

#[test]
fn language_cpp() {
    assert_eq!(language_from_path("main.cpp"), "cpp");
    assert_eq!(language_from_path("header.hpp"), "cpp");
    assert_eq!(language_from_path("main.cc"), "cpp");
    assert_eq!(language_from_path("main.cxx"), "cpp");
}

#[test]
fn language_go() {
    assert_eq!(language_from_path("main.go"), "go");
}

#[test]
fn language_unknown() {
    assert_eq!(language_from_path("Makefile"), "unknown");
    assert_eq!(language_from_path("file.xyz"), "unknown");
}

#[test]
fn language_case_insensitive() {
    assert_eq!(language_from_path("main.RS"), "rust");
    assert_eq!(language_from_path("app.Js"), "javascript");
    assert_eq!(language_from_path("main.CPP"), "cpp");
}

// ========================================================================
// find_provider tests
// ========================================================================

#[test]
fn find_provider_with_language_match() {
    let services = Arc::new(ServiceRegistry::new());
    let registry = services.get_or_create::<LspProviderRegistry>();

    let caps = lsp_types::ServerCapabilities::default();
    let provider: Arc<dyn LspProvider> = Arc::new(MockProvider::active_with_caps(caps));
    registry.register(LspKey::Language("rust".to_owned()), provider);

    let result = find_provider(&services, "src/main.rs");
    assert!(result.is_some());
}

#[test]
fn find_provider_fallback_to_default() {
    let services = Arc::new(ServiceRegistry::new());
    let registry = services.get_or_create::<LspProviderRegistry>();

    let caps = lsp_types::ServerCapabilities::default();
    let provider: Arc<dyn LspProvider> = Arc::new(MockProvider::active_with_caps(caps));
    registry.register(LspKey::Default, provider);

    let result = find_provider(&services, "src/main.xyz");
    assert!(result.is_some());
}

#[test]
fn find_provider_inactive_returns_none() {
    let services = Arc::new(ServiceRegistry::new());
    let registry = services.get_or_create::<LspProviderRegistry>();

    let provider: Arc<dyn LspProvider> = Arc::new(MockProvider::inactive());
    registry.register(LspKey::Language("rust".to_owned()), provider);

    let result = find_provider(&services, "src/main.rs");
    assert!(result.is_none());
}

#[test]
fn find_provider_no_registry() {
    let services = Arc::new(ServiceRegistry::new());
    let result = find_provider(&services, "src/main.rs");
    assert!(result.is_none());
}

#[test]
fn find_provider_inactive_default() {
    let services = Arc::new(ServiceRegistry::new());
    let registry = services.get_or_create::<LspProviderRegistry>();

    let provider: Arc<dyn LspProvider> = Arc::new(MockProvider::inactive());
    registry.register(LspKey::Default, provider);

    let result = find_provider(&services, "src/main.xyz");
    assert!(result.is_none());
}

#[test]
fn find_provider_inactive_language_fallback_to_active_default() {
    let services = Arc::new(ServiceRegistry::new());
    let registry = services.get_or_create::<LspProviderRegistry>();

    let inactive: Arc<dyn LspProvider> = Arc::new(MockProvider::inactive());
    registry.register(LspKey::Language("rust".to_owned()), inactive);

    let caps = lsp_types::ServerCapabilities::default();
    let active: Arc<dyn LspProvider> = Arc::new(MockProvider::active_with_caps(caps));
    registry.register(LspKey::Default, active);

    let result = find_provider(&services, "src/main.rs");
    assert!(result.is_some());
}

// ========================================================================
// Capability check tests
// ========================================================================

#[test]
fn has_definition_capability_with_provider() {
    let caps = lsp_types::ServerCapabilities {
        definition_provider: Some(lsp_types::OneOf::Left(true)),
        ..Default::default()
    };
    let provider = MockProvider::active_with_caps(caps);
    assert!(has_definition_capability(&provider));
}

#[test]
fn has_definition_capability_without() {
    let caps = lsp_types::ServerCapabilities::default();
    let provider = MockProvider::active_with_caps(caps);
    assert!(!has_definition_capability(&provider));
}

#[test]
fn has_definition_capability_none_caps() {
    let provider = MockProvider::inactive();
    assert!(!has_definition_capability(&provider));
}

#[test]
fn has_references_capability_with_provider() {
    let caps = lsp_types::ServerCapabilities {
        references_provider: Some(lsp_types::OneOf::Left(true)),
        ..Default::default()
    };
    let provider = MockProvider::active_with_caps(caps);
    assert!(has_references_capability(&provider));
}

#[test]
fn has_references_capability_without() {
    let caps = lsp_types::ServerCapabilities::default();
    let provider = MockProvider::active_with_caps(caps);
    assert!(!has_references_capability(&provider));
}

#[test]
fn has_references_capability_none_caps() {
    let provider = MockProvider::inactive();
    assert!(!has_references_capability(&provider));
}

// ========================================================================
// Hover capability check tests
// ========================================================================

#[test]
fn has_hover_capability_with_provider() {
    let caps = lsp_types::ServerCapabilities {
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        ..Default::default()
    };
    let provider = MockProvider::active_with_caps(caps);
    assert!(has_hover_capability(&provider));
}

#[test]
fn has_hover_capability_without() {
    let caps = lsp_types::ServerCapabilities::default();
    let provider = MockProvider::active_with_caps(caps);
    assert!(!has_hover_capability(&provider));
}

#[test]
fn has_hover_capability_none_caps() {
    let provider = MockProvider::inactive();
    assert!(!has_hover_capability(&provider));
}

// ========================================================================
// Signature help capability check tests
// ========================================================================

#[test]
fn has_signature_help_capability_with_provider() {
    let caps = lsp_types::ServerCapabilities {
        signature_help_provider: Some(lsp_types::SignatureHelpOptions::default()),
        ..Default::default()
    };
    let provider = MockProvider::active_with_caps(caps);
    assert!(has_signature_help_capability(&provider));
}

#[test]
fn has_signature_help_capability_without() {
    let caps = lsp_types::ServerCapabilities::default();
    let provider = MockProvider::active_with_caps(caps);
    assert!(!has_signature_help_capability(&provider));
}

#[test]
fn has_signature_help_capability_none_caps() {
    let provider = MockProvider::inactive();
    assert!(!has_signature_help_capability(&provider));
}

// ========================================================================
// format_hover_content tests
// ========================================================================

#[test]
fn format_hover_scalar_string() {
    let hover = lsp_types::Hover {
        contents: HoverContents::Scalar(MarkedString::String("hello world".to_string())),
        range: None,
    };
    assert_eq!(format_hover_content(&hover), "hello world");
}

#[test]
fn format_hover_scalar_language_string() {
    let hover = lsp_types::Hover {
        contents: HoverContents::Scalar(MarkedString::LanguageString(lsp_types::LanguageString {
            language: "rust".to_string(),
            value: "fn main() {}".to_string(),
        })),
        range: None,
    };
    assert_eq!(format_hover_content(&hover), "```rust\nfn main() {}\n```");
}

#[test]
fn format_hover_array() {
    let hover = lsp_types::Hover {
        contents: HoverContents::Array(vec![
            MarkedString::String("Type: i32".to_string()),
            MarkedString::LanguageString(lsp_types::LanguageString {
                language: "rust".to_string(),
                value: "let x: i32".to_string(),
            }),
        ]),
        range: None,
    };
    let result = format_hover_content(&hover);
    assert!(result.contains("Type: i32"));
    assert!(result.contains("```rust\nlet x: i32\n```"));
}

#[test]
fn format_hover_markup() {
    let hover = lsp_types::Hover {
        contents: HoverContents::Markup(lsp_types::MarkupContent {
            kind: lsp_types::MarkupKind::Markdown,
            value: "## Documentation\nSome doc text".to_string(),
        }),
        range: None,
    };
    assert_eq!(format_hover_content(&hover), "## Documentation\nSome doc text");
}

#[test]
fn format_hover_empty_array() {
    let hover = lsp_types::Hover {
        contents: HoverContents::Array(vec![]),
        range: None,
    };
    assert!(format_hover_content(&hover).is_empty());
}

// ========================================================================
// hover_content_type tests
// ========================================================================

#[test]
fn hover_content_type_scalar_string() {
    let contents = HoverContents::Scalar(MarkedString::String("plain".into()));
    assert_eq!(hover_content_type(&contents), HoverContentType::PlainText);
}

#[test]
fn hover_content_type_scalar_language_string() {
    let contents = HoverContents::Scalar(MarkedString::LanguageString(lsp_types::LanguageString {
        language: "rust".to_string(),
        value: "fn main()".to_string(),
    }));
    assert_eq!(hover_content_type(&contents), HoverContentType::Markdown);
}

#[test]
fn hover_content_type_array() {
    let contents = HoverContents::Array(vec![]);
    assert_eq!(hover_content_type(&contents), HoverContentType::Markdown);
}

#[test]
fn hover_content_type_markup() {
    let contents = HoverContents::Markup(lsp_types::MarkupContent {
        kind: lsp_types::MarkupKind::Markdown,
        value: "# Title".to_string(),
    });
    assert_eq!(hover_content_type(&contents), HoverContentType::Markdown);
}

#[test]
fn hover_content_type_markup_plaintext() {
    let contents = HoverContents::Markup(lsp_types::MarkupContent {
        kind: lsp_types::MarkupKind::PlainText,
        value: "plain text content".to_string(),
    });
    assert_eq!(hover_content_type(&contents), HoverContentType::PlainText);
}

// ========================================================================
// format_signature_help tests
// ========================================================================

#[test]
fn format_signature_help_single() {
    let help = lsp_types::SignatureHelp {
        signatures: vec![lsp_types::SignatureInformation {
            label: "fn foo(x: i32, y: &str) -> bool".to_string(),
            documentation: None,
            parameters: None,
            active_parameter: None,
        }],
        active_signature: Some(0),
        active_parameter: None,
    };
    assert_eq!(format_signature_help(&help), "fn foo(x: i32, y: &str) -> bool");
}

#[test]
fn format_signature_help_multiple_active_second() {
    let help = lsp_types::SignatureHelp {
        signatures: vec![
            lsp_types::SignatureInformation {
                label: "fn bar(a: u8)".to_string(),
                documentation: None,
                parameters: None,
                active_parameter: None,
            },
            lsp_types::SignatureInformation {
                label: "fn bar(a: u8, b: u8)".to_string(),
                documentation: None,
                parameters: None,
                active_parameter: None,
            },
        ],
        active_signature: Some(1),
        active_parameter: None,
    };
    assert_eq!(format_signature_help(&help), "fn bar(a: u8, b: u8)");
}

#[test]
fn format_signature_help_no_active() {
    let help = lsp_types::SignatureHelp {
        signatures: vec![lsp_types::SignatureInformation {
            label: "fn default()".to_string(),
            documentation: None,
            parameters: None,
            active_parameter: None,
        }],
        active_signature: None,
        active_parameter: None,
    };
    assert_eq!(format_signature_help(&help), "fn default()");
}

#[test]
fn format_signature_help_empty_signatures() {
    let help = lsp_types::SignatureHelp {
        signatures: vec![],
        active_signature: None,
        active_parameter: None,
    };
    assert!(format_signature_help(&help).is_empty());
}

#[test]
fn format_signature_help_active_out_of_range() {
    let help = lsp_types::SignatureHelp {
        signatures: vec![lsp_types::SignatureInformation {
            label: "fn foo()".to_string(),
            documentation: None,
            parameters: None,
            active_parameter: None,
        }],
        active_signature: Some(5),
        active_parameter: None,
    };
    // OOB index returns empty string.
    assert!(format_signature_help(&help).is_empty());
}

// ========================================================================
// Command metadata tests (Hover + SignatureHelp)
// ========================================================================

#[test]
fn hover_command_metadata() {
    use reovim_driver_command::Command;
    let cmd = HoverCommand;
    assert_eq!(cmd.id(), ids::HOVER);
    assert!(!cmd.description().is_empty());
}

#[test]
fn signature_help_command_metadata() {
    use reovim_driver_command::Command;
    let cmd = SignatureHelpCommand;
    assert_eq!(cmd.id(), ids::SIGNATURE_HELP);
    assert!(!cmd.description().is_empty());
}

#[test]
fn command_handlers_count() {
    let handlers = command_handlers();
    assert_eq!(handlers.len(), 4);
}
