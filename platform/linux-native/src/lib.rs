//! `reovim-platform-linux-native` — the POSIX provider (the trap gate).
//!
//! This crate is the platform provider for the hosted Linux modes (both
//! `x86_64` and `aarch64`): it builds a `static` [`PlatformVtable`] directly
//! from the target `arch-sys` raw mechanisms (clock, mmap, futex, socket, fd,
//! clone, termios) and installs it write-once at boot (AB12). This is the
//! `platform -> kabi` edge: the provider implementing the contract `kabi`
//! declares without going through the `reovim-arch` facade.
//!
//! It is the **POSIX trap gate** of the three-knowledges split (OS-Modes §2.2):
//! it knows hardware *and* contract, canonicalizes NATIVE→POSIX, and installs
//! the `kabi/platform` vtable. For kernel-ABI Linux the mapping is a "thin
//! passthrough" (OS-Modes §6.1) — most slots forward identity values — but the
//! three real translations (errno→i64, `Fd` newtype unwrap, open-flag/mode
//! `.bits()`) and the realtime `Timespec`→i64 flatten live here, the single
//! canonicalization place (master invariant 3).
//!
//! ## Direct target-mechanism edge
//!
//! The provider depends on the matching `arch-sys-linux-*` crate for the active
//! target and owns the small adapters that turn those NATIVE floor results into
//! the `kabi/platform` table. It names no POSIX up-face crate and no
//! `reovim-arch` facade.
//!
//! ## Zero heap to build
//!
//! The vtable is a `static` of const function pointers, so constructing it
//! allocates nothing. The provider's allocation slot maps directly with
//! `mmap`, so the install can happen at boot before any higher allocation,
//! dissolving the bootstrap chicken-egg (master plan, Bootstrap resolution).
//!
//! ## Selftests are parked (SP07)
//!
//! The boot/install selftests that previously rode `arch/src/platform_tests.rs`
//! register through `arch_test!`, which now lives in the `reovim-testrt` leaf
//! (sub-plan 01 extracted it). Restoring the provider's selftest harness — a
//! `selftest`-gated dep on `reovim-testrt` plus a per-provider runner — is
//! tracked under sub-plan 07, alongside the parked `arch-sys` floor tests. This
//! flight keeps scope bounded and does not wire them; the original test bodies
//! (`platform_clock_reads_through_installed_handle`,
//! `platform_install_is_write_once`) are preserved in git history at
//! `arch/src/platform_tests.rs` for SP07 to restore.
#![no_std]
// SAFETY (lint): the provider's vtable slots are `unsafe extern "C"` adapters —
// the one ABI shape the `#[repr(C)]` table requires — and they dereference the
// raw pointers the contract passes. The workspace lint stays `warn`, so every
// crate above the provider still flags unsafe; the allow is scoped to this
// crate. Every `unsafe` block carries a `// SAFETY:` comment.
#![allow(unsafe_code)]

use core::{
    alloc::Layout,
    cell::UnsafeCell,
    ptr::NonNull,
    sync::atomic::{
        AtomicBool, AtomicI32,
        Ordering::{Acquire, Relaxed, Release},
    },
};

use reovim_kabi_platform::{Fd, InstallError, Mode, OpenFlags, PlatformVtable, install};

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_sys_linux_aarch64 as target_sys;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_sys_linux_x86_64 as target_sys;

use target_sys::{
    AT_FDCWD, CLOCK_MONOTONIC, CLOCK_REALTIME, CLONE_FILES, CLONE_FS, CLONE_SIGHAND, CLONE_SYSVSEM,
    CLONE_THREAD, CLONE_VM, EINVAL, ENOMEM, Errno, FUTEX_PRIVATE_FLAG, FUTEX_WAIT, FUTEX_WAKE,
    MAP_ANONYMOUS, MAP_PRIVATE, PROT_NONE, PROT_READ, PROT_WRITE, Timespec, accept as sys_accept,
    bind as sys_bind, clock_gettime, clone_into, close as sys_close, connect as sys_connect, exit,
    futex, gettid, ioctl, listen as sys_listen, mmap, mprotect, munmap,
    net::{AF_UNIX, SockaddrUn, UNIX_PATH_MAX},
    openat, read as sys_read, send_nosignal,
    term::{
        BRKINT, ECHO, ECHOE, ECHOK, ICANON, ICRNL, INPCK, ISIG, ISTRIP, IXON, OPOST, TCGETS,
        TCSETS, Termios, VMIN, VTIME,
    },
    unix_stream_socket, unlinkat, write as sys_write,
};

