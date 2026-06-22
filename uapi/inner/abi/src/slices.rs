//! 6.3 §2.2 — Byte-slice types.
//!
//! All types cross the `extern "C"` boundary as raw pointer + length pairs.
//! Ownership semantics are recorded in 6.2 §4:
//! - **caller-owned-for-call**: pointer valid only for the call duration.
//! - **callee-owned-until-release**: pointer valid until `destroy_*` call.

/// Immutable byte slice, caller-owned-for-call (6.3 §2.2).
///
/// `data == NULL` with `len == 0` is a valid empty slice (6.3 open item 3
/// default: yes).
///
/// ```rust
/// use reovim_uapi_abi::slices::ByteSlice;
/// use core::ptr;
///
/// let empty = ByteSlice { data: ptr::null(), len: 0 };
/// assert_eq!(empty.len, 0);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ByteSlice {
    /// Pointer to the first byte; may be null when `len == 0`.
    pub data: *const u8,
    /// Number of bytes.
    pub len: usize,
}

/// Mutable byte slice, caller-owned-for-call (6.3 §2.2).
///
/// ```rust
/// use reovim_uapi_abi::slices::ByteSliceMut;
/// use core::ptr;
///
/// let empty = ByteSliceMut { data: ptr::null_mut(), len: 0 };
/// assert_eq!(empty.len, 0);
/// ```
#[repr(C)]
#[derive(Debug)]
pub struct ByteSliceMut {
    /// Mutable pointer to the first byte; may be null when `len == 0`.
    pub data: *mut u8,
    /// Number of bytes.
    pub len: usize,
}

/// UTF-8 byte slice, caller-owned-for-call (6.3 §2.2).
///
/// Identical layout to [`ByteSlice`]; the UTF-8 invariant is enforced by the
/// kernel before construction.
///
/// ```rust
/// use reovim_uapi_abi::slices::StrSlice;
/// use core::ptr;
///
/// let empty = StrSlice { data: ptr::null(), len: 0 };
/// assert_eq!(empty.len, 0);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StrSlice {
    /// Pointer to the first UTF-8 byte; may be null when `len == 0`.
    pub data: *const u8,
    /// Number of bytes (not chars).
    pub len: usize,
}

/// Callee-owned byte buffer with an explicit free function (6.3 §2.2).
///
/// The cdylib that hands this out must set `free` to a function that
/// releases `data[..cap]`.  The caller invokes `free(data, cap)` when done.
///
/// ```rust
/// use reovim_uapi_abi::slices::ByteBuf;
/// use core::ptr;
///
/// // Construct a zero-length, no-op buffer for testing.
/// unsafe extern "C" fn noop_free(_: *mut u8, _: usize) {}
/// let buf = ByteBuf { data: ptr::null_mut(), cap: 0, len: 0, free: noop_free };
/// assert_eq!(buf.cap, 0);
/// ```
#[repr(C)]
pub struct ByteBuf {
    /// Callee-owned data pointer.
    pub data: *mut u8,
    /// Allocated capacity in bytes.
    pub cap: usize,
    /// Initialized length in bytes (`<= cap`).
    pub len: usize,
    /// Releases `data[..cap]`; called by the owner when done.
    pub free: unsafe extern "C" fn(*mut u8, usize),
}

/// Caller-provided error message buffer (6.2 §3, 6.3 §2.2).
///
/// On a fallible vtable call the cdylib writes a UTF-8 diagnostic into
/// `data[..cap]`, sets `len`, and returns an [`crate::error::ErrorCode`].
/// Caller owns `data`.
///
/// ```rust
/// use reovim_uapi_abi::slices::ErrorBuf;
/// use core::ptr;
///
/// let buf = ErrorBuf { data: ptr::null_mut(), cap: 0, len: 0 };
/// assert_eq!(buf.cap, 0);
/// ```
#[repr(C)]
#[derive(Debug)]
pub struct ErrorBuf {
    /// Caller-owned buffer for the UTF-8 diagnostic.
    pub data: *mut u8,
    /// Buffer capacity in bytes.
    pub cap: usize,
    /// Bytes written by the callee.
    pub len: usize,
}
