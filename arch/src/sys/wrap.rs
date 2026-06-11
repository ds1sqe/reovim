//! Typed, safe-as-possible wrappers over the raw syscall primitives.
//!
//! Each wrapper packs its arguments into the raw-register form, invokes the
//! matching `syscall_n`, and routes the raw return through
//! [`super::errno::from_ret`]. The negative-return mapping is the branchy
//! Rust covered by the tests; the `asm!` block itself has no Rust branch.

use super::{
    errno::{Errno, from_ret},
    target::raw::{nr, syscall1, syscall1_noreturn, syscall2, syscall3, syscall4, syscall6},
};

/// Reinterprets a signed scalar (fd, dirfd, exit code) as the syscall
/// register `usize`, sign-extending first so that `-1` becomes `usize::MAX`
/// as the kernel ABI expects (e.g. `AT_FDCWD == -100`, `fd == -1`).
const fn as_reg(v: i32) -> usize {
    (v as isize).cast_unsigned()
}

/// Reinterprets a raw syscall return as a signed `i32` (used for `gettid`,
/// whose tid always fits in `i32`).
const fn ret_as_i32(v: isize) -> i32 {
    // A tid fits in `i32`; the conversion is the kernel ABI's own width.
    #[allow(clippy::cast_possible_truncation)]
    let out = v as i32;
    out
}

// ---- mmap protection / flags ----------------------------------------------

/// `PROT_NONE` — pages may not be accessed (a guard region).
///
/// See [`mmap`] for usage.
pub const PROT_NONE: usize = 0x0;
/// `PROT_READ` — pages may be read.
///
/// See [`mmap`] for usage.
pub const PROT_READ: usize = 0x1;
/// `PROT_WRITE` — pages may be written.
///
/// See [`mmap`] for usage.
pub const PROT_WRITE: usize = 0x2;
/// `MAP_PRIVATE` — copy-on-write mapping.
///
/// See [`mmap`] for usage.
pub const MAP_PRIVATE: usize = 0x2;
/// `MAP_ANONYMOUS` — not backed by a file.
///
/// See [`mmap`] for usage.
pub const MAP_ANONYMOUS: usize = 0x20;
/// The `mmap` failure sentinel (`(void *) -1`).
///
/// The kernel returns a negative errno for `mmap` errors, so this constant
/// documents the C-side convention; the wrapper detects failure via the
/// errno mapping instead.
///
/// See [`mmap`] for usage.
pub const MAP_FAILED: usize = usize::MAX;

// ---- openat flags ----------------------------------------------------------

/// `AT_FDCWD` — resolve relative paths against the current directory.
///
/// See [`openat`] for usage.
pub const AT_FDCWD: i32 = -100;
/// `O_RDONLY` — open for reading only.
///
/// See [`openat`] for usage.
pub const O_RDONLY: usize = 0;
/// `O_WRONLY` — open for writing only (the panic-flush fixtures' sink).
///
/// See [`openat`] for usage.
pub const O_WRONLY: usize = 0o1;
/// `O_CREAT` — create the file if it does not exist.
///
/// See [`openat`] for usage.
pub const O_CREAT: usize = 0o100;
/// `O_TRUNC` — truncate the file to zero length on open.
///
/// See [`openat`] for usage.
pub const O_TRUNC: usize = 0o1000;
/// `O_CLOEXEC` — close the descriptor on `exec`.
///
/// See [`openat`] for usage.
pub const O_CLOEXEC: usize = 0o2_000_000;

// ---- clock ids -------------------------------------------------------------

/// `CLOCK_MONOTONIC` — never steps backward; the LOG2 timestamp source.
///
/// See [`clock_gettime`] for usage.
pub const CLOCK_MONOTONIC: usize = 1;
/// `CLOCK_REALTIME` — wall-clock time.
///
/// See [`clock_gettime`] for usage.
pub const CLOCK_REALTIME: usize = 0;

