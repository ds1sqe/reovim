//! FFI-safe boundary types and vtables for dynamic client modules (#723).
//!
//! This module defines every `#[repr(C)]` type that crosses the FFI boundary
//! between a host and a dynamic `.so` client module. It also provides the
//! `FfiRenderSurface` and `FfiThemeProvider` vtable wrappers that let modules
//! call back into host resources during a render call.
//!
//! # Allocator hazard
//!
//! Types in this module that own heap allocations (currently only
//! [`FfiTransformedLine`]) MUST have their `from_rust` / `free` methods
//! called from the **same** `.so` or binary that allocated them. The
//! `declare_client_module!` macro handles this correctly by placing both
//! calls inside the module's trampolines (the module allocates via
//! `from_rust`, the host's dispatch reads via [`FfiTransformedLine::read_owned`]
//! and then calls the module's exported free trampoline).
//!
//! **Host code MUST NOT call `FfiTransformedLine::from_rust` directly.**
//! The `#[doc(hidden)]` marker on those methods hides them from rustdoc,
//! but they remain `pub` because the macro expansion lives in user module
//! crates that need to reach them via
//! `::reovim_client_subsys_module::ffi::FfiTransformedLine::from_rust`.
//!
//! Cross-allocator `free` (module allocates, host frees) is undefined
//! behavior and corrupts both heaps.
//!
//! # ABI stability
//!
//! All types and vtables defined here are **frozen** at
//! `CLIENT_MODULE_API_VERSION = 0.4.0`. Field reordering, size changes, or
//! layout changes require a major version bump and a migration path. Adding
//! new methods to a vtable is a minor version bump (old modules see the
//! field list they expect).

#![allow(unsafe_code)]
// FFI boundary — unsafe is intrinsic to this module
// Pedantic allowances justified by the FFI nature of this module:
//
// * `pub_underscore_fields`: `_pad*` fields MUST be `pub` because the macro
//   expansion in user module crates constructs `#[repr(C)]` literals and
//   needs to name every field; renaming to non-underscore would make the
//   name-collision risk real.
// * `match_same_arms`: Several `from_option` match arms produce the same
//   struct literal intentionally because the variants discriminate by tag
//   alone (e.g. `Highlight` vs `Hide`). Collapsing them hides intent.
// * `missing_const_for_fn`: Constructors that store references cannot be
//   `const fn` due to current const-eval limits on `&mut` / `&dyn`.
// * `cast_possible_truncation`: Length fields (`text_len: u8`,
//   `content_len: u16`) are inline-buffer-bounded by construction, which
//   the `.min(N)` calls enforce before the cast.
// * `needless_pass_by_value`: `from_option` takes `Option<Color>` by value
//   to match the trait signatures at call sites; copying a 4-byte enum is
//   cheaper than an indirection.
#![allow(
    clippy::pub_underscore_fields,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::cast_possible_truncation,
    clippy::needless_pass_by_value
)]

use std::{ffi::c_void, marker::PhantomData};

#[cfg(test)]
use std::sync::atomic::AtomicUsize;

use reovim_arch::Color;

use crate::{
    traits::{ChromeSurface, PlatformCapabilities, ThemeProvider},
    types::{
        AnnotationContext, Attributes, BufferId, ColorDepth, ColumnWidth, GutterCell,
        InlineDecoration, Insets, Rect, RenderBehavior, RenderingModel, Style, TransformedLine,
        VirtualLine, VirtualLinePosition,
    },
};

// =============================================================================
// FfiColor
// =============================================================================

