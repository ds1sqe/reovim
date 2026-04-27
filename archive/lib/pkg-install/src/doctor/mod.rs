//! Library-root audit: detect and (optionally) remediate the four
//! fault classes (unloadable, orphan, missing, drift).

mod detect;
mod repair;
mod report;
mod scan;

pub use self::{
    detect::audit,
    repair::{RepairOutcome, repair},
    report::{DoctorReport, DriftFinding, MissingFinding, OrphanFinding, UnloadableFinding},
};
