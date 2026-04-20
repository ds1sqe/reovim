use {
    super::*,
    crate::{SectionId, Style},
};

struct TestProvider;

#[cfg_attr(coverage_nightly, coverage(off))]
impl StatuslineProvider for TestProvider {
    fn render(&self, ctx: &ComponentContext) -> Vec<Section> {
        vec![
            Section::new(SectionId::A, format!(" {} ", ctx.mode), Style::default()),
            Section::new(SectionId::Z, format!(" {}:{} ", ctx.line, ctx.column), Style::default()),
        ]
    }
}

#[test]
fn test_provider_key_service_name() {
    assert_eq!(StatuslineProviderKey::service_name(), "StatuslineProvider");
}

#[test]
fn test_provider_render() {
    let provider = TestProvider;
    let ctx = ComponentContext {
        mode: "NORMAL".to_string(),
        line: 42,
        column: 15,
        ..Default::default()
    };

    let sections = provider.render(&ctx);
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].text, " NORMAL ");
    assert_eq!(sections[1].text, " 42:15 ");
}

#[test]
fn test_provider_default_height() {
    let provider = TestProvider;
    let ctx = ComponentContext::default();
    assert_eq!(provider.height(&ctx), 1);
}

#[test]
fn test_provider_default_visible() {
    let provider = TestProvider;
    let ctx = ComponentContext::default();
    assert!(provider.is_visible(&ctx));
}

#[test]
fn test_provider_default_height_config() {
    let provider = TestProvider;
    let config = provider.height_config();
    // Default should be single row
    assert_eq!(config.min_height, 1);
}

#[test]
fn test_provider_default_layout() {
    let provider = TestProvider;
    let ctx = ComponentContext::default();
    let layout = provider.layout(&ctx);
    // Default should be single row layout
    assert_eq!(layout.rows.len(), 1);
}

#[test]
fn test_provider_calculate_layout() {
    let provider = TestProvider;
    let sections = vec![
        Section::new(SectionId::A, "ABC", Style::default()),
        Section::new(SectionId::Z, "XYZ", Style::default()),
    ];
    let layout = provider.calculate_layout(&sections, 80, 1);
    assert_eq!(layout.rows.len(), 1);
}

#[test]
fn test_provider_calculate_height_result() {
    let provider = TestProvider;
    let sections = vec![
        Section::new(SectionId::A, "Mode", Style::default()),
        Section::new(SectionId::Z, "Pos", Style::default()),
    ];
    let result = provider.calculate_height_result(&sections, 24, 80);
    assert!(result.height >= 1);
}

#[test]
fn test_provider_key_equality() {
    assert_eq!(StatuslineProviderKey::Main, StatuslineProviderKey::Main);
    assert_ne!(StatuslineProviderKey::Main, StatuslineProviderKey::Inactive);
}
