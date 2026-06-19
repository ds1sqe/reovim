//! `reovim-platform-stub-none` — the bare-metal selftest handle scaffold.
//!
//! TEST-ONLY. This crate stands up the minimal freestanding [`PlatformVtable`]
//! the `arch/tests/fixtures/*` bare-metal composition roots (`selftest`,
//! `bootcore`) install so their handle reads (alloc + park/unpark through the
//! `lib/ds` types, the kernel boot, the `file_write` UART echo) resolve on
//! `*-unknown-none`, where the Linux POSIX provider
//! (`reovim-platform-linux-native`) cannot link.
//!
//! It carries the live floor slots that are genuine on bare metal — clock,
//! `alloc`/`dealloc`, `park`/`unpark`/`unpark_all` (futex), and `file_write` (fd 1/2 ->
//! the floor's UART) — and `-ENOSYS`/sentinel stubs for the socket/`clone`/
//! filesystem slots that have no freestanding floor. It is the test-side half
//! of the role the dropped `arch/src/platform.rs` `cfg(not(target_os="linux"))`
//! adapters used to fill.
//!
//! ## Role: the exit-code selftest scaffold
//!
//! This is a *scaffold*, not the system kernel's real provider. The genuine
//! freestanding `reovim-system-kernel` POSIX provider now exists and the real
//! product path (bootcore) installs it. This crate is RETAINED as the minimal
//! handle for the bare exit-code selftest fixtures, which want the `-ENOSYS`
//! scaffold rather than the full console-carrying kernel provider — two distinct
//! consumers (rule of three). It keeps that bare-metal selftest coverage green
//! with zero regression.
//!
//! ## Empty on Linux
//!
//! The whole crate body is gated `cfg(not(target_os = "linux"))`: on a hosted
//! Linux build it compiles to nothing (the fixtures install the real
//! linux-native provider there). The two installers are therefore mutually
//! exclusive by target — exactly one defines a `PLATFORM_VTABLE` per build, so
//! neither double-defines the handle.
#![no_std]
// SAFETY (lint): the vtable slots are `unsafe extern "C"` adapters — the one
// ABI shape the `#[repr(C)]` table requires — and the live ones dereference the
// raw pointers the contract passes. The workspace lint stays `warn`, so every
// crate above still flags unsafe; the allow is scoped to this crate. Every
// `unsafe` block carries a `// SAFETY:` comment.
#![allow(unsafe_code)]
// The whole crate is freestanding-only: on Linux the fixtures install the real
// linux-native provider, so this body must be empty there (no unused-import or
// dead-code diagnostics, no second `PLATFORM_VTABLE` definition).
#![cfg(not(target_os = "linux"))]

use core::alloc::Layout;

use {
    reovim_kabi_platform::{InstallError, PlatformVtable, install},
    reovim_uapi_posix::{Fd, Mode, OpenFlags},
};

use reovim_arch::{
    alloc::{alloc as arch_alloc, dealloc as arch_dealloc},
    sys::{Errno, FUTEX_PRIVATE_FLAG, FUTEX_WAIT, FUTEX_WAKE, futex, write as sys_write},
    time::Instant,
};

/// `FUTEX_WAKE` count meaning "wake everyone".
///
/// The kernel reads the wake count as a signed `int`, so the broadcast value
/// is `i32::MAX` (the glibc convention). `u32::MAX` would arrive as `-1` and
/// the kernel's wake loop (`++woken >= nr_wake`) would stop after a single
/// waiter — a lost broadcast that strands every other sleeper. This matches
/// the wake-all count the `lib/ds` `Condvar`/`RwLock` paths request.
const WAKE_ALL: u32 = 0x7FFF_FFFF; // i32::MAX, expressed unsigned

/// `ENOSYS` errno code — "function not implemented" — the freestanding stubs'
/// negative return. `ENOSYS` = 38 (Linux errno space, the shared floor
/// vocabulary).
const ENOSYS_CODE: i64 = 38;

/// Encodes an arch [`Errno`] as the negative-errno `i64` the fd-op slots
/// return: `-code` (positive `arch::sys::Errno` → negative `-errno` the FFI ABI
/// carries; `kabi::map_fd_ret` recovers it consumer-side).
const fn neg_errno(e: Errno) -> i64 {
    -(e.code() as i64)
}

/// The clock adapter: reads arch's monotonic clock and returns whole
/// nanoseconds, matching the contract's `ClockFn` ABI.
///
/// # Safety
///
/// `unsafe extern "C"` to share the vtable's one ABI shape; the underlying
/// `Instant::now` read has no precondition, so this is sound to call anytime.
unsafe extern "C" fn clock() -> i64 {
    Instant::now().as_nanos()
}

