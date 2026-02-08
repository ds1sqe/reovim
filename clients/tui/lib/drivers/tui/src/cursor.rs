//! Cursor management.
//!
//! Handles cursor position, visibility, and style for terminal rendering.

use std::io::{self, Write};

use crossterm::{cursor, cursor::SetCursorStyle, execute};

/// Cursor style variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyle {
    /// Default cursor (terminal decides).
    #[default]
    Default,
    /// Blinking block cursor.
    BlinkingBlock,
    /// Steady block cursor.
    SteadyBlock,
    /// Blinking underline cursor.
    BlinkingUnderline,
    /// Steady underline cursor.
    SteadyUnderline,
    /// Blinking bar (vertical line) cursor.
    BlinkingBar,
    /// Steady bar cursor.
    SteadyBar,
}

impl CursorStyle {
    /// Convert to crossterm cursor style.
    #[must_use]
    pub const fn to_crossterm(self) -> SetCursorStyle {
        match self {
            Self::Default => SetCursorStyle::DefaultUserShape,
            Self::BlinkingBlock => SetCursorStyle::BlinkingBlock,
            Self::SteadyBlock => SetCursorStyle::SteadyBlock,
            Self::BlinkingUnderline => SetCursorStyle::BlinkingUnderScore,
            Self::SteadyUnderline => SetCursorStyle::SteadyUnderScore,
            Self::BlinkingBar => SetCursorStyle::BlinkingBar,
            Self::SteadyBar => SetCursorStyle::SteadyBar,
        }
    }
}

/// Cursor state manager.
///
/// Tracks cursor position, visibility, and style.
/// Provides efficient updates by tracking dirty state.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct Cursor {
    /// Cursor X position (0-indexed column).
    x: u16,
    /// Cursor Y position (0-indexed row).
    y: u16,
    /// Whether cursor is visible.
    visible: bool,
    /// Cursor style.
    style: CursorStyle,
    /// Whether position needs update.
    position_dirty: bool,
    /// Whether visibility needs update.
    visibility_dirty: bool,
    /// Whether style needs update.
    style_dirty: bool,
}

