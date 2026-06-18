//! The wrapper-level floor, realized over hardware.
//!
//! Same item names, signatures, and constant values as the Linux-family
//! `wrap.rs` — callers above `sys/` compile unchanged — but the realization
//! binds to the machine instead of a kernel: byte I/O is the COM1 UART, the
//! clock is the time-stamp counter, pages come from the static arena, and
//! exit is the QEMU debug-exit channel. Services with no hardware
//! realization (files, threads, page protection) fail through the floor's
//! existing error vocabulary instead of growing a capability API.
//!
//! The constants keep their Linux ABI values even where this target never
//! interprets them (`O_*`, `CLONE_*`): target-neutral callers build flag
//! words from them before reaching the floor, so the arithmetic must behave
//! identically everywhere.

use core::sync::atomic::{AtomicU32, Ordering};

use super::{
    super::errno::{EAGAIN, EBADF, EINVAL, ENOENT, Errno},
    arena, semihost, timer,
};

// ---- mmap prot/flags --------------------------------------------------------

/// `PROT_NONE` — no access. Accepted as vocabulary only: the arena has no
/// permission machinery (see [`mprotect`]).
pub const PROT_NONE: usize = 0x0;
/// `PROT_READ` — readable. Every arena byte already is.
pub const PROT_READ: usize = 0x1;
/// `PROT_WRITE` — writable. Every arena byte already is.
pub const PROT_WRITE: usize = 0x2;
/// `MAP_PRIVATE` — private mapping; the arena's only kind.
pub const MAP_PRIVATE: usize = 0x2;
/// `MAP_ANONYMOUS` — not file-backed; the arena's only kind.
pub const MAP_ANONYMOUS: usize = 0x20;
/// The Linux `mmap` in-band failure value. Floor vocabulary: [`mmap`] here
/// reports failure through `Result`, never through this value.
pub const MAP_FAILED: usize = usize::MAX;

// ---- openat -----------------------------------------------------------------

/// `AT_FDCWD` — "relative to the cwd" sentinel. Vocabulary only: there is
/// no filesystem, so [`openat`] never resolves anything.
pub const AT_FDCWD: i32 = -100;
/// `O_RDONLY` — open read-only.
pub const O_RDONLY: usize = 0;
/// `O_WRONLY` — open write-only.
pub const O_WRONLY: usize = 0o1;
/// `O_CREAT` — create if absent.
pub const O_CREAT: usize = 0o100;
/// `O_TRUNC` — truncate on open.
pub const O_TRUNC: usize = 0o1000;
/// `O_CLOEXEC` — close on exec.
pub const O_CLOEXEC: usize = 0o2_000_000;

// ---- clocks -----------------------------------------------------------------

/// `CLOCK_MONOTONIC` — the time-stamp counter.
pub const CLOCK_MONOTONIC: usize = 1;
/// `CLOCK_REALTIME` — same source as [`CLOCK_MONOTONIC`]: the machine has
/// no RTC, so a boot-relative epoch is the honest wall clock (documented at
/// [`clock_gettime`]).
pub const CLOCK_REALTIME: usize = 0;

/// Seconds + nanoseconds, the shape [`clock_gettime`] fills.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timespec {
    /// Whole seconds.
    pub tv_sec: i64,
    /// Nanoseconds in `0..1_000_000_000`.
    pub tv_nsec: i64,
}

// ---- futex ops ----------------------------------------------------------------

/// `FUTEX_WAIT` — block while `*uaddr == val`.
pub const FUTEX_WAIT: usize = 0;
/// `FUTEX_WAKE` — wake waiters.
pub const FUTEX_WAKE: usize = 1;
/// `FUTEX_PRIVATE_FLAG` — process-private futex. Always true here (one
/// address space), so [`futex`] masks it off and ignores it.
pub const FUTEX_PRIVATE_FLAG: usize = 128;

// ---- clone flags ---------------------------------------------------------------

