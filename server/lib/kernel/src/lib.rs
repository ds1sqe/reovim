#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Core kernel mechanisms for reovim.
//!
//! Linux equivalent: `kernel/`
//!
//! This crate provides pure mechanisms without I/O. Drivers and modules
//! implement policies using these primitives.
//!
//! # Usage
//!
//! All public APIs are accessed via the `api::v1` module:
//!
//! ```ignore
//! use reovim_kernel::api::v1::*;
//!
//! // Check API compatibility
//! check_api_version(Version::new(1, 0, 0))?;
//!
//! // Use kernel types
//! let bus = EventBus::new();
//! pr_info!("kernel initialized");
//! ```
//!
//! # Subsystems
//!
//! Internal subsystems (not directly accessible):
//!
//! - `sched`: Scheduler and runtime (Linux: `kernel/sched/`)
//! - `mm`: Memory management / buffers (Linux: `mm/`)
//! - `ipc`: Inter-process communication / events (Linux: `ipc/`)
//! - `core`: Core primitives (motion, text objects, commands)
//! - `block`: Block operations (undo, transactions)
//! - `printk`: Kernel logging (Linux: `kernel/printk/`)
//! - `debug`: Tracing and metrics
//! - `panic`: Panic handling and recovery
//!
//! All subsystem types are re-exported through `api::v1`.

// ============================================================================
// Public API - the ONLY public module
// ============================================================================

pub mod api;
pub mod testing;

#[cfg(test)]
mod testing_tests;

// ============================================================================
// Internal modules - NOT accessible from outside
// ============================================================================

pub(crate) mod core;
#[allow(dead_code)] // Infrastructure for future use
pub(crate) mod debug;
pub(crate) mod ipc;
pub(crate) mod mm;
#[allow(dead_code)] // Infrastructure for future use
pub(crate) mod panic;
pub(crate) mod printk;
pub(crate) mod sched;

// Re-export arch for internal use only
pub(crate) use reovim_arch as arch;