/// FFI-safe color representation.
///
/// Tag encoding (single byte):
/// * 0 = `Reset`
/// * 1 = `Black`
/// * 2 = `DarkRed`
/// * 3 = `DarkGreen`
/// * 4 = `DarkYellow`
/// * 5 = `DarkBlue`
/// * 6 = `DarkMagenta`
/// * 7 = `DarkCyan`
/// * 8 = `Grey`
/// * 9 = `DarkGrey`
/// * 10 = `Red`
/// * 11 = `Green`
/// * 12 = `Yellow`
/// * 13 = `Blue`
/// * 14 = `Magenta`
/// * 15 = `Cyan`
/// * 16 = `White`
/// * 17 = `AnsiValue(r)` — the `r` byte carries the palette index
/// * 18 = `Rgb { r, g, b }` — all three data bytes carry RGB
/// * 255 = `None` (sentinel used by `FfiStyle::fg`/`bg` for `Option<Color>`)
///
/// The struct is exactly 4 bytes for cache-friendly packing inside `FfiStyle`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiColor {
    pub tag: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl FfiColor {
    /// Sentinel for `Option<Color>::None`.
    pub const NONE: Self = Self {
        tag: 255,
        r: 0,
        g: 0,
        b: 0,
    };
    /// Default color (reset).
    pub const RESET: Self = Self {
        tag: 0,
        r: 0,
        g: 0,
        b: 0,
    };

    /// Encode a `Color` into the FFI representation.
    #[allow(clippy::too_many_lines)] // explicit 1:1 ABI tag mapping is clearer than table indirection here
    #[must_use]
    pub const fn from_color(c: Color) -> Self {
        match c {
            Color::Reset => Self {
                tag: 0,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::Black => Self {
                tag: 1,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::DarkRed => Self {
                tag: 2,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::DarkGreen => Self {
                tag: 3,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::DarkYellow => Self {
                tag: 4,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::DarkBlue => Self {
                tag: 5,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::DarkMagenta => Self {
                tag: 6,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::DarkCyan => Self {
                tag: 7,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::Grey => Self {
                tag: 8,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::DarkGrey => Self {
                tag: 9,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::Red => Self {
                tag: 10,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::Green => Self {
                tag: 11,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::Yellow => Self {
                tag: 12,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::Blue => Self {
                tag: 13,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::Magenta => Self {
                tag: 14,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::Cyan => Self {
                tag: 15,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::White => Self {
                tag: 16,
                r: 0,
                g: 0,
                b: 0,
            },
            Color::AnsiValue(v) => Self {
                tag: 17,
                r: v,
                g: 0,
                b: 0,
            },
            Color::Rgb { r, g, b } => Self { tag: 18, r, g, b },
        }
    }

    /// Encode an `Option<Color>` — `None` becomes the sentinel `NONE`.
    #[must_use]
    pub const fn from_option(c: Option<Color>) -> Self {
        match c {
            Some(c) => Self::from_color(c),
            None => Self::NONE,
        }
    }

    /// Decode back into a concrete `Color`. Unknown tags fall back to
    /// `Color::Reset` (defensive against forward-compatibility).
    #[must_use]
    pub const fn into_color(self) -> Color {
        match self.tag {
            0 => Color::Reset,
            1 => Color::Black,
            2 => Color::DarkRed,
            3 => Color::DarkGreen,
            4 => Color::DarkYellow,
            5 => Color::DarkBlue,
            6 => Color::DarkMagenta,
            7 => Color::DarkCyan,
            8 => Color::Grey,
            9 => Color::DarkGrey,
            10 => Color::Red,
            11 => Color::Green,
            12 => Color::Yellow,
            13 => Color::Blue,
            14 => Color::Magenta,
            15 => Color::Cyan,
            16 => Color::White,
            17 => Color::AnsiValue(self.r),
            18 => Color::Rgb {
                r: self.r,
                g: self.g,
                b: self.b,
            },
            _ => Color::Reset,
        }
    }

    /// Decode into `Option<Color>`; the sentinel `tag == 255` becomes `None`.
    #[must_use]
    pub const fn into_option(self) -> Option<Color> {
        if self.tag == 255 {
            None
        } else {
            Some(self.into_color())
        }
    }
}

impl Default for FfiColor {
    fn default() -> Self {
        Self::NONE
    }
}

// =============================================================================
// FfiStyle
// =============================================================================

/// FFI-safe style representation (12 bytes total).
///
/// `fg` / `bg` use [`FfiColor::NONE`] for `None`. `attributes` carries the
/// raw bitfield from [`Attributes::bits`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiStyle {
    pub fg: FfiColor,
    pub bg: FfiColor,
    pub attributes: u8,
    pub _pad: [u8; 3],
}

impl FfiStyle {
    /// Default style (no fg, no bg, no attributes).
    pub const DEFAULT: Self = Self {
        fg: FfiColor::NONE,
        bg: FfiColor::NONE,
        attributes: 0,
        _pad: [0; 3],
    };

    /// Encode a borrowed `Style` into the FFI representation.
    #[must_use]
    pub fn from_style(s: &Style) -> Self {
        Self {
            fg: FfiColor::from_option(s.fg),
            bg: FfiColor::from_option(s.bg),
            attributes: s.attributes.bits(),
            _pad: [0; 3],
        }
    }

    /// Decode back into an owned `Style`.
    #[must_use]
    pub fn into_style(self) -> Style {
        Style {
            fg: self.fg.into_option(),
            bg: self.bg.into_option(),
            attributes: Attributes::from_bits(self.attributes),
        }
    }
}

impl Default for FfiStyle {
    fn default() -> Self {
        Self::DEFAULT
    }
}

// =============================================================================
// FfiPlatformCaps
// =============================================================================

/// Snapshot of `PlatformCapabilities` for FFI dispatch.
///
/// Captured from a `&dyn PlatformCapabilities` before each render call and
/// read back inside the module. All trait method results are pre-evaluated
/// and stored as flat scalars so the module does not need to call back into
/// the host for basic capability queries.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiPlatformCaps {
    /// Rendering model: 0 = `CellGrid`, 1 = `Canvas`, 2 = `NativeLayout`.
    pub rendering_model: u8,
    /// Color depth: 0 = `Monochrome`, 1 = `Ansi16`, 2 = `Ansi256`, 3 = `TrueColor`.
    pub color_depth: u8,
    /// Boolean capabilities packed into a bitfield:
    /// * bit 0: `reliable_unicode_width`
    /// * bit 1: `dark_mode`
    /// * bit 2: `smooth_scroll`
    /// * bit 3: `pointer_events`
    /// * bit 4: `touch_input`
    /// * bit 5: `haptic`
    /// * bit 6: `has_focus`
    /// * bit 7: `clipboard_available`
    /// * bit 8: `screen_reader_active`
    pub flags: u16,
    pub grid_cols: u16,
    pub grid_rows: u16,
    /// 1 if `grid_size()` returned `Some`, else 0.
    pub has_grid_size: u8,
    pub _pad1: [u8; 3],
    pub pixel_w: u32,
    pub pixel_h: u32,
    /// 1 if `pixel_size()` returned `Some`, else 0.
    pub has_pixel_size: u8,
    pub _pad2: [u8; 3],
    /// Safe area insets (already `#[repr(C)]` since #723 Phase 2).
    pub safe_area: Insets,
}

impl FfiPlatformCaps {
    /// Capture a snapshot of the trait object.
    #[must_use]
    pub fn snapshot(caps: &dyn PlatformCapabilities) -> Self {
        let grid = caps.grid_size();
        let pixel = caps.pixel_size();
        let mut flags: u16 = 0;
        if caps.reliable_unicode_width() {
            flags |= 1 << 0;
        }
        if caps.dark_mode() {
            flags |= 1 << 1;
        }
        if caps.smooth_scroll() {
            flags |= 1 << 2;
        }
        if caps.pointer_events() {
            flags |= 1 << 3;
        }
        if caps.touch_input() {
            flags |= 1 << 4;
        }
        if caps.haptic() {
            flags |= 1 << 5;
        }
        if caps.has_focus() {
            flags |= 1 << 6;
        }
        if caps.clipboard_available() {
            flags |= 1 << 7;
        }
        if caps.screen_reader_active() {
            flags |= 1 << 8;
        }
        Self {
            rendering_model: match caps.rendering_model() {
                RenderingModel::CellGrid => 0,
                RenderingModel::Canvas => 1,
                RenderingModel::NativeLayout => 2,
            },
            color_depth: match caps.color_depth() {
                ColorDepth::Monochrome => 0,
                ColorDepth::Ansi16 => 1,
                ColorDepth::Ansi256 => 2,
                ColorDepth::TrueColor => 3,
            },
            flags,
            grid_cols: grid.map_or(0, |(c, _)| c),
            grid_rows: grid.map_or(0, |(_, r)| r),
            has_grid_size: u8::from(grid.is_some()),
            _pad1: [0; 3],
            pixel_w: pixel.map_or(0, |(w, _)| w),
            pixel_h: pixel.map_or(0, |(_, h)| h),
            has_pixel_size: u8::from(pixel.is_some()),
            _pad2: [0; 3],
            safe_area: caps.safe_area(),
        }
    }
}

/// Module-side `PlatformCapabilities` impl backed by a shared snapshot.
///
/// Construct with [`FfiCapsImpl::new`] inside a trampoline and pass
/// `&caps_impl` to `ClientModule::chrome_render`. The trait methods read
/// from the captured snapshot, so they return the host's values as of the
/// moment `FfiPlatformCaps::snapshot` was called.
pub struct FfiCapsImpl<'a> {
    snap: &'a FfiPlatformCaps,
}

impl<'a> FfiCapsImpl<'a> {
    /// Wrap a borrowed snapshot.
    #[must_use]
    pub const fn new(snap: &'a FfiPlatformCaps) -> Self {
        Self { snap }
    }

    const fn flag(&self, bit: u16) -> bool {
        self.snap.flags & bit != 0
    }
}

impl PlatformCapabilities for FfiCapsImpl<'_> {
    fn rendering_model(&self) -> RenderingModel {
        match self.snap.rendering_model {
            1 => RenderingModel::Canvas,
            2 => RenderingModel::NativeLayout,
            _ => RenderingModel::CellGrid,
        }
    }
    fn grid_size(&self) -> Option<(u16, u16)> {
        if self.snap.has_grid_size == 1 {
            Some((self.snap.grid_cols, self.snap.grid_rows))
        } else {
            None
        }
    }
    fn color_depth(&self) -> ColorDepth {
        match self.snap.color_depth {
            1 => ColorDepth::Ansi16,
            2 => ColorDepth::Ansi256,
            3 => ColorDepth::TrueColor,
            _ => ColorDepth::Monochrome,
        }
    }
    fn pixel_size(&self) -> Option<(u32, u32)> {
        if self.snap.has_pixel_size == 1 {
            Some((self.snap.pixel_w, self.snap.pixel_h))
        } else {
            None
        }
    }
    fn reliable_unicode_width(&self) -> bool {
        self.flag(1 << 0)
    }
    fn dark_mode(&self) -> bool {
        self.flag(1 << 1)
    }
    fn smooth_scroll(&self) -> bool {
        self.flag(1 << 2)
    }
    fn pointer_events(&self) -> bool {
        self.flag(1 << 3)
    }
    fn touch_input(&self) -> bool {
        self.flag(1 << 4)
    }
    fn haptic(&self) -> bool {
        self.flag(1 << 5)
    }
    fn safe_area(&self) -> Insets {
        self.snap.safe_area
    }
    fn has_focus(&self) -> bool {
        self.flag(1 << 6)
    }
    fn clipboard_available(&self) -> bool {
        self.flag(1 << 7)
    }
    fn screen_reader_active(&self) -> bool {
        self.flag(1 << 8)
    }
}

// =============================================================================
// FfiAnnotationContext
// =============================================================================

/// FFI-safe `AnnotationContext`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiAnnotationContext {
    pub buffer_id: usize,
    pub total_lines: usize,
    pub visible_start: usize,
    pub visible_end: usize,
    pub cursor_line: usize,
    pub gutter_style: FfiStyle,
}

impl FfiAnnotationContext {
    /// Encode from a borrowed context.
    #[must_use]
    pub fn from_ctx(ctx: &AnnotationContext) -> Self {
        Self {
            buffer_id: ctx.buffer_id.0,
            total_lines: ctx.total_lines,
            visible_start: ctx.visible_range.0,
            visible_end: ctx.visible_range.1,
            cursor_line: ctx.cursor_line,
            gutter_style: FfiStyle::from_style(&ctx.gutter_style),
        }
    }

    /// Decode back into an owned context.
    #[must_use]
    pub fn into_ctx(self) -> AnnotationContext {
        AnnotationContext {
            buffer_id: BufferId(self.buffer_id),
            total_lines: self.total_lines,
            visible_range: (self.visible_start, self.visible_end),
            cursor_line: self.cursor_line,
            gutter_style: self.gutter_style.into_style(),
        }
    }
}

// =============================================================================
// FfiGutterCell
// =============================================================================

/// FFI-safe gutter cell with a 32-byte inline text buffer.
///
/// Inline buffer (not heap) because gutter text is always short (≤10 bytes
/// for line numbers, git signs, diagnostic markers). Texts longer than 31
/// bytes are truncated on encode and a warning is emitted in debug builds.
///
/// `text_len == 0` is used as the `None` sentinel for `Option<GutterCell>`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiGutterCell {
    /// Inline UTF-8 text (null-padded to 32 bytes).
    pub text: [u8; 32],
    /// Actual text length in bytes (0 = `None`/empty).
    pub text_len: u8,
    pub _pad: [u8; 3],
    pub style: FfiStyle,
}

impl FfiGutterCell {
    /// Sentinel representing `None` (empty gutter cell).
    pub const NONE: Self = Self {
        text: [0; 32],
        text_len: 0,
        _pad: [0; 3],
        style: FfiStyle::DEFAULT,
    };

    /// Whether this cell represents `None`.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        self.text_len == 0
    }

    /// Encode a borrowed `GutterCell`. Text longer than 31 bytes is truncated
    /// on a character boundary. Empty text produces `Self::NONE`.
    #[must_use]
    pub fn from_cell(cell: &GutterCell) -> Self {
        if cell.text.is_empty() {
            return Self::NONE;
        }
        let bytes = cell.text.as_bytes();
        let mut len = bytes.len().min(31);
        // Back off to a valid UTF-8 char boundary so we never split a code point.
        while len > 0 && !cell.text.is_char_boundary(len) {
            len -= 1;
        }
        let mut text = [0u8; 32];
        text[..len].copy_from_slice(&bytes[..len]);
        Self {
            text,
            text_len: len as u8,
            _pad: [0; 3],
            style: FfiStyle::from_style(&cell.style),
        }
    }

    /// Decode into an owned `Option<GutterCell>` (the wire format's `None`
    /// sentinel is `text_len == 0`).
    #[must_use]
    pub fn into_cell(self) -> Option<GutterCell> {
        if self.text_len == 0 {
            return None;
        }
        let len = self.text_len as usize;
        // SAFETY: from_cell trimmed to a char boundary on encode, so the
        // bytes are valid UTF-8. This decode mirrors that invariant.
        let s = std::str::from_utf8(&self.text[..len]).ok()?;
        Some(GutterCell {
            text: s.to_string(),
            style: self.style.into_style(),
        })
    }
}

// =============================================================================
// FfiInlineDecoration
// =============================================================================

/// FFI-safe `InlineDecoration` — all scalar fields.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiInlineDecoration {
    pub col_start: u16,
    pub col_end: u16,
    pub style: FfiStyle,
}

impl FfiInlineDecoration {
    #[must_use]
    pub fn from_deco(d: &InlineDecoration) -> Self {
        Self {
            col_start: d.col_start,
            col_end: d.col_end,
            style: FfiStyle::from_style(&d.style),
        }
    }

    #[must_use]
    pub fn into_deco(self) -> InlineDecoration {
        InlineDecoration {
            col_start: self.col_start,
            col_end: self.col_end,
            style: self.style.into_style(),
        }
    }
}

// =============================================================================
// FfiColumnWidth
// =============================================================================

/// FFI-safe `ColumnWidth`.
///
/// Tag encoding: 0 = `Fixed(value)`, 1 = `Dynamic(value)`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiColumnWidth {
    pub tag: u8,
    pub _pad: u8,
    pub value: u16,
}

impl FfiColumnWidth {
    pub const ZERO: Self = Self {
        tag: 0,
        _pad: 0,
        value: 0,
    };

    #[must_use]
    pub const fn from_width(w: ColumnWidth) -> Self {
        match w {
            ColumnWidth::Fixed(v) => Self {
                tag: 0,
                _pad: 0,
                value: v,
            },
            ColumnWidth::Dynamic(v) => Self {
                tag: 1,
                _pad: 0,
                value: v,
            },
        }
    }

    #[must_use]
    pub const fn into_width(self) -> ColumnWidth {
        match self.tag {
            1 => ColumnWidth::Dynamic(self.value),
            _ => ColumnWidth::Fixed(self.value),
        }
    }
}

// =============================================================================
// FfiFoldRange
// =============================================================================

/// FFI-safe fold range (module's `fold_ranges` return element).
///
/// Rust's native `(usize, usize)` is NOT `#[repr(C)]` and cannot be cast
/// through a raw pointer reinterpretation without UB. This struct exists
/// so the `fold_ranges` trampoline can emit a `*const FfiFoldRange` that
/// the host can safely dereference.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiFoldRange {
    pub start_line: usize,
    pub line_count: usize,
}

// =============================================================================
// FfiVirtualLine
// =============================================================================

/// FFI-safe `VirtualLine` with a 256-byte inline content buffer.
///
/// Longer content is truncated on a character boundary. The `position`
/// byte encodes `VirtualLinePosition`: 0 = Before, 1 = After.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiVirtualLine {
    pub buffer_line: usize,
    /// 0 = `Before`, 1 = `After`.
    pub position: u8,
    pub _pad: [u8; 3],
    pub style: FfiStyle,
    /// Inline content buffer.
    pub content: [u8; 256],
    pub content_len: u16,
    pub _pad2: [u8; 2],
}

impl FfiVirtualLine {
    /// Encode from a borrowed `VirtualLine`.
    #[must_use]
    pub fn from_rust(vl: &VirtualLine) -> Self {
        let bytes = vl.content.as_bytes();
        let mut len = bytes.len().min(255);
        while len > 0 && !vl.content.is_char_boundary(len) {
            len -= 1;
        }
        let mut content = [0u8; 256];
        content[..len].copy_from_slice(&bytes[..len]);
        Self {
            buffer_line: vl.buffer_line,
            position: match vl.position {
                VirtualLinePosition::Before => 0,
                VirtualLinePosition::After => 1,
            },
            _pad: [0; 3],
            style: FfiStyle::from_style(&vl.style),
            content,
            content_len: len as u16,
            _pad2: [0; 2],
        }
    }

    /// Decode into an owned `VirtualLine`.
    #[must_use]
    pub fn into_rust(self) -> VirtualLine {
        let len = self.content_len as usize;
        let content = std::str::from_utf8(&self.content[..len])
            .unwrap_or("")
            .to_string();
        VirtualLine {
            buffer_line: self.buffer_line,
            position: match self.position {
                1 => VirtualLinePosition::After,
                _ => VirtualLinePosition::Before,
            },
            content,
            style: self.style.into_style(),
        }
    }
}

// =============================================================================
// FfiTransformedLine + FfiTransformedSegment
// =============================================================================

/// A single segment inside a transformed line.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiTransformedSegment {
    /// UTF-8 text pointer (into the enclosing line's `text_buf`).
    pub text_ptr: *const u8,
    /// Text length in bytes.
    pub text_len: usize,
    /// 1 if the segment carries a style, 0 if `None`.
    pub has_style: u8,
    pub _pad: [u8; 7],
    pub style: FfiStyle,
}

// SAFETY: FfiTransformedSegment contains raw pointers but is only ever passed
// through FFI, never shared across threads. The pointers reference the
// enclosing FfiTransformedLine's text_buf which lives for the duration of
// the read+free cycle.
unsafe impl Send for FfiTransformedSegment {}
unsafe impl Sync for FfiTransformedSegment {}

/// FFI-safe `TransformedLine` with host/module allocator-split ownership.
///
/// # Allocator contract
///
/// This type owns two heap allocations: `segments_ptr` (a `Vec<FfiTransformedSegment>`)
/// and `text_buf` (a contiguous UTF-8 byte buffer that all segment pointers
/// index into). Both are allocated by the module's `from_rust` call inside
/// the module's `.so` and must be freed by the module's exported
/// `reovim_client_module_free_transformed_line` trampoline.
///
/// **Host code MUST NOT call `from_rust` or `free`.** The host uses
/// [`Self::read_owned`] to copy the fields out, then calls the module's
/// free trampoline to release the module-allocator memory.
#[repr(C)]
pub struct FfiTransformedLine {
    /// Segment array pointer (module-allocated).
    pub segments_ptr: *mut FfiTransformedSegment,
    /// Number of segments.
    pub segments_len: usize,
    /// Capacity of the underlying Vec — recorded so the module-side free
    /// trampoline can reconstruct the exact allocation.
    pub segments_cap: usize,
    /// Backing text buffer (all segment `text_ptr`s point into this).
    pub text_buf: *mut u8,
    pub text_buf_len: usize,
    pub text_buf_cap: usize,
}

// SAFETY: same rationale as FfiTransformedSegment — boundary-only type.
unsafe impl Send for FfiTransformedLine {}
unsafe impl Sync for FfiTransformedLine {}

#[cfg(test)]
static TRANSFORMED_LINE_ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static TRANSFORMED_LINE_FREE_COUNT: AtomicUsize = AtomicUsize::new(0);

impl FfiTransformedLine {
    /// Allocate an `FfiTransformedLine` from a Rust `TransformedLine`.
    ///
    /// # Host hazard
    ///
    /// **DO NOT CALL FROM HOST CODE.** This function must only run inside
    /// the module's `.so` context (i.e., from a trampoline generated by
    /// `declare_client_module!`). The allocations it produces are only
    /// freeable by the same `.so`'s `free` method. Cross-allocator free
    /// corrupts both heaps.
    #[doc(hidden)]
    #[must_use]
    pub fn from_rust(tl: TransformedLine) -> *mut Self {
        #[cfg(test)]
        TRANSFORMED_LINE_ALLOC_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Concatenate all segment text into one contiguous buffer.
        let mut text_buf: Vec<u8> = Vec::new();
        for (s, _) in &tl.segments {
            text_buf.extend_from_slice(s.as_bytes());
        }

        // Record capacity so the free trampoline can reconstruct the Vec
        // with the exact allocation. Vec::shrink_to_fit ensures len == cap.
        text_buf.shrink_to_fit();
        let text_buf_len = text_buf.len();
        let text_buf_cap = text_buf.capacity();
        let text_buf_ptr = if text_buf_cap == 0 {
            std::ptr::null_mut::<u8>()
        } else {
            let b = text_buf.into_boxed_slice();
            Box::into_raw(b).cast::<u8>()
        };

        // Build segments, pointing into text_buf.
        let mut offset: usize = 0;
        let mut segments: Vec<FfiTransformedSegment> = Vec::with_capacity(tl.segments.len());
        for (s, style) in &tl.segments {
            let seg_ptr = if text_buf_ptr.is_null() {
                std::ptr::null()
            } else {
                // SAFETY: offset is always within text_buf_len by construction.
                unsafe { text_buf_ptr.add(offset).cast_const() }
            };
            let len = s.len();
            offset += len;
            segments.push(FfiTransformedSegment {
                text_ptr: seg_ptr,
                text_len: len,
                has_style: u8::from(style.is_some()),
                _pad: [0; 7],
                style: style
                    .as_ref()
                    .map_or(FfiStyle::DEFAULT, FfiStyle::from_style),
            });
        }

        segments.shrink_to_fit();
        let segments_len = segments.len();
        let segments_cap = segments.capacity();
        let segments_ptr = if segments_cap == 0 {
            std::ptr::null_mut::<FfiTransformedSegment>()
        } else {
            let b = segments.into_boxed_slice();
            Box::into_raw(b).cast::<FfiTransformedSegment>()
        };

        Box::into_raw(Box::new(Self {
            segments_ptr,
            segments_len,
            segments_cap,
            text_buf: text_buf_ptr,
            text_buf_len,
            text_buf_cap,
        }))
    }

    /// Free an `FfiTransformedLine` and its owned allocations.
    ///
    /// # Host hazard
    ///
    /// **DO NOT CALL FROM HOST CODE.** This must only run inside the
    /// module's `.so` context from its `reovim_client_module_free_transformed_line`
    /// trampoline, using the same allocator that `from_rust` used.
    ///
    /// # Safety
    ///
    /// * `ptr` must be a pointer returned by `Self::from_rust` from the
    ///   same `.so` load.
    /// * After this call, the pointer (and all of its fields) are dangling.
    #[doc(hidden)]
    pub unsafe fn free(ptr: *mut Self) {
        if ptr.is_null() {
            return;
        }
        // SAFETY: per the contract, ptr came from Box::into_raw in from_rust.
        let this = unsafe { Box::from_raw(ptr) };
        if !this.segments_ptr.is_null() && this.segments_cap > 0 {
            // SAFETY: from_rust used Box::into_raw on an into_boxed_slice
            // with len == cap (shrink_to_fit). Reconstruct the same shape.
            let _ = unsafe {
                Vec::from_raw_parts(this.segments_ptr, this.segments_len, this.segments_cap)
            };
        }
        if !this.text_buf.is_null() && this.text_buf_cap > 0 {
            let _ =
                unsafe { Vec::from_raw_parts(this.text_buf, this.text_buf_len, this.text_buf_cap) };
        }
        #[cfg(test)]
        TRANSFORMED_LINE_FREE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // `this` (Box<Self>) drops here.
    }

    /// Host-side reader: copy the contents of a module-allocated
    /// `FfiTransformedLine` into a host-owned `TransformedLine` without
    /// taking ownership of the module's allocations.
    ///
    /// After this returns, the caller MUST call the module's
    /// `reovim_client_module_free_transformed_line` trampoline to release
    /// the module-side memory.
    ///
    /// # Safety
    ///
    /// * `ptr` must point to a valid `FfiTransformedLine` whose
    ///   `segments_ptr` and `text_buf` are live allocations in the
    ///   module's allocator.
    /// * The caller must invoke the module's free trampoline afterwards;
    ///   otherwise the module-side memory leaks.
    #[doc(hidden)]
    #[must_use]
    pub unsafe fn read_owned(ptr: *mut Self) -> TransformedLine {
        if ptr.is_null() {
            return TransformedLine {
                segments: Vec::new(),
            };
        }
        // SAFETY: caller guarantees ptr is a valid FfiTransformedLine and
        // the referenced allocations are live.
        let this = unsafe { &*ptr };

        let mut segments = Vec::with_capacity(this.segments_len);
        for i in 0..this.segments_len {
            // SAFETY: segments_ptr points to an array of segments_len elements
            // (from_rust guarantees this). Index i < segments_len.
            let seg = unsafe { &*this.segments_ptr.add(i) };
            // SAFETY: text_ptr/text_len were recorded at from_rust time and
            // point into text_buf which is still live until free is called.
            let bytes = if seg.text_ptr.is_null() || seg.text_len == 0 {
                &[][..]
            } else {
                unsafe { std::slice::from_raw_parts(seg.text_ptr, seg.text_len) }
            };
            let text = String::from_utf8_lossy(bytes).into_owned();
            let style = if seg.has_style == 1 {
                Some(seg.style.into_style())
            } else {
                None
            };
            segments.push((text, style));
        }
        TransformedLine { segments }
    }
}

// =============================================================================
// FfiRenderBehavior
// =============================================================================

/// FFI-safe `RenderBehavior`.
///
/// # Layout frozen at 0.4.0
///
/// The current design uses fixed fields for `color` / `style` regardless of
/// variant (so unused variants carry dead bytes). This was evaluated against
/// a union layout in the #723 countdown addendum and kept for simplicity;
/// the size overhead (~60 bytes) is acceptable for the expected per-frame
/// token counts.
///
/// Tag encoding:
/// * 0 = `Highlight`
/// * 1 = `Conceal { replacement }` — inline buffer holds the replacement
/// * 2 = `Background(color)`
/// * 3 = `Hide`
/// * 4 = `FullWidthLine { ch, style }`
/// * 255 = `None` (the whole return value of `classify_token` is `Option<_>`)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiRenderBehavior {
    pub tag: u8,
    pub _pad: [u8; 3],
    /// Conceal replacement text (max 31 bytes + null pad).
    pub text: [u8; 32],
    pub text_len: u8,
    pub _pad2: [u8; 3],
    /// For `Background`: the color.
    pub color: FfiColor,
    /// For `FullWidthLine`: the character (as u32 scalar).
    pub ch: u32,
    /// For `FullWidthLine`: the style.
    pub style: FfiStyle,
}

impl FfiRenderBehavior {
    /// Sentinel for `Option<RenderBehavior>::None`.
    pub const NONE: Self = Self {
        tag: 255,
        _pad: [0; 3],
        text: [0; 32],
        text_len: 0,
        _pad2: [0; 3],
        color: FfiColor::NONE,
        ch: 0,
        style: FfiStyle::DEFAULT,
    };

    /// Encode an `Option<RenderBehavior>`. `None` becomes [`Self::NONE`].
    #[must_use]
    pub fn from_option(rb: Option<RenderBehavior>) -> Self {
        match rb {
            None => Self::NONE,
            Some(RenderBehavior::Highlight) => Self {
                tag: 0,
                _pad: [0; 3],
                text: [0; 32],
                text_len: 0,
                _pad2: [0; 3],
                color: FfiColor::NONE,
                ch: 0,
                style: FfiStyle::DEFAULT,
            },
            Some(RenderBehavior::Conceal { replacement }) => {
                let bytes = replacement.as_bytes();
                let mut len = bytes.len().min(31);
                while len > 0 && !replacement.is_char_boundary(len) {
                    len -= 1;
                }
                let mut text = [0u8; 32];
                text[..len].copy_from_slice(&bytes[..len]);
                Self {
                    tag: 1,
                    _pad: [0; 3],
                    text,
                    text_len: len as u8,
                    _pad2: [0; 3],
                    color: FfiColor::NONE,
                    ch: 0,
                    style: FfiStyle::DEFAULT,
                }
            }
            Some(RenderBehavior::Background(c)) => Self {
                tag: 2,
                _pad: [0; 3],
                text: [0; 32],
                text_len: 0,
                _pad2: [0; 3],
                color: FfiColor::from_color(c),
                ch: 0,
                style: FfiStyle::DEFAULT,
            },
            Some(RenderBehavior::Hide) => Self {
                tag: 3,
                _pad: [0; 3],
                text: [0; 32],
                text_len: 0,
                _pad2: [0; 3],
                color: FfiColor::NONE,
                ch: 0,
                style: FfiStyle::DEFAULT,
            },
            Some(RenderBehavior::FullWidthLine { ch, style }) => Self {
                tag: 4,
                _pad: [0; 3],
                text: [0; 32],
                text_len: 0,
                _pad2: [0; 3],
                color: FfiColor::NONE,
                ch: ch as u32,
                style: FfiStyle::from_style(&style),
            },
        }
    }

    /// Decode back into `Option<RenderBehavior>`.
    #[must_use]
    pub fn into_option(self) -> Option<RenderBehavior> {
        match self.tag {
            0 => Some(RenderBehavior::Highlight),
            1 => {
                let len = self.text_len as usize;
                let s = std::str::from_utf8(&self.text[..len]).ok()?;
                Some(RenderBehavior::Conceal {
                    replacement: std::borrow::Cow::Owned(s.to_string()),
                })
            }
            2 => Some(RenderBehavior::Background(self.color.into_color())),
            3 => Some(RenderBehavior::Hide),
            4 => {
                let ch = char::from_u32(self.ch).unwrap_or('\0');
                Some(RenderBehavior::FullWidthLine {
                    ch,
                    style: self.style.into_style(),
                })
            }
            _ => None,
        }
    }
}

// =============================================================================
// FfiRenderSurface — vtable for `&mut dyn ChromeSurface` across FFI
// =============================================================================

/// FFI-safe render surface vtable.
///
/// The host constructs this by wrapping `&mut dyn ChromeSurface` inside
/// [`FfiRenderSurfaceHost`] and calling [`Self::from_host`]. The module
/// receives `*mut FfiRenderSurface` and calls through the function pointers
/// to reach the host's real surface.
///
/// The `opaque` pointer is a `*mut FfiRenderSurfaceHost`. The module treats
/// it as opaque and passes it back verbatim to each vtable function.
///
/// **Scoped lifetime**: the vtable is only valid for the duration of a
/// single render trampoline call. Modules that cache the pointer across
/// return will dangle when the host's stack frame unwinds.
#[repr(C)]
pub struct FfiRenderSurface {
    opaque: *mut c_void,
    pub write_styled:
        unsafe extern "C" fn(*mut c_void, u16, u16, *const u8, usize, FfiStyle) -> u16,
    pub apply_style: unsafe extern "C" fn(*mut c_void, u16, u16, FfiStyle),
    pub overlay_bg: unsafe extern "C" fn(*mut c_void, u16, u16, FfiColor),
    pub fill: unsafe extern "C" fn(*mut c_void, Rect, u32, FfiStyle),
    pub clear: unsafe extern "C" fn(*mut c_void, Rect),
    pub size: unsafe extern "C" fn(*mut c_void) -> u32,
}

/// Host-side wrapper holding the fat pointer `&mut dyn ChromeSurface`.
///
/// Lives on the host's stack frame for the duration of a render
/// trampoline call. The `PhantomData` anchors the lifetime `'a` so the
/// borrow checker rejects attempts to concurrently re-borrow the host
/// struct while an `FfiRenderSurface` derived from it is live.
///
/// Note: the compile-time anchor only prevents **concurrent re-borrow** of
/// `FfiRenderSurfaceHost`, not module-side pointer capture. The latter is
/// a runtime/protocol contract enforced by `catch_unwind` isolation and
/// module-author documentation.
pub struct FfiRenderSurfaceHost<'a> {
    pub(crate) surface: &'a mut dyn ChromeSurface,
    _anchor: PhantomData<&'a mut dyn ChromeSurface>,
}

impl<'a> FfiRenderSurfaceHost<'a> {
    /// Wrap a borrowed render surface for FFI dispatch.
    #[must_use]
    pub fn new(surface: &'a mut dyn ChromeSurface) -> Self {
        Self {
            surface,
            _anchor: PhantomData,
        }
    }
}

impl FfiRenderSurface {
    /// Construct an `FfiRenderSurface` vtable pointing at a host wrapper.
    ///
    /// The `&'a mut FfiRenderSurfaceHost<'a>` signature uses lifetime
    /// invariance to prevent the host from re-borrowing the wrapper while
    /// the returned vtable is alive.
    #[must_use]
    pub fn from_host<'a>(host: &'a mut FfiRenderSurfaceHost<'a>) -> Self {
        let opaque = std::ptr::from_mut::<FfiRenderSurfaceHost<'a>>(host).cast::<c_void>();
        Self {
            opaque,
            write_styled: ffi_rs_write_styled,
            apply_style: ffi_rs_apply_style,
            overlay_bg: ffi_rs_overlay_bg,
            fill: ffi_rs_fill,
            clear: ffi_rs_clear,
            size: ffi_rs_size,
        }
    }
}

// -- FfiRenderSurface vtable trampoline functions ---------------------------

unsafe extern "C" fn ffi_rs_write_styled(
    opaque: *mut c_void,
    x: u16,
    y: u16,
    text_ptr: *const u8,
    text_len: usize,
    style: FfiStyle,
) -> u16 {
    // SAFETY: opaque was constructed from a &mut FfiRenderSurfaceHost in
    // from_host. text_ptr/text_len came from a Rust &str on the host side
    // (type-system UTF-8 guarantee) and remain valid for this call.
    unsafe {
        let host = &mut *(opaque.cast::<FfiRenderSurfaceHost<'_>>());
        let text = std::str::from_utf8_unchecked(std::slice::from_raw_parts(text_ptr, text_len));
        host.surface.write_styled(x, y, text, style.into_style())
    }
}

unsafe extern "C" fn ffi_rs_apply_style(opaque: *mut c_void, x: u16, y: u16, style: FfiStyle) {
    // SAFETY: opaque constructed from &mut FfiRenderSurfaceHost in from_host.
    unsafe {
        let host = &mut *(opaque.cast::<FfiRenderSurfaceHost<'_>>());
        host.surface.apply_style(x, y, style.into_style());
    }
}

unsafe extern "C" fn ffi_rs_overlay_bg(opaque: *mut c_void, x: u16, y: u16, color: FfiColor) {
    // SAFETY: opaque constructed from &mut FfiRenderSurfaceHost in from_host.
    unsafe {
        let host = &mut *(opaque.cast::<FfiRenderSurfaceHost<'_>>());
        host.surface.overlay_bg(x, y, color.into_color());
    }
}

unsafe extern "C" fn ffi_rs_fill(opaque: *mut c_void, rect: Rect, ch: u32, style: FfiStyle) {
    // SAFETY: opaque constructed from &mut FfiRenderSurfaceHost in from_host.
    unsafe {
        let host = &mut *(opaque.cast::<FfiRenderSurfaceHost<'_>>());
        let ch = char::from_u32(ch).unwrap_or('\0');
        host.surface.fill(rect, ch, style.into_style());
    }
}

unsafe extern "C" fn ffi_rs_clear(opaque: *mut c_void, rect: Rect) {
    // SAFETY: opaque constructed from &mut FfiRenderSurfaceHost in from_host.
    unsafe {
        let host = &mut *(opaque.cast::<FfiRenderSurfaceHost<'_>>());
        host.surface.clear(rect);
    }
}

unsafe extern "C" fn ffi_rs_size(opaque: *mut c_void) -> u32 {
    // SAFETY: opaque constructed from &mut FfiRenderSurfaceHost in from_host.
    unsafe {
        let host = &*(opaque.cast::<FfiRenderSurfaceHost<'_>>());
        let (cols, rows) = host.surface.size();
        (u32::from(cols) << 16) | u32::from(rows)
    }
}

/// Module-side wrapper: implements `ChromeSurface` by calling through the
/// vtable. Used inside the `chrome_render` trampoline.
pub struct FfiRenderSurfaceRef<'a> {
    ffi: &'a mut FfiRenderSurface,
}

impl<'a> FfiRenderSurfaceRef<'a> {
    #[must_use]
    pub fn new(ffi: &'a mut FfiRenderSurface) -> Self {
        Self { ffi }
    }
}

impl ChromeSurface for FfiRenderSurfaceRef<'_> {
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        // SAFETY: vtable pointer is valid for the duration of the call.
        unsafe {
            (self.ffi.write_styled)(
                self.ffi.opaque,
                x,
                y,
                text.as_ptr(),
                text.len(),
                FfiStyle::from_style(&style),
            )
        }
    }
    fn apply_style(&mut self, x: u16, y: u16, style: Style) {
        // SAFETY: vtable pointer valid for call duration.
        unsafe {
            (self.ffi.apply_style)(self.ffi.opaque, x, y, FfiStyle::from_style(&style));
        }
    }
    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        // SAFETY: vtable pointer valid for call duration.
        unsafe {
            (self.ffi.overlay_bg)(self.ffi.opaque, x, y, FfiColor::from_color(bg));
        }
    }
    fn fill(&mut self, rect: Rect, ch: char, style: Style) {
        // SAFETY: vtable pointer valid for call duration.
        unsafe {
            (self.ffi.fill)(self.ffi.opaque, rect, ch as u32, FfiStyle::from_style(&style));
        }
    }
    fn clear(&mut self, rect: Rect) {
        // SAFETY: vtable pointer valid for call duration.
        unsafe {
            (self.ffi.clear)(self.ffi.opaque, rect);
        }
    }
    fn size(&self) -> (u16, u16) {
        // SAFETY: vtable pointer valid for call duration.
        let packed = unsafe { (self.ffi.size)(self.ffi.opaque) };
        #[allow(clippy::cast_possible_truncation)]
        ((packed >> 16) as u16, (packed & 0xFFFF) as u16)
    }
}

// =============================================================================
// FfiThemeProvider — vtable for `&dyn ThemeProvider` across FFI
// =============================================================================

/// FFI-safe vtable wrapping `&dyn ThemeProvider`.
///
/// Same scoped-lifetime contract as [`FfiRenderSurface`]: valid only for
/// the duration of the `on_theme_changed` trampoline call.
///
/// `highlight_with_fallback` is intentionally omitted — it takes `&[&str]`
/// which does not have a straightforward FFI representation, and modules
/// that need it can call `highlight` in a loop instead.
#[repr(C)]
pub struct FfiThemeProvider {
    opaque: *const c_void,
    pub highlight: unsafe extern "C" fn(*const c_void, *const u8, usize) -> FfiStyle,
    pub foreground: unsafe extern "C" fn(*const c_void) -> FfiStyle,
    pub background: unsafe extern "C" fn(*const c_void) -> FfiStyle,
    /// Returns 0 or 1.
    pub is_dark: unsafe extern "C" fn(*const c_void) -> i32,
}

// SAFETY: FfiThemeProvider contains a `*const c_void` opaque pointer whose
// target (`FfiThemeProviderHost`) borrows a `&dyn ThemeProvider`, and the
// `ThemeProvider` trait itself requires `Send + Sync`. The vtable function
// pointers are `extern "C" fn(...)` which are always `Send + Sync`. Marking
// the struct `Send + Sync` lets `FfiThemeRef` (which wraps `&FfiThemeProvider`)
// satisfy the `ThemeProvider: Send + Sync` bound.
unsafe impl Send for FfiThemeProvider {}
unsafe impl Sync for FfiThemeProvider {}

/// Host-side wrapper holding `&'a dyn ThemeProvider`.
pub struct FfiThemeProviderHost<'a> {
    pub(crate) inner: &'a dyn ThemeProvider,
    _anchor: PhantomData<&'a dyn ThemeProvider>,
}

impl<'a> FfiThemeProviderHost<'a> {
    #[must_use]
    pub fn new(inner: &'a dyn ThemeProvider) -> Self {
        Self {
            inner,
            _anchor: PhantomData,
        }
    }
}

impl FfiThemeProvider {
    /// Construct an `FfiThemeProvider` vtable pointing at a host wrapper.
    #[must_use]
    pub fn from_host<'a>(host: &'a FfiThemeProviderHost<'a>) -> Self {
        let opaque = std::ptr::from_ref::<FfiThemeProviderHost<'a>>(host).cast::<c_void>();
        Self {
            opaque,
            highlight: ffi_theme_highlight,
            foreground: ffi_theme_foreground,
            background: ffi_theme_background,
            is_dark: ffi_theme_is_dark,
        }
    }
}

unsafe extern "C" fn ffi_theme_highlight(
    opaque: *const c_void,
    group_ptr: *const u8,
    group_len: usize,
) -> FfiStyle {
    // SAFETY: opaque constructed from &FfiThemeProviderHost in from_host;
    // group_ptr/group_len came from a Rust &str (UTF-8 by type system).
    unsafe {
        let host = &*(opaque.cast::<FfiThemeProviderHost<'_>>());
        let group = std::str::from_utf8_unchecked(std::slice::from_raw_parts(group_ptr, group_len));
        FfiStyle::from_style(&host.inner.highlight(group))
    }
}

unsafe extern "C" fn ffi_theme_foreground(opaque: *const c_void) -> FfiStyle {
    // SAFETY: opaque constructed from &FfiThemeProviderHost in from_host.
    unsafe {
        let host = &*(opaque.cast::<FfiThemeProviderHost<'_>>());
        FfiStyle::from_style(&host.inner.foreground())
    }
}

unsafe extern "C" fn ffi_theme_background(opaque: *const c_void) -> FfiStyle {
    // SAFETY: opaque constructed from &FfiThemeProviderHost in from_host.
    unsafe {
        let host = &*(opaque.cast::<FfiThemeProviderHost<'_>>());
        FfiStyle::from_style(&host.inner.background())
    }
}

unsafe extern "C" fn ffi_theme_is_dark(opaque: *const c_void) -> i32 {
    // SAFETY: opaque constructed from &FfiThemeProviderHost in from_host.
    unsafe {
        let host = &*(opaque.cast::<FfiThemeProviderHost<'_>>());
        i32::from(host.inner.is_dark())
    }
}

/// Module-side wrapper: implements `ThemeProvider` by calling through the
/// FFI vtable. Used inside the `on_theme_changed` trampoline.
pub struct FfiThemeRef<'a> {
    ffi: &'a FfiThemeProvider,
}

impl<'a> FfiThemeRef<'a> {
    #[must_use]
    pub fn new(ffi: &'a FfiThemeProvider) -> Self {
        Self { ffi }
    }
}

impl ThemeProvider for FfiThemeRef<'_> {
    fn highlight(&self, group: &str) -> Style {
        // SAFETY: vtable pointer valid for call duration.
        unsafe { (self.ffi.highlight)(self.ffi.opaque, group.as_ptr(), group.len()).into_style() }
    }
    fn highlight_with_fallback(&self, groups: &[&str]) -> Style {
        // Not part of the FFI vtable — emulate by iterating.
        for g in groups {
            let s = self.highlight(g);
            if s != Style::default() {
                return s;
            }
        }
        Style::default()
    }
    fn foreground(&self) -> Style {
        // SAFETY: vtable pointer valid for call duration.
        unsafe { (self.ffi.foreground)(self.ffi.opaque).into_style() }
    }
    fn background(&self) -> Style {
        // SAFETY: vtable pointer valid for call duration.
        unsafe { (self.ffi.background)(self.ffi.opaque).into_style() }
    }
    fn is_dark(&self) -> bool {
        // SAFETY: vtable pointer valid for call duration.
        unsafe { (self.ffi.is_dark)(self.ffi.opaque) != 0 }
    }
}

#[cfg(test)]
#[path = "ffi_tests.rs"]
mod tests;
