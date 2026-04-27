#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Client debug-surface subsystem.
//!
//! Provides the `ClientDebugSurface` driver trait, the `DebugObserver`
//! sub-handle trait, and the C-stable ABI sibling types used by
//! runtime-loaded debug drivers. The contract is the driver-side half of
//! the gRPC `DebugStream` protocol: each driver declares its observe/drive
//! schemas statically (via `DebugProbe`) and carries opaque bytes across
//! the CLI/server transport.

pub mod abi;
pub mod client_debug;
pub mod observer;

pub use {
    client_debug::{ClientDebugSurface, DebugError, DebugProbe, MAX_SCHEMAS, SCHEMA_NAME_LEN},
    observer::DebugObserver,
};
