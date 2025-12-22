//! Fold events for the event bus
//!
//! These events are emitted when fold actions occur and can be subscribed to
//! by plugins for analytics, extensions, or alternative implementations.

use reovim_core::{event_bus::Event, folding::FoldRange};

/// Event emitted when a fold is toggled
#[derive(Debug, Clone)]
pub struct FoldToggleEvent {
    /// Buffer ID where the fold was toggled
    pub buffer_id: usize,
    /// Line number where the fold starts
    pub line: u32,
    /// Whether the fold is now collapsed (true) or expanded (false)
    pub is_collapsed: bool,
}

impl Event for FoldToggleEvent {
    fn priority(&self) -> u32 {
        100
    }
}

/// Event emitted when a fold is opened (expanded)
#[derive(Debug, Clone)]
pub struct FoldOpenEvent {
    /// Buffer ID where the fold was opened
    pub buffer_id: usize,
    /// Line number where the fold starts
    pub line: u32,
}

impl Event for FoldOpenEvent {
    fn priority(&self) -> u32 {
        100
    }
}

/// Event emitted when a fold is closed (collapsed)
#[derive(Debug, Clone)]
pub struct FoldCloseEvent {
    /// Buffer ID where the fold was closed
    pub buffer_id: usize,
    /// Line number where the fold starts
    pub line: u32,
}

impl Event for FoldCloseEvent {
    fn priority(&self) -> u32 {
        100
    }
}

/// Event emitted when all folds in a buffer are opened
#[derive(Debug, Clone)]
pub struct FoldOpenAllEvent {
    /// Buffer ID where all folds were opened
    pub buffer_id: usize,
}

impl Event for FoldOpenAllEvent {
    fn priority(&self) -> u32 {
        100
    }
}

/// Event emitted when all folds in a buffer are closed
#[derive(Debug, Clone)]
pub struct FoldCloseAllEvent {
    /// Buffer ID where all folds were closed
    pub buffer_id: usize,
}

impl Event for FoldCloseAllEvent {
    fn priority(&self) -> u32 {
        100
    }
}

/// Event emitted when fold ranges are updated (e.g., after parsing)
#[derive(Debug, Clone)]
pub struct FoldRangesUpdatedEvent {
    /// Buffer ID where ranges were updated
    pub buffer_id: usize,
    /// The new fold ranges
    pub ranges: Vec<FoldRange>,
}

impl Event for FoldRangesUpdatedEvent {
    fn priority(&self) -> u32 {
        50 // Higher priority for range updates
    }
}
