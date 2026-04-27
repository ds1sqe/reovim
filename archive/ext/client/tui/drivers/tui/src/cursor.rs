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

/// Dirty flags tracking which cursor attributes need terminal update.
#[derive(Debug, Clone, Copy)]
struct DirtyFlags(u8);

impl DirtyFlags {
    const POSITION: u8 = 1 << 0;
    const VISIBILITY: u8 = 1 << 1;
    const STYLE: u8 = 1 << 2;
    const ALL: u8 = Self::POSITION | Self::VISIBILITY | Self::STYLE;

    const fn all() -> Self {
        Self(Self::ALL)
    }

    const fn any(self) -> bool {
        self.0 != 0
    }

    const fn position(self) -> bool {
        self.0 & Self::POSITION != 0
    }

    const fn visibility(self) -> bool {
        self.0 & Self::VISIBILITY != 0
    }

    const fn style(self) -> bool {
        self.0 & Self::STYLE != 0
    }

    const fn set_position(&mut self) {
        self.0 |= Self::POSITION;
    }

    const fn set_visibility(&mut self) {
        self.0 |= Self::VISIBILITY;
    }

    const fn set_style(&mut self) {
        self.0 |= Self::STYLE;
    }

    const fn set_all(&mut self) {
        self.0 = Self::ALL;
    }

    const fn clear_position(&mut self) {
        self.0 &= !Self::POSITION;
    }

    const fn clear_visibility(&mut self) {
        self.0 &= !Self::VISIBILITY;
    }

    const fn clear_style(&mut self) {
        self.0 &= !Self::STYLE;
    }
}

/// Cursor state manager.
///
/// Tracks cursor position, visibility, and style.
/// Provides efficient updates by tracking dirty state.
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
    /// Dirty flags tracking which attributes need update.
    dirty: DirtyFlags,
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
            dirty: DirtyFlags::all(),
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
            dirty: DirtyFlags::all(),
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub const fn move_to(&mut self, x: u16, y: u16) {
        if self.x != x || self.y != y {
            self.x = x;
            self.y = y;
            self.dirty.set_position();
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
            self.dirty.set_visibility();
        }
    }

    /// Hide cursor.
    pub const fn hide(&mut self) {
        if self.visible {
            self.visible = false;
            self.dirty.set_visibility();
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
            self.dirty.set_style();
        }
    }

    /// Apply cursor state to terminal.
    ///
    /// Only updates changed attributes for efficiency.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal operations fail.
    #[cfg_attr(coverage_nightly, coverage(off))]
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
        if self.dirty.visibility() {
            if self.visible {
                execute!(writer, cursor::Show)?;
            } else {
                execute!(writer, cursor::Hide)?;
            }
            self.dirty.clear_visibility();
        }

        // Apply style
        if self.dirty.style() {
            execute!(writer, self.style.to_crossterm())?;
            self.dirty.clear_style();
        }

        // Apply position (only if visible)
        if self.dirty.position() && self.visible {
            execute!(writer, cursor::MoveTo(self.x, self.y))?;
            self.dirty.clear_position();
        }

        Ok(())
    }

    /// Force position update on next apply.
    ///
    /// Call after operations that move the terminal cursor unpredictably
    /// (e.g., `Screen::render()` leaves cursor at last updated cell).
    pub const fn invalidate_position(&mut self) {
        self.dirty.set_position();
    }

    /// Force full update on next apply.
    pub const fn invalidate(&mut self) {
        self.dirty.set_all();
    }

    /// Check if cursor needs update.
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub const fn is_dirty(&self) -> bool {
        self.dirty.any()
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "cursor_tests.rs"]
mod tests;