/// `FUTEX_WAKE` count meaning "wake everyone".
///
/// The kernel reads the wake count as a signed `int`, so the broadcast value
/// is `i32::MAX` (the glibc convention). `u32::MAX` would arrive as `-1` and
/// the kernel's wake loop (`++woken >= nr_wake`) would stop after a single
/// waiter — a lost broadcast that strands every other sleeper. This matches
/// the wake-all count the `lib/ds` `Condvar`/`RwLock` paths request.
const WAKE_ALL: u32 = 0x7FFF_FFFF; // i32::MAX, expressed unsigned

/// System page size for the hosted Linux targets this provider supports.
const PAGE_SIZE: usize = 4096;
/// Nanoseconds in one second.
const NANOS_PER_SEC: i64 = 1_000_000_000;

/// Header stored immediately before each provider allocation.
///
/// The public deallocation ABI carries `(ptr, size, align)`, but the mmap
/// teardown needs the original mapping base/length after the returned pointer
/// has been alignment-adjusted.
#[repr(C)]
#[derive(Clone, Copy)]
struct AllocationHeader {
    base: usize,
    map_len: usize,
}

/// Provider allocation header size in bytes.
const ALLOC_HEADER_SIZE: usize = core::mem::size_of::<AllocationHeader>();

/// Rounds `len` up to whole pages.
fn page_round_up(len: usize) -> Option<usize> {
    len.checked_add(PAGE_SIZE - 1).map(|n| n & !(PAGE_SIZE - 1))
}

/// Rounds `addr` up to `align`.
fn align_up(addr: usize, align: usize) -> Option<usize> {
    debug_assert!(align.is_power_of_two());
    addr.checked_add(align - 1).map(|n| n & !(align - 1))
}

/// Allocates a block through the target mmap floor.
fn alloc_block(layout: Layout) -> Result<NonNull<u8>, i32> {
    if layout.size() == 0 {
        return Err(ENOMEM_CODE);
    }
    let padded = ALLOC_HEADER_SIZE
        .checked_add(layout.size())
        .and_then(|n| n.checked_add(layout.align() - 1))
        .ok_or(ENOMEM_CODE)?;
    let map_len = page_round_up(padded).ok_or(ENOMEM_CODE)?;
    let base = mmap(0, map_len, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0)
        .map_err(|_| ENOMEM_CODE)?;
    let Some(start) = base.checked_add(ALLOC_HEADER_SIZE) else {
        let _ = munmap(base, map_len);
        return Err(ENOMEM_CODE);
    };
    let Some(user_addr) = align_up(start, layout.align()) else {
        let _ = munmap(base, map_len);
        return Err(ENOMEM_CODE);
    };
    let header_addr = user_addr - ALLOC_HEADER_SIZE;
    // SAFETY: `base..base + map_len` is a fresh writable mapping. The padded
    // length reserves room for the header plus any alignment slack before the
    // user pointer, and `header_addr` remains usize-aligned on the supported
    // 64-bit targets.
    unsafe {
        core::ptr::write(header_addr as *mut AllocationHeader, AllocationHeader { base, map_len });
    }
    NonNull::new(user_addr as *mut u8).ok_or(ENOMEM_CODE)
}

/// Releases a block returned by [`alloc_block`].
///
/// # Safety
///
/// `ptr` must be a live pointer returned by [`alloc_block`] and not yet freed.
unsafe fn dealloc_block(ptr: NonNull<u8>) {
    let header_addr = ptr.as_ptr().addr() - ALLOC_HEADER_SIZE;
    // SAFETY: the allocation adapter wrote this header immediately before the
    // returned user pointer, and the caller guarantees the block is still live.
    let header = unsafe { core::ptr::read(header_addr as *const AllocationHeader) };
    let _ = munmap(header.base, header.map_len);
}

/// Converts a target timespec to whole nanoseconds.
fn timespec_to_nanos(ts: Timespec) -> i64 {
    ts.tv_sec
        .saturating_mul(NANOS_PER_SEC)
        .saturating_add(ts.tv_nsec)
}

/// Reads a target clock and returns whole nanoseconds.
fn read_clock_nanos(clock_id: usize) -> i64 {
    let mut ts = Timespec::default();
    if clock_gettime(clock_id, &mut ts).is_err() {
        return 0;
    }
    timespec_to_nanos(ts)
}

/// The clock adapter: reads the target monotonic clock and returns whole
/// nanoseconds, matching the contract's `ClockFn` ABI.
///
/// # Safety
///
/// `unsafe extern "C"` to share the vtable's one ABI shape; the underlying
/// clock read has no precondition, so this is sound to call anytime.
unsafe extern "C" fn clock() -> i64 {
    read_clock_nanos(CLOCK_MONOTONIC)
}

/// The allocation adapter: reconstructs a `Layout` from `(size, align)` and
/// maps memory through the target floor, returning the raw pointer (null on
/// failure) the contract's `AllocFn` ABI expects.
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
        .map_or(null, |layout| alloc_block(layout).map_or(null, NonNull::as_ptr))
}

