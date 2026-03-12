use {
    super::*,
    reovim_kernel::api::v1::{OptionSpec, OptionValue},
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

#[test]
fn test_collect_all_returns_all_sections() {
    let kernel = test_kernel();
    let sections = collect_all(&kernel);
    assert_eq!(sections.len(), 5);
    assert_eq!(sections[0].title, "System");
    assert_eq!(sections[1].title, "Language Servers");
    assert_eq!(sections[2].title, "Syntax Highlighting");
    assert_eq!(sections[3].title, "Clipboard");
    assert_eq!(sections[4].title, "Options");
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
