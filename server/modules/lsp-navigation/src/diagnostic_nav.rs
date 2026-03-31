//! Diagnostic navigation commands: `]d`/`[d`, `]e`/`[e`, `]w`/`[w`.
//!
//!
//! Navigate to next/previous diagnostic in the current buffer,
//! optionally filtered by severity.

use {
    reovim_driver_command::{Command, CommandHandler, CommandResult},
    reovim_driver_command_types::CommandContext,
    reovim_driver_lsp::diagnostic_snapshot::{DiagnosticSeverity, DiagnosticSnapshot},
    reovim_driver_session::{BufferApi, ChangeTracker, ExtensionApi, SessionRuntime},
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

// ============================================================================
// Navigation direction
// ============================================================================

#[derive(Clone, Copy)]
enum NavDirection {
    Next,
    Prev,
}

/// Optional severity filter for diagnostic navigation.
#[derive(Clone, Copy)]
enum SeverityFilter {
    /// All diagnostics.
    Any,
    /// Only errors.
    Error,
    /// Only warnings.
    Warning,
}

// ============================================================================
// Command structs
// ============================================================================

macro_rules! define_diag_nav {
    ($name:ident, $id:expr, $desc:expr, $dir:expr, $filter:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl Command for $name {
            fn id(&self) -> CommandId {
                $id
            }

            fn description(&self) -> &'static str {
                $desc
            }
        }

        impl CommandHandler for $name {
            #[cfg_attr(coverage_nightly, coverage(off))]
            fn execute(
                &self,
                runtime: &mut SessionRuntime<'_>,
                _args: &CommandContext,
            ) -> CommandResult {
                navigate_diagnostic(runtime, $dir, $filter)
            }
        }
    };
}

define_diag_nav!(
    NextDiagnostic,
    ids::NEXT_DIAGNOSTIC,
    "Next diagnostic",
    NavDirection::Next,
    SeverityFilter::Any
);
define_diag_nav!(
    PrevDiagnostic,
    ids::PREV_DIAGNOSTIC,
    "Previous diagnostic",
    NavDirection::Prev,
    SeverityFilter::Any
);
define_diag_nav!(
    NextError,
    ids::NEXT_ERROR,
    "Next error",
    NavDirection::Next,
    SeverityFilter::Error
);
define_diag_nav!(
    PrevError,
    ids::PREV_ERROR,
    "Previous error",
    NavDirection::Prev,
    SeverityFilter::Error
);
define_diag_nav!(
    NextWarning,
    ids::NEXT_WARNING,
    "Next warning",
    NavDirection::Next,
    SeverityFilter::Warning
);
define_diag_nav!(
    PrevWarning,
    ids::PREV_WARNING,
    "Previous warning",
    NavDirection::Prev,
    SeverityFilter::Warning
);

// ============================================================================
// Navigation logic
// ============================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
fn navigate_diagnostic(
    runtime: &mut SessionRuntime<'_>,
    direction: NavDirection,
    filter: SeverityFilter,
) -> CommandResult {
    let Some(buf_id) = runtime.active_buffer() else {
        return CommandResult::Success;
    };

    // Extract diagnostic positions from shared snapshot (drops borrow before mutating).
    let target_pos = {
        let Some(snap) = runtime.shared_ext::<DiagnosticSnapshot>() else {
            return CommandResult::Success;
        };

        let mut positions: Vec<(u32, u32, DiagnosticSeverity)> = snap
            .entries
            .iter()
            .filter(|e| e.buffer_id == buf_id.as_usize() as u64)
            .flat_map(|e| &e.diagnostics)
            .filter(|d| matches_filter(d.severity, filter))
            .map(|d| (d.start_line, d.start_col, d.severity))
            .collect();

        if positions.is_empty() {
            return CommandResult::Success;
        }

        positions.sort_unstable_by_key(|&(l, c, _)| (l, c));

        let (cursor_line, cursor_col) = {
            let Some(win) = runtime.windows().active() else {
                return CommandResult::Success;
            };
            #[allow(clippy::cast_possible_truncation)]
            (win.cursor.line as u32, win.cursor.column as u32)
        };

        match direction {
            NavDirection::Next => find_next_pos(&positions, cursor_line, cursor_col),
            NavDirection::Prev => find_prev_pos(&positions, cursor_line, cursor_col),
        }
    };

    if let Some((line, col)) = target_pos {
        if let Some(win) = runtime.windows_mut().active_mut() {
            win.cursor.line = line as usize;
            win.cursor.column = col as usize;
        }
        runtime.record_cursor_move(buf_id);
    }

    CommandResult::Success
}

fn matches_filter(severity: DiagnosticSeverity, filter: SeverityFilter) -> bool {
    match filter {
        SeverityFilter::Any => true,
        SeverityFilter::Error => severity == DiagnosticSeverity::Error,
        SeverityFilter::Warning => severity == DiagnosticSeverity::Warning,
    }
}

/// Find the next diagnostic position after (`cursor_line`, `cursor_col`), wrapping.
fn find_next_pos(
    positions: &[(u32, u32, DiagnosticSeverity)],
    cursor_line: u32,
    cursor_col: u32,
) -> Option<(u32, u32)> {
    positions
        .iter()
        .find(|(l, c, _)| (*l, *c) > (cursor_line, cursor_col))
        .or_else(|| positions.first())
        .map(|(l, c, _)| (*l, *c))
}

/// Find the previous diagnostic position before (`cursor_line`, `cursor_col`), wrapping.
fn find_prev_pos(
    positions: &[(u32, u32, DiagnosticSeverity)],
    cursor_line: u32,
    cursor_col: u32,
) -> Option<(u32, u32)> {
    positions
        .iter()
        .rev()
        .find(|(l, c, _)| (*l, *c) < (cursor_line, cursor_col))
        .or_else(|| positions.last())
        .map(|(l, c, _)| (*l, *c))
}

/// Create all diagnostic navigation command handlers.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(NextDiagnostic),
        Box::new(PrevDiagnostic),
        Box::new(NextError),
        Box::new(PrevError),
        Box::new(NextWarning),
        Box::new(PrevWarning),
    ]
}

#[cfg(test)]
#[path = "diagnostic_nav_tests.rs"]
mod tests;
