//! Portable composed policy witnesses, not an authoritative editing engine.

mod chain;
mod families;
pub mod native;
mod policies;
mod receipt_faults;
mod request_faults;
mod snapshot_faults;
mod snapshots;

pub use chain::{Evidence, heading, keyed};
pub use receipt_faults::exercise as exercise_receipt_fences;
pub use request_faults::exercise as exercise_request_fences;
pub use snapshot_faults::exercise as exercise_snapshot_fences;
