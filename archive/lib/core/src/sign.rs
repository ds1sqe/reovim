//! Generic sign column system for gutter markers (diagnostics, git, breakpoints)

use crate::highlight::Style;

/// A sign displayed in the gutter
///
/// Generic abstraction used by plugins to mark lines. Examples:
/// - LSP diagnostics: "E", "W", "I", "H" with colors
/// - Git changes: "+", "~", "-" for added/modified/deleted
/// - Debugger: "●" for breakpoints
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sign {
    /// Visual icon (1-2 characters typical)
    pub icon: String,

    /// Style for rendering the icon
    pub style: Style,

    /// Priority (higher = wins when multiple signs on same line)
    ///
    /// Suggested ranges:
    /// - 0-99: Git signs (lowest priority)
    /// - 100-199: Bookmarks/marks
    /// - 200-299: Breakpoints
    /// - 300-399: LSP diagnostics (highest priority)
    pub priority: u32,
}

impl Sign {
    /// Create a new sign
    #[must_use]
    pub const fn new(icon: String, style: Style, priority: u32) -> Self {
        Self {
            icon,
            style,
            priority,
        }
    }
}

/// Per-line sign (None = no sign on this line)
pub type LineSign = Option<Sign>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_creation() {
        let sign = Sign::new("●".to_string(), Style::new(), 100);
        assert_eq!(sign.icon, "●");
        assert_eq!(sign.priority, 100);
    }

    #[test]
    fn test_sign_equality() {
        let sign1 = Sign {
            icon: "E".to_string(),
            style: Style::new(),
            priority: 304,
        };
        let sign2 = Sign {
            icon: "E".to_string(),
            style: Style::new(),
            priority: 304,
        };
        assert_eq!(sign1, sign2);
    }

    #[test]
    fn test_sign_priority_ordering() {
        let error_sign = Sign {
            icon: "●".to_string(),
            style: Style::new(),
            priority: 304,
        };
        let warning_sign = Sign {
            icon: "◐".to_string(),
            style: Style::new(),
            priority: 303,
        };
        let git_sign = Sign {
            icon: "+".to_string(),
            style: Style::new(),
            priority: 50,
        };

        // Higher priority should win
        assert!(error_sign.priority > warning_sign.priority);
        assert!(warning_sign.priority > git_sign.priority);
    }

    #[test]
    fn test_line_sign_option() {
        let sign: LineSign = Some(Sign::new("●".to_string(), Style::new(), 100));
        assert!(sign.is_some());

        let no_sign: LineSign = None;
        assert!(no_sign.is_none());
    }
}
