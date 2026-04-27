//! Motion type classification for operator-pending mode.
//!
//! This module provides [`MotionType`] for classifying motions as characterwise
//! or linewise. This classification is used by operators (d, y, c) to determine
//! how to apply the operation.
//!
//! # Design (Epic #415)
//!
//! `MotionType` lives in the **command-types** crate to be usable in
//! [`CommandResult`]. This allows motions to report their type when
//! executed in operator-pending mode.
//!
//! # Motion Classification
//!
//! | Type | Motions | Behavior |
//! |------|---------|----------|
//! | Characterwise | w, b, e, h, l, 0, $, ^, f, F, t, T | Operates on character range |
//! | Linewise | j, k, gg, G, {, }, +, -, H, M, L | Operates on full lines |
//!
//! # Example
//!
//! ```
//! use reovim_subsys_command_types::MotionType;
//!
//! // Characterwise motions operate on character ranges
//! let word_motion = MotionType::Characterwise;
//! assert!(!word_motion.is_linewise());
//!
//! // Linewise motions operate on full lines
//! let line_motion = MotionType::Linewise;
//! assert!(line_motion.is_linewise());
//! ```

/// Motion type classification.
///
/// Determines how operators interpret the motion's range:
/// - Characterwise: operate on the exact character range
/// - Linewise: operate on full lines containing the range
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MotionType {
    /// Character-wise motion (w, b, e, h, l, 0, $, ^, f, F, t, T, etc.).
    ///
    /// The operator affects the exact range from start to end position.
    /// This is the default motion type.
    #[default]
    Characterwise,

    /// Line-wise motion (j, k, gg, G, {, }, +, -, H, M, L, etc.).
    ///
    /// The operator affects full lines, regardless of the exact
    /// column positions. The range is expanded to include complete lines.
    Linewise,
    // Blockwise - deferred to visual block mode implementation
}

impl MotionType {
    /// Check if this is a linewise motion.
    #[must_use]
    pub const fn is_linewise(&self) -> bool {
        matches!(self, Self::Linewise)
    }

    /// Check if this is a characterwise motion.
    #[must_use]
    pub const fn is_characterwise(&self) -> bool {
        matches!(self, Self::Characterwise)
    }
}

#[cfg(test)]
#[path = "motion_tests.rs"]
mod tests;
