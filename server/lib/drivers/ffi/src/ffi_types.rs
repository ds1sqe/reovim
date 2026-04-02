//! `#[repr(C)]` types for the FFI boundary.
//!
//! These types are used by C-callable functions to pass structured data
//! across the FFI boundary. All types have stable ABI guarantees.
//!
//! # Memory Layout
//!
//! | Type | Size (bytes) | Alignment |
//! |------|-------------|-----------|
//! | `ReovimPosition` | 8 | 4 |
//! | `ReovimStringResult` | 8 | 4 |
//! | `ReovimCommandArgs` | 28 | 4 |

/// A line/column position in a buffer.
///
/// Corresponds to `kernel::Position` but with C-compatible layout.
/// Both `line` and `column` are zero-based.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReovimPosition {
    /// Zero-based line number.
    pub line: u32,
    /// Zero-based column number.
    pub column: u32,
}

impl ReovimPosition {
    /// Create a new position.
    #[must_use]
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

impl From<reovim_types_text::Position> for ReovimPosition {
    #[allow(clippy::cast_possible_truncation)]
    fn from(pos: reovim_types_text::Position) -> Self {
        Self {
            line: pos.line as u32,
            column: pos.column as u32,
        }
    }
}

impl From<ReovimPosition> for reovim_types_text::Position {
    fn from(pos: ReovimPosition) -> Self {
        Self::new(pos.line as usize, pos.column as usize)
    }
}

/// Result of a string read operation.
///
/// When a C-callable function fills a caller-owned buffer with a string,
/// this struct reports the status and actual length. If `length > buf_len`,
/// the string was truncated.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReovimStringResult {
    /// Status code (`REOVIM_OK` or an error).
    pub status: i32,
    /// Actual string length in bytes (may exceed the provided buffer size,
    /// indicating truncation).
    pub length: u32,
}

impl ReovimStringResult {
    /// Create a success result.
    #[must_use]
    pub const fn ok(length: u32) -> Self {
        Self {
            status: crate::error::REOVIM_OK,
            length,
        }
    }

    /// Create an error result.
    #[must_use]
    pub const fn err(status: i32) -> Self {
        Self { status, length: 0 }
    }
}

/// Parsed command arguments passed to FFI command callbacks.
///
/// This mirrors the essential fields from `CommandContext` in a C-safe layout.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReovimCommandArgs {
    /// Whether a count prefix was provided (0 = no, 1 = yes).
    pub has_count: i32,
    /// Count value (valid only if `has_count` is 1).
    pub count: u32,
    /// Whether a register was specified (0 = no, 1 = yes).
    pub has_register: i32,
    /// Register character (valid only if `has_register` is 1).
    pub register: u8,
    /// Padding for alignment after the `u8` register field.
    pub pad: [u8; 3],
    /// Whether cursor position is available (0 = no, 1 = yes).
    pub has_cursor: i32,
    /// Cursor position (valid only if `has_cursor` is 1).
    pub cursor: ReovimPosition,
}

impl ReovimCommandArgs {
    /// Create empty command args (no count, no register, no cursor).
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            has_count: 0,
            count: 0,
            has_register: 0,
            register: 0,
            pad: [0; 3],
            has_cursor: 0,
            cursor: ReovimPosition { line: 0, column: 0 },
        }
    }
}

/// Yank type for register operations.
///
/// Determines paste behavior: characterwise inserts at cursor,
/// linewise inserts above/below current line.
///
/// Corresponds to kernel `YankType` but with C-compatible layout.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReovimYankType {
    /// Characterwise yank (e.g., `yw`, `y$`).
    Characterwise = 0,
    /// Linewise yank (e.g., `yy`, `dd`).
    Linewise = 1,
}

impl From<reovim_kernel::api::v1::YankType> for ReovimYankType {
    fn from(yt: reovim_kernel::api::v1::YankType) -> Self {
        match yt {
            reovim_kernel::api::v1::YankType::Characterwise => Self::Characterwise,
            reovim_kernel::api::v1::YankType::Linewise => Self::Linewise,
        }
    }
}

impl From<ReovimYankType> for reovim_kernel::api::v1::YankType {
    fn from(yt: ReovimYankType) -> Self {
        match yt {
            ReovimYankType::Characterwise => Self::Characterwise,
            ReovimYankType::Linewise => Self::Linewise,
        }
    }
}

// ============================================================================
// Compile-time size and alignment assertions
// ============================================================================

const _: () = {
    assert!(std::mem::size_of::<ReovimPosition>() == 8);
    assert!(std::mem::align_of::<ReovimPosition>() == 4);
};

const _: () = {
    assert!(std::mem::size_of::<ReovimStringResult>() == 8);
    assert!(std::mem::align_of::<ReovimStringResult>() == 4);
};

const _: () = {
    assert!(std::mem::size_of::<ReovimCommandArgs>() == 28);
    assert!(std::mem::align_of::<ReovimCommandArgs>() == 4);
};

const _: () = {
    assert!(std::mem::size_of::<ReovimYankType>() == 4);
    assert!(std::mem::align_of::<ReovimYankType>() == 4);
};

#[cfg(test)]
#[path = "ffi_types_tests.rs"]
mod tests;
