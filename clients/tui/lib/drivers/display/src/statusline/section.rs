//! Statusline section types.
//!
//! Defines lualine-style sections (A, B, C | X, Y, Z) for organizing
//! statusline content.

use crate::{Style, ui::display_width};

/// Section identifier for lookup and ordering.
///
/// Lualine-style sections:
/// - A, B, C: Left-aligned sections (mode, branch, filename)
/// - X, Y, Z: Right-aligned sections (encoding, filetype, position)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionId {
    /// Section A (leftmost, typically mode indicator).
    A,
    /// Section B (left, typically git branch).
    B,
    /// Section C (left-center, typically filename).
    C,
    /// Section X (right-center, typically encoding).
    X,
    /// Section Y (right, typically filetype).
    Y,
    /// Section Z (rightmost, typically position).
    Z,
}

impl SectionId {
    /// Get the section position (left or right).
    #[must_use]
    pub const fn position(self) -> SectionPosition {
        match self {
            Self::A | Self::B | Self::C => SectionPosition::Left,
            Self::X | Self::Y | Self::Z => SectionPosition::Right,
        }
    }

    /// All sections in render order (left to right).
    pub const ALL: &'static [Self] = &[Self::A, Self::B, Self::C, Self::X, Self::Y, Self::Z];

    /// Left sections in order.
    pub const LEFT: &'static [Self] = &[Self::A, Self::B, Self::C];

    /// Right sections in order.
    pub const RIGHT: &'static [Self] = &[Self::X, Self::Y, Self::Z];
}

/// Section position on the statusline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionPosition {
    /// Left-aligned sections (A, B, C).
    Left,
    /// Right-aligned sections (X, Y, Z).
    Right,
}

/// A rendered section with content and style.
///
/// Sections contain rendered text from components and can have
/// section-level style overrides.
#[derive(Debug, Clone)]
pub struct Section {
    /// Section identifier.
    pub id: SectionId,
    /// Rendered text content.
    pub text: String,
    /// Section style.
    pub style: Style,
    /// Truncation priority (higher = truncate later, 0-255).
    /// Default is 128 (middle priority).
    pub priority: u8,
}

impl Section {
    /// Default priority for sections.
    pub const DEFAULT_PRIORITY: u8 = 128;

    /// Create a new section with the given content.
    #[must_use]
    pub fn new(id: SectionId, text: impl Into<String>, style: Style) -> Self {
        Self {
            id,
            text: text.into(),
            style,
            priority: Self::DEFAULT_PRIORITY,
        }
    }

    /// Create a section with custom priority.
    #[must_use]
    pub fn with_priority(
        id: SectionId,
        text: impl Into<String>,
        style: Style,
        priority: u8,
    ) -> Self {
        Self {
            id,
            text: text.into(),
            style,
            priority,
        }
    }

    /// Create an empty section.
    #[must_use]
    pub fn empty(id: SectionId) -> Self {
        Self {
            id,
            text: String::new(),
            style: Style::default(),
            priority: Self::DEFAULT_PRIORITY,
        }
    }

    /// Set the priority (builder pattern).
    #[must_use]
    pub const fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Check if the section is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Calculate the display width of this section.
    #[must_use]
    pub fn display_width(&self) -> usize {
        display_width(&self.text)
    }

    /// Get the section position.
    #[must_use]
    pub const fn position(&self) -> SectionPosition {
        self.id.position()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_id_position() {
        assert_eq!(SectionId::A.position(), SectionPosition::Left);
        assert_eq!(SectionId::B.position(), SectionPosition::Left);
        assert_eq!(SectionId::C.position(), SectionPosition::Left);
        assert_eq!(SectionId::X.position(), SectionPosition::Right);
        assert_eq!(SectionId::Y.position(), SectionPosition::Right);
        assert_eq!(SectionId::Z.position(), SectionPosition::Right);
    }

    #[test]
    fn test_section_new() {
        let section = Section::new(SectionId::A, " NORMAL ", Style::default());
        assert_eq!(section.id, SectionId::A);
        assert_eq!(section.text, " NORMAL ");
        assert_eq!(section.display_width(), 8);
    }

    #[test]
    fn test_section_empty() {
        let section = Section::empty(SectionId::B);
        assert!(section.is_empty());
        assert_eq!(section.display_width(), 0);
    }

    #[test]
    fn test_section_all_order() {
        assert_eq!(
            SectionId::ALL,
            &[
                SectionId::A,
                SectionId::B,
                SectionId::C,
                SectionId::X,
                SectionId::Y,
                SectionId::Z
            ]
        );
    }

    #[test]
    fn test_section_priority_builder() {
        let section = Section::new(SectionId::A, "test", Style::default()).priority(200);
        assert_eq!(section.priority, 200);
        assert_eq!(section.text, "test");
        assert_eq!(section.id, SectionId::A);
    }

    #[test]
    fn test_section_with_priority_constructor() {
        let section = Section::with_priority(SectionId::B, "hello", Style::default(), 42);
        assert_eq!(section.priority, 42);
        assert_eq!(section.text, "hello");
        assert_eq!(section.id, SectionId::B);
    }

    #[test]
    fn test_section_position() {
        let left = Section::new(SectionId::A, "left", Style::default());
        assert_eq!(left.position(), SectionPosition::Left);

        let right = Section::new(SectionId::Z, "right", Style::default());
        assert_eq!(right.position(), SectionPosition::Right);
    }

    #[test]
    fn test_section_is_empty() {
        let empty = Section::new(SectionId::A, "", Style::default());
        assert!(empty.is_empty());

        let not_empty = Section::new(SectionId::A, "x", Style::default());
        assert!(!not_empty.is_empty());
    }
}
