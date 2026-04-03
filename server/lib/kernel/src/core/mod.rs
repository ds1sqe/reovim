//! Core primitives for the kernel.
//!
//! Linux equivalent: Core kernel functionality
//!
//! This module provides kernel-owned abstractions:
//!
//! - **Jumplist**: Navigation history per client
//! - **Mark**: Bookmark operations
//! - **Mode**: Mode identity and mode stack
//! - **Option**: Editor option registry
//! - **Config**: Configuration system
//!
//! Text-specific algorithms (`Motion`, `TextObject`, `Register`) have been
//! extracted to `reovim-types-text` as part of #740.
//!
//! # Example
//!
//! ```
//! use reovim_provider_text::Buffer;
//! use reovim_types_text::{Cursor, Position, Direction, Motion, MotionEngine, WordBoundary};
//!
//! let buffer = Buffer::from_string("hello world");
//! let cursor = Cursor::new(Position::new(0, 0));
//!
//! let new_pos = MotionEngine::calculate(
//!     &buffer,
//!     &cursor,
//!     Motion::Word {
//!         direction: Direction::Forward,
//!         boundary: WordBoundary::Word,
//!         end: false,
//!     },
//!     1,
//! );
//!
//! assert_eq!(new_pos, Some(Position::new(0, 6)));
//! ```

mod config;
mod jumplist;
mod mark;
mod mode;
mod option;

// Re-export jumplist types
pub use jumplist::{JumpEntry, Jumplist, MAX_JUMPLIST_SIZE};

// Re-export mark types
pub use mark::{Mark, MarkBank, MarkResult, SpecialMark};

// Re-export option types
pub use option::{
    ConstraintError, OptionConstraint, OptionError, OptionRegistry, OptionScope, OptionScopeId,
    OptionSpec, OptionValue, SetResult,
};

// Re-export config types
pub use config::{Config, ConfigError, ConfigPaths, ConfigValue};

// Re-export mode types
pub use mode::{CommandId, CursorStyle, Mode, ModeId, ModeStack};

#[cfg(test)]
mod tests;
