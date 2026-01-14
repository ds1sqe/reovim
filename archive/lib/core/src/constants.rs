//! Editor constants and configuration values

/// Channel capacity for the main event loop mpsc channel (low-priority events)
pub const EVENT_CHANNEL_CAPACITY: usize = 255;

/// Channel capacity for high-priority events (user input, mode changes)
pub const HI_PRIORITY_CHANNEL_CAPACITY: usize = 64;

/// Maximum low-priority events to drain per high-priority batch (fairness)
pub const MAX_LO_DRAIN: usize = 16;

/// Channel capacity for key event broadcast
pub const KEY_EVENT_CHANNEL_CAPACITY: usize = 255;

/// ANSI escape sequence to reset all formatting
pub const RESET_STYLE: &str = "\x1b[0m";
