use {
    super::*,
    reovim_driver_lsp::{DiagnosticCache, LspProvider, LspRequest, lsp_types::ServerInfo},
    reovim_driver_module_loader::report::ModuleLoadReport,
    reovim_kernel::api::v1::{ModuleId, OptionSpec, OptionValue},
    std::{
        path::{Path, PathBuf},
        sync::Arc,
    },
};

// ========================================================================
// Status display
// ========================================================================

#[test]
fn test_status_display() {
    assert_eq!(format!("{}", Status::Ok), "[OK]");
    assert_eq!(format!("{}", Status::Warning), "[!!]");
    assert_eq!(format!("{}", Status::Info), "[--]");
}

// ========================================================================
// DiagnosticEntry
// ========================================================================

#[test]
fn test_entry_new() {
    let entry = DiagnosticEntry::new(Status::Ok, "label", "detail");
    assert_eq!(entry.status, Status::Ok);
    assert_eq!(entry.label, "label");
    assert_eq!(entry.detail, "detail");
}

#[test]
fn test_entry_clone() {
    let entry = DiagnosticEntry::new(Status::Warning, "test", "data");
    let cloned = entry.clone();
    assert_eq!(entry, cloned);
}

// ========================================================================
// DiagnosticSection
// ========================================================================

#[test]
fn test_section_new() {
    let section = DiagnosticSection::new("Title");
    assert_eq!(section.title, "Title");
    assert!(section.entries.is_empty());
}

#[test]
fn test_section_clone() {
    let mut section = DiagnosticSection::new("Test");
    section
        .entries
        .push(DiagnosticEntry::new(Status::Ok, "a", "b"));
    let cloned = section.clone();
    assert_eq!(section, cloned);
}

// ========================================================================
// collect_system
// ========================================================================

#[test]
fn test_collect_system() {
    let section = collect_system();
    assert_eq!(section.title, "System");
    assert_eq!(section.entries.len(), 3);

    assert_eq!(section.entries[0].label, "reovim version");
    assert_eq!(section.entries[0].status, Status::Ok);

    assert_eq!(section.entries[1].label, "API version");
    assert_eq!(section.entries[1].detail, API_VERSION_STR);

    assert_eq!(section.entries[2].label, "Platform");
}

// ========================================================================
// collect_lsp - empty registry
// ========================================================================

fn test_kernel() -> KernelContext {
    KernelContext::default()
}

#[test]
fn test_collect_lsp_no_registry() {
    let kernel = test_kernel();
    let section = collect_lsp(&kernel);
    assert_eq!(section.title, "Language Servers");
    assert_eq!(section.entries.len(), 1);
    assert_eq!(section.entries[0].status, Status::Info);
    assert!(section.entries[0].detail.contains("no providers"));
}

#[test]
fn test_collect_lsp_empty_registry() {
    let kernel = test_kernel();
    // Create an empty registry
    kernel
        .services
        .register(std::sync::Arc::new(LspProviderRegistry::new()));
    let section = collect_lsp(&kernel);
    assert_eq!(section.entries.len(), 1);
    assert!(section.entries[0].detail.contains("no providers"));
}

// ========================================================================
// collect_lsp - multiple servers
// ========================================================================

/// Mock LSP provider for testing diagnostics with registered providers.
struct MockLspProvider {
    active: bool,
    info: Option<ServerInfo>,
    cache: DiagnosticCache,
}

impl MockLspProvider {
    fn new(active: bool, info: Option<ServerInfo>) -> Self {
        Self {
            active,
            info,
            cache: DiagnosticCache::new(),
        }
    }
}