/// The deallocation adapter: validates the `Layout` shape and unmaps the
/// provider-owned block.
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
    let Ok(_layout) = Layout::from_size_align(size, align) else {
        return;
    };
    // SAFETY: by the contract `nn` is a block a prior `alloc` returned and the
    // block is still live; the header carries the mapping teardown facts.
    unsafe { dealloc_block(nn) }
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

// ── net + thread fd-op adapters ─────────────────────────────────────────────
//
// The socket + `clone` floor the provider names is Linux kernel-ABI surface from
// the target arch-sys crate. The provider is the hosted Linux provider for both
// Linux arches, so every adapter is realized here; the freestanding `-ENOSYS`
// stub variants belong to the system kernel (sub-plan 04), not this crate.

/// `EINVAL` errno code, the negative-return used when a caller's path is
/// malformed (empty, no NUL, or longer than `UNIX_PATH_MAX - 1`). Derived from
/// the target floor's canonical `Errno` so the provider never restates it.
const EINVAL_CODE: i32 = EINVAL.code();
/// `ENOMEM` errno code, the negative-return used when a thread stack or its
/// shared block cannot be mapped/allocated. Derived from the target floor's
/// canonical `Errno` so the provider never restates it.
const ENOMEM_CODE: i32 = ENOMEM.code();

/// Encodes a target-floor [`Errno`] as the negative-errno `i64` the fd-op slots
/// return: `-code` (positive target-floor `Errno` -> negative `-errno` the FFI
/// ABI carries; `kabi::map_fd_ret` recovers it consumer-side).
const fn neg_errno(e: Errno) -> i64 {
    -(e.code() as i64)
}

/// Builds a pathname [`SockaddrUn`] from `path`/`path_len` (a NUL-terminated
/// byte string), returning the address + its `addrlen`, or `None` when the path
/// is empty, missing its NUL, or too long.
///
/// # Safety
///
/// `path` must point to `path_len` readable bytes.
unsafe fn sockaddr_from_raw(path: *const u8, path_len: usize) -> Option<(SockaddrUn, usize)> {
    // SAFETY: the caller guarantees `path` is valid for `path_len` bytes.
    let bytes = unsafe { core::slice::from_raw_parts(path, path_len) };
    let nul = bytes.iter().position(|&b| b == 0)?;
    if nul == 0 || nul >= UNIX_PATH_MAX {
        return None;
    }
    let mut sa = SockaddrUn::zeroed();
    sa.sun_family = AF_UNIX;
    sa.sun_path[..=nul].copy_from_slice(&bytes[..=nul]);
    let addrlen = sa.addrlen();
    Some((sa, addrlen))
}

/// The connect adapter: opens a stream socket, connects it to the pathname in
/// `path`, and returns the connected fd or a negative errno.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `path` must point to `path_len`
/// readable bytes; a malformed path returns `-EINVAL` rather than UB.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
unsafe extern "C" fn unix_connect(path: *const u8, path_len: usize) -> i64 {
    // SAFETY: caller guarantees `path` is valid for `path_len` bytes.
    let Some((sa, addrlen)) = (unsafe { sockaddr_from_raw(path, path_len) }) else {
        return -i64::from(EINVAL_CODE);
    };
    // A valid fd fits in i32 (< 2^31); the narrowing cannot wrap.
    let fd = match unix_stream_socket() {
        Ok(fd) => fd as i32,
        Err(e) => return neg_errno(e),
    };
    if let Err(e) = sys_connect(fd, &sa, addrlen) {
        let _ = sys_close(fd);
        return neg_errno(e);
    }
    i64::from(fd)
}

/// The listen adapter: unlinks any stale socket at `path`, binds a fresh stream
/// socket there, marks it passive (backlog 8), and returns the listening fd or
/// a negative errno.
///
/// The pre-bind unlink makes a repeated bind (a restarted server) succeed: the
/// `panic = "abort"` profile means a prior listener's Drop-unlink may not have
/// run, so the bind owns the cleanup rather than relying on teardown.
///
/// # Safety
///
/// As [`unix_connect`]: `path` must point to `path_len` readable bytes.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
unsafe extern "C" fn unix_listen(path: *const u8, path_len: usize) -> i64 {
    // SAFETY: caller guarantees `path` is valid for `path_len` bytes.
    let Some((sa, addrlen)) = (unsafe { sockaddr_from_raw(path, path_len) }) else {
        return -i64::from(EINVAL_CODE);
    };
    // SAFETY: `path` is valid for `path_len` bytes; the slice borrows it for the
    // unlink call only.
    let path_slice = unsafe { core::slice::from_raw_parts(path, path_len) };
    // Best-effort pre-bind unlink of a stale socket file (ignore the result:
    // ENOENT on a fresh path is expected).
    let _ = unlinkat(AT_FDCWD, path_slice, 0);

    let fd = match unix_stream_socket() {
        Ok(fd) => fd as i32,
        Err(e) => return neg_errno(e),
    };
    if let Err(e) = sys_bind(fd, &sa, addrlen) {
        let _ = sys_close(fd);
        return neg_errno(e);
    }
    if let Err(e) = sys_listen(fd, 8) {
        let _ = sys_close(fd);
        let _ = unlinkat(AT_FDCWD, path_slice, 0);
        return neg_errno(e);
    }
    i64::from(fd)
}