/// `CLONE_VM` — share the address space. Vocabulary only: `clone_into`
/// has no realization on this target.
pub const CLONE_VM: usize = 0x0000_0100;
/// `CLONE_FS` — share filesystem context. Vocabulary only.
pub const CLONE_FS: usize = 0x0000_0200;
/// `CLONE_FILES` — share the fd table. Vocabulary only.
pub const CLONE_FILES: usize = 0x0000_0400;
/// `CLONE_SIGHAND` — share signal handlers. Vocabulary only.
pub const CLONE_SIGHAND: usize = 0x0000_0800;
/// `CLONE_THREAD` — same thread group. Vocabulary only.
pub const CLONE_THREAD: usize = 0x0001_0000;
/// `CLONE_SYSVSEM` — share semaphore undo. Vocabulary only.
pub const CLONE_SYSVSEM: usize = 0x0004_0000;
/// `CLONE_PARENT_SETTID` — write the child tid for the parent. Vocabulary
/// only.
pub const CLONE_PARENT_SETTID: usize = 0x0010_0000;
/// `CLONE_CHILD_CLEARTID` — clear + futex-wake the tid word on child exit.
/// Vocabulary only.
pub const CLONE_CHILD_CLEARTID: usize = 0x0020_0000;

// ---- wrappers -------------------------------------------------------------------

/// Writes `buf` to the UART for the standard output fds.
///
/// fd 1 and fd 2 are both COM1 — bare metal has one byte sink, so stdout
/// and stderr coincide. The write is total once started (polled port I/O
/// cannot short-write), so the full length is always reported.
///
/// # Errors
///
/// `EBADF` for any other fd: nothing else can be open ([`openat`] never
/// succeeds).
pub fn write(fd: i32, buf: &[u8]) -> Result<usize, Errno> {
    if fd == 1 || fd == 2 {
        super::uart::write_bytes(buf);
        Ok(buf.len())
    } else {
        Err(EBADF)
    }
}

/// Reports end-of-input for fd 0.
///
/// The selftest payload consumes no input, so the UART's receive side is
/// left unwired and stdin is permanently at EOF — the `Ok(0)` shape, which
/// keeps read-loop callers terminating instead of erroring.
///
/// # Errors
///
/// `EBADF` for any other fd: nothing else can be open ([`openat`] never
/// succeeds).
pub const fn read(fd: i32, _buf: &mut [u8]) -> Result<usize, Errno> {
    if fd == 0 { Ok(0) } else { Err(EBADF) }
}

/// Refuses: no fd here is closeable.
///
/// fds 0-2 are permanent hardware channels and [`openat`] never creates
/// others, so every close request names a fd this target does not manage.
///
/// # Errors
///
/// Always `EBADF`.
pub const fn close(_fd: i32) -> Result<usize, Errno> {
    Err(EBADF)
}

/// Maps anonymous private pages from the static arena.
///
/// The floor's only mapping shape is the anonymous private read-write
/// request the allocator and thread module issue, so that is the only one
/// realized: a file-backed request (`fd >= 0`) or a file offset is refused.
/// The address hint and `prot`/`flags` words are accepted but not applied —
/// the arena places pages itself and has no permission machinery (every
/// byte is readable-writable; the one `PROT_NONE` consumer is the stack
/// guard, whose [`mprotect`] reports the missing machinery honestly).
///
/// # Errors
///
/// `EINVAL` for a zero length, a file-backed request, or a nonzero offset;
/// `ENOMEM` when the arena is exhausted (the same errno the Linux backends
/// surface on kernel OOM).
pub fn mmap(
    _addr: usize,
    len: usize,
    _prot: usize,
    _flags: usize,
    fd: i32,
    off: usize,
) -> Result<usize, Errno> {
    if len == 0 || fd >= 0 || off != 0 {
        return Err(EINVAL);
    }
    arena::alloc_pages(len)
}

/// Accepts and ignores the unmap: arena pages are never reclaimed.
///
/// The slab allocator above never returns its small-class pages either —
/// floor allocations are long-lived by design — and the only other caller
/// ignores the result. A leaked large mapping is bounded by the arena, and
/// exhaustion surfaces as `ENOMEM` at the next [`mmap`].
///
/// # Errors
///
/// None; the no-op always reports success.
pub const fn munmap(_addr: usize, _len: usize) -> Result<usize, Errno> {
    Ok(0)
}

/// Refuses: the arena has no page-permission machinery.
///
/// The only caller is the thread module's stack-guard arming, where a
/// refusal makes spawn fail — the correct outcome on a target whose
/// `clone_into` cannot spawn anyway, reached before any thread state is
/// built.
///
/// # Errors
///
/// Always `EINVAL`.
pub const fn mprotect(_addr: usize, _len: usize, _prot: usize) -> Result<usize, Errno> {
    Err(EINVAL)
}

