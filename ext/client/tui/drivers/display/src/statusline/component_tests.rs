use super::*;

struct TestComponent;

#[cfg_attr(coverage_nightly, coverage(off))]
impl ComponentProvider for TestComponent {
    fn id(&self) -> &'static str {
        "test"
    }

    fn render(&self, ctx: &ComponentContext) -> ComponentOutput {
        ComponentOutput::new(format!("Mode: {}", ctx.mode))
    }
}

#[test]
fn test_component_context_default() {
    let ctx = ComponentContext::default();
    assert!(ctx.mode.is_empty());
    assert!(ctx.filename.is_none());
    assert_eq!(ctx.line, 0);
    assert_eq!(ctx.column, 0);
}

#[test]
fn test_component_output_new() {
    let output = ComponentOutput::new("test");
    assert_eq!(output.text, "test");
    assert!(output.visible);
    assert!(output.style.is_none());
}

#[test]
fn test_component_output_hidden() {
    let output = ComponentOutput::hidden();
    assert!(output.text.is_empty());
    assert!(!output.visible);
}

#[test]
fn test_component_output_builder() {
    let output = ComponentOutput::new("test")
        .with_min_width(10)
        .with_priority(200)
        .when(true);

    assert_eq!(output.min_width, Some(10));
    assert_eq!(output.truncation_priority, 200);
    assert!(output.visible);
}

#[test]
fn test_component_provider_render() {
    let component = TestComponent;
    let ctx = ComponentContext {
        mode: "NORMAL".to_string(),
        ..Default::default()
    };

    let output = component.render(&ctx);
    assert_eq!(output.text, "Mode: NORMAL");
}

#[test]
fn test_diagnostic_counts() {
    let counts = DiagnosticCounts {
        errors: 2,
        warnings: 3,
        info: 1,
        hints: 0,
    };

    assert!(!counts.is_empty());
    assert_eq!(counts.total(), 6);

    let empty = DiagnosticCounts::default();
    assert!(empty.is_empty());
    assert_eq!(empty.total(), 0);
}

#[test]
fn test_component_provider_key() {
    let key = ComponentProviderKey::new("branch");
    assert_eq!(key.id(), "branch");

    let key2 = ComponentProviderKey::new("branch".to_string());
    assert_eq!(key, key2);
}

#[test]
fn test_component_provider_key_service_name() {
    assert_eq!(ComponentProviderKey::service_name(), "ComponentProvider");
}

#[test]
fn test_component_provider_registry() {
    use std::sync::Arc;

    let registry = ComponentProviderRegistry::new();

    // Register a component
    let key = ComponentProviderKey::new("test");
    registry.register(key.clone(), Arc::new(TestComponent));

    // Lookup the component
    let component = registry.get(&key);
    assert!(component.is_some());

    let ctx = ComponentContext::default();
    let output = component.unwrap().render(&ctx);
    assert!(output.text.contains("Mode:"));
}

#[test]
fn test_component_output_with_style() {
    let style = Style::new().fg(crate::Color::Red);
    let output = ComponentOutput::new("styled").with_style(style.clone());

    assert_eq!(output.text, "styled");
    assert!(output.visible);
    assert_eq!(output.style, Some(style));
}

#[test]
fn test_component_output_default() {
    let output = ComponentOutput::default();
    assert!(output.text.is_empty());
    assert!(!output.visible);
    assert!(output.style.is_none());
    assert_eq!(output.truncation_priority, 0);
}

#[test]
fn test_component_provider_style_for_mode_default() {
    let component = TestComponent;
    assert!(component.style_for_mode("NORMAL").is_none());
    assert!(component.style_for_mode("INSERT").is_none());
}

#[test]
fn test_component_provider_needs_frequent_update_default() {
    let component = TestComponent;
    assert!(!component.needs_frequent_update());
}

/// A component that overrides the default trait methods.
struct CustomComponent;

#[cfg_attr(coverage_nightly, coverage(off))]
impl ComponentProvider for CustomComponent {
    fn id(&self) -> &'static str {
        "custom"
    }

    fn render(&self, _ctx: &ComponentContext) -> ComponentOutput {
        ComponentOutput::new("custom")
    }

    fn style_for_mode(&self, mode: &str) -> Option<Style> {
        if mode == "INSERT" {
            Some(Style::new().fg(crate::Color::Green))
        } else {
            None
        }
    }

    fn needs_frequent_update(&self) -> bool {
        true
    }
}

#[test]
fn test_diagnostic_counts_is_empty_individual_nonzero() {
    // Line 97: exercise each individual non-zero count causing is_empty() to be false
    // errors != 0
    let counts = DiagnosticCounts {
        errors: 1,
        warnings: 0,
        info: 0,
        hints: 0,
    };
    assert!(!counts.is_empty());

    // warnings != 0
    let counts = DiagnosticCounts {
        errors: 0,
        warnings: 1,
        info: 0,
        hints: 0,
    };
    assert!(!counts.is_empty());

    // info != 0
    let counts = DiagnosticCounts {
        errors: 0,
        warnings: 0,
        info: 1,
        hints: 0,
    };
    assert!(!counts.is_empty());

    // hints != 0
    let counts = DiagnosticCounts {
        errors: 0,
        warnings: 0,
        info: 0,
        hints: 1,
    };
    assert!(!counts.is_empty());
}

