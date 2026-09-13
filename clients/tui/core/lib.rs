#![no_std]
#![forbid(unsafe_code)]
//! Portable TUI contracts and first-party editing policy.

extern crate alloc;

pub mod context;
pub mod custody;
pub mod operation;
pub mod view;
pub mod viewport;

#[path = "../modules/editing.rs"]
pub mod editing;
#[path = "../contract/input.rs"]
pub mod input;

impl input::Action {
    /// Planned absolute scalar cursor, applied only after matching state.
    /// Invalid or overflowing ranges have no planned cursor.
    #[must_use]
    pub fn cursor_after(&self) -> Option<u64> {
        match self {
            Self::Insert(start, end, text, origin)
                if end >= start && start >= origin =>
            {
                start.checked_add(u64::try_from(text.chars().count()).ok()?)
            }
            Self::Insert(..) => None,
            _ => Some(0),
        }
    }
}
