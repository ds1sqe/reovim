//! Filetype component for statusline.
//!
//! Displays the current buffer's filetype.

use reovim_driver_display::{ComponentContext, ComponentOutput, ComponentProvider};

/// Filetype component.
///
/// Displays the detected filetype (e.g., "rust", "python", "markdown").
pub struct FiletypeComponent;

impl FiletypeComponent {
    /// Create a new filetype component.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for FiletypeComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentProvider for FiletypeComponent {
    fn id(&self) -> &'static str {
        "filetype"
    }

    fn render(&self, ctx: &ComponentContext) -> ComponentOutput {
        match &ctx.filetype {
            Some(ft) if !ft.is_empty() => {
                ComponentOutput::new(format!(" {ft} ")).with_priority(100)
            }
            _ => ComponentOutput::hidden(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filetype_component_id() {
        let component = FiletypeComponent::new();
        assert_eq!(component.id(), "filetype");
    }

    #[test]
    fn test_filetype_with_type() {
        let component = FiletypeComponent::new();
        let ctx = ComponentContext {
            filetype: Some("rust".to_string()),
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.visible);
        assert!(output.text.contains("rust"));
    }

    #[test]
    fn test_filetype_no_type() {
        let component = FiletypeComponent::new();
        let ctx = ComponentContext::default();

        let output = component.render(&ctx);
        assert!(!output.visible);
    }

    #[test]
    fn test_filetype_empty_type() {
        let component = FiletypeComponent::new();
        let ctx = ComponentContext {
            filetype: Some(String::new()),
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(!output.visible);
    }
}