impl LspProvider for MockLspProvider {
    fn send_request(&self, _request: LspRequest) -> bool {
        false
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn capabilities(&self) -> Option<Arc<reovim_driver_lsp::lsp_types::ServerCapabilities>> {
        None
    }

    fn diagnostics(&self) -> &DiagnosticCache {
        &self.cache
    }

    fn root_path(&self) -> &Path {
        Path::new("/tmp")
    }

    #[allow(clippy::unnecessary_literal_bound)]
    fn language_id(&self) -> &str {
        "test"
    }

    fn server_info(&self) -> Option<&ServerInfo> {
        self.info.as_ref()
    }
}

#[test]
fn test_collect_lsp_single_active_server() {
    let kernel = test_kernel();
    let registry = LspProviderRegistry::new();
    registry.register(
        LspKey::Default,
        Arc::new(MockLspProvider::new(
            true,
            Some(ServerInfo {
                name: "rust-analyzer".to_string(),
                version: Some("1.0.0".to_string()),
            }),
        )),
    );
    kernel.services.register(Arc::new(registry));

    let section = collect_lsp(&kernel);
    assert_eq!(section.entries.len(), 1);
    assert_eq!(section.entries[0].status, Status::Ok);
    assert!(section.entries[0].detail.contains("rust-analyzer"));
    assert!(section.entries[0].detail.contains("1.0.0"));
    assert!(section.entries[0].detail.contains("active"));
}

#[test]
fn test_collect_lsp_multiple_servers() {
    let kernel = test_kernel();
    let registry = LspProviderRegistry::new();

    // Register a default server
    registry.register(
        LspKey::Default,
        Arc::new(MockLspProvider::new(
            true,
            Some(ServerInfo {
                name: "generic-lsp".to_string(),
                version: Some("2.0.0".to_string()),
            }),
        )),
    );

    // Register a per-language server
    registry.register(
        LspKey::Language("rust".to_string()),
        Arc::new(MockLspProvider::new(
            true,
            Some(ServerInfo {
                name: "rust-analyzer".to_string(),
                version: Some("1.5.0".to_string()),
            }),
        )),
    );

    // Register an inactive per-language server
    registry.register(
        LspKey::Language("python".to_string()),
        Arc::new(MockLspProvider::new(
            false,
            Some(ServerInfo {
                name: "pyright".to_string(),
                version: None,
            }),
        )),
    );

    kernel.services.register(Arc::new(registry));

    let section = collect_lsp(&kernel);
    assert_eq!(section.entries.len(), 3);

    // Verify counts (order may vary due to HashMap keys)
    let active_count = section
        .entries
        .iter()
        .filter(|e| e.status == Status::Ok)
        .count();
    let warning_count = section
        .entries
        .iter()
        .filter(|e| e.status == Status::Warning)
        .count();

    assert_eq!(active_count, 2);
    assert_eq!(warning_count, 1);

    // The inactive server should be a warning
    let inactive = section
        .entries
        .iter()
        .find(|e| e.status == Status::Warning)
        .expect("should have a warning entry");
    assert!(inactive.detail.contains("pyright"));
    assert!(inactive.detail.contains("inactive"));
    assert!(inactive.detail.contains("unknown")); // version is None
}

#[test]
fn test_collect_lsp_server_with_no_info() {
    let kernel = test_kernel();
    let registry = LspProviderRegistry::new();
    registry.register(LspKey::Default, Arc::new(MockLspProvider::new(false, None)));
    kernel.services.register(Arc::new(registry));

    let section = collect_lsp(&kernel);
    assert_eq!(section.entries.len(), 1);
    assert_eq!(section.entries[0].status, Status::Warning);
    assert!(section.entries[0].detail.contains("inactive"));
    assert!(section.entries[0].detail.contains("no server info"));
}

// ========================================================================
// collect_treesitter
// ========================================================================

#[test]
fn test_collect_treesitter_no_stores() {
    let kernel = test_kernel();
    let section = collect_treesitter(&kernel);
    assert_eq!(section.title, "Syntax Highlighting");
    assert_eq!(section.entries.len(), 1);
    assert!(section.entries[0].detail.contains("no grammars"));
}

#[test]
fn test_collect_treesitter_empty_stores() {
    let kernel = test_kernel();
    kernel
        .services
        .register(std::sync::Arc::new(SyntaxFactoryStore::new()));
    kernel
        .services
        .register(std::sync::Arc::new(LanguageInfoStore::new()));
    let section = collect_treesitter(&kernel);
    assert_eq!(section.entries.len(), 1);
    assert!(section.entries[0].detail.contains("no grammars"));
}

// ========================================================================
// collect_clipboard
// ========================================================================

#[test]
fn test_collect_clipboard_no_registry() {
    let kernel = test_kernel();
    let section = collect_clipboard(&kernel);
    assert_eq!(section.title, "Clipboard");
    assert_eq!(section.entries.len(), 1);
    assert_eq!(section.entries[0].status, Status::Warning);
    assert!(section.entries[0].detail.contains("no providers"));
}

#[test]
fn test_collect_clipboard_empty_registry() {
    let kernel = test_kernel();
    kernel
        .services
        .register(std::sync::Arc::new(ClipboardProviderRegistry::new()));
    let section = collect_clipboard(&kernel);
    assert_eq!(section.entries.len(), 1);
    assert!(
        section.entries[0]
            .detail
            .contains("default provider not found")
    );
}

// ========================================================================
// collect_options
// ========================================================================

#[test]
fn test_collect_options_empty() {
    let kernel = test_kernel();
    let section = collect_options(&kernel);
    assert_eq!(section.title, "Options");
    assert_eq!(section.entries.len(), 1);
    assert_eq!(section.entries[0].label, "Options registered");
    assert_eq!(section.entries[0].detail, "0");
}

#[test]
fn test_collect_options_with_defaults() {
    let kernel = test_kernel();
    kernel
        .options
        .register(OptionSpec::new("number", "Show line numbers", OptionValue::bool(false)))
        .unwrap();
    kernel
        .options
        .register(OptionSpec::new("tabwidth", "Tab width", OptionValue::int(8)))
        .unwrap();

    let section = collect_options(&kernel);
    assert_eq!(section.entries.len(), 1);
    assert_eq!(section.entries[0].detail, "2");
}

#[test]
fn test_collect_options_with_overrides() {
    let kernel = test_kernel();
    kernel
        .options
        .register(OptionSpec::new("number", "Show line numbers", OptionValue::bool(false)))
        .unwrap();
    kernel
        .options
        .set_global("number", OptionValue::bool(true))
        .unwrap();

    let section = collect_options(&kernel);
    assert_eq!(section.entries.len(), 2);
    assert_eq!(section.entries[1].label, "Changed from defaults");
    assert_eq!(section.entries[1].detail, "1");
}

// ========================================================================
// collect_all
// ========================================================================

// ========================================================================
// collect_modules (#610)
// ========================================================================

#[test]
fn test_collect_modules_no_report() {
    let kernel = test_kernel();
    let section = collect_modules(&kernel);
    assert_eq!(section.title, "Modules");
    assert_eq!(section.entries.len(), 1);
    assert_eq!(section.entries[0].status, Status::Info);
    assert!(section.entries[0].detail.contains("no load report"));
}

#[test]
fn test_collect_modules_with_report() {
    let kernel = test_kernel();
    let mut report = ModuleLoadReport::new();
    report.loaded.push(ModuleId::new("vim"));
    report.loaded.push(ModuleId::new("editor"));
    report
        .failed
        .push((ModuleId::new("tetris"), "init error".to_string()));
    report.disabled.push(ModuleId::new("snippet"));
    kernel.services.register(std::sync::Arc::new(report));

    let section = collect_modules(&kernel);
    assert_eq!(section.entries.len(), 4);
    assert_eq!(section.entries[0].status, Status::Ok);
    assert_eq!(section.entries[0].detail, "loaded");
    assert_eq!(section.entries[2].status, Status::Warning);
    assert!(section.entries[2].detail.contains("init error"));
    assert_eq!(section.entries[3].status, Status::Info);
    assert!(section.entries[3].detail.contains("disabled"));
}

#[test]
fn test_collect_modules_empty_report() {
    let kernel = test_kernel();
    let report = ModuleLoadReport::new();
    kernel.services.register(std::sync::Arc::new(report));
    let section = collect_modules(&kernel);
    assert!(section.entries.is_empty());
}

// ========================================================================
// collect_dependencies (#610)
// ========================================================================

#[test]
fn test_collect_dependencies_no_report() {
    let kernel = test_kernel();
    let section = collect_dependencies(&kernel);
    assert_eq!(section.title, "Dependencies");
    assert_eq!(section.entries.len(), 1);
    assert_eq!(section.entries[0].status, Status::Info);
}

#[test]
fn test_collect_dependencies_all_satisfied() {
    let kernel = test_kernel();
    let report = ModuleLoadReport::new();
    kernel.services.register(std::sync::Arc::new(report));

    let section = collect_dependencies(&kernel);
    assert_eq!(section.entries.len(), 1);
    assert_eq!(section.entries[0].status, Status::Ok);
    assert!(section.entries[0].detail.contains("all satisfied"));
}

#[test]
fn test_collect_dependencies_missing() {
    let kernel = test_kernel();
    let mut report = ModuleLoadReport::new();
    report
        .missing_deps
        .push((ModuleId::new("snippet"), ModuleId::new("vim")));
    kernel.services.register(std::sync::Arc::new(report));

    let section = collect_dependencies(&kernel);
    assert_eq!(section.entries.len(), 1);
    assert_eq!(section.entries[0].status, Status::Warning);
    assert!(section.entries[0].detail.contains("requires 'vim'"));
}

// ========================================================================
// collect_configuration (#610)
// ========================================================================

#[test]
fn test_collect_configuration_no_report() {
    let kernel = test_kernel();
    let section = collect_configuration(&kernel);
    assert_eq!(section.title, "Configuration");
    assert_eq!(section.entries.len(), 1);
    assert_eq!(section.entries[0].status, Status::Info);
}

#[test]
fn test_collect_configuration_with_config_path() {
    let kernel = test_kernel();
    let mut report = ModuleLoadReport::new();
    report.config_path = Some(PathBuf::from("/home/user/.config/reovim/modules.toml"));
    kernel.services.register(std::sync::Arc::new(report));

    let section = collect_configuration(&kernel);
    assert!(
        section
            .entries
            .iter()
            .any(|e| e.label == "Config file" && e.status == Status::Ok)
    );
}

#[test]
fn test_collect_configuration_no_config_path() {
    let kernel = test_kernel();
    let report = ModuleLoadReport::new();
    kernel.services.register(std::sync::Arc::new(report));

    let section = collect_configuration(&kernel);
    assert!(
        section
            .entries
            .iter()
            .any(|e| e.label == "Config file" && e.detail.contains("not found"))
    );
}

#[test]
fn test_collect_configuration_with_search_paths() {
    let kernel = test_kernel();
    let mut report = ModuleLoadReport::new();
    report
        .search_paths
        .push(PathBuf::from("/usr/lib/reovim/modules"));
    kernel.services.register(std::sync::Arc::new(report));

    let section = collect_configuration(&kernel);
    assert!(
        section
            .entries
            .iter()
            .any(|e| e.label == "Module search path")
    );
}

#[test]
fn test_collect_configuration_isolation_active() {
    let kernel = test_kernel();
    let mut report = ModuleLoadReport::new();
    report.isolation_active = true;
    kernel.services.register(std::sync::Arc::new(report));

    let section = collect_configuration(&kernel);
    assert!(
        section
            .entries
            .iter()
            .any(|e| e.label == "Isolation" && e.status == Status::Ok)
    );
}

#[test]
fn test_collect_configuration_isolation_not_shown_when_inactive() {
    let kernel = test_kernel();
    let report = ModuleLoadReport::new();
    kernel.services.register(std::sync::Arc::new(report));

    let section = collect_configuration(&kernel);
    assert!(!section.entries.iter().any(|e| e.label == "Isolation"));
}

// ========================================================================
// collect_all (updated for new sections)
// ========================================================================

#[test]
fn test_collect_all_returns_all_sections() {
    let kernel = test_kernel();
    let sections = collect_all(&kernel);
    assert_eq!(sections.len(), 8);
    assert_eq!(sections[0].title, "System");
    assert_eq!(sections[1].title, "Modules");
    assert_eq!(sections[2].title, "Dependencies");
    assert_eq!(sections[3].title, "Configuration");
    assert_eq!(sections[4].title, "Language Servers");
    assert_eq!(sections[5].title, "Syntax Highlighting");
    assert_eq!(sections[6].title, "Clipboard");
    assert_eq!(sections[7].title, "Options");
}

// ========================================================================
// format_report
// ========================================================================

#[test]
fn test_format_report_empty() {
    let report = format_report(&[]);
    assert!(report.is_empty());
}

#[test]
fn test_format_report_single_section() {
    let mut section = DiagnosticSection::new("Test");
    section
        .entries
        .push(DiagnosticEntry::new(Status::Ok, "item", "value"));

    let report = format_report(&[section]);
    assert!(report.contains("=== Test ==="));
    assert!(report.contains("[OK] item: value"));
}

#[test]
fn test_format_report_empty_section() {
    let section = DiagnosticSection::new("Empty");
    let report = format_report(&[section]);
    assert!(report.contains("=== Empty ==="));
    assert!(report.contains("(no data)"));
}

#[test]
fn test_format_report_multiple_sections() {
    let mut s1 = DiagnosticSection::new("First");
    s1.entries.push(DiagnosticEntry::new(Status::Ok, "a", "1"));
    let mut s2 = DiagnosticSection::new("Second");
    s2.entries
        .push(DiagnosticEntry::new(Status::Warning, "b", "2"));

    let report = format_report(&[s1, s2]);
    assert!(report.contains("=== First ==="));
    assert!(report.contains("=== Second ==="));
    assert!(report.contains("[OK] a: 1"));
    assert!(report.contains("[!!] b: 2"));
}

#[test]
fn test_format_report_all_statuses() {
    let mut section = DiagnosticSection::new("All");
    section
        .entries
        .push(DiagnosticEntry::new(Status::Ok, "ok", "pass"));
    section
        .entries
        .push(DiagnosticEntry::new(Status::Warning, "warn", "caution"));
    section
        .entries
        .push(DiagnosticEntry::new(Status::Info, "info", "note"));

    let report = format_report(&[section]);
    assert!(report.contains("[OK] ok: pass"));
    assert!(report.contains("[!!] warn: caution"));
    assert!(report.contains("[--] info: note"));
}

// ========================================================================
// collect_treesitter - with factories
// ========================================================================

#[test]
fn test_collect_treesitter_with_factories() {
    use reovim_driver_syntax::{SyntaxDriver, SyntaxDriverFactory};

    struct StubFactory;
    impl SyntaxDriverFactory for StubFactory {
        fn create(&self, _language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
            None
        }
        fn supported_languages(&self) -> Vec<&str> {
            vec!["test"]
        }
    }

    let kernel = test_kernel();
    let store = SyntaxFactoryStore::new();
    store.add(Arc::new(StubFactory));
    kernel.services.register(Arc::new(store));

    let section = collect_treesitter(&kernel);
    assert!(
        section
            .entries
            .iter()
            .any(|e| e.label == "Syntax factories" && e.detail.contains('1'))
    );
}

#[test]
fn test_collect_treesitter_with_language_defs() {
    use reovim_driver_syntax::CommentTokens;

    let kernel = test_kernel();
    let store = LanguageInfoStore::new();
    store.add(reovim_driver_syntax::LanguageInfo {
        id: "rust".to_string(),
        name: "Rust".to_string(),
        extensions: vec!["rs".to_string()],
        mime_types: vec![],
        comment_tokens: CommentTokens::default(),
    });
    kernel.services.register(Arc::new(store));

    let section = collect_treesitter(&kernel);
    assert!(
        section
            .entries
            .iter()
            .any(|e| e.label == "Language definitions" && e.detail.contains('1'))
    );
}

#[test]
fn test_collect_treesitter_both_factories_and_langs() {
    use reovim_driver_syntax::{CommentTokens, SyntaxDriver, SyntaxDriverFactory};

    struct StubFactory;
    impl SyntaxDriverFactory for StubFactory {
        fn create(&self, _language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
            None
        }
        fn supported_languages(&self) -> Vec<&str> {
            vec!["test"]
        }
    }

    let kernel = test_kernel();
    let factory_store = SyntaxFactoryStore::new();
    factory_store.add(Arc::new(StubFactory));
    kernel.services.register(Arc::new(factory_store));

    let lang_store = LanguageInfoStore::new();
    lang_store.add(reovim_driver_syntax::LanguageInfo {
        id: "rust".to_string(),
        name: "Rust".to_string(),
        extensions: vec!["rs".to_string()],
        mime_types: vec![],
        comment_tokens: CommentTokens::default(),
    });
    kernel.services.register(Arc::new(lang_store));

    let section = collect_treesitter(&kernel);
    assert_eq!(section.entries.len(), 2);
    assert!(
        section
            .entries
            .iter()
            .any(|e| e.label == "Syntax factories")
    );
    assert!(
        section
            .entries
            .iter()
            .any(|e| e.label == "Language definitions")
    );
}

// ========================================================================
// collect_clipboard - with providers
// ========================================================================

use reovim_driver_clipboard::ClipboardProvider;

/// Mock clipboard provider for testing diagnostics.
struct MockClipboardProvider {
    clipboard_available: bool,
    selection_available: bool,
}

impl ClipboardProvider for MockClipboardProvider {
    fn clipboard_available(&self) -> bool {
        self.clipboard_available
    }

