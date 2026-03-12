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
#[path = "section_tests.rs"]
mod tests;
