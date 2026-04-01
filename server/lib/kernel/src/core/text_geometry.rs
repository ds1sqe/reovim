//! Read-only text geometry trait for polymorphic motions and text objects.
//!
//! `TextGeometry` provides the minimal line-based read interface needed by
//! `MotionEngine` and `TextObjectEngine`. Both `Buffer` (Rope) and
//! `VirtualBuffer` (piece table) implement this trait.
//!
//! # Design
//!
//! - 3 required methods: `line_count`, `line`, `line_len`
//! - 1 default method: `is_empty`
//! - Returns `Cow<'_, str>` for zero-copy on Rope, owned on `VirtualBuffer`
//! - Object-safe (all `&self`, no generics)
//!
//! # Zero-test-rewrite guarantee
//!
//! `&Buffer` auto-coerces to `&dyn TextGeometry` — existing tests pass unchanged.

use std::borrow::Cow;

/// Minimal read-only text geometry for motion and text object calculations.
///
/// Implementors provide line-based access to text content. This is the
/// abstraction boundary between storage (Rope, piece table, etc.) and
/// navigation (motions, text objects).
pub trait TextGeometry {
    /// Number of lines in the buffer.
    fn line_count(&self) -> usize;

    /// Get a line by 0-based index.
    ///
    /// Returns `Cow::Borrowed` for Rope (zero-copy) and `Cow::Owned` for
    /// `VirtualBuffer` (materialized from pieces).
    fn line(&self, idx: usize) -> Option<Cow<'_, str>>;

    /// Length of a line in characters (Unicode scalar values).
    fn line_len(&self, idx: usize) -> Option<usize>;

    /// Whether the buffer has no lines.
    fn is_empty(&self) -> bool {
        self.line_count() == 0
    }
}

#[cfg(test)]
#[path = "text_geometry_tests.rs"]
mod tests;
