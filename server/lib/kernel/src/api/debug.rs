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

use crate::{
    core::{MarkBank, ModeStack, RegisterBank, SpecialMark, YankType},
    mm::{BufferId, Position},
};

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

/// Yank type for snapshot (mirrors `YankType` but decoupled).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YankTypeSnapshot {
    /// Characterwise yank.
    Characterwise,
    /// Linewise yank.
    Linewise,
}

impl From<YankType> for YankTypeSnapshot {
    fn from(yt: YankType) -> Self {
        match yt {
            YankType::Characterwise => Self::Characterwise,
            YankType::Linewise => Self::Linewise,
        }
    }
}

/// Snapshot of a single register.
#[derive(Debug, Clone)]
pub struct RegisterSnapshot {
    /// Register name ('"' for unnamed, 'a'-'z' for named).
    pub name: char,
    /// Register text content.
    pub text: String,
    /// Yank type.
    pub yank_type: YankTypeSnapshot,
}

/// Snapshot of all registers.
#[derive(Debug, Clone)]
pub struct RegistersSnapshot {
    /// Unnamed register (").
    pub unnamed: RegisterSnapshot,
    /// Named registers (a-z), only non-empty ones.
    pub named: Vec<RegisterSnapshot>,
}

/// Snapshot of a single mark.
#[derive(Debug, Clone)]
pub struct MarkSnapshot {
    /// Mark name ('a'-'z' for local, 'A'-'Z' for global, special chars for special marks).
    pub name: String,
    /// Mark position.
    pub position: Position,
    /// Buffer ID for global marks.
    pub buffer_id: Option<BufferId>,
}

/// Snapshot of all marks.
#[derive(Debug, Clone)]
pub struct MarksSnapshot {
    /// Local marks (a-z) for current buffer.
    pub local: Vec<MarkSnapshot>,
    /// Global marks (A-Z) across all buffers.
    pub global: Vec<MarkSnapshot>,
    /// Special marks ('.', '^', etc.).
    pub special: Vec<MarkSnapshot>,
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

/// Create a registers snapshot from `RegisterBank`.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn snapshot_registers(bank: &RegisterBank) -> RegistersSnapshot {
    // Get unnamed register
    let unnamed_content = bank.get();
    let unnamed = RegisterSnapshot {
        name: '"',
        text: unnamed_content.text.clone(),
        yank_type: unnamed_content.yank_type.into(),
    };

    // Get non-empty named registers
    let mut named = Vec::new();
    for c in 'a'..='z' {
        if let Some(content) = bank.get_named(c)
            && !content.text.is_empty()
        {
            named.push(RegisterSnapshot {
                name: c,
                text: content.text.clone(),
                yank_type: content.yank_type.into(),
            });
        }
    }

    RegistersSnapshot { unnamed, named }
}

/// Create a marks snapshot from `MarkBank`.
#[must_use]
pub fn snapshot_marks(bank: &MarkBank) -> MarksSnapshot {
    // Get local marks
    let local: Vec<MarkSnapshot> = bank
        .list_local()
        .into_iter()
        .map(|(name, pos)| MarkSnapshot {
            name: name.to_string(),
            position: pos,
            buffer_id: None,
        })
        .collect();

    // Get global marks
    let global: Vec<MarkSnapshot> = bank
        .list_global()
        .into_iter()
        .map(|(name, mark)| MarkSnapshot {
            name: name.to_string(),
            position: mark.position,
            buffer_id: Some(mark.buffer_id),
        })
        .collect();

    // Get special marks
    let special_marks = [
        (SpecialMark::LastJump, "'"),
        (SpecialMark::LastEdit, "."),
        (SpecialMark::LastInsert, "^"),
        (SpecialMark::VisualStart, "<"),
        (SpecialMark::VisualEnd, ">"),
        (SpecialMark::LastExitInsert, "]"),
    ];

    let special: Vec<MarkSnapshot> = special_marks
        .iter()
        .filter_map(|(mark_type, name)| {
            bank.get_special(*mark_type).map(|mark| MarkSnapshot {
                name: (*name).to_string(),
                position: mark.position,
                buffer_id: Some(mark.buffer_id),
            })
        })
        .collect();

    MarksSnapshot {
        local,
        global,
        special,
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
