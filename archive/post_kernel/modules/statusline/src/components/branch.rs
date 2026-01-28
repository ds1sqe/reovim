//! Git branch component.
//!
//! Example component showing how to implement a cached async data component.
//! This component uses internal caching to avoid blocking the render thread.
//!
//! # Cross-Module Registration
//!
//! A git module would register this component during its init:
//!
//! ```ignore
//! // In git module's init():
//! let registry = ctx.services.get_or_create::<ComponentProviderRegistry>();
//! let branch_component = Arc::new(BranchComponent::new());
//!
//! // Register for lookup by statusline
//! registry.register(
//!     ComponentProviderKey::new("branch"),
//!     branch_component.clone(),
//! );
//!
//! // Subscribe to git events to update cache
//! ctx.event_bus.subscribe::<GitBranchChanged>(move |event| {
//!     branch_component.update_branch(Some(event.branch.clone()));
//!     EventResult::Handled
//! });
//! ```

use std::sync::RwLock;

use reovim_driver_display::{ComponentContext, ComponentOutput, ComponentProvider, Style};

/// Git branch component with internal caching.
///
/// This component demonstrates the caching pattern for async data:
/// - Render always uses cached data (fast, non-blocking)
/// - Cache is updated via `update_branch()` from event handlers
pub struct BranchComponent {
    /// Cached branch name (updated via event subscription).
    cached_branch: RwLock<Option<String>>,
    /// Icon to display before branch name.
    icon: &'static str,
}

impl BranchComponent {
    /// Create a new branch component with default settings.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cached_branch: RwLock::new(None),
            icon: " ", // Git branch icon (Nerd Font)
        }
    }

    /// Create a branch component with a custom icon.
    #[must_use]
    pub const fn with_icon(icon: &'static str) -> Self {
        Self {
            cached_branch: RwLock::new(None),
            icon,
        }
    }

    /// Update the cached branch name.
    ///
    /// Called from event handlers when the git branch changes.
    /// This is the only way to update the branch - `render()` never blocks.
    pub fn update_branch(&self, branch: Option<String>) {
        if let Ok(mut cached) = self.cached_branch.write() {
            *cached = branch;
        }
    }

    /// Get the current cached branch name.
    #[must_use]
    pub fn branch(&self) -> Option<String> {
        self.cached_branch.read().ok().and_then(|b| b.clone())
    }
}

impl Default for BranchComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentProvider for BranchComponent {
    fn id(&self) -> &'static str {
        "branch"
    }

    fn render(&self, _ctx: &ComponentContext) -> ComponentOutput {
        // Use cached data - never block here
        self.branch()
            .map_or_else(ComponentOutput::hidden, |branch| {
                ComponentOutput::new(format!("{}{} ", self.icon, branch)).with_priority(100)
            })
    }

    fn style_for_mode(&self, _mode: &str) -> Option<Style> {
        // Branch doesn't change color by mode
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_component_id() {
        let component = BranchComponent::new();
        assert_eq!(component.id(), "branch");
    }

    #[test]
    fn test_branch_hidden_when_no_branch() {
        let component = BranchComponent::new();
        let ctx = ComponentContext::default();

        let output = component.render(&ctx);
        assert!(!output.visible);
    }

    #[test]
    fn test_branch_visible_when_set() {
        let component = BranchComponent::new();
        component.update_branch(Some("main".to_string()));

        let ctx = ComponentContext::default();
        let output = component.render(&ctx);

        assert!(output.visible);
        assert!(output.text.contains("main"));
    }

    #[test]
    fn test_branch_with_icon() {
        let component = BranchComponent::with_icon("🌿 ");
        component.update_branch(Some("feature".to_string()));

        let ctx = ComponentContext::default();
        let output = component.render(&ctx);

        assert!(output.text.contains("🌿"));
        assert!(output.text.contains("feature"));
    }

    #[test]
    fn test_branch_can_be_cleared() {
        let component = BranchComponent::new();
        component.update_branch(Some("main".to_string()));

        let ctx = ComponentContext::default();
        let output = component.render(&ctx);
        assert!(output.visible);

        // Clear the branch
        component.update_branch(None);
        let output = component.render(&ctx);
        assert!(!output.visible);
    }
}