/// Refuses: there is no filesystem to resolve a path against.
///
/// # Errors
///
/// Always `ENOENT`.
pub const fn openat(
    _dirfd: i32,
    _path: &[u8],
    _flags: usize,
    _mode: usize,
) -> Result<usize, Errno> {
    Err(ENOENT)
}

/// Reads the time-stamp counter into `tp`.
///
/// `CLOCK_MONOTONIC` is the counter divided by its frequency.
/// `CLOCK_REALTIME` returns the same boot-relative value: the machine has
/// no RTC, so a fabricated wall-clock epoch would be a lie — callers get a
/// clock that is honest about being boot-relative.
///
/// # Errors
///
/// `EINVAL` for any other clock id. The frequency source
/// ([`timer::frequency`]) never reports zero, so the division is always
/// well-defined.
pub fn clock_gettime(clockid: usize, tp: &mut Timespec) -> Result<usize, Errno> {
    if clockid != CLOCK_MONOTONIC && clockid != CLOCK_REALTIME {
        return Err(EINVAL);
    }
    // `timer::frequency()` never returns zero (it falls back to a nonzero
    // constant), so the divisions below are always well-defined.
    let freq = timer::frequency();
    let count = timer::counter();
    // The sub-second remainder is < freq, so the scaled quotient is provably
    // < 1_000_000_000; the u128 intermediate keeps the multiply exact for
    // any frequency, and the narrowing casts cannot truncate or wrap.
    let sub_ns = u128::from(count % freq) * 1_000_000_000 / u128::from(freq);
    #[allow(clippy::cast_possible_truncation)]
    let tv_nsec = sub_ns as i64;
    tp.tv_sec = (count / freq).cast_signed();
    tp.tv_nsec = tv_nsec;
    Ok(0)
}

/// Futex wait/wake, realized as a `pause`-spun poll.
///
/// Single address space, so `FUTEX_PRIVATE_FLAG` is masked off and ignored.
/// The timeout word is accepted and ignored: every floor caller passes 0
/// (wait forever). x86 has no inter-core event register (the aarch64
/// backend's `wfe`/`sev`), so the wait spins on the word with `pause` and
/// the wake is a no-op — these wait/wake paths are reachable only through
/// spawned threads, which this target's `clone_into` cannot create, so the
/// word can only change on the waiting core itself; the dumbest correct
/// form is deliberate.
///
/// # Errors
///
/// `EAGAIN` when the wait's expected value no longer matches (the caller
/// must re-check its state); `EINVAL` for an unsupported op.
pub fn futex(
    uaddr: usize,
    op: usize,
    val: u32,
    _timeout: usize,
    _uaddr2: usize,
    _val3: u32,
) -> Result<usize, Errno> {
    match op & !FUTEX_PRIVATE_FLAG {
        FUTEX_WAIT => {
            // SAFETY: the caller contract is the Linux futex wrapper's:
            // `uaddr` names a valid, aligned, live 32-bit atomic word.
            let word = unsafe { &*(uaddr as *const AtomicU32) };
            if word.load(Ordering::Acquire) != val {
                return Err(EAGAIN);
            }
            while word.load(Ordering::Acquire) == val {
                core::hint::spin_loop();
            }
            Ok(0)
        }
        FUTEX_WAKE => {
            // No inter-core event register on x86 and no second thread to
            // wake (see the doc comment); a waiter observes the word change
            // through its own spin.
            Ok(0)
        }
        _ => Err(EINVAL),
    }
}

/// Terminates the run with `code` via the QEMU debug-exit channel.
///
/// Process exit and thread exit coincide on this target — see [`exit`].
pub fn exit_group(code: i32) -> ! {
    semihost::exit(code)
}

/// Terminates the run with `code` via the QEMU debug-exit channel.
///
/// On Linux this ends only the calling thread; here the boot core is the
/// only thread that can exist (`clone_into` never spawns), so thread exit
/// and process exit are the same event.
pub fn exit(code: i32) -> ! {
    semihost::exit(code)
}

/// Returns the only thread id this target can have: the boot core's, 0.
///
/// Secondary cores are parked at entry and the thread floor cannot spawn,
/// so every caller runs on core 0. Note the Linux wrappers' "tid is always
/// positive" property does not carry over.
#[must_use]
pub const fn gettid() -> i32 {
    0
}
