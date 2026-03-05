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

impl From<reovim_kernel::api::v1::Position> for ReovimPosition {
    #[allow(clippy::cast_possible_truncation)]
    fn from(pos: reovim_kernel::api::v1::Position) -> Self {
        Self {
            line: pos.line as u32,
            column: pos.column as u32,
        }
    }
}

impl From<ReovimPosition> for reovim_kernel::api::v1::Position {
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

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::Position};

    // ========================================================================
    // ReovimPosition tests
    // ========================================================================

    #[test]
    fn test_position_new() {
        let pos = ReovimPosition::new(10, 20);
        assert_eq!(pos.line, 10);
        assert_eq!(pos.column, 20);
    }

    #[test]
    fn test_position_from_kernel() {
        let kernel_pos = Position::new(42, 7);
        let ffi_pos = ReovimPosition::from(kernel_pos);
        assert_eq!(ffi_pos.line, 42);
        assert_eq!(ffi_pos.column, 7);
    }

    #[test]
    fn test_position_to_kernel() {
        let ffi_pos = ReovimPosition::new(5, 3);
        let kernel_pos = Position::from(ffi_pos);
        assert_eq!(kernel_pos.line, 5);
        assert_eq!(kernel_pos.column, 3);
    }

    #[test]
    fn test_position_roundtrip() {
        let original = Position::new(100, 200);
        let ffi = ReovimPosition::from(original);
        let back = Position::from(ffi);
        assert_eq!(original, back);
    }

    #[test]
    fn test_position_size_and_alignment() {
        assert_eq!(std::mem::size_of::<ReovimPosition>(), 8);
        assert_eq!(std::mem::align_of::<ReovimPosition>(), 4);
    }

    #[test]
    fn test_position_repr_c_layout() {
        let pos = ReovimPosition::new(1, 2);
        let ptr = (&raw const pos).cast::<u32>();
        unsafe {
            assert_eq!(*ptr, 1); // line
            assert_eq!(*ptr.add(1), 2); // column
        }
    }

    #[test]
    fn test_position_zero() {
        let pos = ReovimPosition::new(0, 0);
        assert_eq!(pos, ReovimPosition { line: 0, column: 0 });
    }

    #[test]
    fn test_position_max_values() {
        let pos = ReovimPosition::new(u32::MAX, u32::MAX);
        assert_eq!(pos.line, u32::MAX);
        assert_eq!(pos.column, u32::MAX);
    }

    #[test]
    fn test_position_debug() {
        let pos = ReovimPosition::new(3, 7);
        let debug = format!("{pos:?}");
        assert!(debug.contains('3'));
        assert!(debug.contains('7'));
    }

    #[test]
    fn test_position_clone_copy() {
        let pos = ReovimPosition::new(5, 10);
        let copied = pos;
        assert_eq!(pos, copied);
    }

    // ========================================================================
    // ReovimStringResult tests
    // ========================================================================

    #[test]
    fn test_string_result_ok() {
        let r = ReovimStringResult::ok(42);
        assert_eq!(r.status, crate::error::REOVIM_OK);
        assert_eq!(r.length, 42);
    }

    #[test]
    fn test_string_result_err() {
        let r = ReovimStringResult::err(crate::error::REOVIM_ERR_NOT_FOUND);
        assert_eq!(r.status, crate::error::REOVIM_ERR_NOT_FOUND);
        assert_eq!(r.length, 0);
    }

    #[test]
    fn test_string_result_size_and_alignment() {
        assert_eq!(std::mem::size_of::<ReovimStringResult>(), 8);
        assert_eq!(std::mem::align_of::<ReovimStringResult>(), 4);
    }

    #[test]
    fn test_string_result_zero_length() {
        let r = ReovimStringResult::ok(0);
        assert_eq!(r.status, crate::error::REOVIM_OK);
        assert_eq!(r.length, 0);
    }

    #[test]
    fn test_string_result_debug() {
        let r = ReovimStringResult::ok(5);
        let debug = format!("{r:?}");
        assert!(debug.contains("ReovimStringResult"));
    }

    #[test]
    fn test_string_result_clone_copy() {
        let r = ReovimStringResult::ok(10);
        let copied = r;
        assert_eq!(r, copied);
    }

    // ========================================================================
    // ReovimCommandArgs tests
    // ========================================================================

    #[test]
    fn test_command_args_empty() {
        let args = ReovimCommandArgs::empty();
        assert_eq!(args.has_count, 0);
        assert_eq!(args.count, 0);
        assert_eq!(args.has_register, 0);
        assert_eq!(args.register, 0);
        assert_eq!(args.has_cursor, 0);
        assert_eq!(args.cursor, ReovimPosition::new(0, 0));
    }

    #[test]
    fn test_command_args_size_and_alignment() {
        assert_eq!(std::mem::size_of::<ReovimCommandArgs>(), 28);
        assert_eq!(std::mem::align_of::<ReovimCommandArgs>(), 4);
    }

    #[test]
    fn test_command_args_with_count() {
        let args = ReovimCommandArgs {
            has_count: 1,
            count: 5,
            ..ReovimCommandArgs::empty()
        };
        assert_eq!(args.has_count, 1);
        assert_eq!(args.count, 5);
    }

    #[test]
    fn test_command_args_with_register() {
        let args = ReovimCommandArgs {
            has_register: 1,
            register: b'a',
            ..ReovimCommandArgs::empty()
        };
        assert_eq!(args.has_register, 1);
        assert_eq!(args.register, b'a');
    }

    #[test]
    fn test_command_args_with_cursor() {
        let args = ReovimCommandArgs {
            has_cursor: 1,
            cursor: ReovimPosition::new(10, 5),
            ..ReovimCommandArgs::empty()
        };
        assert_eq!(args.has_cursor, 1);
        assert_eq!(args.cursor.line, 10);
        assert_eq!(args.cursor.column, 5);
    }

    #[test]
    fn test_command_args_fully_populated() {
        let args = ReovimCommandArgs {
            has_count: 1,
            count: 3,
            has_register: 1,
            register: b'"',
            pad: [0; 3],
            has_cursor: 1,
            cursor: ReovimPosition::new(42, 7),
        };
        assert_eq!(args.has_count, 1);
        assert_eq!(args.count, 3);
        assert_eq!(args.has_register, 1);
        assert_eq!(args.register, b'"');
        assert_eq!(args.has_cursor, 1);
        assert_eq!(args.cursor, ReovimPosition::new(42, 7));
    }

    #[test]
    fn test_command_args_debug() {
        let args = ReovimCommandArgs::empty();
        let debug = format!("{args:?}");
        assert!(debug.contains("ReovimCommandArgs"));
    }

    #[test]
    fn test_command_args_clone_copy() {
        let args = ReovimCommandArgs {
            has_count: 1,
            count: 99,
            ..ReovimCommandArgs::empty()
        };
        let copied = args;
        assert_eq!(args, copied);
    }
}
