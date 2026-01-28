//! LSP diagnostics component.
//!
//! Example component showing how to display diagnostic counts from LSP.
//! Uses the `DiagnosticCounts` provided in `ComponentContext`.
//!
//! # Cross-Module Registration
//!
//! An LSP module would register this component during its init:
//!
//! ```ignore
//! // In LSP module's init():
//! let registry = ctx.services.get_or_create::<ComponentProviderRegistry>();
//! registry.register(
//!     ComponentProviderKey::new("diagnostics"),
//!     Arc::new(DiagnosticsComponent::new()),
//! );
//!
//! // Update diagnostics in ComponentContext when they change
//! // The session state builder includes diagnostics from LSP
//! ```

use reovim_driver_display::{Color, ComponentContext, ComponentOutput, ComponentProvider, Style};

/// LSP diagnostics component.
///
/// Displays error/warning counts with colored icons.
pub struct DiagnosticsComponent {
    /// Error icon (default: )
    error_icon: &'static str,
    /// Warning icon (default: )
    warn_icon: &'static str,
    /// Info icon (default: )
    info_icon: &'static str,
    /// Hint icon (default: )
    hint_icon: &'static str,
    /// Whether to show zeros (default: false)
    show_zeros: bool,
}

impl DiagnosticsComponent {
    /// Create a new diagnostics component with default settings.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            error_icon: " ",
            warn_icon: " ",
            info_icon: " ",
            hint_icon: " ",
            show_zeros: false,
        }
    }

    /// Set whether to show zero counts.
    #[must_use]
    pub const fn show_zeros(mut self, show: bool) -> Self {
        self.show_zeros = show;
        self
    }

    /// Format a diagnostic count with its icon.
    fn format_count(&self, icon: &str, count: usize, color: Color) -> Option<(String, Style)> {
        if count == 0 && !self.show_zeros {
            return None;
        }
        Some((format!("{icon}{count}"), Style::new().fg(color)))
    }
}

impl Default for DiagnosticsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentProvider for DiagnosticsComponent {
    fn id(&self) -> &'static str {
        "diagnostics"
    }

    fn render(&self, ctx: &ComponentContext) -> ComponentOutput {
        let Some(ref diagnostics) = ctx.diagnostics else {
            return ComponentOutput::hidden();
        };

        if diagnostics.is_empty() && !self.show_zeros {
            return ComponentOutput::hidden();
        }

        let mut parts = Vec::new();

        // Errors (red)
        if let Some((text, _)) = self.format_count(self.error_icon, diagnostics.errors, Color::Red)
        {
            parts.push(text);
        }

        // Warnings (yellow)
        if let Some((text, _)) =
            self.format_count(self.warn_icon, diagnostics.warnings, Color::Yellow)
        {
            parts.push(text);
        }

        // Info (blue)
        if let Some((text, _)) = self.format_count(self.info_icon, diagnostics.info, Color::Blue) {
            parts.push(text);
        }

        // Hints (cyan)
        if let Some((text, _)) = self.format_count(self.hint_icon, diagnostics.hints, Color::Cyan) {
            parts.push(text);
        }

        if parts.is_empty() {
            return ComponentOutput::hidden();
        }

        ComponentOutput::new(format!(" {} ", parts.join(" "))).with_priority(80)
    }
}

#[cfg(test)]
mod tests {
    use reovim_driver_display::DiagnosticCounts;

    use super::*;

    #[test]
    fn test_diagnostics_component_id() {
        let component = DiagnosticsComponent::new();
        assert_eq!(component.id(), "diagnostics");
    }

    #[test]
    fn test_diagnostics_hidden_when_none() {
        let component = DiagnosticsComponent::new();
        let ctx = ComponentContext::default();

        let output = component.render(&ctx);
        assert!(!output.visible);
    }

    #[test]
    fn test_diagnostics_hidden_when_empty() {
        let component = DiagnosticsComponent::new();
        let ctx = ComponentContext {
            diagnostics: Some(DiagnosticCounts::default()),
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(!output.visible);
    }

    #[test]
    fn test_diagnostics_shows_errors() {
        let component = DiagnosticsComponent::new();
        let ctx = ComponentContext {
            diagnostics: Some(DiagnosticCounts {
                errors: 3,
                ..Default::default()
            }),
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.visible);
        assert!(output.text.contains('3'));
    }

    #[test]
    fn test_diagnostics_shows_multiple() {
        let component = DiagnosticsComponent::new();
        let ctx = ComponentContext {
            diagnostics: Some(DiagnosticCounts {
                errors: 2,
                warnings: 5,
                info: 1,
                hints: 0,
            }),
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.visible);
        assert!(output.text.contains('2'));
        assert!(output.text.contains('5'));
        assert!(output.text.contains('1'));
    }

    #[test]
    fn test_diagnostics_show_zeros() {
        let component = DiagnosticsComponent::new().show_zeros(true);
        let ctx = ComponentContext {
            diagnostics: Some(DiagnosticCounts::default()),
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.visible);
        // Should show all zeros
        assert!(output.text.contains('0'));
    }
}
