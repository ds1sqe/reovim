//! Drawing primitives shared across capability-tier trait definitions.
//!
//! `Style`, `Rect`, `Attributes`, `Color`, `ColorDepth`, `RenderingModel`, and
//! `Insets` are grouped here because they appear in the method signatures of
//! `ChromeSurface` and `PlatformCapabilities`. Placing them below those traits
//! allows the module crate to depend on capability for these types without
//! creating cycles.

pub use reovim_arch::Color;

// =============================================================================
// Attributes
// =============================================================================

/// Bitflags for text attributes (bold, italic, etc.).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attributes(u8);

impl Attributes {
    pub const BOLD: Self = Self(0b0000_0001);
    pub const ITALIC: Self = Self(0b0000_0010);
    pub const UNDERLINE: Self = Self(0b0000_0100);
    pub const STRIKETHROUGH: Self = Self(0b0000_1000);
    pub const REVERSE: Self = Self(0b0001_0000);
    pub const DIM: Self = Self(0b0010_0000);

    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn set(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub const fn unset(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Reconstruct `Attributes` from its raw bit representation.
    ///
    /// Used by `FfiStyle` round-trip: the wire format carries the raw `u8` and
    /// the host rehydrates via this helper. Unknown bits are preserved verbatim
    /// for forward-compatibility with attributes added in a future minor version.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
}

impl std::ops::BitOr for Attributes {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for Attributes {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

// =============================================================================
// Style
// =============================================================================

/// Platform-agnostic text style.
///
/// Kept behind the capability contract so drawing surfaces and platform adapters
/// share a single definition without depending on higher-level module types.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub attributes: Attributes,
}

impl Style {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: Attributes::new(),
        }
    }

    /// Set foreground color (builder pattern).
    #[must_use]
    pub const fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    /// Set background color (builder pattern).
    #[must_use]
    pub const fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    /// Enable bold attribute (builder pattern).
    #[must_use]
    pub const fn bold(mut self) -> Self {
        self.attributes.set(Attributes::BOLD);
        self
    }

    /// Enable italic attribute (builder pattern).
    #[must_use]
    pub const fn italic(mut self) -> Self {
        self.attributes.set(Attributes::ITALIC);
        self
    }

    /// Enable underline attribute (builder pattern).
    #[must_use]
    pub const fn underline(mut self) -> Self {
        self.attributes.set(Attributes::UNDERLINE);
        self
    }

    /// Enable dim attribute (builder pattern).
    #[must_use]
    pub const fn dim(mut self) -> Self {
        self.attributes.set(Attributes::DIM);
        self
    }

    /// Enable reverse attribute (builder pattern).
    #[must_use]
    pub const fn reverse(mut self) -> Self {
        self.attributes.set(Attributes::REVERSE);
        self
    }
}

// =============================================================================
// Geometry
// =============================================================================

/// Axis-aligned rectangle in screen coordinates.
///
/// `#[repr(C)]` because `Rect` crosses FFI as a render-trampoline argument
/// and must have a stable layout. All fields are `u16` so the layout is
/// identical to `[u16; 4]` on every supported target.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Compute the intersection of two rectangles.
    ///
    /// Returns `None` if they don't overlap.
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x.saturating_add(self.width)).min(other.x.saturating_add(other.width));
        let y2 = (self.y.saturating_add(self.height)).min(other.y.saturating_add(other.height));

        if x1 < x2 && y1 < y2 {
            Some(Self::new(x1, y1, x2 - x1, y2 - y1))
        } else {
            None
        }
    }

    /// Check if a point is inside this rectangle.
    #[must_use]
    pub const fn contains_point(&self, x: u16, y: u16) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x.saturating_add(self.width)
            && y < self.y.saturating_add(self.height)
    }
}

/// Edge insets (padding/margin from screen edges).
///
/// `#[repr(C)]` because `Insets` is embedded in `FfiPlatformCaps` and
/// must share layout with the module-side binding.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Insets {
    pub top: u16,
    pub bottom: u16,
    pub left: u16,
    pub right: u16,
}

impl Insets {
    /// Zero insets (no padding on any side).
    pub const ZERO: Self = Self {
        top: 0,
        bottom: 0,
        left: 0,
        right: 0,
    };

    #[must_use]
    pub const fn new(top: u16, bottom: u16, left: u16, right: u16) -> Self {
        Self {
            top,
            bottom,
            left,
            right,
        }
    }
}

// =============================================================================
// Platform display model
// =============================================================================

/// Color depth supported by the display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    Monochrome,
    Ansi16,
    Ansi256,
    TrueColor,
}

/// Rendering model supported by the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderingModel {
    /// Terminal-style cell grid (TUI).
    CellGrid,
    /// Canvas-based rendering (Web).
    Canvas,
    /// Native layout engine (iOS/Android).
    NativeLayout,
}

#[cfg(test)]
#[path = "draw_tests.rs"]
mod tests;
