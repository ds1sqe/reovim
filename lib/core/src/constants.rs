//! Editor constants and configuration values

/// Channel capacity for the main event loop mpsc channel
pub const EVENT_CHANNEL_CAPACITY: usize = 255;

/// Channel capacity for key event broadcast
pub const KEY_EVENT_CHANNEL_CAPACITY: usize = 255;

/// ANSI escape sequence to reset all formatting
pub const RESET_STYLE: &str = "\x1b[0m";

/// ANSI color code for visual selection background (gray)
pub const VISUAL_SELECTION_BG: u8 = 240;