    fn selection_available(&self) -> bool {
        self.selection_available
    }

    fn copy_to_clipboard(
        &self,
        _text: &str,
    ) -> Result<(), reovim_driver_clipboard::ClipboardError> {
        Ok(())
    }

    fn paste_from_clipboard(
        &self,
    ) -> Result<Option<String>, reovim_driver_clipboard::ClipboardError> {
        Ok(None)
    }

    fn copy_to_selection(
        &self,
        _text: &str,
    ) -> Result<(), reovim_driver_clipboard::ClipboardError> {
        Ok(())
    }

    fn paste_from_selection(
        &self,
    ) -> Result<Option<String>, reovim_driver_clipboard::ClipboardError> {
        Ok(None)
    }
}

#[test]
fn test_collect_clipboard_both_available() {
    let kernel = test_kernel();
    let registry = ClipboardProviderRegistry::new();
    registry.register(
        ClipboardKey::Default,
        Arc::new(MockClipboardProvider {
            clipboard_available: true,
            selection_available: true,
        }),
    );
    kernel.services.register(Arc::new(registry));

    let section = collect_clipboard(&kernel);
    assert_eq!(section.entries.len(), 2);
    assert_eq!(section.entries[0].status, Status::Ok);
    assert!(section.entries[0].detail.contains("available"));
    assert_eq!(section.entries[1].status, Status::Ok);
    assert!(section.entries[1].detail.contains("available"));
}

#[test]
fn test_collect_clipboard_none_available() {
    let kernel = test_kernel();
    let registry = ClipboardProviderRegistry::new();
    registry.register(
        ClipboardKey::Default,
        Arc::new(MockClipboardProvider {
            clipboard_available: false,
            selection_available: false,
        }),
    );
    kernel.services.register(Arc::new(registry));

    let section = collect_clipboard(&kernel);
    assert_eq!(section.entries.len(), 2);
    assert_eq!(section.entries[0].status, Status::Warning);
    assert!(section.entries[0].detail.contains("not available"));
    assert_eq!(section.entries[1].status, Status::Info);
    assert!(section.entries[1].detail.contains("not available"));
}

#[test]
fn test_collect_clipboard_only_clipboard() {
    let kernel = test_kernel();
    let registry = ClipboardProviderRegistry::new();
    registry.register(
        ClipboardKey::Default,
        Arc::new(MockClipboardProvider {
            clipboard_available: true,
            selection_available: false,
        }),
    );
    kernel.services.register(Arc::new(registry));

    let section = collect_clipboard(&kernel);
    assert_eq!(section.entries[0].status, Status::Ok);
    assert_eq!(section.entries[1].status, Status::Info);
}

// ========================================================================
// collect_lsp - additional edge cases
// ========================================================================

#[test]
fn test_collect_lsp_active_no_server_info() {
    let kernel = test_kernel();
    let registry = LspProviderRegistry::new();
    registry.register(LspKey::Default, Arc::new(MockLspProvider::new(true, None)));
    kernel.services.register(Arc::new(registry));

    let section = collect_lsp(&kernel);
    assert_eq!(section.entries.len(), 1);
    assert_eq!(section.entries[0].status, Status::Ok);
    assert!(section.entries[0].detail.contains("active"));
    assert!(section.entries[0].detail.contains("no server info"));
}

#[test]
fn test_collect_lsp_inactive_with_version_none() {
    let kernel = test_kernel();
    let registry = LspProviderRegistry::new();
    registry.register(
        LspKey::Language("go".to_string()),
        Arc::new(MockLspProvider::new(
            false,
            Some(ServerInfo {
                name: "gopls".to_string(),
                version: None,
            }),
        )),
    );
    kernel.services.register(Arc::new(registry));

    let section = collect_lsp(&kernel);
    assert_eq!(section.entries[0].status, Status::Warning);
    assert!(section.entries[0].detail.contains("gopls"));
    assert!(section.entries[0].detail.contains("unknown"));
    assert!(section.entries[0].detail.contains("inactive"));
}
