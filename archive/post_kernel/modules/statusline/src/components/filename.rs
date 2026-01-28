//! Filename component for statusline.
//!
//! Displays the current buffer's filename with modified indicator.

use reovim_driver_display::{ComponentContext, ComponentOutput, ComponentProvider};

/// Filename component.
///
/// Displays the current filename with modified/readonly indicators.
pub struct FilenameComponent;

impl FilenameComponent {
    /// Create a new filename component.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for FilenameComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentProvider for FilenameComponent {
    fn id(&self) -> &'static str {
        "filename"
    }

    fn render(&self, ctx: &ComponentContext) -> ComponentOutput {
        let filename = ctx.filename.as_deref().unwrap_or("[No Name]");

        // Build indicators
        let mut indicators = String::new();
        if ctx.modified {
            indicators.push_str(" [+]");
        }
        if ctx.readonly {
            indicators.push_str(" [RO]");
        }

        let text = format!(" {filename}{indicators} ");
        ComponentOutput::new(text).with_priority(200)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filename_component_id() {
        let component = FilenameComponent::new();
        assert_eq!(component.id(), "filename");
    }

    #[test]
    fn test_filename_with_name() {
        let component = FilenameComponent::new();
        let ctx = ComponentContext {
            filename: Some("main.rs".to_string()),
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.visible);
        assert!(output.text.contains("main.rs"));
    }

    #[test]
    fn test_filename_no_name() {
        let component = FilenameComponent::new();
        let ctx = ComponentContext::default();

        let output = component.render(&ctx);
        assert!(output.text.contains("[No Name]"));
    }

    #[test]
    fn test_filename_modified() {
        let component = FilenameComponent::new();
        let ctx = ComponentContext {
            filename: Some("test.rs".to_string()),
            modified: true,
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.text.contains("[+]"));
    }

    #[test]
    fn test_filename_readonly() {
        let component = FilenameComponent::new();
        let ctx = ComponentContext {
            filename: Some("test.rs".to_string()),
            readonly: true,
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.text.contains("[RO]"));
    }
}