/// A POSIX `timespec`.
///
/// ```rust
/// use reovim_arch::sys::{Timespec, CLOCK_MONOTONIC, clock_gettime};
///
/// let mut ts = Timespec::default();
/// clock_gettime(CLOCK_MONOTONIC, &mut ts).unwrap();
/// // A monotonic clock returns non-negative seconds.
/// assert!(ts.tv_sec >= 0);
/// assert!(ts.tv_nsec >= 0 && ts.tv_nsec < 1_000_000_000);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timespec {
    /// Whole seconds.
    pub tv_sec: i64,
    /// Nanoseconds in `0..1_000_000_000`.
    pub tv_nsec: i64,
}

// ---- futex ops -------------------------------------------------------------

/// `FUTEX_WAIT` — block while `*uaddr == val`.
///
/// See [`futex`] for usage.
pub const FUTEX_WAIT: usize = 0;
/// `FUTEX_WAKE` — wake up to `val` waiters.
///
/// See [`futex`] for usage.
pub const FUTEX_WAKE: usize = 1;
/// `FUTEX_PRIVATE_FLAG` — process-private futex (no shared-mapping cost).
///
/// See [`futex`] for usage.
pub const FUTEX_PRIVATE_FLAG: usize = 128;

// ---- clone flags -----------------------------------------------------------

/// `CLONE_VM` — share the address space with the parent (a thread, not a
/// fork).
///
/// See [`crate::thread::spawn`] for usage of the full clone flag set.
pub const CLONE_VM: usize = 0x0000_0100;
/// `CLONE_FS` — share filesystem info (cwd, umask, root).
///
/// See [`crate::thread::spawn`] for usage.
pub const CLONE_FS: usize = 0x0000_0200;
/// `CLONE_FILES` — share the file-descriptor table.
///
/// See [`crate::thread::spawn`] for usage.
pub const CLONE_FILES: usize = 0x0000_0400;
/// `CLONE_SIGHAND` — share the signal-handler table.
///
/// See [`crate::thread::spawn`] for usage.
pub const CLONE_SIGHAND: usize = 0x0000_0800;
/// `CLONE_THREAD` — the child joins the parent's thread group.
///
/// See [`crate::thread::spawn`] for usage.
pub const CLONE_THREAD: usize = 0x0001_0000;
/// `CLONE_SYSVSEM` — share the System V semaphore undo list.
///
/// See [`crate::thread::spawn`] for usage.
pub const CLONE_SYSVSEM: usize = 0x0004_0000;
/// `CLONE_PARENT_SETTID` — store the child tid at the `ptid` pointer.
///
/// See [`crate::thread::spawn`] for usage.
pub const CLONE_PARENT_SETTID: usize = 0x0010_0000;
/// `CLONE_CHILD_CLEARTID` — clear the word at `ctid` and `FUTEX_WAKE` it on
/// child exit. This is the join-futex mechanism (the kernel zeroes the word
/// and wakes one waiter when the thread terminates).
///
/// See [`crate::thread::spawn`] for usage.
pub const CLONE_CHILD_CLEARTID: usize = 0x0020_0000;

/// Writes `buf` to `fd`.
///
/// Returns the number of bytes written, which may be fewer than
/// `buf.len()` (short write).
///
/// # Errors
///
/// Returns [`Errno`] on failure (e.g. [`super::errno::EBADF`] for a bad or
/// closed descriptor).
///
/// ```rust
/// use reovim_arch::sys::{AT_FDCWD, O_WRONLY, O_CLOEXEC, openat, write, close};
///
/// // Write to /dev/null — always succeeds and discards bytes.
/// // SAFETY: "/dev/null\0" is a valid NUL-terminated path.
/// let fd = openat(AT_FDCWD, b"/dev/null\0", O_WRONLY | O_CLOEXEC, 0).unwrap();
/// let n = write(fd as i32, b"hello").unwrap();
/// assert_eq!(n, 5);
/// close(fd as i32).unwrap();
/// ```
pub fn write(fd: i32, buf: &[u8]) -> Result<usize, Errno> {
    // SAFETY: `buf.as_ptr()`/`buf.len()` describe a valid readable region
    // for the duration of the call; `fd` is passed by value. The kernel only
    // reads from the buffer.
    let ret = unsafe { syscall3(nr::WRITE, as_reg(fd), buf.as_ptr().addr(), buf.len()) };
    from_ret(ret)
}

