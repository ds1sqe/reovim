//! Capability identifiers for abstract provides/requires matching.
//!
//! These constants are the single source of truth for capability strings
//! advertised by modules that implement this subsys's trait surface.
//! Consumers import via
//! `reovim_subsys_input::capabilities::<CONST>`.

/// Mode management (a module owns mode lifecycle: enter/exit, default mode).
pub const MODE_MANAGEMENT: &str = "mode-management";

/// Motion commands (a module registers cursor-motion command handlers).
pub const MOTION_COMMANDS: &str = "motion-commands";

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