/// The accept adapter: blocks until a connection arrives on `listener_fd`,
/// returning the connected fd or a negative errno.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `listener_fd` is a scalar; a bad fd
/// returns a negative errno.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
unsafe extern "C" fn unix_accept(listener_fd: Fd) -> i64 {
    // A valid accepted fd fits in i32; the narrowing cannot wrap. The canonical
    // `Fd` newtype maps to the raw `i32` the floor's `accept` wrapper takes.
    match sys_accept(listener_fd.as_i32()) {
        Ok(fd) => i64::from(fd as i32),
        Err(e) => neg_errno(e),
    }
}

/// The fd-read adapter: reads up to `len` bytes from `fd` into `buf`, returning
/// the byte count (`0` at EOF) or a negative errno.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `buf` must point to `len` writable
/// bytes; the adapter writes at most the returned count.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
unsafe extern "C" fn fd_read(fd: Fd, buf: *mut u8, len: usize) -> i64 {
    // SAFETY: caller guarantees `buf` is valid for `len` writable bytes.
    let slice = unsafe { core::slice::from_raw_parts_mut(buf, len) };
    // A byte count never exceeds `isize::MAX`, so the cast cannot wrap. The
    // canonical `Fd` maps to the raw `i32` the floor's `read` wrapper takes.
    match sys_read(fd.as_i32(), slice) {
        Ok(n) => n as i64,
        Err(e) => neg_errno(e),
    }
}

/// The fd-write adapter: writes up to `len` bytes from `buf` to `fd`
/// (`send(MSG_NOSIGNAL)` so a closed peer surfaces as `EPIPE`, not a
/// process-killing `SIGPIPE`), returning the byte count or a negative errno.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `buf` must point to `len` readable
/// bytes.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
unsafe extern "C" fn fd_write(fd: Fd, buf: *const u8, len: usize) -> i64 {
    // SAFETY: caller guarantees `buf` is valid for `len` readable bytes.
    let slice = unsafe { core::slice::from_raw_parts(buf, len) };
    // A byte count never exceeds `isize::MAX`, so the cast cannot wrap. The
    // canonical `Fd` maps to the raw `i32` the floor's `send` wrapper takes.
    match send_nosignal(fd.as_i32(), slice) {
        Ok(n) => n as i64,
        Err(e) => neg_errno(e),
    }
}

/// The file-write adapter: writes up to `len` bytes from `buf` to `fd` with the
/// ordinary `write(2)` syscall, returning the byte count or a negative errno.
///
/// The plain-write counterpart to [`fd_write`] (a socket `send`): `write(2)` is
/// valid on any fd — a regular file (the kernel log sink), a pipe, or a tty
/// (the tui's stdout) — whereas `send` is socket-only (it returns `ENOTSOCK` on
/// a file). The two write slots split on the `lib/ds` module boundary: `net`
/// uses `fd_write`, `fs` uses this slot.
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

/// The fd-close adapter: closes `fd`, returning `0` or a negative errno.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `fd` is a scalar; an already-closed
/// fd returns a negative errno.
unsafe extern "C" fn fd_close(fd: Fd) -> i64 {
    // The canonical `Fd` maps to the raw `i32` the floor's `close` wrapper takes.
    match sys_close(fd.as_i32()) {
        Ok(_) => 0,
        Err(e) => neg_errno(e),
    }
}

/// The thread-spawn adapter: starts a **detached** thread that calls
/// `entry(arg)`, returning the new tid or a negative errno.
///
/// Detached, not joinable: the thread owns nothing the caller must reclaim. The
/// adapter maps a guarded stack, stores the `(entry, arg)` pair at the stack
/// top, and `clone`s into a fixed local trampoline that runs `entry(arg)` then
/// exits the thread. The stack mapping is intentionally leaked on thread exit
/// (the child runs on it and cannot free it; no joiner exists to reclaim it) —
/// the same deliberate no-Drop leak the `thread::JoinHandle` documents for an
/// un-joined handle. The only floor consumer (the server accept/connection
/// loops) never joins, so a joinable handle is deferred (rule of three).
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `entry` must be a valid function
/// safe to call with `arg`; `arg` must be valid for the whole of `entry`'s
/// execution. The new thread takes ownership of `arg`.
unsafe extern "C" fn thread_spawn(entry: unsafe extern "C" fn(*mut u8), arg: *mut u8) -> i64 {
    // SAFETY: the caller upholds the `entry`/`arg` contract; `spawn_detached`
    // maps a stack, stores the pair, and clones into the trampoline.
    match unsafe { spawn_detached(entry, arg) } {
        Ok(tid) => i64::from(tid),
        Err(code) => -i64::from(code),
    }
}

