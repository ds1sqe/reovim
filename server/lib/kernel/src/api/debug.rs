//! Debug snapshot types and functions.
//!
//! This module provides types and functions for extracting debug-oriented
//! snapshots of kernel state. These are "mechanism" functions that extract
//! data - the policy of what to do with them is handled by runner handlers.
//!
//! # Design Philosophy
//!
//! - **No serde**: Snapshot types are plain Rust structs. Serialization is
//!   handled by the protocol crate.
//! - **Pure extraction**: These functions only read state, never modify it.
//! - **Clean boundary**: Snapshot types don't expose internal implementation
//!   details like `HashMap` structures.

use crate::{core::ModeStack, mm::BufferId};

use super::context::KernelContext;

// ============================================================================
// Snapshot Types
// ============================================================================

/// Snapshot of kernel state for debugging.
#[derive(Debug, Clone)]
pub struct KernelStateSnapshot {
    /// Number of buffers currently loaded.
    pub buffer_count: usize,
    /// List of all buffer IDs.
    pub buffer_ids: Vec<BufferId>,
    /// Number of event handlers registered.
    pub event_handlers: usize,
    /// Number of events in the queue.
    pub event_queue_len: usize,
}

/// Snapshot of mode stack.
#[derive(Debug, Clone)]
pub struct ModeStackSnapshot {
    /// Current active mode display string.
    pub current: String,
    /// Full mode stack (bottom to top).
    pub stack: Vec<String>,
    /// Stack depth.
    pub depth: usize,
}

// ============================================================================
// Snapshot Functions
// ============================================================================

/// Create a kernel state snapshot from `KernelContext`.
#[must_use]
pub fn snapshot_kernel_state(ctx: &KernelContext) -> KernelStateSnapshot {
    let buffer_ids = ctx.buffers.list();
    let buffer_count = buffer_ids.len();

    KernelStateSnapshot {
        buffer_count,
        buffer_ids,
        event_handlers: ctx.event_bus.total_handler_count(),
        event_queue_len: ctx.event_bus.queue_len(),
    }
}

/// Create a mode stack snapshot from `ModeStack`.
#[must_use]
pub fn snapshot_mode_stack(stack: &ModeStack) -> ModeStackSnapshot {
    let current = stack.current().to_string();
    let depth = stack.depth();

    // Get all modes in stack
    let stack_vec: Vec<String> = stack.as_slice().iter().map(ToString::to_string).collect();

    ModeStackSnapshot {
        current,
        stack: stack_vec,
        depth,
    }
}
