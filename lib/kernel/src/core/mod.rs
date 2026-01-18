//! Core primitives for the kernel.
//!
//! Linux equivalent: Core kernel functionality
//!
//! This module provides the foundational abstractions for editor operations:
//!
//! - **Motion**: Pure cursor movement calculations
//! - **`TextObject`**: Text object range calculations for operators
//! - **Register**: Yank/paste storage (without clipboard integration)
//! - **Mark**: Bookmark operations
//!
//! # Design Philosophy
//!
//! Following Linux kernel "mechanism, not policy":
//! - Kernel provides *how* to calculate motions (mechanisms)
//! - Modules decide *what* keys trigger which motions (policies)
//!
//! All calculations are pure functions with no side effects.
//! The buffer is never modified by these operations.
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//!
//! let buffer = Buffer::from_string("hello world");
//! let cursor = Cursor::new(Position::new(0, 0));
//!
//! // Calculate where 'w' motion would land
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
mod direction;
mod jumplist;
mod mark;
mod mode;
mod motion;
mod option;
mod register;
mod textobj;

// Re-export direction types
pub use direction::{Direction, LinePosition, WordBoundary};

// Re-export jumplist types
pub use jumplist::{JumpEntry, Jumplist, MAX_JUMPLIST_SIZE};

// Re-export mark types
pub use mark::{Mark, MarkBank, MarkResult, SpecialMark};

// Re-export motion types
pub use motion::{Motion, MotionEngine};

// Re-export register types
pub use register::{RegisterBank, RegisterContent, YankType};

// Re-export text object types
pub use textobj::{TextObject, TextObjectEngine};

// Re-export option types
pub use option::{
    ConstraintError, OptionConstraint, OptionError, OptionRegistry, OptionScope, OptionScopeId,
    OptionSpec, OptionValue, SetResult,
};

// Re-export config types
pub use config::{Config, ConfigError, ConfigPaths, ConfigValue};

// Re-export mode types
pub use mode::{CommandId, CursorStyle, Mode, ModeId, ModeStack};
