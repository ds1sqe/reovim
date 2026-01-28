//! Statusline configuration.
//!
//! Defines configuration types for the statusline module.
//!
//! # Component Conditions
//!
//! Components can have visibility conditions that control when they appear:
//!
//! ```ignore
//! let config = ComponentConfig::new(ComponentId::Branch)
//!     .with_condition(ComponentCondition::WhenNotEmpty("git_branch"));
//!
//! let config = ComponentConfig::new(ComponentId::Encoding)
//!     .with_condition(ComponentCondition::WhenNot("encoding", "utf-8"));
//! ```

use reovim_driver_display::{ComponentContext, HeightConfig, SectionId, StatuslineSeparator};

/// Configuration for the statusline module.
#[derive(Debug, Clone)]
pub struct StatuslineConfig {
    /// Which components to show in each section.
    pub sections: SectionConfig,
    /// Separator style.
    pub separator: StatuslineSeparator,
    /// Whether to use powerline coloring for separators.
    pub powerline_coloring: bool,
    /// Height configuration for dynamic statusline height.
    pub height: HeightConfig,
}

impl Default for StatuslineConfig {
    fn default() -> Self {
        Self {
            sections: SectionConfig::default(),
            separator: StatuslineSeparator::POWERLINE_ARROW,
            powerline_coloring: true,
            height: HeightConfig::default(),
        }
    }
}

/// Configuration for which components appear in each section.
#[derive(Debug, Clone)]
pub struct SectionConfig {
    /// Components for section A (leftmost, typically mode).
    pub a: Vec<ComponentId>,
    /// Components for section B (left, typically git branch).
    pub b: Vec<ComponentId>,
    /// Components for section C (left-center, typically filename).
    pub c: Vec<ComponentId>,
    /// Components for section X (right-center, typically encoding).
    pub x: Vec<ComponentId>,
    /// Components for section Y (right, typically filetype).
    pub y: Vec<ComponentId>,
    /// Components for section Z (rightmost, typically position).
    pub z: Vec<ComponentId>,
}

impl SectionConfig {
    /// Get components for a section by ID.
    #[must_use]
    pub fn get(&self, section: SectionId) -> &[ComponentId] {
        match section {
            SectionId::A => &self.a,
            SectionId::B => &self.b,
            SectionId::C => &self.c,
            SectionId::X => &self.x,
            SectionId::Y => &self.y,
            SectionId::Z => &self.z,
        }
    }
}

impl Default for SectionConfig {
    fn default() -> Self {
        Self {
            a: vec![ComponentId::Mode],
            b: vec![], // Future: git branch
            c: vec![ComponentId::Filename],
            x: vec![], // Future: encoding
            y: vec![ComponentId::Filetype],
            z: vec![ComponentId::Position],
        }
    }
}

/// Built-in component identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentId {
    /// Mode indicator (NORMAL, INSERT, etc.).
    Mode,
    /// Git branch name.
    Branch,
    /// Filename with modified indicator.
    Filename,
    /// File encoding (utf-8, etc.).
    Encoding,
    /// File format (unix, dos, mac).
    FileFormat,
    /// Filetype (rust, python, etc.).
    Filetype,
    /// Cursor position (line:col).
    Position,
    /// Percentage through file.
    Progress,
    /// Diagnostic counts (errors, warnings).
    Diagnostics,
}

/// Conditions for component visibility.
///
/// Components can be conditionally shown based on editor state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ComponentCondition {
    /// Always show the component.
    #[default]
    Always,
    /// Show only when the buffer has been modified.
    WhenModified,
    /// Show only when the buffer is readonly.
    WhenReadonly,
    /// Show only when there's a git branch.
    WhenInGitRepo,
    /// Show only when there are diagnostics.
    WhenHasDiagnostics,
    /// Show only when there are diagnostic errors.
    WhenHasErrors,
    /// Show only when encoding is not UTF-8.
    WhenNonUtf8,
    /// Show only in specific modes.
    InModes(Vec<String>),
    /// Show only when NOT in specific modes.
    NotInModes(Vec<String>),
    /// Show only when file has a specific extension.
    WhenFileExtension(Vec<String>),
}

impl ComponentCondition {
    /// Evaluate the condition against the current context.
    #[must_use]
    pub fn evaluate(&self, ctx: &ComponentContext) -> bool {
        match self {
            Self::Always => true,
            Self::WhenModified => ctx.modified,
            Self::WhenReadonly => ctx.readonly,
            Self::WhenInGitRepo => ctx.git_branch.is_some(),
            Self::WhenHasDiagnostics => ctx.diagnostics.as_ref().is_some_and(|d| !d.is_empty()),
            Self::WhenHasErrors => ctx.diagnostics.as_ref().is_some_and(|d| d.errors > 0),
            Self::WhenNonUtf8 => !ctx.encoding.is_empty() && ctx.encoding.to_lowercase() != "utf-8",
            Self::InModes(modes) => modes.iter().any(|m| m.eq_ignore_ascii_case(&ctx.mode)),
            Self::NotInModes(modes) => !modes.iter().any(|m| m.eq_ignore_ascii_case(&ctx.mode)),
            Self::WhenFileExtension(exts) => ctx
                .filename
                .as_ref()
                .is_some_and(|name| exts.iter().any(|ext| name.ends_with(ext))),
        }
    }
}

