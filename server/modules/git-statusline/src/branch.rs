//! Git branch statusline component.

use std::{path::Path, sync::Arc};

use {
    reovim_driver_display::statusline::{ComponentContext, ComponentOutput, ComponentProvider},
    reovim_driver_git::GitProvider,
};

/// Statusline component that renders the current git branch name.
///
/// Derives the working directory from `ComponentContext::filepath`
/// and calls `GitProvider::current_branch()`.
pub struct BranchComponent {
    provider: Arc<dyn GitProvider>,
}

impl BranchComponent {
    /// Create a new branch component.
    pub fn new(provider: Arc<dyn GitProvider>) -> Self {
        Self { provider }
    }
}

impl ComponentProvider for BranchComponent {
    fn id(&self) -> &'static str {
        "branch"
    }

    fn render(&self, ctx: &ComponentContext) -> ComponentOutput {
        let Some(filepath) = &ctx.filepath else {
            return ComponentOutput::hidden();
        };

        let path = Path::new(filepath);
        let cwd = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => return ComponentOutput::hidden(),
        };

        self.provider
            .current_branch(cwd)
            .map_or_else(ComponentOutput::hidden, |branch| {
                ComponentOutput::new(format!(" {branch} "))
            })
    }
}

#[cfg(test)]
#[path = "branch_tests.rs"]
mod tests;
