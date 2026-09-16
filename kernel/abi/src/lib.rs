#![no_std]
#![deny(unsafe_code)]
//! The Driver contract in C, include/reovim.h, mirrored in Rust, and the
//! adapter that presents a C descriptor to the Kernel as `Module` and
//! `Driver`. The header is the contract; this crate follows it.

extern crate alloc;

mod native;

use core::ffi::{c_char, c_void};

use reovim_kernel_interface::Error;

pub use native::{Native, boot};

/// `RV_MAGIC`: "RVM1" little endian.
pub const MAGIC: u32 = 0x314d_5652;
/// `RV_ABI`.
pub const ABI: u32 = 1;
/// `RV_PENDING`.
pub const PENDING: u32 = 10;

/// `rv_bytes`: borrowed bytes; a null `ptr` with `len` 0 is none.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RvBytes {
    pub ptr: *const u8,
    pub len: usize,
}

impl RvBytes {
    pub const NONE: Self = Self { ptr: core::ptr::null(), len: 0 };

    /// Borrow `bytes`.
    #[must_use]
    pub const fn of(bytes: &[u8]) -> Self {
        Self { ptr: bytes.as_ptr(), len: bytes.len() }
    }
}

/// `rv_reply`: `status` 0 is ok, 1 to 9 the nine causes, 10 pending.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RvReply {
    pub status: u32,
    pub value: u64,
}

impl RvReply {
    #[must_use]
    pub const fn ok(value: u64) -> Self {
        Self { status: 0, value }
    }

    #[must_use]
    pub const fn err(error: Error) -> Self {
        Self { status: status_of(error), value: 0 }
    }

    #[must_use]
    pub const fn pending(token: u64) -> Self {
        Self { status: PENDING, value: token }
    }
}

/// The nine causes in 00-core.md order, from 1.
#[must_use]
pub const fn status_of(error: Error) -> u32 {
    match error {
        Error::Denied => 1,
        Error::Unsupported => 2,
        Error::Invalid => 3,
        Error::Stale => 4,
        Error::Busy => 5,
        Error::NoSpace => 6,
        Error::Closed => 7,
        Error::Gone => 8,
        Error::Cancelled => 9,
    }
}

/// The cause a status names; an unknown one is `Invalid`.
#[must_use]
pub const fn error_of(status: u32) -> Error {
    match status {
        1 => Error::Denied,
        2 => Error::Unsupported,
        4 => Error::Stale,
        5 => Error::Busy,
        6 => Error::NoSpace,
        7 => Error::Closed,
        8 => Error::Gone,
        9 => Error::Cancelled,
        _ => Error::Invalid,
    }
}

/// `void *ctx`.
pub type Ctx = *mut c_void;

/// `rv_calls`: the seven calls, valid during one slot call.
#[repr(C)]
pub struct RvCalls {
    pub ctx: Ctx,
    pub register_:
        Option<unsafe extern "C" fn(Ctx, u32, u64, *const c_char) -> RvReply>,
    pub name: Option<unsafe extern "C" fn(Ctx, u64, *const c_char) -> RvReply>,
    pub report: Option<
        unsafe extern "C" fn(Ctx, u64, u32, *const RvBytes, usize) -> RvReply,
    >,
    pub unregister: Option<unsafe extern "C" fn(Ctx, u64) -> RvReply>,
    pub changed: Option<unsafe extern "C" fn(Ctx, u64, u32) -> RvReply>,
    pub done: Option<unsafe extern "C" fn(Ctx, u64, u32, RvBytes) -> RvReply>,
    pub open: Option<unsafe extern "C" fn(Ctx, u64, u32) -> RvReply>,
    pub submit:
        Option<unsafe extern "C" fn(Ctx, RvBytes, *mut RvBytes) -> RvReply>,
}

/// `rv_driver`: the seven slots; a `None` slot is `Unsupported`.
#[repr(C)]
pub struct RvDriver {
    pub ctx: Ctx,
    pub lookup: Option<
        unsafe extern "C" fn(
            Ctx,
            *const RvCalls,
            u64,
            *const c_char,
        ) -> RvReply,
    >,
    pub open:
        Option<unsafe extern "C" fn(Ctx, *const RvCalls, u64, u32) -> RvReply>,
    pub read: Option<
        unsafe extern "C" fn(
            Ctx,
            *const RvCalls,
            u64,
            u64,
            *mut u8,
            usize,
        ) -> RvReply,
    >,
    pub write: Option<
        unsafe extern "C" fn(Ctx, *const RvCalls, u64, u64, RvBytes) -> RvReply,
    >,
    pub call: Option<
        unsafe extern "C" fn(
            Ctx,
            *const RvCalls,
            u64,
            u64,
            u16,
            RvBytes,
            *mut RvBytes,
        ) -> RvReply,
    >,
    pub watch:
        Option<unsafe extern "C" fn(Ctx, *const RvCalls, u64, u32) -> RvReply>,
    pub close:
        Option<unsafe extern "C" fn(Ctx, *const RvCalls, u64) -> RvReply>,
}

/// `rv_module`: one Module, checked by `magic` and `abi` before use.
#[repr(C)]
pub struct RvModule {
    pub magic: u32,
    pub abi: u32,
    pub name: *const c_char,
    pub ctx: Ctx,
    pub start:
        Option<unsafe extern "C" fn(Ctx, *const RvCalls, u64) -> RvReply>,
    pub stop: Option<unsafe extern "C" fn(Ctx)>,
    pub drivers: *const RvDriver,
    pub driver_count: usize,
}
