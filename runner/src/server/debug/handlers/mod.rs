//! Debug RPC handlers.
//!
//! This module contains handlers for debug endpoints:
//! - `info`: version and uptime
//! - `inspect`: kernel state, registers, marks (Phase 2)
//! - `metrics`: performance metrics (Phase 3)
//! - `log`: log access (Phase 4)
//! - `snapshot`: visual debug snapshot (Phase 5)

pub mod info;
pub mod inspect;
pub mod log;
pub mod metrics;
pub mod snapshot;

pub use {
    info::{debug_uptime, debug_version},
    inspect::{debug_kernel_state, debug_marks, debug_mode_stack, debug_registers},
    log::{debug_log_level, debug_log_tail},
    metrics::{debug_handlers, debug_metrics},
    snapshot::debug_visual_snapshot,
};
