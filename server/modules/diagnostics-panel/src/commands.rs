//! Command handlers for the diagnostics panel.

use {
    reovim_driver_command::{ArgKind, ArgSpec, Command, CommandHandler, CommandResult},
    reovim_driver_command_types::CommandContext,
    reovim_driver_session::{ExtensionApi, SessionRuntime},
    reovim_kernel::api::v1::CommandId,
};

use crate::{ids, items::PanelMode, state::DiagnosticsState};

// ============================================================================
// TroubleOpen — :Trouble [mode]
// ============================================================================

/// Open the diagnostics panel.
#[derive(Debug, Clone, Copy, Default)]
pub struct TroubleOpen;

impl Command for TroubleOpen {
    fn id(&self) -> CommandId {
        ids::TROUBLE_OPEN
    }

    fn description(&self) -> &'static str {
        "Open diagnostics panel"
    }

    fn names(&self) -> &[&'static str] {
        &["Trouble"]
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "mode",
            ArgKind::String,
            "Panel mode: diagnostics, quickfix, references, todo",
        )]
    }
}

impl CommandHandler for TroubleOpen {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let mode = args
            .string("mode")
            .and_then(PanelMode::from_arg)
            .unwrap_or(PanelMode::Diagnostics);

        let state = runtime.ext_mut::<DiagnosticsState>();
        state.active = true;
        state.mode = mode;
        CommandResult::Success
    }
}

// ============================================================================
// TroubleClose
// ============================================================================

/// Close the diagnostics panel.
#[derive(Debug, Clone, Copy, Default)]
pub struct TroubleClose;

impl Command for TroubleClose {
    fn id(&self) -> CommandId {
        ids::TROUBLE_CLOSE
    }

    fn description(&self) -> &'static str {
        "Close diagnostics panel"
    }

    fn names(&self) -> &[&'static str] {
        &["TroubleClose"]
    }
}

impl CommandHandler for TroubleClose {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<DiagnosticsState>();
        state.active = false;
        state.items.clear();
        state.selected = 0;
        CommandResult::Success
    }
}

// ============================================================================
// TroubleToggle
// ============================================================================

/// Toggle the diagnostics panel.
#[derive(Debug, Clone, Copy, Default)]
pub struct TroubleToggle;

impl Command for TroubleToggle {
    fn id(&self) -> CommandId {
        ids::TROUBLE_TOGGLE
    }

    fn description(&self) -> &'static str {
        "Toggle diagnostics panel"
    }
}

impl CommandHandler for TroubleToggle {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<DiagnosticsState>();
        state.active = !state.active;
        if !state.active {
            state.items.clear();
            state.selected = 0;
        }
        CommandResult::Success
    }
}

// ============================================================================
// Navigation commands
// ============================================================================

/// Select next item in the panel.
#[derive(Debug, Clone, Copy, Default)]
pub struct TroubleNext;

impl Command for TroubleNext {
    fn id(&self) -> CommandId {
        ids::TROUBLE_NEXT
    }

    fn description(&self) -> &'static str {
        "Next diagnostic item"
    }
}

impl CommandHandler for TroubleNext {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<DiagnosticsState>();
        state.select_next();
        CommandResult::Success
    }
}

/// Select previous item in the panel.
#[derive(Debug, Clone, Copy, Default)]
pub struct TroublePrev;

impl Command for TroublePrev {
    fn id(&self) -> CommandId {
        ids::TROUBLE_PREV
    }

    fn description(&self) -> &'static str {
        "Previous diagnostic item"
    }
}

impl CommandHandler for TroublePrev {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<DiagnosticsState>();
        state.select_prev();
        CommandResult::Success
    }
}

/// Jump to the selected item's location.
#[derive(Debug, Clone, Copy, Default)]
pub struct TroubleSelect;

impl Command for TroubleSelect {
    fn id(&self) -> CommandId {
        ids::TROUBLE_SELECT
    }

    fn description(&self) -> &'static str {
        "Jump to selected diagnostic"
    }
}

impl CommandHandler for TroubleSelect {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<DiagnosticsState>();
        let _item = state.selected_item().cloned();
        // Jump logic will use BufferApi/ModeApi when available in integration
        CommandResult::Success
    }
}

// ============================================================================
// Filter commands
// ============================================================================

/// Filter to show only errors.
#[derive(Debug, Clone, Copy, Default)]
pub struct TroubleFilterError;

impl Command for TroubleFilterError {
    fn id(&self) -> CommandId {
        ids::TROUBLE_FILTER_ERROR
    }

    fn description(&self) -> &'static str {
        "Show only errors"
    }
}

impl CommandHandler for TroubleFilterError {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<DiagnosticsState>();
        state.severity_filter = crate::items::SeverityFilter::errors_only();
        state.refilter_and_sort();
        CommandResult::Success
    }
}

/// Filter to show only warnings.
#[derive(Debug, Clone, Copy, Default)]
pub struct TroubleFilterWarning;

impl Command for TroubleFilterWarning {
    fn id(&self) -> CommandId {
        ids::TROUBLE_FILTER_WARNING
    }

    fn description(&self) -> &'static str {
        "Show only warnings"
    }
}

impl CommandHandler for TroubleFilterWarning {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<DiagnosticsState>();
        state.severity_filter = crate::items::SeverityFilter::warnings_only();
        state.refilter_and_sort();
        CommandResult::Success
    }
}

/// Show all diagnostics (reset filter).
#[derive(Debug, Clone, Copy, Default)]
pub struct TroubleFilterAll;

impl Command for TroubleFilterAll {
    fn id(&self) -> CommandId {
        ids::TROUBLE_FILTER_ALL
    }

    fn description(&self) -> &'static str {
        "Show all severities"
    }
}

impl CommandHandler for TroubleFilterAll {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<DiagnosticsState>();
        state.severity_filter = crate::items::SeverityFilter::all();
        state.refilter_and_sort();
        CommandResult::Success
    }
}

// ============================================================================
// Sort command
// ============================================================================

/// Cycle sort order.
#[derive(Debug, Clone, Copy, Default)]
pub struct TroubleSort;

impl Command for TroubleSort {
    fn id(&self) -> CommandId {
        ids::TROUBLE_SORT
    }

    fn description(&self) -> &'static str {
        "Cycle sort order"
    }
}

impl CommandHandler for TroubleSort {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<DiagnosticsState>();
        state.sort_order = state.sort_order.next();
        state.refilter_and_sort();
        CommandResult::Success
    }
}

// ============================================================================
// Refresh command
// ============================================================================

/// Manually refresh diagnostics.
#[derive(Debug, Clone, Copy, Default)]
pub struct TroubleRefresh;

impl Command for TroubleRefresh {
    fn id(&self) -> CommandId {
        ids::TROUBLE_REFRESH
    }

    fn description(&self) -> &'static str {
        "Refresh diagnostics panel"
    }
}

impl CommandHandler for TroubleRefresh {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Refresh is triggered by the bridge tick re-reading DiagnosticSnapshot
        CommandResult::Success
    }
}

// ============================================================================
// Factory
// ============================================================================

/// Create all command handlers for the diagnostics panel module.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(TroubleOpen),
        Box::new(TroubleClose),
        Box::new(TroubleToggle),
        Box::new(TroubleNext),
        Box::new(TroublePrev),
        Box::new(TroubleSelect),
        Box::new(TroubleFilterError),
        Box::new(TroubleFilterWarning),
        Box::new(TroubleFilterAll),
        Box::new(TroubleSort),
        Box::new(TroubleRefresh),
    ]
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
