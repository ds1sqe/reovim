//! 6.3 §7 — `RawInput` and per-kind payload types.
//!
//! All per-kind payload structs have `#[repr(C)]` and their golden sizes
//! are specified in the catalog:
//! - [`KeyEvent`]: size 24, align 8.
//! - [`MouseEvent`]: size 32, align 8.
//! - [`TriggerEvent`]: size 16, align 8.
//! - [`ImeEvent`]: 16-byte header (the struct itself, excluding the appended
//!   UTF-8 bytes).
use crate::slices::ByteSlice;

/// Raw input event as it crosses the ABI boundary (6.3 §7).
///
/// The `payload` interprets per `kind`:
/// - [`RawInputKind::Key`] → `payload` is `&KeyEvent` cast to bytes.
/// - [`RawInputKind::Mouse`] → `payload` is `&MouseEvent` cast to bytes.
/// - [`RawInputKind::Text`] / [`RawInputKind::Paste`] → `payload` is raw
///   UTF-8 bytes (no header struct).
/// - [`RawInputKind::Trigger`] → `payload` is `&TriggerEvent` cast to bytes.
/// - [`RawInputKind::Ime`] → `payload` is `ImeEvent` header (16 bytes) +
///   UTF-8 text continuation.
///
/// ```rust
/// use reovim_uapi_abi::input::{RawInput, RawInputKind};
/// use reovim_uapi_abi::slices::ByteSlice;
/// use core::ptr;
///
/// let ev = RawInput {
///     kind:    RawInputKind::Key,
///     payload: ByteSlice { data: ptr::null(), len: 0 },
/// };
/// assert_eq!(ev.kind, RawInputKind::Key);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawInput {
    /// Discriminates the active payload shape.
    pub kind: RawInputKind,
    /// Payload bytes; interpret per `kind`.
    pub payload: ByteSlice,
}

/// Discriminates the payload of a [`RawInput`] event (6.3 §7).
///
/// Values `128..=255` are reserved for per-cdylib custom kinds (see 8.3
/// CL10).
///
/// ```rust
/// use reovim_uapi_abi::input::RawInputKind;
///
/// assert_eq!(RawInputKind::Key as u8, 1);
/// assert_eq!(RawInputKind::Ime as u8, 7);
/// ```
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawInputKind {
    /// Keyboard event; payload is [`KeyEvent`].
    Key = 1,
    /// Mouse event; payload is [`MouseEvent`].
    Mouse = 2,
    /// Typed character run (bracketed input); payload is raw UTF-8 bytes.
    Text = 3,
    /// Explicit clipboard paste; payload is raw UTF-8 bytes.
    Paste = 4,
    /// Reserved; payload locked with the WASM-peer decision (out of v4
    /// target).
    Web = 5,
    /// Software trigger; payload is [`TriggerEvent`].
    Trigger = 6,
    /// IME composition; payload is [`ImeEvent`] header + UTF-8 text.
    Ime = 7,
    // 128..=255 reserved for module-defined custom kinds (per-cdylib).
}

/// Keyboard event payload (6.3 §7.1).
///
/// Size 24, align 8.  Platforms that cannot observe key release (classic
/// terminals) emit `press` only; modules MUST NOT require release events
/// for correctness.
///
/// ## Keycode vocabulary (6.3 §7.1.1)
///
/// - `0x0001..=0xFFFF`: USB HID usage IDs from usage page `0x07`.
/// - `0x0001_0001..`: Reovim extension range (`Compose`, `ImeToggle`, …).
///
/// ## Modifier bitfield (6.3 §7.1.2)
///
/// | Bit | Modifier |
/// |---|---|
/// | 0 | Shift |
/// | 1 | Ctrl |
/// | 2 | Alt |
/// | 3 | Super |
/// | 4 | AltGr |
/// | 5 | CapsLock |
/// | 6 | NumLock |
/// | 7 | Reserved |
///
/// ```rust
/// use reovim_uapi_abi::input::KeyEvent;
/// use core::mem;
///
/// assert_eq!(mem::size_of::<KeyEvent>(), 24);
/// assert_eq!(mem::align_of::<KeyEvent>(), 8);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    /// USB HID keycode or Reovim extension (6.3 §7.1.1).
    pub keycode: u32,
    /// Modifier bitfield (6.3 §7.1.2).
    pub mods: u8,
    /// 0 = press, 1 = release, 2 = repeat.
    pub action: u8,
    /// Padding.
    pub pad: [u8; 2],
    /// Monotonic timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// Inline UTF-8: `utf8[0]` is the byte length (0..=7); `utf8[1..=len]`
    /// are the bytes (6.3 §7.1.3).
    pub utf8: [u8; 8],
}