/// The allocation adapter: reconstructs a `Layout` from `(size, align)` and
/// calls arch's allocator, returning the raw pointer (null on failure) the
/// contract's `AllocFn` ABI expects.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `align` must be a power of two and
/// `size` non-zero (the consumer's `Layout` guarantees both); a malformed
/// `(size, align)` that `Layout::from_size_align` rejects maps to a null
/// return, the same "allocation refused" signal as a kernel OOM.
unsafe extern "C" fn alloc(size: usize, align: usize) -> *mut u8 {
    // A layout the front cannot represent, and a refused allocation, both report
    // failure as null — the same channel a refused mmap uses; the consumer maps
    // null back to `AllocError`.
    let null = core::ptr::null_mut();
    Layout::from_size_align(size, align)
        .map_or(null, |layout| arch_alloc(layout).map_or(null, core::ptr::NonNull::as_ptr))
}

/// The deallocation adapter: reconstructs the `Layout` and frees through arch.
///
/// # Safety
///
/// `ptr` must come from a prior [`alloc`] call with the identical
/// `(size, align)` and not have been freed; `align` is a power of two and
/// `size` non-zero. A null `ptr` (or a `(size, align)` `Layout` rejects) is a
/// no-op — the consumer never frees a null, but guarding keeps the adapter
/// total against a malformed pair.
unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize, align: usize) {
    let Some(nn) = core::ptr::NonNull::new(ptr) else {
        return;
    };
    let Ok(layout) = Layout::from_size_align(size, align) else {
        return;
    };
    // SAFETY: by the contract `nn`/`layout` are the exact pair a prior `alloc`
    // returned and the block is still live; arch's `dealloc` upholds the rest.
    unsafe { arch_dealloc(nn, layout) }
}

/// The park adapter: futex-waits on `*word` while it equals `expected`.
///
/// # Safety
///
/// `word` must point to a live, aligned `u32` valid across the (possibly
/// blocking) call. Spurious/early returns are permitted — the caller loops on
/// its own condition. The futex result is ignored: `EAGAIN` (the word already
/// changed) and a genuine wake both mean "re-check", which the caller does.
unsafe extern "C" fn park(word: *const u32, expected: u32) {
    let addr = word as usize;
    // FUTEX_PRIVATE_FLAG: this sync is process-local (the `lib/ds` mutex/
    // condvar/rwlock all live in one process), so the private-futex fast path
    // is both correct and faster. park/unpark/unpark_all all carry the flag, so
    // they share one hash bucket — a private waiter is only woken by a private
    // wake, matching the call sites the floor's sync primitives use.
    let _ = futex(addr, FUTEX_WAIT | FUTEX_PRIVATE_FLAG, expected, 0, 0, 0);
}

/// The unpark adapter: futex-wakes one thread parked on `word`.
///
/// # Safety
///
/// `word` must point to a live, aligned `u32`. Waking a `word` no thread is
/// parked on is a no-op.
unsafe extern "C" fn unpark(word: *const u32) {
    let addr = word as usize;
    // Wake exactly one waiter (the matched `park`'s counterpart), private-futex
    // to match `park`'s key. The result is ignored: zero woken (no waiter) is a
    // valid no-op.
    let _ = futex(addr, FUTEX_WAKE | FUTEX_PRIVATE_FLAG, 1, 0, 0, 0);
}

/// The wake-all adapter: futex-wakes every thread parked on `word`.
///
/// Used by the `lib/ds` `Condvar::notify_all` and `RwLock` release paths,
/// which must wake the whole waiter cohort, not one.
///
/// # Safety
///
/// `word` must point to a live, aligned `u32`. Waking a `word` no thread is
/// parked on is a no-op.
unsafe extern "C" fn unpark_all(word: *const u32) {
    let addr = word as usize;
    // Wake every waiter (`WAKE_ALL` = i32::MAX), private-futex to match `park`.
    // The result is ignored: zero woken (no waiters) is a valid no-op.
    let _ = futex(addr, FUTEX_WAKE | FUTEX_PRIVATE_FLAG, WAKE_ALL, 0, 0, 0);
}

// ── freestanding stub adapters ────────────────────────────────────────────────
//
// Freestanding targets have no socket/`clone` floor. The slots still exist (the
// vtable is one `#[repr(C)]` shape on every target); they report `-ENOSYS` so a
// consumer that reaches them on a bare-metal build gets a typed error rather
// than a link failure.

