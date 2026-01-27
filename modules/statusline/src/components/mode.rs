//! Mode component for statusline.
//!
//! Displays the current editor mode (NORMAL, INSERT, VISUAL, etc.).

use reovim_driver_display::{Color, ComponentContext, ComponentOutput, ComponentProvider, Style};

/// Mode indicator component.
///
/// Displays the current mode with mode-specific styling.
pub struct ModeComponent;

impl ModeComponent {
    /// Create a new mode component.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Get display text for a mode.
    fn mode_text(mode: &str) -> &'static str {
        // Normalize mode names
        match mode.to_uppercase().as_str() {
            "INSERT" => " INSERT ",
            "VISUAL" => " VISUAL ",
            "VISUAL_LINE" | "V-LINE" => " V-LINE ",
            "VISUAL_BLOCK" | "V-BLOCK" => " V-BLOCK ",
            "REPLACE" => " REPLACE ",
            "COMMAND" => " COMMAND ",
            "SELECT" => " SELECT ",
            "TERMINAL" => " TERMINAL ",
            "OPERATOR" | "OPERATOR_PENDING" => " O-PEND ",
            // Default to NORMAL for unknown modes
            _ => " NORMAL ",
        }
    }
}

impl Default for ModeComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentProvider for ModeComponent {
    fn id(&self) -> &'static str {
        "mode"
    }

    fn render(&self, ctx: &ComponentContext) -> ComponentOutput {
        let text = Self::mode_text(&ctx.mode);
        let style = self.style_for_mode(&ctx.mode);

        let mut output = ComponentOutput::new(text)
            .with_min_width(9)
            .with_priority(255); // Mode is highest priority
        if let Some(s) = style {
            output = output.with_style(s);
        }
        output
    }

    fn style_for_mode(&self, mode: &str) -> Option<Style> {
        let style = match mode.to_uppercase().as_str() {
            "INSERT" | "TERMINAL" => Style::new().bg(Color::Green).fg(Color::Black),
            "VISUAL" | "VISUAL_LINE" | "VISUAL_BLOCK" | "V-LINE" | "V-BLOCK" | "OPERATOR"
            | "OPERATOR_PENDING" => Style::new().bg(Color::Magenta).fg(Color::Black),
            "REPLACE" => Style::new().bg(Color::Red).fg(Color::White),
            "COMMAND" => Style::new().bg(Color::Yellow).fg(Color::Black),
            "SELECT" => Style::new().bg(Color::Cyan).fg(Color::Black),
            // Default to blue for NORMAL and unknown modes
            _ => Style::new().bg(Color::Blue).fg(Color::Black),
        };
        Some(style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_component_id() {
        let component = ModeComponent::new();
        assert_eq!(component.id(), "mode");
    }

    #[test]
    fn test_mode_component_render_normal() {
        let component = ModeComponent::new();
        let ctx = ComponentContext {
            mode: "NORMAL".to_string(),
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.visible);
        assert!(output.text.contains("NORMAL"));
        assert!(output.style.is_some());
    }

    #[test]
    fn test_mode_component_render_insert() {
        let component = ModeComponent::new();
        let ctx = ComponentContext {
            mode: "INSERT".to_string(),
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.text.contains("INSERT"));
    }

    #[test]
    fn test_mode_style_varies_by_mode() {
        let component = ModeComponent::new();

        let normal_style = component.style_for_mode("NORMAL");
        let insert_style = component.style_for_mode("INSERT");

        // Styles should be different
        assert!(normal_style.is_some());
        assert!(insert_style.is_some());
        assert_ne!(normal_style.unwrap().bg, insert_style.unwrap().bg);
    }
}