/// Mouse event payload (6.3 §7.2).
///
/// Size 32, align 8.  Grid cells are the primary coordinate frame.
/// Pixel fields (`px_x`, `px_y`) are valid only when `flags bit 0` is set.
///
/// ```rust
/// use reovim_uapi_abi::input::MouseEvent;
/// use core::mem;
///
/// assert_eq!(mem::size_of::<MouseEvent>(), 32);
/// assert_eq!(mem::align_of::<MouseEvent>(), 8);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    /// 0 = press, 1 = release, 2 = motion, 3 = scroll.
    pub action: u8,
    /// 0 = none, 1 = left, 2 = right, 3 = middle, 4..=7 = extra.
    pub button: u8,
    /// Modifier bitfield; same as [`KeyEvent::mods`].
    pub mods: u8,
    /// Flags; bit 0: pixel fields are valid.
    pub flags: u8,
    /// Grid cell column, window-local (primary frame).
    pub col: i32,
    /// Grid cell row, window-local.
    pub row: i32,
    /// Pixel offset X, window-local; valid iff `flags & 1`.
    pub px_x: i32,
    /// Pixel offset Y, window-local; valid iff `flags & 1`.
    pub px_y: i32,
    /// Horizontal scroll delta in 1/8-line units (smooth scroll).
    pub scroll_x: i16,
    /// Vertical scroll delta in 1/8-line units.
    pub scroll_y: i16,
    /// Monotonic timestamp in nanoseconds.
    pub timestamp_ns: u64,
}

/// Software-trigger event payload (6.3 §7.4).
///
/// Size 16, align 8.  Opaque to the kernel beyond routing (5.2 §6).
///
/// ```rust
/// use reovim_uapi_abi::input::TriggerEvent;
/// use core::mem;
///
/// assert_eq!(mem::size_of::<TriggerEvent>(), 16);
/// assert_eq!(mem::align_of::<TriggerEvent>(), 8);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TriggerEvent {
    /// Trigger identifier.
    pub trigger_id: u32,
    /// Padding.
    pub pad: u32,
    /// Monotonic timestamp in nanoseconds.
    pub timestamp_ns: u64,
}

/// IME composition event header (6.3 §7.5).
///
/// This is the 16-byte header; the full payload is this struct followed by
/// `payload.len - 16` bytes of UTF-8 text.
///
/// Keymap modules MUST pass `Ime` events through to the focused Domain
/// rather than interpreting them as key sequences.
///
/// ```rust
/// use reovim_uapi_abi::input::ImeEvent;
/// use core::mem;
///
/// // Only the header struct; text follows in the payload buffer.
/// assert_eq!(mem::size_of::<ImeEvent>(), 16);
/// assert_eq!(mem::align_of::<ImeEvent>(), 8);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ImeEvent {
    /// 0 = begin, 1 = update, 2 = commit, 3 = cancel.
    pub phase: u8,
    /// Padding.
    pub pad: [u8; 3],
    /// Caret position within the preedit text, in bytes.
    pub caret_byte: u32,
    /// Monotonic timestamp in nanoseconds.
    pub timestamp_ns: u64,
    // payload continues: UTF-8 preedit (begin/update) or committed text
    // (commit); text length = `total_payload_len - 16`.
}
