//! Command handlers for module manager navigation (#622).
//!
//! Handles open, close, next, prev, toggle-filter, toggle-detail actions
//! that mutate per-client `ModuleManagerState`.

use {
    reovim_driver_command::CommandHandler,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_session::{ExtensionApi, ModeApi, SessionRuntime, TransitionContext},
    reovim_kernel::api::v1::CommandId,
    reovim_subsys_module_loader::report::ModuleLoadReport,
};

use crate::{
    ids,
    modes::ManagerMode,
    state::{ModuleEntry, ModuleManagerState, ModuleStatus},
};

// ============================================================================
// Open command
// ============================================================================

/// Open the module manager panel.
#[derive(Debug, Clone, Copy, Default)]
pub struct Open;

impl reovim_driver_command::Command for Open {
    fn id(&self) -> CommandId {
        ids::OPEN
    }

    fn description(&self) -> &'static str {
        "Open module manager panel"
    }
}

impl CommandHandler for Open {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let entries = collect_module_entries(runtime);

        let state = runtime.ext_mut::<ModuleManagerState>();
        state.active = true;
        state.modules = entries;
        state.selected = 0;
        state.detail_visible = false;

        runtime.push_mode(ManagerMode::MANAGER_ID, TransitionContext::new());
        CommandResult::Success
    }
}

// ============================================================================
// Close command
// ============================================================================

/// Close the module manager panel.
#[derive(Debug, Clone, Copy, Default)]
pub struct Close;

impl reovim_driver_command::Command for Close {
    fn id(&self) -> CommandId {
        ids::CLOSE
    }

    fn description(&self) -> &'static str {
        "Close module manager panel"
    }
}

impl CommandHandler for Close {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ModuleManagerState>();
        state.active = false;

        let _ = runtime.pop_mode(None);
        CommandResult::Success
    }
}

// ============================================================================
// Navigation commands
// ============================================================================

/// Move selection to the next module.
#[derive(Debug, Clone, Copy, Default)]
pub struct Next;

impl reovim_driver_command::Command for Next {
    fn id(&self) -> CommandId {
        ids::NEXT
    }

    fn description(&self) -> &'static str {
        "Select next module"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Next {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ModuleManagerState>();
        state.next();
        CommandResult::Success
    }
}

/// Move selection to the previous module.
#[derive(Debug, Clone, Copy, Default)]
pub struct Prev;

impl reovim_driver_command::Command for Prev {
    fn id(&self) -> CommandId {
        ids::PREV
    }

    fn description(&self) -> &'static str {
        "Select previous module"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Prev {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ModuleManagerState>();
        state.prev();
        CommandResult::Success
    }
}

/// Cycle through filter views.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleFilter;

impl reovim_driver_command::Command for ToggleFilter {
    fn id(&self) -> CommandId {
        ids::TOGGLE_FILTER
    }

    fn description(&self) -> &'static str {
        "Cycle module filter (All/Loaded/Disabled/Failed)"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for ToggleFilter {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ModuleManagerState>();
        state.toggle_filter();
        CommandResult::Success
    }
}

/// Toggle detail view for the selected module.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleDetail;

impl reovim_driver_command::Command for ToggleDetail {
    fn id(&self) -> CommandId {
        ids::TOGGLE_DETAIL
    }

    fn description(&self) -> &'static str {
        "Toggle module detail sidebar"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for ToggleDetail {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<ModuleManagerState>();
        state.toggle_detail();
        CommandResult::Success
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Collect module entries from the `ModuleLoadReport`.
fn collect_module_entries(runtime: &SessionRuntime<'_>) -> Vec<ModuleEntry> {
    let Some(report) = runtime.kernel().services.get::<ModuleLoadReport>() else {
        return Vec::new();
    };

    let mut entries = Vec::new();

    for id in &report.loaded {
        entries.push(ModuleEntry {
            id: id.as_str().to_string(),
            version: String::new(),
            status: ModuleStatus::Loaded,
            reason: None,
        });
    }

    for id in &report.disabled {
        entries.push(ModuleEntry {
            id: id.as_str().to_string(),
            version: String::new(),
            status: ModuleStatus::Disabled,
            reason: Some("disabled by config".into()),
        });
    }

    for (id, reason) in &report.failed {
        entries.push(ModuleEntry {
            id: id.as_str().to_string(),
            version: String::new(),
            status: ModuleStatus::Failed,
            reason: Some(reason.clone()),
        });
    }

    entries
}

/// Collect all command handlers for registration.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(Open),
        Box::new(Close),
        Box::new(Next),
        Box::new(Prev),
        Box::new(ToggleFilter),
        Box::new(ToggleDetail),
    ]
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