/// The `(entry, arg)` pair the detached spawn hands to its child trampoline.
///
/// Heap-boxed at spawn and stored by pointer at the child's stack top; the
/// trampoline copies both fields out and frees the box before running `entry`,
/// so the only thing the detached thread leaks is its own stack mapping (which
/// it runs on and cannot free).
struct SpawnPair {
    entry: unsafe extern "C" fn(*mut u8),
    arg: *mut u8,
}

/// Detached-thread page size (`x86_64`/`aarch64` 4 KiB base pages).
const SPAWN_PAGE_SIZE: usize = 4096;
/// Usable detached-thread stack (1 MiB), excluding the guard page.
const SPAWN_STACK_SIZE: usize = 1024 * 1024;
/// Total mapped detached-thread region: guard page + usable stack.
const SPAWN_MAP_SIZE: usize = SPAWN_PAGE_SIZE + SPAWN_STACK_SIZE;

/// The `clone` flag set for a detached TLS-free worker thread.
///
/// `CLONE_VM|FS|FILES|SIGHAND|THREAD|SYSVSEM` make a real thread sharing the
/// process address space, fd table, signal handlers and `SysV` sem-undo list.
/// Unlike `thread::spawn`, NO `CLONE_PARENT_SETTID`/`CLONE_CHILD_CLEARTID`: a
/// detached thread has no join word, so the kernel neither sets nor clears a
/// ctid — the parent gets the tid as the `clone` return and never waits on it.
const SPAWN_CLONE_FLAGS: usize =
    CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND | CLONE_THREAD | CLONE_SYSVSEM;

/// Maps a guarded stack, boxes the `(entry, arg)` pair, and `clone`s a detached
/// child into [`spawn_trampoline`]. Returns the child tid on success, or a
/// positive errno code on failure (the caller negates it for the slot return).
///
/// # Safety
///
/// `entry`/`arg` must satisfy the [`thread_spawn`] contract: `entry` is a valid
/// function safe to call with `arg`, and `arg` lives for `entry`'s execution.
unsafe fn spawn_detached(entry: unsafe extern "C" fn(*mut u8), arg: *mut u8) -> Result<i32, i32> {
    // Map the stack: a low guard page (PROT_NONE) below the usable stack, so a
    // stack overflow traps on the guard rather than corrupting an adjacent map.
    let stack_base =
        mmap(0, SPAWN_MAP_SIZE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0)
            .map_err(|_| ENOMEM_CODE)?;
    if mprotect(stack_base, SPAWN_PAGE_SIZE, PROT_NONE).is_err() {
        let _ = munmap(stack_base, SPAWN_MAP_SIZE);
        return Err(ENOMEM_CODE);
    }

    // Box the (entry, arg) pair through the provider's mmap-backed allocator.
    let pair_layout = Layout::new::<SpawnPair>();
    let Ok(pair) = alloc_block(pair_layout) else {
        let _ = munmap(stack_base, SPAWN_MAP_SIZE);
        return Err(ENOMEM_CODE);
    };
    let pair = pair.cast::<SpawnPair>();
    // SAFETY: `pair` is a fresh, correctly-sized, aligned allocation; the write
    // initializes both fields.
    unsafe {
        core::ptr::write(pair.as_ptr(), SpawnPair { entry, arg });
    }

    // Lay the child's initial stack: the trampoline's single argument word (the
    // pair pointer) at `[sp]`, with `sp == 8 mod 16` (the `clone_into` contract,
    // matching `thread::spawn_inner`).
    let stack_top = stack_base + SPAWN_MAP_SIZE;
    let arg_slot = ((stack_top & !0xF) - 8) - 16;
    // SAFETY: `arg_slot` is well within the mapped, writable stack region (far
    // above the guard page, below the top) and 8-byte aligned for a usize write.
    unsafe {
        core::ptr::write(arg_slot as *mut usize, pair.as_ptr() as usize);
    }

    // SAFETY: `SPAWN_CLONE_FLAGS` is a coherent thread set; `arg_slot` is the top
    // of a valid stack the child owns with the pair pointer stored at `[arg_slot]`;
    // `join_word` is 0 (detached — no ctid). The child enters `spawn_trampoline`,
    // never returning to Rust. On a `clone` error the parent tears down the stack
    // and the pair box below.
    let ret = unsafe {
        clone_into(SPAWN_CLONE_FLAGS, arg_slot, 0, (spawn_trampoline as *const ()).addr())
    };
    match ret {
        Ok(tid) => {
            // The child owns the pair box and the stack now.
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            Ok(tid as i32)
        }
        Err(e) => {
            // The child never started: free the pair box and unmap the stack.
            // SAFETY: `pair` is the live block we just wrote; nothing else holds
            // it (the child did not start), so dropping + freeing is sound.
            unsafe {
                core::ptr::drop_in_place(pair.as_ptr());
                dealloc_block(pair.cast());
            }
            let _ = munmap(stack_base, SPAWN_MAP_SIZE);
            Err(e.code())
        }
    }
}