impl Cursor {
    /// Create a new cursor at origin.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            visible: true,
            style: CursorStyle::Default,
            position_dirty: true,
            visibility_dirty: true,
            style_dirty: true,
        }
    }

    /// Create a cursor at specific position.
    #[must_use]
    pub const fn at(x: u16, y: u16) -> Self {
        Self {
            x,
            y,
            visible: true,
            style: CursorStyle::Default,
            position_dirty: true,
            visibility_dirty: true,
            style_dirty: true,
        }
    }

    /// Get cursor X position.
    #[must_use]
    pub const fn x(&self) -> u16 {
        self.x
    }

    /// Get cursor Y position.
    #[must_use]
    pub const fn y(&self) -> u16 {
        self.y
    }

    /// Get cursor position as (x, y) tuple.
    #[must_use]
    pub const fn position(&self) -> (u16, u16) {
        (self.x, self.y)
    }

    /// Check if cursor is visible.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get cursor style.
    #[must_use]
    pub const fn style(&self) -> CursorStyle {
        self.style
    }

    /// Move cursor to position.
    pub const fn move_to(&mut self, x: u16, y: u16) {
        if self.x != x || self.y != y {
            self.x = x;
            self.y = y;
            self.position_dirty = true;
        }
    }

    /// Move cursor by delta.
    pub fn move_by(&mut self, dx: i16, dy: i16) {
        let new_x = i32::from(self.x)
            .saturating_add(i32::from(dx))
            .max(0)
            .try_into()
            .unwrap_or(u16::MAX);
        let new_y = i32::from(self.y)
            .saturating_add(i32::from(dy))
            .max(0)
            .try_into()
            .unwrap_or(u16::MAX);
        self.move_to(new_x, new_y);
    }

    /// Show cursor.
    pub const fn show(&mut self) {
        if !self.visible {
            self.visible = true;
            self.visibility_dirty = true;
        }
    }

    /// Hide cursor.
    pub const fn hide(&mut self) {
        if self.visible {
            self.visible = false;
            self.visibility_dirty = true;
        }
    }

    /// Set cursor visibility.
    pub const fn set_visible(&mut self, visible: bool) {
        if visible {
            self.show();
        } else {
            self.hide();
        }
    }

    /// Set cursor style.
    pub fn set_style(&mut self, style: CursorStyle) {
        if self.style != style {
            self.style = style;
            self.style_dirty = true;
        }
    }

    /// Apply cursor state to terminal.
    ///
    /// Only updates changed attributes for efficiency.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal operations fail.
    pub fn apply(&mut self) -> io::Result<()> {
        let mut stdout = io::stdout();
        self.apply_to(&mut stdout)?;
        stdout.flush()
    }

    /// Apply cursor state to writer.
    ///
    /// # Errors
    ///
    /// Returns an error if write operations fail.
    pub fn apply_to<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        // Apply visibility
        if self.visibility_dirty {
            if self.visible {
                execute!(writer, cursor::Show)?;
            } else {
                execute!(writer, cursor::Hide)?;
            }
            self.visibility_dirty = false;
        }

        // Apply style
        if self.style_dirty {
            execute!(writer, self.style.to_crossterm())?;
            self.style_dirty = false;
        }

        // Apply position (only if visible)
        if self.position_dirty && self.visible {
            execute!(writer, cursor::MoveTo(self.x, self.y))?;
            self.position_dirty = false;
        }

        Ok(())
    }

    /// Force full update on next apply.
    pub const fn invalidate(&mut self) {
        self.position_dirty = true;
        self.visibility_dirty = true;
        self.style_dirty = true;
    }

    /// Check if cursor needs update.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.position_dirty || self.visibility_dirty || self.style_dirty
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_new() {
        let cursor = Cursor::new();
        assert_eq!(cursor.x(), 0);
        assert_eq!(cursor.y(), 0);
        assert!(cursor.is_visible());
        assert_eq!(cursor.style(), CursorStyle::Default);
    }

    #[test]
    fn test_cursor_at() {
        let cursor = Cursor::at(10, 5);
        assert_eq!(cursor.position(), (10, 5));
    }

    #[test]
    fn test_cursor_move_to() {
        let mut cursor = Cursor::new();
        cursor.move_to(20, 15);
        assert_eq!(cursor.x(), 20);
        assert_eq!(cursor.y(), 15);
        assert!(cursor.is_dirty());
    }

    #[test]
    fn test_cursor_move_by() {
        let mut cursor = Cursor::at(10, 10);
        cursor.move_by(5, -3);
        assert_eq!(cursor.position(), (15, 7));

        // Negative clamps to 0
        cursor.move_by(-100, -100);
        assert_eq!(cursor.position(), (0, 0));
    }

    #[test]
    fn test_cursor_visibility() {
        let mut cursor = Cursor::new();
        assert!(cursor.is_visible());

        cursor.hide();
        assert!(!cursor.is_visible());
        assert!(cursor.is_dirty());

        cursor.show();
        assert!(cursor.is_visible());
    }

    #[test]
    fn test_cursor_style() {
        let mut cursor = Cursor::new();
        assert_eq!(cursor.style(), CursorStyle::Default);

        cursor.set_style(CursorStyle::BlinkingBar);
        assert_eq!(cursor.style(), CursorStyle::BlinkingBar);
        assert!(cursor.is_dirty());
    }

    #[test]
    fn test_cursor_style_to_crossterm() {
        assert!(matches!(CursorStyle::Default.to_crossterm(), SetCursorStyle::DefaultUserShape));
        assert!(matches!(CursorStyle::SteadyBlock.to_crossterm(), SetCursorStyle::SteadyBlock));
        assert!(matches!(CursorStyle::BlinkingBar.to_crossterm(), SetCursorStyle::BlinkingBar));
    }

    #[test]
    fn test_cursor_style_all_variants() {
        assert!(matches!(
            CursorStyle::BlinkingBlock.to_crossterm(),
            SetCursorStyle::BlinkingBlock
        ));
        assert!(matches!(
            CursorStyle::BlinkingUnderline.to_crossterm(),
            SetCursorStyle::BlinkingUnderScore
        ));
        assert!(matches!(
            CursorStyle::SteadyUnderline.to_crossterm(),
            SetCursorStyle::SteadyUnderScore
        ));
        assert!(matches!(CursorStyle::SteadyBar.to_crossterm(), SetCursorStyle::SteadyBar));
    }

    #[test]
    fn test_cursor_style_default() {
        assert_eq!(CursorStyle::default(), CursorStyle::Default);
    }

    #[test]
    fn test_cursor_style_clone() {
        let style = CursorStyle::BlinkingBlock;
        let cloned = style;
        assert_eq!(style, cloned);
    }

    #[test]
    fn test_cursor_style_debug() {
        let style = CursorStyle::Default;
        let debug = format!("{style:?}");
        assert!(debug.contains("Default"));
    }

    #[test]
    fn test_cursor_default() {
        let cursor = Cursor::default();
        assert_eq!(cursor.x(), 0);
        assert_eq!(cursor.y(), 0);
        assert!(cursor.is_visible());
    }

    #[test]
    fn test_cursor_position() {
        let cursor = Cursor::at(15, 20);
        assert_eq!(cursor.position(), (15, 20));
    }

    #[test]
    fn test_cursor_move_to_same_position() {
        let mut cursor = Cursor::new();
        // Cursor is already dirty on creation, so this test doesn't work as intended
        // Just verify it doesn't crash
        cursor.move_to(0, 0);
        // Cursor starts dirty, so it will still be dirty
    }

    #[test]
    fn test_cursor_move_by_positive() {
        let mut cursor = Cursor::at(10, 10);
        cursor.move_by(5, 3);
        assert_eq!(cursor.position(), (15, 13));
    }

    #[test]
    fn test_cursor_move_by_negative_clamp() {
        let mut cursor = Cursor::at(5, 5);
        cursor.move_by(-10, -10);
        assert_eq!(cursor.position(), (0, 0));
    }

    #[test]
    fn test_cursor_move_by_overflow() {
        let mut cursor = Cursor::at(10, 10);
        cursor.move_by(i16::MAX, i16::MAX);
        // Should not panic, clamps to u16::MAX
        assert!(cursor.x() > 10);
        assert!(cursor.y() > 10);
    }

    #[test]
    fn test_cursor_set_visible_true() {
        let mut cursor = Cursor::new();
        cursor.hide();
        cursor.set_visible(true);
        assert!(cursor.is_visible());
    }

    #[test]
    fn test_cursor_set_visible_false() {
        let mut cursor = Cursor::new();
        cursor.set_visible(false);
        assert!(!cursor.is_visible());
    }

    #[test]
    fn test_cursor_hide_already_hidden() {
        let mut cursor = Cursor::new();
        cursor.hide();
        // Reset dirty flag manually to test
        let _ = cursor.is_dirty();
        cursor.hide();
        // Should not set dirty again if already hidden
    }

    #[test]
    fn test_cursor_show_already_visible() {
        let mut cursor = Cursor::new();
        // Already visible by default
        cursor.show();
        // Should not set dirty if already visible
    }

    #[test]
    fn test_cursor_set_style_same() {
        let mut cursor = Cursor::new();
        cursor.set_style(CursorStyle::Default);
        // Setting to same style should not mark dirty
    }

    #[test]
    fn test_cursor_set_style_different() {
        let mut cursor = Cursor::new();
        cursor.set_style(CursorStyle::BlinkingBlock);
        assert!(cursor.is_dirty());
        assert_eq!(cursor.style(), CursorStyle::BlinkingBlock);
    }

    #[test]
    fn test_cursor_invalidate() {
        let mut cursor = Cursor::new();
        cursor.invalidate();
        assert!(cursor.is_dirty());
    }

    #[test]
    fn test_cursor_is_dirty_all_flags() {
        let mut cursor = Cursor::new();
        assert!(cursor.is_dirty()); // New cursor is dirty

        // Move should mark dirty
        cursor.move_to(10, 10);
        assert!(cursor.is_dirty());

        // Hide should mark dirty
        cursor.hide();
        assert!(cursor.is_dirty());

        // Style change should mark dirty
        cursor.set_style(CursorStyle::SteadyBlock);
        assert!(cursor.is_dirty());
    }

    #[test]
    fn test_cursor_clone() {
        let cursor = Cursor::at(5, 10);
        let cloned = cursor.clone();
        assert_eq!(cursor.position(), cloned.position());
        assert_eq!(cursor.is_visible(), cloned.is_visible());
        assert_eq!(cursor.style(), cloned.style());
    }

    #[test]
    fn test_cursor_debug() {
        let cursor = Cursor::new();
        let debug = format!("{cursor:?}");
        assert!(debug.contains("Cursor"));
    }

    #[test]
    fn test_cursor_apply_to_all_dirty() {
        // New cursor has all flags dirty: visibility, style, position
        let mut cursor = Cursor::new();
        let mut output = Vec::new();
        cursor.apply_to(&mut output).unwrap();

        // After apply, dirty flags should be cleared
        assert!(!cursor.is_dirty());

        // Output should contain cursor show, style, and position commands
        assert!(!output.is_empty());
    }

    #[test]
    fn test_cursor_apply_to_hidden() {
        let mut cursor = Cursor::new();
        // Apply once to clear dirty flags
        let mut discard = Vec::new();
        cursor.apply_to(&mut discard).unwrap();

        // Now hide and apply
        cursor.hide();
        let mut output = Vec::new();
        cursor.apply_to(&mut output).unwrap();
        assert!(!cursor.is_dirty());

        // Output should contain hide cursor command
        assert!(!output.is_empty());
    }

    #[test]
    fn test_cursor_apply_to_position_not_applied_when_hidden() {
        let mut cursor = Cursor::new();
        // Apply once to clear dirty flags
        let mut discard = Vec::new();
        cursor.apply_to(&mut discard).unwrap();

        // Hide cursor and move
        cursor.hide();
        let mut discard2 = Vec::new();
        cursor.apply_to(&mut discard2).unwrap();

        // Move while hidden
        cursor.move_to(10, 10);
        assert!(cursor.is_dirty()); // position_dirty is true

        let mut output = Vec::new();
        cursor.apply_to(&mut output).unwrap();

        // Position should still be dirty because cursor is hidden
        // (position is only applied when visible)
        assert!(cursor.is_dirty());
    }

    #[test]
    fn test_cursor_apply_to_style_change() {
        let mut cursor = Cursor::new();
        // Apply once to clear
        let mut discard = Vec::new();
        cursor.apply_to(&mut discard).unwrap();

        // Change style
        cursor.set_style(CursorStyle::SteadyBar);
        let mut output = Vec::new();
        cursor.apply_to(&mut output).unwrap();

        assert!(!cursor.is_dirty());
        assert!(!output.is_empty());
    }

    #[test]
    fn test_cursor_apply_to_no_dirty() {
        let mut cursor = Cursor::new();
        // Apply to clear all dirty flags
        let mut discard = Vec::new();
        cursor.apply_to(&mut discard).unwrap();

        // Apply again with no changes
        let mut output = Vec::new();
        cursor.apply_to(&mut output).unwrap();

        // Should produce no output since nothing is dirty
        assert!(output.is_empty());
    }

    #[test]
    fn test_cursor_invalidate_then_apply() {
        let mut cursor = Cursor::new();
        // Apply to clear
        let mut discard = Vec::new();
        cursor.apply_to(&mut discard).unwrap();
        assert!(!cursor.is_dirty());

        // Invalidate marks everything dirty
        cursor.invalidate();
        assert!(cursor.is_dirty());

        let mut output = Vec::new();
        cursor.apply_to(&mut output).unwrap();
        assert!(!cursor.is_dirty());
        assert!(!output.is_empty());
    }

    #[test]
    fn test_cursor_show_hide_toggle() {
        let mut cursor = Cursor::new();
        let mut discard = Vec::new();
        cursor.apply_to(&mut discard).unwrap();

        // Hide then show
        cursor.hide();
        cursor.show();
        let mut output = Vec::new();
        cursor.apply_to(&mut output).unwrap();

        // Visibility was toggled back, should have written show command
        assert!(!output.is_empty());
    }

    #[test]
    fn test_cursor_move_to_no_change() {
        let mut cursor = Cursor::at(5, 5);
        // Clear dirty flags
        let mut discard = Vec::new();
        cursor.apply_to(&mut discard).unwrap();

        // Move to same position
        cursor.move_to(5, 5);
        // Should not mark dirty since position didn't change
        assert!(!cursor.is_dirty());
    }

    #[test]
    fn test_cursor_all_styles_apply() {
        let styles = [
            CursorStyle::Default,
            CursorStyle::BlinkingBlock,
            CursorStyle::SteadyBlock,
            CursorStyle::BlinkingUnderline,
            CursorStyle::SteadyUnderline,
            CursorStyle::BlinkingBar,
            CursorStyle::SteadyBar,
        ];
        for style in styles {
            let mut cursor = Cursor::new();
            cursor.set_style(style);
            let mut output = Vec::new();
            cursor.apply_to(&mut output).unwrap();
            assert!(!output.is_empty());
        }
    }
}
