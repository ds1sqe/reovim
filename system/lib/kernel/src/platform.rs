//! The freestanding system-kernel platform provider.
//!
//! The genuine bare-metal [`PlatformVtable`] the bootcore composition root
//! installs at `Init::boot` — the real product-path successor to the
//! `reovim-platform-stub-none` selftest scaffold. This is the freestanding
//! analog of `reovim-platform-linux-native`: the bare-metal *canonicalizer*
//! (invariant #3). NATIVE→POSIX translation lives HERE — the `-ENOSYS` stubs
//! and the errno→i64 [`neg_errno`] encoding — never in the `arch-sys-none-*`
//! raw-mechanism crates below.
//!
//! It carries the live floor slots that are genuine on bare metal — the
//! monotonic clock (the generic timer / TSC via `clock_gettime`), the page
//! allocator (the static boot arena), `park`/`unpark`/`unpark_all` (futex over
//! the event stream), and `file_write` (fd 1/2 → the floor's `write`, which
//! fans the UART plus the standing console installed in 04b) — and
//! `-ENOSYS`/sentinel stubs for the socket/`clone`/filesystem slots that have
//! no freestanding floor.
//!
//! ## The §11 impl edge (invariant #2)
//!
//! The provider reaches the world ONLY downward, through the two freestanding
//! raw-mechanism crates it implements over (`reovim-arch-sys-none-{aarch64,
//! x86-64}`) and the canonical POSIX newtypes at the slot boundary
//! (`reovim-uapi-posix`). It installs via [`install`] (downward into
//! kabi/platform). There is no reverse `arch-sys-none → system-kernel` edge:
//! `file_write` reaches the floor `write`, not the other way.
//!
//! ## Freestanding-only
//!
//! The whole module is gated `cfg(target_os = "none")`: the live slots name the
//! `arch-sys-none-*` raw mechanism, which only exists on the bare-metal targets.
//! A hosted Linux build of this crate (the host `cargo test` of the migrated
//! unit tests) compiles the module to nothing; there the composition root
//! installs the real linux-native provider instead.

use core::alloc::Layout;

use {
    reovim_kabi_platform::{InstallError, PlatformVtable, install},
    reovim_uapi_posix::{Fd, Mode, OpenFlags},
};

// The §11 system-kernel → arch-sys-none impl edge: the live slots are realized
// over the matching freestanding raw-mechanism crate. The two arches expose an
// identical accessor surface (`clock_gettime` + `CLOCK_MONOTONIC` + `Timespec`,
// `arena_alloc_pages`, `futex` + the `FUTEX_*` constants, `write`, `Errno`), so
// one provider body serves both — only the backend `use` differs by target.
#[cfg(target_arch = "aarch64")]
use reovim_arch_sys_none_aarch64 as floor;
#[cfg(target_arch = "x86_64")]
use reovim_arch_sys_none_x86_64 as floor;

use floor::{
    CLOCK_MONOTONIC, Errno, FUTEX_PRIVATE_FLAG, FUTEX_WAIT, FUTEX_WAKE, Timespec,
    arena_alloc_pages, clock_gettime, futex, write as floor_write,
};

/// `FUTEX_WAKE` count meaning "wake everyone".
///
/// The kernel reads the wake count as a signed `int`, so the broadcast value is
/// `i32::MAX` (the glibc convention). `u32::MAX` would arrive as `-1` and a wake
/// loop (`++woken >= nr_wake`) would stop after a single waiter — a lost
/// broadcast that strands every other sleeper. This matches the wake-all count
/// the `lib/ds` `Condvar`/`RwLock` paths request. The freestanding `futex`
/// broadcasts regardless (`sev` wakes every parked core), but the slot still
/// passes the honest count so the ABI is identical to the hosted provider's.
const WAKE_ALL: u32 = 0x7FFF_FFFF; // i32::MAX, expressed unsigned

/// The hand-out granularity of the static boot arena, in bytes.
///
/// Mirrors the arena's own `PAGE_SIZE`. The arena returns page-aligned
/// addresses, so any requested alignment up to a page is satisfied for free; a
/// larger alignment cannot be honored and the [`alloc`] slot refuses it.
const PAGE_SIZE: usize = 4096;

/// `ENOSYS` errno code — "function not implemented" — the freestanding stubs'
/// negative return. `ENOSYS` = 38 (Linux errno space, the shared floor
/// vocabulary).
const ENOSYS_CODE: i64 = 38;

/// Encodes an arch [`Errno`] as the negative-errno `i64` the fd-op slots return:
/// `-code` (positive [`Errno`] → negative `-errno` the FFI ABI carries;
/// `kabi::map_fd_ret` recovers it consumer-side).
const fn neg_errno(e: Errno) -> i64 {
    -(e.code() as i64)
}

/// The clock adapter: reads the freestanding monotonic clock and returns whole
/// nanoseconds, matching the contract's `ClockFn` ABI.
///
/// `CLOCK_MONOTONIC` on the bare-metal floor is the generic timer (aarch64) /
/// TSC (x86) counter scaled to seconds + sub-second nanos, so the whole-nanos
/// value never steps backward. A clock read that fails (an unprogrammed timer)
/// reports `0` — a degenerate but monotonic origin — rather than a wild value.
///
/// # Safety
///
/// `unsafe extern "C"` to share the vtable's one ABI shape; the underlying
/// `clock_gettime` read has no precondition, so this is sound to call anytime.
unsafe extern "C" fn clock() -> i64 {
    let mut ts = Timespec::default();
    if clock_gettime(CLOCK_MONOTONIC, &mut ts).is_err() {
        return 0;
    }
    // The freestanding clock is boot-relative, so `tv_sec` is small and the
    // `* 1e9 + tv_nsec` product stays well within `i64` for any realistic
    // uptime — the same whole-nanos shape `arch::time::Instant::as_nanos`
    // computes on the hosted backend.
    ts.tv_sec
        .saturating_mul(1_000_000_000)
        .saturating_add(ts.tv_nsec)
}

