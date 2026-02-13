//! Statusline provider trait and registry.
//!
//! Defines the provider interface for statusline content generation.
//! Policy modules implement [`StatuslineProvider`] to control what
//! content appears in each section.

use reovim_kernel::api::v1::{MultiServiceRegistry, ServiceKey};

use super::{
    ComponentContext, Section,
    height::{ContentMetrics, HeightConfig, HeightResult, calculate_height_with_metrics},
    layout::{LayoutCalculator, MultiRowLayout},
};

/// Typed key for statusline provider lookup.
///
/// Supports different statusline instances (main, inactive windows, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatuslineProviderKey {
    /// Main statusline at screen bottom (laststatus=3 style).
    Main,
    /// Statusline for inactive windows (future use).
    Inactive,
}

impl ServiceKey for StatuslineProviderKey {
    fn service_name() -> &'static str {
        "StatuslineProvider"
    }
}

/// Trait for statusline content providers.
///
/// Providers assemble components into sections and handle the overall
/// layout strategy. The default implementation places components in
/// lualine-style sections (A, B, C | X, Y, Z).
///
/// # Example
///
/// ```ignore
/// struct DefaultStatuslineProvider {
///     components: HashMap<SectionId, Vec<Arc<dyn ComponentProvider>>>,
/// }
///
/// impl StatuslineProvider for DefaultStatuslineProvider {
///     fn render(&self, ctx: &ComponentContext) -> Vec<Section> {
///         let mut sections = Vec::new();
///         for section_id in SectionId::ALL {
///             if let Some(components) = self.components.get(section_id) {
///                 let text = components.iter()
///                     .map(|c| c.render(ctx))
///                     .filter(|o| o.visible)
///                     .map(|o| o.text)
///                     .collect::<Vec<_>>()
///                     .join("");
///                 sections.push(Section::new(*section_id, text, self.style_for(*section_id)));
///             }
///         }
///         sections
///     }
/// }
/// ```
pub trait StatuslineProvider: Send + Sync {
    /// Render all sections with the given context.
    ///
    /// Returns a vector of sections to be rendered by the statusline renderer.
    /// Empty sections may be included (renderer will skip them).
    fn render(&self, ctx: &ComponentContext) -> Vec<Section>;

    /// Get the height of the statusline in rows.
    ///
    /// Default is 1 row. Providers can return more for multi-row statuslines.
    /// Override this to implement dynamic height based on content.
    fn height(&self, _ctx: &ComponentContext) -> u16 {
        1
    }

    /// Check if the statusline is visible.
    ///
    /// Default is always visible. Providers can hide based on context
    /// (e.g., hide in certain modes or when screen is too small).
    fn is_visible(&self, _ctx: &ComponentContext) -> bool {
        true
    }

    /// Get the height configuration for dynamic height calculation.
    ///
    /// Default returns a single-row configuration. Override to support
    /// dynamic height based on screen size and content overflow.
    fn height_config(&self) -> HeightConfig {
        HeightConfig::single_row()
    }

    /// Get the multi-row layout for the statusline.
    ///
    /// Default returns a single-row layout. When height > 1, this determines
    /// how sections are distributed across rows.
    fn layout(&self, _ctx: &ComponentContext) -> MultiRowLayout {
        MultiRowLayout::single_row()
    }

    /// Calculate the optimal layout based on rendered sections.
    ///
    /// This is a convenience method that calculates layout from content metrics.
    /// Default implementation uses the `LayoutCalculator`.
    fn calculate_layout(
        &self,
        sections: &[Section],
        available_width: u16,
        max_rows: u16,
    ) -> MultiRowLayout {
        let metrics = ContentMetrics::from_sections(sections);
        let calculator = LayoutCalculator::new(available_width);
        calculator.calculate(&metrics.section_widths, max_rows)
    }

    /// Calculate dynamic height based on content and screen size.
    ///
    /// Returns height information including whether overflow occurred.
    fn calculate_height_result(
        &self,
        sections: &[Section],
        screen_height: u16,
        available_width: u16,
    ) -> HeightResult {
        let metrics = ContentMetrics::from_sections(sections);
        calculate_height_with_metrics(
            screen_height,
            &metrics,
            available_width,
            &self.height_config(),
        )
    }
}

/// Registry for statusline providers.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_display::statusline::{
///     StatuslineProviderKey, StatuslineProviderRegistry,
/// };
///
/// let registry = StatuslineProviderRegistry::new();
/// registry.register(StatuslineProviderKey::Main, Arc::new(my_provider));
///
/// let provider = registry.get(&StatuslineProviderKey::Main);
/// ```
pub type StatuslineProviderRegistry =
    MultiServiceRegistry<StatuslineProviderKey, dyn StatuslineProvider>;

#[cfg(test)]
mod tests {
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
                Section::new(
                    SectionId::Z,
                    format!(" {}:{} ", ctx.line, ctx.column),
                    Style::default(),
                ),
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
}
