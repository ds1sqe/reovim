//! Rendering modules for screen components
//!
//! This module organizes rendering logic for different parts of the screen:
//! - `chrome` - Status line, tab line, command line rendering
//! - `separator` - Window separator rendering
//! - `pipeline` - Main render orchestration
//! - `line` - Line and gutter rendering

pub mod chrome;
pub mod line;
pub mod pipeline;
pub mod separator;