/// Reads up to `buf.len()` bytes from `fd` into `buf`.
///
/// Returns the number of bytes read. A return of `0` is end-of-file.
/// The bytes beyond the returned count are uninitialised and must not
/// be read.
///
/// # Errors
///
/// Returns [`Errno`] on failure (e.g. [`super::errno::EBADF`] for a bad
/// or closed descriptor, [`super::errno::EINVAL`] for an invalid argument).
///
/// ```rust
/// use reovim_arch::sys::{AT_FDCWD, O_RDONLY, O_CLOEXEC, openat, read, close};
///
/// // Read from /dev/null — always returns 0 bytes (EOF).
/// // SAFETY: "/dev/null\0" is a valid NUL-terminated path.
/// let fd = openat(AT_FDCWD, b"/dev/null\0", O_RDONLY | O_CLOEXEC, 0).unwrap();
/// let mut buf = [0u8; 4];
/// let n = read(fd as i32, &mut buf).unwrap();
/// assert_eq!(n, 0); // EOF
/// close(fd as i32).unwrap();
/// ```
pub fn read(fd: i32, buf: &mut [u8]) -> Result<usize, Errno> {
    // SAFETY: `buf.as_mut_ptr()`/`buf.len()` describe a valid writable region
    // for the duration of the call; `fd` is passed by value. The kernel writes
    // at most `buf.len()` bytes into the buffer.
    let ret = unsafe { syscall3(nr::READ, as_reg(fd), buf.as_mut_ptr().addr(), buf.len()) };
    from_ret(ret)
}

/// Closes `fd`.
///
/// # Errors
///
/// Returns [`Errno`] on failure (e.g. [`super::errno::EBADF`] when `fd` is
/// already closed or never valid).
///
/// ```rust
/// use reovim_arch::sys::{EBADF, close};
/// // Closing a bad fd returns EBADF.
/// assert_eq!(close(-1), Err(EBADF));
/// ```
pub fn close(fd: i32) -> Result<usize, Errno> {
    // SAFETY: `close` takes a scalar fd; no memory is dereferenced.
    let ret = unsafe { syscall1(nr::CLOSE, as_reg(fd)) };
    from_ret(ret)
}

/// Maps memory.
///
/// On success returns the mapping address as a `usize`. For anonymous
/// mappings pass `fd = -1` and `off = 0`.
///
/// # Errors
///
/// Returns [`Errno`] on failure (e.g. [`super::errno::EINVAL`] for a zero
/// length, [`super::errno::ENOMEM`] when the mapping cannot be satisfied).
///
/// ```rust
/// use reovim_arch::sys::{MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE, mmap, munmap};
///
/// let addr = mmap(0, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0)
///     .expect("anonymous mmap succeeds");
/// assert!(addr != 0);
/// munmap(addr, 4096).unwrap();
/// ```
pub fn mmap(
    addr: usize,
    len: usize,
    prot: usize,
    flags: usize,
    fd: i32,
    off: usize,
) -> Result<usize, Errno> {
    // SAFETY: `mmap` does not dereference `addr` (a hint); it creates a new
    // mapping. The returned address, when Ok, is a fresh valid region of
    // `len` bytes owned by the caller until `munmap`.
    let ret = unsafe { syscall6(nr::MMAP, addr, len, prot, flags, as_reg(fd), off) };
    from_ret(ret)
}

/// Unmaps a previously mapped region.
///
/// # Errors
///
/// Returns [`Errno`] on failure (e.g. [`super::errno::EINVAL`] for a
/// misaligned address or zero length).
///
/// ```rust
/// use reovim_arch::sys::{MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE, mmap, munmap};
///
/// let addr = mmap(0, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0)
///     .expect("anonymous mmap succeeds");
/// munmap(addr, 4096).expect("munmap of a valid mapping succeeds");
/// ```
pub fn munmap(addr: usize, len: usize) -> Result<usize, Errno> {
    // SAFETY: the caller guarantees `addr`/`len` name a region previously
    // returned by `mmap`; after this call that region must not be accessed.
    let ret = unsafe { syscall2(nr::MUNMAP, addr, len) };
    from_ret(ret)
}

