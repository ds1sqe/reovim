//! Command and module IDs for the diagnostics panel.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier for diagnostics-panel.
pub const MODULE: ModuleId = ModuleId::new("diagnostics-panel");

/// Open the diagnostics panel.
pub const TROUBLE_OPEN: CommandId = CommandId::new(MODULE, "trouble-open");

/// Close the diagnostics panel.
pub const TROUBLE_CLOSE: CommandId = CommandId::new(MODULE, "trouble-close");

/// Toggle the diagnostics panel.
pub const TROUBLE_TOGGLE: CommandId = CommandId::new(MODULE, "trouble-toggle");

/// Select next item in the panel.
pub const TROUBLE_NEXT: CommandId = CommandId::new(MODULE, "trouble-next");

/// Select previous item in the panel.
pub const TROUBLE_PREV: CommandId = CommandId::new(MODULE, "trouble-prev");

/// Jump to the selected item's location.
pub const TROUBLE_SELECT: CommandId = CommandId::new(MODULE, "trouble-select");

/// Filter to show only errors.
pub const TROUBLE_FILTER_ERROR: CommandId = CommandId::new(MODULE, "trouble-filter-error");

/// Filter to show only warnings.
pub const TROUBLE_FILTER_WARNING: CommandId = CommandId::new(MODULE, "trouble-filter-warning");

/// Show all diagnostics (reset filter).
pub const TROUBLE_FILTER_ALL: CommandId = CommandId::new(MODULE, "trouble-filter-all");

/// Cycle sort order.
pub const TROUBLE_SORT: CommandId = CommandId::new(MODULE, "trouble-sort");

/// Manually refresh diagnostics.
pub const TROUBLE_REFRESH: CommandId = CommandId::new(MODULE, "trouble-refresh");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
