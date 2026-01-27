//! Default statusline provider.
//!
//! Assembles components into sections for rendering.

use reovim_driver_display::{
    ComponentContext, ComponentProvider, HeightConfig, MultiRowLayout, Section, SectionId,
    StatuslineProvider, Style,
};

use crate::{
    components::{FilenameComponent, FiletypeComponent, ModeComponent, PositionComponent},
    config::{ComponentId, StatuslineConfig},
    theme::StatuslineTheme,
};

/// Default statusline provider implementation.
///
/// Renders components into lualine-style sections (A, B, C | X, Y, Z).
pub struct DefaultStatuslineProvider {
    config: StatuslineConfig,
    theme: StatuslineTheme,
    // Built-in components
    mode: ModeComponent,
    filename: FilenameComponent,
    filetype: FiletypeComponent,
    position: PositionComponent,
}

impl DefaultStatuslineProvider {
    /// Create a new provider with the given configuration.
    #[must_use]
    pub fn new(config: StatuslineConfig) -> Self {
        Self {
            config,
            theme: StatuslineTheme::new(),
            mode: ModeComponent::new(),
            filename: FilenameComponent::new(),
            filetype: FiletypeComponent::new(),
            position: PositionComponent::new(),
        }
    }

    /// Create a new provider with custom theme.
    #[must_use]
    pub const fn with_theme(config: StatuslineConfig, theme: StatuslineTheme) -> Self {
        Self {
            config,
            theme,
            mode: ModeComponent::new(),
            filename: FilenameComponent::new(),
            filetype: FiletypeComponent::new(),
            position: PositionComponent::new(),
        }
    }

    /// Render a single section.
    fn render_section(&self, section_id: SectionId, ctx: &ComponentContext) -> Section {
        let component_ids = self.config.sections.get(section_id);

        if component_ids.is_empty() {
            return Section::empty(section_id);
        }

        // Get section colors based on current mode
        let section_colors = self.theme.section_colors_for_mode(&ctx.mode);

        // Collect rendered text from all components in this section
        let mut text = String::new();
        let mut component_style: Option<Style> = None;

        for component_id in component_ids {
            let component: &dyn ComponentProvider = match component_id {
                ComponentId::Mode => &self.mode,
                ComponentId::Filename => &self.filename,
                ComponentId::Filetype => &self.filetype,
                ComponentId::Position => &self.position,
                // Future components return empty for now
                ComponentId::Branch
                | ComponentId::Encoding
                | ComponentId::FileFormat
                | ComponentId::Progress
                | ComponentId::Diagnostics => continue,
            };

            let output = component.render(ctx);
            if output.visible {
                text.push_str(&output.text);
                // Use the first component's style if it has one
                if component_style.is_none() {
                    component_style = output.style;
                }
            }
        }

        // Determine the final style:
        // 1. Component-specific style (if provided)
        // 2. Theme's section style (based on current mode)
        let style = component_style.unwrap_or_else(|| section_colors.style_for_section(section_id));

        Section::new(section_id, text, style)
    }
}

impl StatuslineProvider for DefaultStatuslineProvider {
    fn render(&self, ctx: &ComponentContext) -> Vec<Section> {
        SectionId::ALL
            .iter()
            .map(|&id| self.render_section(id, ctx))
            .collect()
    }

    fn height(&self, ctx: &ComponentContext) -> u16 {
        // Calculate dynamic height based on content and screen size
        let sections = self.render(ctx);
        let height_result =
            self.calculate_height_result(&sections, ctx.terminal_height, ctx.terminal_width);
        height_result.height
    }

    fn is_visible(&self, _ctx: &ComponentContext) -> bool {
        true
    }

    fn height_config(&self) -> HeightConfig {
        self.config.height.clone()
    }

    fn layout(&self, ctx: &ComponentContext) -> MultiRowLayout {
        let sections = self.render(ctx);
        let height_result =
            self.calculate_height_result(&sections, ctx.terminal_height, ctx.terminal_width);

        // Determine layout based on calculated height
        match height_result.height {
            1 => MultiRowLayout::single_row(),
            2 => MultiRowLayout::two_rows(),
            _ => MultiRowLayout::three_rows(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_render_all_sections() {
        let config = StatuslineConfig::default();
        let provider = DefaultStatuslineProvider::new(config);

        let ctx = ComponentContext {
            mode: "NORMAL".to_string(),
            filename: Some("main.rs".to_string()),
            filetype: Some("rust".to_string()),
            line: 42,
            column: 15,
            ..Default::default()
        };

        let sections = provider.render(&ctx);

        // Should have 6 sections (A-Z)
        assert_eq!(sections.len(), 6);

        // Section A should have mode
        let section_a = sections.iter().find(|s| s.id == SectionId::A).unwrap();
        assert!(section_a.text.contains("NORMAL"));

        // Section C should have filename
        let section_c = sections.iter().find(|s| s.id == SectionId::C).unwrap();
        assert!(section_c.text.contains("main.rs"));

        // Section Y should have filetype
        let section_y = sections.iter().find(|s| s.id == SectionId::Y).unwrap();
        assert!(section_y.text.contains("rust"));

        // Section Z should have position
        let section_z = sections.iter().find(|s| s.id == SectionId::Z).unwrap();
        assert!(section_z.text.contains("42"));
        assert!(section_z.text.contains("15"));
    }

    #[test]
    fn test_provider_empty_sections() {
        let config = StatuslineConfig::default();
        let provider = DefaultStatuslineProvider::new(config);

        let ctx = ComponentContext::default();
        let sections = provider.render(&ctx);

        // Section B should be empty (no git branch component configured)
        let section_b = sections.iter().find(|s| s.id == SectionId::B).unwrap();
        assert!(section_b.is_empty());
    }
}
