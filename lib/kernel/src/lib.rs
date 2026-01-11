//! Core kernel mechanisms for reovim.
//!
//! Linux equivalent: `kernel/`
//!
//! This crate provides pure mechanisms without I/O. Drivers and modules
//! implement policies using these primitives.
//!
//! # Subsystems
//!
//! - `sched`: Scheduler and runtime (Linux: `kernel/sched/`)
//! - `mm`: Memory management / buffers (Linux: `mm/`)
//! - `ipc`: Inter-process communication / events (Linux: `ipc/`)
//! - `core`: Core primitives (motion, text objects, commands)
//! - `block`: Block operations (undo, transactions)
//! - `api`: Stable kernel API (Linux: `include/linux/`)
//! - `printk`: Kernel logging (Linux: `kernel/printk/`)
//! - `debug`: Tracing and metrics
//! - `panic`: Panic handling and recovery

pub mod api;
pub mod block;
pub mod core;
pub mod debug;
pub mod ipc;
pub mod mm;
pub mod panic;
pub mod printk;
pub mod sched;

// Re-export arch for convenience
pub use reovim_arch as arch;
