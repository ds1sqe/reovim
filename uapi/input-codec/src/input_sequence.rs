//! `InputSequence` — an ordered series of opaque input event payloads.
//!
//! Used by the keymap registry to represent multi-event binding keys
//! (e.g. `<C-w>h` is a two-event sequence).  Each element is a raw
//! `Vec<u8>` — the same bytes that would appear in `InputEvent::payload()`.
//!
//! `InputSequence` intentionally has no knowledge of kind values or
//! body layouts; those are codec-layer concerns.

use std::hash::{Hash, Hasher};

/// An ordered sequence of opaque input event payloads.
///
/// Each element is a raw payload (`Vec<u8>`) produced by a platform codec.
/// The sequence is the unit of keybinding lookup in the server registry.
#[derive(Debug, Clone, Default)]
pub struct InputSequence {
    events: Vec<Vec<u8>>,
}

impl InputSequence {
    /// Create an empty sequence.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a payload to the sequence.
    pub fn push(&mut self, payload: Vec<u8>) {
        self.events.push(payload);
    }

    /// Number of events in the sequence.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns `true` if the sequence has no events.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// View the raw payload slice.
    #[must_use]
    pub fn as_slice(&self) -> &[Vec<u8>] {
        &self.events
    }

    /// Remove all events from the sequence.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Returns `true` if `self` starts with `other` (prefix test).
    ///
    /// An empty `other` always returns `true`.
    #[must_use]
    pub fn starts_with(&self, other: &Self) -> bool {
        if other.events.len() > self.events.len() {
            return false;
        }
        self.events
            .iter()
            .zip(other.events.iter())
            .all(|(a, b)| a == b)
    }
}

impl PartialEq for InputSequence {
    fn eq(&self, other: &Self) -> bool {
        self.events == other.events
    }
}

impl Eq for InputSequence {}

impl Hash for InputSequence {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.events.hash(state);
    }
}