#[test]
fn test_custom_component_style_for_mode() {
    let component = CustomComponent;
    assert!(component.style_for_mode("INSERT").is_some());
    assert!(component.style_for_mode("NORMAL").is_none());
}

#[test]
fn test_custom_component_needs_frequent_update() {
    let component = CustomComponent;
    assert!(component.needs_frequent_update());
}

// ========================================================================
// DataProviderAdapter + From conversion tests
// ========================================================================

/// Test data provider for adapter tests.
struct TestDataProvider;

impl ComponentDataProvider for TestDataProvider {
    fn id(&self) -> &'static str {
        "test-data"
    }

    fn data(&self, ctx: &ComponentDataContext) -> ComponentData {
        ctx.git_branch
            .as_ref()
            .map_or_else(ComponentData::hidden, |branch| ComponentData::new(format!(" {branch} ")))
    }
}

#[test]
fn test_from_component_context_to_data_context() {
    let ctx = ComponentContext {
        mode: "NORMAL".to_string(),
        mode_subtype: Some("CHAR".to_string()),
        filename: Some("main.rs".to_string()),
        filepath: Some("/home/user/src/main.rs".to_string()),
        modified: true,
        readonly: false,
        filetype: Some("rust".to_string()),
        line: 42,
        column: 10,
        total_lines: 100,
        encoding: "utf-8".to_string(),
        line_ending: "unix".to_string(),
        terminal_width: 120,
        terminal_height: 40,
        git_branch: Some("main".to_string()),
        breadcrumb: Some("fn test".to_string()),
        diagnostics: Some(DiagnosticCounts {
            errors: 1,
            warnings: 2,
            info: 3,
            hints: 4,
        }),
    };

    let data_ctx = ComponentDataContext::from(&ctx);
    assert_eq!(data_ctx.mode, "NORMAL");
    assert_eq!(data_ctx.mode_subtype.as_deref(), Some("CHAR"));
    assert_eq!(data_ctx.filename.as_deref(), Some("main.rs"));
    assert_eq!(data_ctx.filepath.as_deref(), Some("/home/user/src/main.rs"));
    assert!(data_ctx.modified);
    assert!(!data_ctx.readonly);
    assert_eq!(data_ctx.filetype.as_deref(), Some("rust"));
    assert_eq!(data_ctx.line, 42);
    assert_eq!(data_ctx.column, 10);
    assert_eq!(data_ctx.total_lines, 100);
    assert_eq!(data_ctx.encoding, "utf-8");
    assert_eq!(data_ctx.line_ending, "unix");
    assert_eq!(data_ctx.terminal_width, 120);
    assert_eq!(data_ctx.terminal_height, 40);
    assert_eq!(data_ctx.git_branch.as_deref(), Some("main"));
    assert_eq!(data_ctx.breadcrumb.as_deref(), Some("fn test"));
    let diag = data_ctx.diagnostics.as_ref().unwrap();
    assert_eq!(diag.errors, 1);
    assert_eq!(diag.warnings, 2);
    assert_eq!(diag.info, 3);
    assert_eq!(diag.hints, 4);
}

#[test]
fn test_from_component_context_default() {
    let ctx = ComponentContext::default();
    let data_ctx = ComponentDataContext::from(&ctx);
    assert!(data_ctx.mode.is_empty());
    assert!(data_ctx.filename.is_none());
    assert!(data_ctx.diagnostics.is_none());
}

#[test]
fn test_adapter_id() {
    let adapter = DataProviderAdapter::new(Arc::new(TestDataProvider));
    assert_eq!(adapter.id(), "test-data");
}

#[test]
fn test_adapter_render_visible() {
    let adapter = DataProviderAdapter::new(Arc::new(TestDataProvider));
    let ctx = ComponentContext {
        git_branch: Some("main".to_string()),
        ..ComponentContext::default()
    };
    let output = adapter.render(&ctx);
    assert!(output.visible);
    assert_eq!(output.text, " main ");
    assert!(output.style.is_none());
}

#[test]
fn test_adapter_render_hidden() {
    let adapter = DataProviderAdapter::new(Arc::new(TestDataProvider));
    let ctx = ComponentContext::default();
    let output = adapter.render(&ctx);
    assert!(!output.visible);
}

#[test]
fn test_adapter_preserves_min_width_and_priority() {
    struct PriorityProvider;
    impl ComponentDataProvider for PriorityProvider {
        fn id(&self) -> &'static str {
            "priority"
        }
        fn data(&self, _ctx: &ComponentDataContext) -> ComponentData {
            ComponentData::new("test")
                .with_min_width(15)
                .with_priority(200)
        }
    }

    let adapter = DataProviderAdapter::new(Arc::new(PriorityProvider));
    let output = adapter.render(&ComponentContext::default());
    assert!(output.visible);
    assert_eq!(output.min_width, Some(15));
    assert_eq!(output.truncation_priority, 200);
}