const unsafe extern "C" fn unix_connect(_path: *const u8, _path_len: usize) -> i64 {
    -ENOSYS_CODE
}
const unsafe extern "C" fn unix_listen(_path: *const u8, _path_len: usize) -> i64 {
    -ENOSYS_CODE
}
const unsafe extern "C" fn unix_accept(_listener_fd: Fd) -> i64 {
    -ENOSYS_CODE
}
const unsafe extern "C" fn fd_read(_fd: Fd, _buf: *mut u8, _len: usize) -> i64 {
    -ENOSYS_CODE
}
const unsafe extern "C" fn fd_write(_fd: Fd, _buf: *const u8, _len: usize) -> i64 {
    -ENOSYS_CODE
}
const unsafe extern "C" fn fd_close(_fd: Fd) -> i64 {
    -ENOSYS_CODE
}
unsafe extern "C" fn thread_spawn(_entry: unsafe extern "C" fn(*mut u8), _arg: *mut u8) -> i64 {
    -ENOSYS_CODE
}

/// Freestanding wall-clock stub: returns `0`, the Unix-epoch sentinel. A
/// bare-metal target has no real-time clock, so the wall anchor reads as the
/// epoch (the consumer only uses it as a human-readable origin, never for
/// ordering).
const unsafe extern "C" fn realtime_adapter() -> i64 {
    0
}

/// Freestanding file-open stub: `-ENOSYS` (no filesystem floor on bare metal),
/// the same fallible shape as the other fd stubs.
const unsafe extern "C" fn file_open(
    _path: *const u8,
    _path_len: usize,
    _flags: OpenFlags,
    _mode: Mode,
) -> i64 {
    -ENOSYS_CODE
}

/// Freestanding thread-id stub: returns `0`, the single-bare-metal-thread
/// sentinel (there is one thread of control, identified as id 0).
const unsafe extern "C" fn thread_id() -> i64 {
    0
}

/// Freestanding file-write adapter: writes up to `len` bytes from `buf` to `fd`
/// through the floor's byte sink, returning the byte count or a negative errno.
///
/// There is no filesystem floor, but `sys::write` routes the standard fds (1/2)
/// to the PL011 UART (aarch64) / COM1 (x86), so the kernel's stdout/stderr —
/// including the boot-stage `stderr_echo` — reaches the console live. `fd_write`
/// (socket send) stays `-ENOSYS`.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `buf` must point to `len` readable
/// bytes.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
unsafe extern "C" fn file_write(fd: Fd, buf: *const u8, len: usize) -> i64 {
    // SAFETY: caller guarantees `buf` is valid for `len` readable bytes.
    let slice = unsafe { core::slice::from_raw_parts(buf, len) };
    // A byte count never exceeds `isize::MAX`, so the cast cannot wrap.
    match sys_write(fd.as_i32(), slice) {
        Ok(n) => n as i64,
        Err(e) => neg_errno(e),
    }
}

/// The bare-metal scaffold's platform vtable: a `static` of const function
/// pointers (zero heap to build), the table the selftest/bootcore composition
/// roots install on `*-unknown-none`.
///
/// The slot order matches [`PlatformVtable`]'s append-only (AB3) declaration.
static PLATFORM_VTABLE: PlatformVtable = PlatformVtable {
    clock,
    alloc,
    dealloc,
    park,
    unpark,
    unpark_all,
    // Net/thread slots: `-ENOSYS` stubs (no socket/`clone` floor on bare metal).
    unix_connect,
    unix_listen,
    unix_accept,
    fd_read,
    fd_write,
    fd_close,
    thread_spawn,
    // Time/file/thread-identity slots: epoch/id-0 sentinels and the `-ENOSYS`
    // file-open stub, plus the live `file_write` UART sink.
    realtime: realtime_adapter,
    file_open,
    thread_id,
    file_write,
};

/// Installs the scaffold's platform vtable as the process-wide handle
/// (write-once).
///
/// The bare-metal fixture composition root calls this once as the first
/// statement of its `entry!` closure, after the allocator is up (it inits
/// heap-free, so it is already usable) and the `static` vtable is built (it is
/// `const`, so it is built at compile time). The first install wins; a second
/// returns [`InstallError::AlreadyInstalled`] (AB12). Installing as the
/// closure's first statement preserves the no-read-before-install invariant:
/// every `kabi::handle` read runs after it.
///
/// # Errors
///
/// Returns [`InstallError::AlreadyInstalled`] if a handle is already installed
/// — a double-boot bug, surfaced rather than silently overwritten.
///
/// # Example
///
/// ```ignore
/// // First statement of a bare-metal fixture `entry!` closure, before any read:
/// reovim_platform_stub_none::install_platform().expect("vtable already installed");
/// ```
pub fn install_platform() -> Result<(), InstallError> {
    install(&PLATFORM_VTABLE)
}