/// The detached child's entry point: reads the `(entry, arg)` pair from the
/// pointer the parent stored at the stack top, frees the pair box, runs the
/// user `entry(arg)`, then exits the thread.
///
/// `extern "C"` so its calling convention matches the hand-written child branch
/// in `clone_into` (`arg` arrives in the target's first C-ABI argument register).
/// The stack mapping is NOT freed here — the thread runs on it and no joiner
/// exists to reclaim it (the deliberate detached-thread leak; see [`thread_spawn`]).
extern "C" fn spawn_trampoline(pair_ptr: usize) -> ! {
    let pair = pair_ptr as *mut SpawnPair;
    // SAFETY: `pair_ptr` is the pair-box pointer the parent stored at the stack
    // top and kept alive (it does not free the box on the success path); this is
    // the unique live access now that the child runs.
    let SpawnPair { entry, arg } = unsafe { core::ptr::read(pair) };
    // Free the pair box (entry + arg are copied out); the box is no longer needed.
    // SAFETY: `pair` is the live box; we just read its contents out, so freeing
    // it here is the unique teardown (the parent handed ownership to the child).
    unsafe {
        dealloc_block(core::ptr::NonNull::new_unchecked(pair.cast()));
    }
    // Run the user closure trampoline.
    // SAFETY: `entry`/`arg` satisfy the `thread_spawn` contract (the caller's
    // obligation); `entry` is safe to call with `arg`.
    unsafe { entry(arg) }
    // Exit the THREAD only (not the process). No ctid to clear (detached).
    exit(0)
}

// ── time + file + thread-identity adapters ───────────────────────────────────

/// The wall-clock adapter: reads the target real-time clock and returns whole
/// nanoseconds since the Unix epoch, matching the contract's `RealtimeFn` ABI.
///
/// The target floor fills a [`Timespec`]; this flattens it to a single `i64`
/// nanos value (no `Timespec` repr crosses the contract — the slot is a scalar
/// return).
///
/// # Safety
///
/// `unsafe extern "C"` to share the vtable's one ABI shape; the underlying
/// real-time clock read has no precondition, so this is sound to call anytime.
unsafe extern "C" fn realtime_adapter() -> i64 {
    read_clock_nanos(CLOCK_REALTIME)
}

/// The file-open adapter: opens the pathname in `path` (`AT_FDCWD`-relative)
/// with the Linux open `flags`/`mode` and returns the new fd or a negative
/// errno.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `path` must point to `path_len`
/// readable bytes containing a NUL-terminated pathname; a refused open maps to
/// a negative errno rather than UB.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
unsafe extern "C" fn file_open(
    path: *const u8,
    path_len: usize,
    flags: OpenFlags,
    mode: Mode,
) -> i64 {
    // SAFETY: caller guarantees `path` is valid for `path_len` readable bytes;
    // the slice borrows it for the openat call only.
    let slice = unsafe { core::slice::from_raw_parts(path, path_len) };
    // `flags` carries a Linux open-flag bit pattern; the cast to the `usize`
    // the syscall wrapper takes reinterprets those bits (sign loss is
    // intentional — a flag set is not a signed value). `mode` is the unsigned
    // permission word, widened to the wrapper's `usize`.
    match openat(AT_FDCWD, slice, flags.bits() as usize, mode.bits() as usize) {
        // A valid fd is a small non-negative usize (< 2^31); the narrowing
        // cannot wrap.
        Ok(fd) => fd as i64,
        Err(e) => neg_errno(e),
    }
}

/// The thread-id adapter: returns the calling thread's kernel thread id.
///
/// # Safety
///
/// `unsafe extern "C"` to share the vtable's one ABI shape; `gettid` takes no
/// argument, dereferences no memory, and cannot fail.
unsafe extern "C" fn thread_id() -> i64 {
    i64::from(gettid())
}

