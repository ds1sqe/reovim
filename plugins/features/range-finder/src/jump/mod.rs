//! Jump navigation subsystem for range-finder plugin
//!
//! Provides leap-style jump navigation with label-based targeting:
//! - Multi-char search (2 characters + label)
//! - Enhanced f/t motions (single char + label)

pub mod command;
pub mod input;
pub mod render;
pub mod search;
pub mod state;

// Re-export command and event types
pub use command::{
    Direction, JumpCancel, JumpExecute, JumpFindChar, JumpFindCharBack, JumpFindCharStarted,
    JumpInputChar, JumpMode, JumpSearch, JumpSearchStarted, JumpSelectLabel, JumpTillChar,
    JumpTillCharBack,
};

// Re-export state types
#[allow(unused_imports)] // JumpMatch used by render, types used in tests
pub use state::{JumpMatch, JumpPhase, JumpState, OperatorContext, OperatorType, SharedJumpState};