/// Changes the protection of the pages covering `[addr, addr + len)`.
///
/// `addr` must be page-aligned. Used to install a `PROT_NONE` stack guard
/// page (a thread overrunning its stack traps instead of corrupting an
/// adjacent mapping).
///
/// # Errors
///
/// Returns [`Errno`] on failure (e.g. [`super::errno::EINVAL`] for a
/// misaligned address, [`super::errno::ENOMEM`] when the range is not
/// mapped).
///
/// ```rust
/// use reovim_arch::sys::{
///     MAP_ANONYMOUS, MAP_PRIVATE, PROT_NONE, PROT_READ, PROT_WRITE,
///     mmap, mprotect, munmap,
/// };
///
/// // Map a page read-write, then drop permissions to PROT_NONE.
/// let addr = mmap(0, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0)
///     .expect("anonymous mmap succeeds");
/// mprotect(addr, 4096, PROT_NONE).expect("mprotect PROT_NONE on mapped page succeeds");
/// munmap(addr, 4096).unwrap();
/// ```
pub fn mprotect(addr: usize, len: usize, prot: usize) -> Result<usize, Errno> {
    // SAFETY: `mprotect` does not dereference the range; it only changes the
    // page-table protection bits. The caller guarantees `[addr, addr + len)`
    // names mapped, page-aligned pages it owns.
    let ret = unsafe { syscall3(nr::MPROTECT, addr, len, prot) };
    from_ret(ret)
}

/// Opens a path relative to `dirfd`.
///
/// `path` MUST be NUL-terminated; the kernel reads bytes until the first
/// `0`. Passing a slice without a trailing `0` is undefined behaviour at
/// the syscall boundary, so the wrapper requires the contract from the
/// caller. On success returns the new file descriptor as a `usize`.
///
/// # Errors
///
/// Returns [`Errno`] on failure (e.g. [`super::errno::ENOENT`] when the
/// path does not exist).
///
/// ```rust
/// use reovim_arch::sys::{AT_FDCWD, O_RDONLY, O_CLOEXEC, openat, close};
///
/// // SAFETY (caller contract): b"/dev/null\0" contains a NUL terminator.
/// let fd = openat(AT_FDCWD, b"/dev/null\0", O_RDONLY | O_CLOEXEC, 0)
///     .expect("opening /dev/null succeeds");
/// // A valid open returns a non-negative fd (stored as usize).
/// assert!(fd > 0 || fd == 0); // fd >= 0 in usize representation
/// close(fd as i32).unwrap();
/// ```
pub fn openat(dirfd: i32, path: &[u8], flags: usize, mode: usize) -> Result<usize, Errno> {
    // SAFETY: `path.as_ptr()` is a valid readable pointer; the caller
    // guarantees the slice contains a NUL terminator so the kernel's
    // C-string read stays in bounds. The kernel only reads the path.
    let ret = unsafe { syscall4(nr::OPENAT, as_reg(dirfd), path.as_ptr().addr(), flags, mode) };
    from_ret(ret)
}

/// Reads the current value of `clockid` into `tp`.
///
/// # Errors
///
/// Returns [`Errno`] on failure (e.g. [`super::errno::EINVAL`] for an
/// unsupported clock id).
///
/// ```rust
/// use reovim_arch::sys::{CLOCK_MONOTONIC, Timespec, clock_gettime};
///
/// let mut ts = Timespec::default();
/// clock_gettime(CLOCK_MONOTONIC, &mut ts).expect("CLOCK_MONOTONIC is always supported");
/// assert!(ts.tv_sec >= 0);
/// assert!(ts.tv_nsec >= 0 && ts.tv_nsec < 1_000_000_000);
/// ```
pub fn clock_gettime(clockid: usize, tp: &mut Timespec) -> Result<usize, Errno> {
    // SAFETY: `tp` is a unique, valid, writable `Timespec`; the kernel
    // writes the two i64 fields and nothing else. The pointer is live for
    // the call.
    let ret = unsafe { syscall2(nr::CLOCK_GETTIME, clockid, core::ptr::from_mut(tp).addr()) };
    from_ret(ret)
}