/// Configuration for a single component.
#[derive(Debug, Clone)]
pub struct ComponentConfig {
    /// The component to display.
    pub id: ComponentId,
    /// Condition for showing the component.
    pub condition: ComponentCondition,
}

impl ComponentConfig {
    /// Create a new component config that always shows.
    #[must_use]
    pub const fn new(id: ComponentId) -> Self {
        Self {
            id,
            condition: ComponentCondition::Always,
        }
    }

    /// Set the visibility condition.
    #[must_use]
    pub fn with_condition(mut self, condition: ComponentCondition) -> Self {
        self.condition = condition;
        self
    }

    /// Check if the component should be shown.
    #[must_use]
    pub fn should_show(&self, ctx: &ComponentContext) -> bool {
        self.condition.evaluate(ctx)
    }
}

impl From<ComponentId> for ComponentConfig {
    fn from(id: ComponentId) -> Self {
        Self::new(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = StatuslineConfig::default();
        assert!(config.powerline_coloring);
    }

    #[test]
    fn test_default_section_config() {
        let sections = SectionConfig::default();

        // Section A should have Mode
        assert_eq!(sections.a, vec![ComponentId::Mode]);

        // Section B should be empty (no git yet)
        assert!(sections.b.is_empty());

        // Section C should have Filename
        assert_eq!(sections.c, vec![ComponentId::Filename]);

        // Section Z should have Position
        assert_eq!(sections.z, vec![ComponentId::Position]);
    }

    #[test]
    fn test_section_config_get() {
        let sections = SectionConfig::default();

        assert_eq!(sections.get(SectionId::A), &[ComponentId::Mode]);
        assert_eq!(sections.get(SectionId::Z), &[ComponentId::Position]);
    }

    #[test]
    fn test_condition_always() {
        let condition = ComponentCondition::Always;
        let ctx = ComponentContext::default();
        assert!(condition.evaluate(&ctx));
    }

    #[test]
    fn test_condition_when_modified() {
        let condition = ComponentCondition::WhenModified;

        let ctx_unmodified = ComponentContext::default();
        assert!(!condition.evaluate(&ctx_unmodified));

        let ctx_modified = ComponentContext {
            modified: true,
            ..Default::default()
        };
        assert!(condition.evaluate(&ctx_modified));
    }

    #[test]
    fn test_condition_in_modes() {
        let condition =
            ComponentCondition::InModes(vec!["INSERT".to_string(), "VISUAL".to_string()]);

        let ctx_normal = ComponentContext {
            mode: "NORMAL".to_string(),
            ..Default::default()
        };
        assert!(!condition.evaluate(&ctx_normal));

        let ctx_insert = ComponentContext {
            mode: "INSERT".to_string(),
            ..Default::default()
        };
        assert!(condition.evaluate(&ctx_insert));

        // Case insensitive
        let ctx_visual_lower = ComponentContext {
            mode: "visual".to_string(),
            ..Default::default()
        };
        assert!(condition.evaluate(&ctx_visual_lower));
    }

    #[test]
    fn test_condition_not_in_modes() {
        let condition = ComponentCondition::NotInModes(vec!["INSERT".to_string()]);

        let ctx_normal = ComponentContext {
            mode: "NORMAL".to_string(),
            ..Default::default()
        };
        assert!(condition.evaluate(&ctx_normal));

        let ctx_insert = ComponentContext {
            mode: "INSERT".to_string(),
            ..Default::default()
        };
        assert!(!condition.evaluate(&ctx_insert));
    }

    #[test]
    fn test_condition_when_non_utf8() {
        let condition = ComponentCondition::WhenNonUtf8;

        let ctx_utf8 = ComponentContext {
            encoding: "utf-8".to_string(),
            ..Default::default()
        };
        assert!(!condition.evaluate(&ctx_utf8));

        let ctx_latin1 = ComponentContext {
            encoding: "latin1".to_string(),
            ..Default::default()
        };
        assert!(condition.evaluate(&ctx_latin1));
    }

    #[test]
    fn test_component_config() {
        let config = ComponentConfig::new(ComponentId::Encoding)
            .with_condition(ComponentCondition::WhenNonUtf8);

        let ctx_utf8 = ComponentContext {
            encoding: "utf-8".to_string(),
            ..Default::default()
        };
        assert!(!config.should_show(&ctx_utf8));

        let ctx_latin1 = ComponentContext {
            encoding: "latin1".to_string(),
            ..Default::default()
        };
        assert!(config.should_show(&ctx_latin1));
    }

    #[test]
    fn test_component_config_from_id() {
        let config: ComponentConfig = ComponentId::Mode.into();
        assert_eq!(config.id, ComponentId::Mode);
        assert_eq!(config.condition, ComponentCondition::Always);
    }
}