/// The path-unlink adapter: removes the pathname in `path`
/// (`AT_FDCWD`-relative, no flags) and returns `0` or a negative errno.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `path` must point to `path_len`
/// readable bytes containing a NUL-terminated pathname; a refused unlink maps
/// to a negative errno rather than UB.
unsafe extern "C" fn path_unlink(path: *const u8, path_len: usize) -> i64 {
    // SAFETY: caller guarantees `path` is valid for `path_len` readable bytes;
    // the slice borrows it for the unlinkat call only.
    let slice = unsafe { core::slice::from_raw_parts(path, path_len) };
    match unlinkat(AT_FDCWD, slice, 0) {
        Ok(_) => 0,
        Err(e) => neg_errno(e),
    }
}

// ── termios contract ──────────────────────────────────────────────────────────
//
// The raw-mode flag policy + the TCGETS/TCSETS ioctls are provider mechanism
// (master invariant 3): they moved here from the old `arch::term::RawMode`. The
// contract carries only `(fd) → token`/`(fd) → ()`; the saved `Termios` never
// crosses it — the provider keys it by fd in a fixed, no-heap table below.

/// Number of concurrently-raw fds the provider can hold saved state for.
///
/// The tui rawing exactly one fd (stdin), a handful of slots is ample; the
/// table is a fixed `static` (no heap, `no_std`) so a coarser-than-needed bound
/// costs only a few stack-free bytes. An exhausted table refuses with `-ENOTTY`'s
/// sibling refusal (`term_set_raw` returns the ioctl error path), never panics.
const TERM_TABLE_SLOTS: usize = 8;

/// A single fd→saved-`Termios` table entry: the fd key plus its saved cooked
/// settings. `fd == -1` marks the slot free.
struct TermSlot {
    /// The raw fd this slot holds saved state for; `-1` when free.
    fd: AtomicI32,
    /// The pre-raw `Termios` to restore. Read/written only under the table lock.
    saved: UnsafeCell<Termios>,
}

/// The provider-side fd-keyed saved-state table (a `static`, no heap).
///
/// `term_set_raw` populates an entry; `term_restore` consumes + clears it. The
/// table being keyed by fd (not by a guard instance) is what lets the tui panic
/// hook restore after its `RawMode` guard is gone — and what makes
/// `term_restore` idempotent (an absent fd is a no-op).
struct TermTable {
    /// A spinlock serializing the (rare, boot/teardown-time) table mutations.
    lock: AtomicBool,
    /// The fixed slot array.
    slots: [TermSlot; TERM_TABLE_SLOTS],
}

// SAFETY: every access to a slot's `UnsafeCell<Termios>` happens while the
// caller holds `lock` (see `term_lock`/`term_unlock`), so there is never an
// aliased mutable reference across threads. The `AtomicI32` fd key and the
// `AtomicBool` lock are `Sync` by construction. The table is only mutated at
// raw-mode enter/restore (boot + teardown), so contention is effectively nil.
unsafe impl Sync for TermTable {}

/// The single provider-wide saved-state table.
static TERM_TABLE: TermTable = TermTable {
    lock: AtomicBool::new(false),
    slots: [const {
        TermSlot {
            fd: AtomicI32::new(-1),
            saved: UnsafeCell::new(Termios::zeroed()),
        }
    }; TERM_TABLE_SLOTS],
};

