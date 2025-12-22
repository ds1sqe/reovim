//! Theme/colorscheme picker implementation

use std::{future::Future, pin::Pin};

use reovim_core::highlight::ThemeName;

use super::super::{
    item::{TelescopeData, TelescopeItem},
    state::PreviewContent,
};

use super::{Picker, PickerContext, TelescopeAction};

/// Picker for selecting colorschemes/themes
pub struct ThemesPicker;

impl ThemesPicker {
    /// Create a new themes picker
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Get all available themes
    fn available_themes() -> Vec<(ThemeName, &'static str, &'static str)> {
        vec![
            (ThemeName::Dark, "Dark", "Default dark theme with classic syntax colors"),
            (ThemeName::Light, "Light", "Light theme for bright environments"),
            (
                ThemeName::TokyoNightOrange,
                "Tokyo Night Orange",
                "Modern dark theme with orange accents",
            ),
        ]
    }
}

impl Default for ThemesPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for ThemesPicker {
    fn name(&self) -> &'static str {
        "themes"
    }

    fn title(&self) -> &'static str {
        "Select Colorscheme"
    }

    fn prompt(&self) -> &'static str {
        "Theme> "
    }

    fn fetch(
        &self,
        _ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<TelescopeItem>> + Send + '_>> {
        Box::pin(async move {
            Self::available_themes()
                .into_iter()
                .map(|(name, display, description)| {
                    TelescopeItem::new(display, display, TelescopeData::Theme(name), "themes")
                        .with_detail(description)
                        .with_icon('🎨')
                })
                .collect()
        })
    }

    fn on_select(&self, item: &TelescopeItem) -> TelescopeAction {
        match &item.data {
            TelescopeData::Theme(name) => TelescopeAction::ApplyTheme(*name),
            _ => TelescopeAction::Nothing,
        }
    }

    fn preview(
        &self,
        item: &TelescopeItem,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        let display = item.display.clone();
        let description = item.detail.clone();

        Box::pin(async move {
            let mut lines = vec![format!("Theme: {display}"), String::new()];

            if let Some(desc) = description {
                lines.push(format!("Description: {desc}"));
                lines.push(String::new());
            }

            // Add color preview hints based on theme
            lines.push("Color Preview:".to_string());
            match display.as_str() {
                "Dark" => {
                    lines.push("  Keywords:  Magenta (bold)".to_string());
                    lines.push("  Functions: Blue".to_string());
                    lines.push("  Strings:   Green".to_string());
                    lines.push("  Comments:  Gray (italic)".to_string());
                }
                "Light" => {
                    lines.push("  Keywords:  Blue (bold)".to_string());
                    lines.push("  Functions: Dark Blue".to_string());
                    lines.push("  Strings:   Green".to_string());
                    lines.push("  Comments:  Gray (italic)".to_string());
                }
                "Tokyo Night Orange" => {
                    lines.push("  Keywords:  Purple #bb9af7 (bold, italic)".to_string());
                    lines.push("  Functions: Blue #7aa2f7".to_string());
                    lines.push("  Strings:   Green #9ece6a".to_string());
                    lines.push("  Numbers:   Orange #ff9e64".to_string());
                    lines.push("  Comments:  Gray #c7c7c7 (italic)".to_string());
                }
                _ => {}
            }

            Some(PreviewContent::new(lines))
        })
    }
}
