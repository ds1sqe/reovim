//! Pure text types and algorithms for reovim.
//!
//! This crate provides fundamental text types, traits, and algorithms.
//! Everything here operates on abstract text geometry — no concrete
//! buffer, rope, or storage dependency.
//!
//! # Domain
//!
//! - [`Text`] — text domain marker implementing [`Domain`](reovim_domain::Domain)
//! - [`TextPosition`] — line/column location in a text buffer
//! - [`TextEdit`] — atomic insert/delete operation for undo/redo
//!
//! # Backward Compatibility
//!
//! [`Position`] and [`Edit`] are type aliases for [`TextPosition`] and
//! [`TextEdit`] respectively, providing backward compatibility during
//! the kernel extraction migration.
//!
//! # Types
//!
//! - [`Cursor`] — cursor state (position + anchor + preferred column)
//! - [`TextDimensions`] — shape of a text string (line count, last line length)
//! - [`Selection`] — selection state (anchor + mode)
//! - [`SelectionMode`] — character/line/block selection
//!
//! # Traits
//!
//! - [`TextGeometry`] — read-only line-based text access (object-safe)
//!
//! # Algorithms
//!
//! - [`MotionEngine`] — pure cursor motion calculations
//! - [`TextObjectEngine`] — text object range calculations
//! - Word boundary detection ([`word_start`], [`word_end`], etc.)
//!
//! # Data Structures
//!
//! - [`Rope`] — balanced B-tree text storage with O(1) clone
//! - [`RegisterBank`] — yank/paste register storage
//! - [`HistoryRing`] — clipboard history ring buffer

// --- Data types ---
mod edit;
mod position;
mod rope;
mod selection;

// --- Traits ---
mod text_geometry;

// --- Direction and motion types ---
mod direction;
mod motion;
mod textobj;

// --- Word boundary ---
mod word;

// --- Register and history ---
mod history;
mod register;

// --- Domain ---
mod domain;

// Data types (canonical names)
pub use edit::{TextDimensions, TextEdit, delete_end, text_dimensions, transform_position};
pub use position::{Cursor, TextPosition};
pub use selection::{Selection, SelectionMode};

// Backward compatibility aliases (removed in Phase 5)
pub type Position = TextPosition;
pub type Edit = TextEdit;

// Domain marker
pub use domain::Text;

// Traits and helpers
pub use text_geometry::{SimpleText, TextGeometry};

// Direction and motion
pub use direction::{Direction, LinePosition, WordBoundary};
pub use motion::{Motion, MotionEngine};
pub use textobj::{TextObject, TextObjectEngine};

// Word boundary
pub use word::{
    CharKind, WordType, char_kind, next_word_end, next_word_start, word_bounds, word_end,
    word_start,
};

// Rope
pub use rope::{Rope, RopeChunks, RopeLines};

// Register and history
pub use history::HistoryRing;
pub use register::{Register, RegisterBank, RegisterContent, YankType};