/// Acquires the table spinlock (test-and-set, spin on contention).
fn term_lock() {
    while TERM_TABLE
        .lock
        .compare_exchange(false, true, Acquire, Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
}

/// Releases the table spinlock.
fn term_unlock() {
    TERM_TABLE.lock.store(false, Release);
}

/// The set-raw adapter: reads `fd`'s current termios, applies the raw-mode flag
/// policy, writes it back immediately, stashes the saved settings keyed by `fd`,
/// and returns `fd` as the token — or a negative errno (`-ENOTTY` when `fd` is
/// not a terminal).
///
/// The flag policy is byte-identical to the old `arch::term::RawMode::enter`:
/// clear `BRKINT|ICRNL|INPCK|ISTRIP|IXON` from `c_iflag`, `OPOST` from
/// `c_oflag`, `ECHO|ECHOE|ECHOK|ICANON|ISIG` from `c_lflag`, set `VMIN=1`,
/// `VTIME=0`, and apply via `TCSETS` (immediate, no drain).
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `fd` is a plain scalar; a non-tty or
/// bad fd maps to a negative errno (via the `TCGETS` ioctl), never UB.
unsafe extern "C" fn term_set_raw(fd: i32) -> i64 {
    // Read the current termios via TCGETS.
    let mut saved = Termios::zeroed();
    if let Err(e) = ioctl(fd, TCGETS, core::ptr::from_mut(&mut saved).addr()) {
        return neg_errno(e);
    }

    // Build the raw-mode termios from the saved baseline (byte-identical to the
    // old `arch::term::RawMode::enter` policy).
    let mut raw = saved;
    // Input flags: turn off cooked-mode translations.
    raw.c_iflag &= !(BRKINT | ICRNL | INPCK | ISTRIP | IXON);
    // Output flags: disable post-processing so \n isn't mapped to \r\n.
    raw.c_oflag &= !OPOST;
    // Local flags: disable canonical, echo, and signal-generating chars.
    raw.c_lflag &= !(ECHO | ECHOE | ECHOK | ICANON | ISIG);
    // Read control: block until at least 1 byte; no timeout.
    raw.c_cc[VMIN] = 1;
    raw.c_cc[VTIME] = 0;

    // Apply raw termios via TCSETS (immediate, no drain).
    if let Err(e) = ioctl(fd, TCSETS, core::ptr::from_ref(&raw).addr()) {
        return neg_errno(e);
    }

    // Stash the saved settings keyed by fd (reuse an existing entry for this fd,
    // else claim a free slot). A full table refuses with -ENOTTY's refusal — the
    // raw mode is already applied, so the worst case is a restore that no-ops,
    // not a corrupted terminal.
    term_lock();
    let mut placed = false;
    let mut free: Option<usize> = None;
    for (i, slot) in TERM_TABLE.slots.iter().enumerate() {
        let key = slot.fd.load(Relaxed);
        if key == fd {
            // SAFETY: the table lock is held, so this is the unique writer of
            // this slot's cell; no other thread reads/writes it concurrently.
            unsafe { *slot.saved.get() = saved };
            placed = true;
            break;
        }
        if key == -1 && free.is_none() {
            free = Some(i);
        }
    }
    if !placed && let Some(i) = free {
        let slot = &TERM_TABLE.slots[i];
        // SAFETY: the table lock is held; this slot was free (fd == -1), so we
        // are its unique writer. Write the saved state before publishing the fd.
        unsafe { *slot.saved.get() = saved };
        slot.fd.store(fd, Relaxed);
    }
    term_unlock();

    i64::from(fd)
}

/// The restore adapter: restores `fd`'s saved termios (TCSETS) and clears its
/// table entry. Idempotent — an absent fd is a best-effort no-op, never an error
/// (the tui panic hook relies on this after the guard `Drop` already restored).
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `fd` is a plain scalar; an unknown
/// fd is a no-op, never UB.
unsafe extern "C" fn term_restore(fd: i32) {
    // Snapshot the saved settings under the lock, then clear the entry, so the
    // restore ioctl runs outside the lock (it may block briefly).
    term_lock();
    let mut found: Option<Termios> = None;
    for slot in &TERM_TABLE.slots {
        if slot.fd.load(Relaxed) == fd {
            // SAFETY: the table lock is held; this is the unique reader of the
            // slot's cell. Copy the saved value out before clearing the key.
            found = Some(unsafe { *slot.saved.get() });
            slot.fd.store(-1, Relaxed);
            break;
        }
    }
    term_unlock();

    if let Some(saved) = found {
        // Best-effort restore; ignore errors (the fd may already be closed, or
        // the process may be tearing down).
        let _ = ioctl(fd, TCSETS, core::ptr::from_ref(&saved).addr());
    }
    // An absent fd: nothing to restore (idempotent no-op).
}

/// The provider's platform vtable: a `static` of const function pointers (zero
/// heap to build), the table the boot path installs.
///
/// The slot order matches [`PlatformVtable`]'s append-only (AB3) declaration.
static PLATFORM_VTABLE: PlatformVtable = PlatformVtable {
    clock,
    alloc,
    dealloc,
    park,
    unpark,
    unpark_all,
    unix_connect,
    unix_listen,
    unix_accept,
    fd_read,
    fd_write,
    fd_close,
    thread_spawn,
    realtime: realtime_adapter,
    file_open,
    thread_id,
    file_write,
    term_set_raw,
    term_restore,
    path_unlink,
};

/// Installs the provider's platform vtable as the process-wide handle
/// (write-once).
///
/// The `apps/*` composition root calls this once as the first statement of its
/// `entry!` closure, after the allocator is up (it inits heap-free, so it is
/// already usable) and the `static` vtable is built (it is `const`, so it is
/// built at compile time). The first install wins; a second returns
/// [`InstallError::AlreadyInstalled`] (AB12), mirroring the `arch::panic`
/// write-once seams. Installing as the closure's first statement preserves the
/// no-read-before-install invariant: every `kabi::handle` read runs after it.
///
/// # Errors
///
/// Returns [`InstallError::AlreadyInstalled`] if a handle is already installed
/// — a double-boot bug, surfaced rather than silently overwritten.
///
/// # Example
///
/// ```ignore
/// // First statement of the `apps/*` `entry!` closure, before any handle read:
/// reovim_platform_linux_native::install_platform().expect("vtable already installed");
/// ```
pub fn install_platform() -> Result<(), InstallError> {
    install(&PLATFORM_VTABLE)
}
