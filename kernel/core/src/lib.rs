#![no_std]
#![forbid(unsafe_code)]
//! Native-state Buffer authority, independent of policy and presentation.
//!
//! Runtime prepares native candidates and bounds outstanding immutable leases.
//! Core retains accepted states and owns their versions and History relations.
//! Callers serialize access; a mutable installation call performs only scalar
//! checks and infallible moves into already-reserved storage. Drop rejected
//! work and eventually the Buffer itself outside that serialization.

extern crate alloc;

mod buffer;
mod config;
mod error;
mod history;
mod lease;
mod staged;

pub use buffer::{Buffer, BufferCommit, Installation};
pub use config::{BufferConfig, BufferLimits};
pub use error::{BufferError, Rejected};
pub use history::{HistorySelection, StagedHistory};
pub use lease::StateLease;
pub use staged::StagedEdit;