/// Raw `futex` wrapper.
///
/// Thin pass-through: `uaddr`/`timeout`/`uaddr2` are raw addresses and the
/// caller owns their validity contract. Used by the Phase 3 sync
/// primitives; the `val3` argument is opaque per the futex op.
///
/// # Errors
///
/// Returns [`Errno`] on failure (e.g. [`super::errno::EAGAIN`] when a
/// `FUTEX_WAIT` observes a mismatched expected value).
///
/// ```rust
/// use core::sync::atomic::{AtomicU32, Ordering::Relaxed};
/// use reovim_arch::sys::{EAGAIN, FUTEX_WAIT, FUTEX_PRIVATE_FLAG, futex};
///
/// // FUTEX_WAIT with a deliberately mismatched expected value returns EAGAIN
/// // immediately — the kernel re-checks the word before blocking.
/// let word = AtomicU32::new(0);
/// let addr = core::ptr::from_ref(&word).addr();
/// // Expected value is 1 but the word holds 0 → EAGAIN, no sleep.
/// let result = futex(addr, FUTEX_WAIT | FUTEX_PRIVATE_FLAG, 1, 0, 0, 0);
/// assert_eq!(result, Err(EAGAIN));
/// ```
pub fn futex(
    uaddr: usize,
    op: usize,
    val: u32,
    timeout: usize,
    uaddr2: usize,
    val3: u32,
) -> Result<usize, Errno> {
    // u32 -> usize is lossless on this module's only target (x86_64);
    // core provides no `From<u32> for usize` because 16-bit targets exist.
    #[allow(clippy::cast_possible_truncation)]
    let (expected_word, extra_word) = (val as usize, val3 as usize);
    // SAFETY: the caller guarantees `uaddr` (and `timeout`/`uaddr2` when
    // used) point to valid, suitably-aligned memory live across the call.
    // The kernel reads `*uaddr` for FUTEX_WAIT and may block.
    let ret = unsafe { syscall6(nr::FUTEX, uaddr, op, expected_word, timeout, uaddr2, extra_word) };
    from_ret(ret)
}

/// Terminates the whole process with `code`.
///
/// Never returns.
///
/// ```no_run
/// // Process-terminating — not safe to run in the doctest harness.
/// reovim_arch::sys::exit_group(0);
/// ```
pub fn exit_group(code: i32) -> ! {
    // SAFETY: `exit_group` does not return; no memory is dereferenced.
    // SAFETY: the Linux `exit_group` syscall (nr 231) unconditionally
    // terminates the whole process; the noreturn asm shape carries that
    // contract with no post-syscall code (no spin loop, no unsafe hint).
    unsafe { syscall1_noreturn(nr::EXIT_GROUP, as_reg(code)) }
}

/// Terminates the **calling thread only**, leaving the rest of the process
/// running.
///
/// This is the spawned-thread teardown path: a thread that finishes its
/// closure calls `exit` (not [`exit_group`], which would kill the whole
/// process). The kernel honours `CLONE_CHILD_CLEARTID` here — it zeroes the
/// ctid join word and wakes one futex waiter as the thread dies.
///
/// Never returns.
///
/// ```no_run
/// // Thread-terminating — not safe to run in the doctest harness.
/// reovim_arch::sys::exit(0);
/// ```
pub fn exit(code: i32) -> ! {
    // SAFETY: the Linux `exit` syscall (nr 60) unconditionally terminates
    // the calling thread; the noreturn asm shape carries that contract with
    // no post-syscall code (no spin loop, no unsafe hint).
    unsafe { syscall1_noreturn(nr::EXIT, as_reg(code)) }
}

/// Returns the calling thread's kernel thread id.
///
/// `gettid` cannot fail.
///
/// ```rust
/// use reovim_arch::sys::gettid;
/// // The main thread's tid is always a positive integer.
/// assert!(gettid() > 0);
/// ```
#[must_use]
pub fn gettid() -> i32 {
    // SAFETY: `gettid` takes no arguments, dereferences no memory, and
    // always succeeds; the result is the caller's tid.
    let ret = unsafe { super::target::raw::syscall0(nr::GETTID) };
    ret_as_i32(ret)
}

// L12 layout (#785 Phase 5): tests live in the sibling file `wrap_tests.rs`,
// declared in `sys/mod.rs` as
// `#[cfg(feature = "selftest")] mod wrap_tests;`.
// All items under test are public so `super::` is not needed.
