//! Pure text types and algorithms for reovim.
//!
//! This is a zero-dependency leaf crate providing fundamental text types,
//! traits, and algorithms. Everything here operates on abstract text
//! geometry — no concrete buffer, rope, or storage dependency.
//!
//! # Types
//!
//! - [`Position`] — line/column location in a text buffer
//! - [`Cursor`] — cursor state (position + anchor + preferred column)
//! - [`Edit`] — atomic insert/delete operation for undo/redo
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
//! - [`RegisterBank`] — yank/paste register storage
//! - [`HistoryRing`] — clipboard history ring buffer

// --- Data types ---
mod edit;
mod position;
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

// Data types
pub use edit::{Edit, TextDimensions, delete_end, text_dimensions, transform_position};
pub use position::{Cursor, Position};
pub use selection::{Selection, SelectionMode};

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

// Register and history
pub use history::HistoryRing;
pub use register::{Register, RegisterBank, RegisterContent, YankType};
