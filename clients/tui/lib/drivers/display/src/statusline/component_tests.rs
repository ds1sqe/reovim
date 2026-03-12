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
