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
//! use reovim_driver_command_types::MotionType;
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
mod tests {
    use super::*;

    #[test]
    fn test_motion_type_default() {
        let motion = MotionType::default();
        assert_eq!(motion, MotionType::Characterwise);
        assert!(motion.is_characterwise());
        assert!(!motion.is_linewise());
    }

    #[test]
    fn test_motion_type_characterwise() {
        let motion = MotionType::Characterwise;
        assert!(motion.is_characterwise());
        assert!(!motion.is_linewise());
    }

    #[test]
    fn test_motion_type_linewise() {
        let motion = MotionType::Linewise;
        assert!(motion.is_linewise());
        assert!(!motion.is_characterwise());
    }

    #[test]
    fn test_motion_type_equality() {
        assert_eq!(MotionType::Characterwise, MotionType::Characterwise);
        assert_eq!(MotionType::Linewise, MotionType::Linewise);
        assert_ne!(MotionType::Characterwise, MotionType::Linewise);
    }

    #[test]
    fn test_motion_type_copy_clone() {
        let motion = MotionType::Linewise;
        let copied = motion;
        #[allow(clippy::clone_on_copy)]
        let cloned = motion.clone();
        assert_eq!(motion, copied);
        assert_eq!(motion, cloned);
    }

    #[test]
    fn test_motion_type_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(MotionType::Characterwise);
        set.insert(MotionType::Linewise);

        assert_eq!(set.len(), 2);
        assert!(set.contains(&MotionType::Characterwise));
        assert!(set.contains(&MotionType::Linewise));
    }

    #[test]
    fn test_motion_type_hash_duplicate_insert() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(MotionType::Characterwise);
        set.insert(MotionType::Characterwise);

        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_motion_type_debug() {
        let debug_char = format!("{:?}", MotionType::Characterwise);
        assert_eq!(debug_char, "Characterwise");

        let debug_line = format!("{:?}", MotionType::Linewise);
        assert_eq!(debug_line, "Linewise");
    }
}
