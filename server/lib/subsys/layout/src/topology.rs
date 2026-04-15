//! Layout topology types for domain-neutral window arrangement.
//!
//! A `LayoutTopology` describes the recursive window arrangement (split tree)
//! without any text-domain concepts.  It is intended to be stored, serialized,
//! and reconstructed by layout drivers independently of what those windows
//! actually display.

use {
    crate::{SplitDirection, WindowId},
    std::fmt,
};

// ---------------------------------------------------------------------------
// SplitError
// ---------------------------------------------------------------------------

/// Errors produced when validating split parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitError {
    /// The supplied ratio value exceeds 1000 (parts-per-thousand).
    InvalidRatio(u16),
    /// The target dimension is too small to perform the split.
    TooSmall,
}

impl fmt::Display for SplitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRatio(v) => write!(f, "invalid split ratio {v}: must be 0–1000"),
            Self::TooSmall => write!(f, "window too small to split"),
        }
    }
}

impl std::error::Error for SplitError {}

// ---------------------------------------------------------------------------
// Permil
// ---------------------------------------------------------------------------

/// Split ratio in parts per thousand (0–1000).
///
/// `Permil(500)` means an even 50/50 split; `Permil(1000)` means the first
/// pane occupies the full space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Permil(u16);

impl Permil {
    /// The first pane takes the full space (ratio = 1000).
    pub const FULL: Self = Self(1000);

    /// Equal split between both panes (ratio = 500).
    pub const HALF: Self = Self(500);

    /// Construct a `Permil` from a raw value.
    ///
    /// # Errors
    ///
    /// Returns [`SplitError::InvalidRatio`] when `value > 1000`.
    pub const fn new(value: u16) -> Result<Self, SplitError> {
        if value > 1000 {
            Err(SplitError::InvalidRatio(value))
        } else {
            Ok(Self(value))
        }
    }

    /// Return the raw parts-per-thousand value.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// LayoutTopology
// ---------------------------------------------------------------------------

/// Domain-neutral window arrangement topology.
///
/// A recursive binary split tree where leaves are individual windows and
/// interior nodes describe how space is divided between two sub-trees.
#[derive(Debug, Clone)]
pub enum LayoutTopology {
    /// A single window occupying the full available space.
    Single(WindowId),

    /// Two sub-topologies separated by a split.
    Split {
        /// Axis along which the split runs.
        direction: SplitDirection,
        /// Proportion assigned to `first`, in parts per thousand.
        ratio: Permil,
        /// The sub-topology that receives the leading portion of the space.
        first: Box<Self>,
        /// The sub-topology that receives the trailing portion of the space.
        second: Box<Self>,
    },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "topology_tests.rs"]
mod tests;
