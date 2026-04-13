//! Git branch statusline component.

use std::{path::Path, sync::Arc};

use {
    reovim_subsys_git::GitProvider,
    reovim_subsys_statusline::{ComponentData, ComponentDataContext, ComponentDataProvider},
};

/// Statusline data provider that produces the current git branch name.
///
/// Derives the working directory from `ComponentDataContext::filepath`
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

impl ComponentDataProvider for BranchComponent {
    fn id(&self) -> &'static str {
        "branch"
    }

    fn data(&self, ctx: &ComponentDataContext) -> ComponentData {
        let Some(filepath) = &ctx.filepath else {
            return ComponentData::hidden();
        };

        let path = Path::new(filepath);
        let cwd = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => return ComponentData::hidden(),
        };

        self.provider
            .current_branch(cwd)
            .map_or_else(ComponentData::hidden, |branch| ComponentData::new(format!(" {branch} ")))
    }
}

#[cfg(test)]
#[path = "branch_tests.rs"]
mod tests;