/// The allocation adapter: serves `(size, align)` from the static boot arena,
/// returning the raw pointer (null on failure) the contract's `AllocFn` ABI
/// expects.
///
/// The freestanding floor's only heap source is the page arena
/// ([`arena_alloc_pages`]), which hands out page-aligned, page-rounded blocks
/// that are never reclaimed (the matching [`dealloc`] is a no-op, exactly as the
/// floor's `munmap` is). Every arena block is page-aligned, so any alignment up
/// to a page is satisfied by construction; a larger alignment, a zero size, or
/// an exhausted arena all report failure as null — the same channel a refused
/// mmap uses, which the consumer maps back to `AllocError`.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `align` must be a power of two and
/// `size` non-zero (the consumer's `Layout` guarantees both).
unsafe extern "C" fn alloc(size: usize, align: usize) -> *mut u8 {
    let null = core::ptr::null_mut();
    // A `Layout` the front cannot represent, an alignment coarser than a page
    // (the arena's fixed grain), or a zero size all refuse as null.
    if Layout::from_size_align(size, align).is_err() || align > PAGE_SIZE || size == 0 {
        return null;
    }
    // The arena returns a page-aligned address (`>= align`, since `align <=
    // PAGE_SIZE`) covering at least `size` bytes; an exhausted arena is `ENOMEM`,
    // mapped to null.
    arena_alloc_pages(size).map_or(null, |addr| addr as *mut u8)
}

/// The deallocation adapter: a no-op.
///
/// The boot arena never reclaims pages — floor allocations are long-lived by
/// design, and a freed block's pages stay leaked (bounded by the 1 MiB arena),
/// mirroring the floor's `munmap` no-op. The slot exists so the `#[repr(C)]`
/// table has one shape on every target and the consumer's `dealloc` call
/// resolves; it simply does nothing.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. The arguments are never
/// dereferenced.
const unsafe extern "C" fn dealloc(_ptr: *mut u8, _size: usize, _align: usize) {}

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
    // FUTEX_PRIVATE_FLAG: this sync is process-local (the `lib/ds` mutex /
    // condvar / rwlock all live in one address space), so the private-futex
    // path is correct; park/unpark/unpark_all all carry the flag so they share
    // one key. The freestanding `futex` masks the flag off and waits via `wfe`.
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
    // Wake one waiter (the matched `park`'s counterpart), private-futex to match
    // `park`'s key. The result is ignored: zero woken (no waiter) is a valid
    // no-op.
    let _ = futex(addr, FUTEX_WAKE | FUTEX_PRIVATE_FLAG, 1, 0, 0, 0);
}

/// The wake-all adapter: futex-wakes every thread parked on `word`.
///
/// Used by the `lib/ds` `Condvar::notify_all` and `RwLock` release paths, which
/// must wake the whole waiter cohort, not one.
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
// Freestanding targets have no socket/`clone`/filesystem floor. The slots still
// exist (the vtable is one `#[repr(C)]` shape on every target); they report
// `-ENOSYS` so a consumer that reaches them on a bare-metal build gets a typed
// error rather than a link failure. The canonicalization (NATIVE→POSIX) of
// "this floor has no such service" lives here, not below (invariant #3).

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
/// There is no filesystem floor, but the floor's `write` routes the standard fds
/// (1/2) to the PL011 UART (aarch64) / COM1 (x86) AND fans them to the standing
/// framebuffer console installed in 04b, so the kernel's stdout/stderr —
/// including the boot-stage `stderr_echo` and the boot splash — reaches both the
/// serial line and the HDMI surface live. `fd_write` (socket send) stays
/// `-ENOSYS`.
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
    match floor_write(fd.as_i32(), slice) {
        Ok(n) => n as i64,
        Err(e) => neg_errno(e),
    }
}

/// The freestanding system kernel's platform vtable: a `static` of const /
/// freestanding function pointers (zero heap to build), the table the bootcore
/// composition root installs on `*-unknown-none`.
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
    // file-open stub, plus the live `file_write` UART + standing-console sink.
    realtime: realtime_adapter,
    file_open,
    thread_id,
    file_write,
};

/// Installs the system kernel's freestanding platform vtable as the process-wide
/// handle (write-once).
///
/// The bare-metal composition root (bootcore / the boot image) calls this once
/// as the first action of its boot, before any `kabi::handle` read — the kernel
/// allocates and writes through the handle, so the no-read-before-install
/// invariant requires the install to precede the boot. The first install wins;
/// a second returns [`InstallError::AlreadyInstalled`] (AB12), surfacing a
/// double-boot bug rather than silently overwriting the handle.
///
/// This is the real product-path provider — it supersedes
/// `reovim-platform-stub-none` for bootcore (Q3). The stub-none scaffold is
/// retained for the bare exit-code selftest fixtures, which want the minimal
/// `-ENOSYS` table, not the full console-carrying kernel.
///
/// # Errors
///
/// Returns [`InstallError::AlreadyInstalled`] if a handle is already installed.
///
/// # Example
///
/// ```ignore
/// // First action of a bare-metal composition root, before any handle read:
/// reovim_system_kernel::platform::install_platform().expect("vtable already installed");
/// ```
pub fn install_platform() -> Result<(), InstallError> {
    install(&PLATFORM_VTABLE)
}
