//! Position component for statusline.
//!
//! Displays the cursor position (line:column).

use reovim_driver_display::{ComponentContext, ComponentOutput, ComponentProvider};

/// Position component.
///
/// Displays cursor line and column, optionally with percentage through file.
pub struct PositionComponent {
    /// Whether to show percentage through file.
    show_percentage: bool,
}

impl PositionComponent {
    /// Create a new position component.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            show_percentage: true,
        }
    }

    /// Create a position component without percentage.
    #[must_use]
    pub const fn without_percentage() -> Self {
        Self {
            show_percentage: false,
        }
    }

    /// Calculate percentage through file.
    fn percentage(line: usize, total: usize) -> String {
        if total == 0 {
            return "Top".to_string();
        }
        if line <= 1 {
            return "Top".to_string();
        }
        if line >= total {
            return "Bot".to_string();
        }

        // Calculate percentage (safe: line < total and total > 0)
        let pct = (line * 100) / total;
        format!("{pct}%")
    }
}

impl Default for PositionComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentProvider for PositionComponent {
    fn id(&self) -> &'static str {
        "position"
    }

    fn render(&self, ctx: &ComponentContext) -> ComponentOutput {
        let line = ctx.line.max(1);
        let col = ctx.column.max(1);

        let text = if self.show_percentage {
            let pct = Self::percentage(line, ctx.total_lines);
            format!(" {line}:{col} {pct} ")
        } else {
            format!(" {line}:{col} ")
        };

        ComponentOutput::new(text).with_priority(150)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_component_id() {
        let component = PositionComponent::new();
        assert_eq!(component.id(), "position");
    }

    #[test]
    fn test_position_render() {
        let component = PositionComponent::new();
        let ctx = ComponentContext {
            line: 42,
            column: 15,
            total_lines: 100,
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.visible);
        assert!(output.text.contains("42"));
        assert!(output.text.contains("15"));
    }

    #[test]
    fn test_position_percentage_top() {
        let component = PositionComponent::new();
        let ctx = ComponentContext {
            line: 1,
            column: 1,
            total_lines: 100,
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.text.contains("Top"));
    }

    #[test]
    fn test_position_percentage_bottom() {
        let component = PositionComponent::new();
        let ctx = ComponentContext {
            line: 100,
            column: 1,
            total_lines: 100,
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.text.contains("Bot"));
    }

    #[test]
    fn test_position_percentage_middle() {
        let component = PositionComponent::new();
        let ctx = ComponentContext {
            line: 50,
            column: 1,
            total_lines: 100,
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.text.contains("50%"));
    }

    #[test]
    fn test_position_without_percentage() {
        let component = PositionComponent::without_percentage();
        let ctx = ComponentContext {
            line: 42,
            column: 15,
            total_lines: 100,
            ..Default::default()
        };

        let output = component.render(&ctx);
        assert!(output.text.contains("42:15"));
        assert!(!output.text.contains('%'));
        assert!(!output.text.contains("Top"));
    }

    #[test]
    fn test_position_zero_values() {
        let component = PositionComponent::new();
        let ctx = ComponentContext::default();

        let output = component.render(&ctx);
        // Should show 1:1 for zero values (1-indexed display)
        assert!(output.text.contains("1:1"));
    }
}
