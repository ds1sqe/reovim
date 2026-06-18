//! arch's implementation of the down-face platform contract.
//!
//! `arch` is the platform provider: this module builds a `static`
//! [`PlatformVtable`] from arch's real primitives (`time`, `alloc`, `sync`/
//! `thread` futex) and installs it write-once at boot (AB12). This is the
//! `arch → kabi` edge — arch implementing the contract `kabi` declares.
//!
//! ## SP02 scope: clock wired, the rest declared
//!
//! SP02 wired the `clock` slot end-to-end (the boot selftest reads it through
//! the installed handle). SP03 wires the rest: the
//! `alloc`/`dealloc`/`park`/`unpark`/`unpark_all` slots point at arch's genuine
//! backends and `lib/ds` consumes them through the handle's safe wrappers.
//! `unpark_all` is the AB3 trailing append for the wake-all sync paths
//! (`Condvar::notify_all`, `RwLock` release). Pointing every slot at a real
//! backend keeps the vtable honest: the install proves the whole table.
//!
//! ## Zero heap to build
//!
//! The vtable is a `static` of const function pointers, so constructing it
//! allocates nothing. arch's allocator inits heap-free (it is the heap source),
//! so the install can happen at boot before any allocation, dissolving the
//! bootstrap chicken-egg (master plan, Bootstrap resolution).

use core::alloc::Layout;

use {
    reovim_kabi_platform::{InstallError, PlatformVtable, install},
    reovim_uapi_posix::{Fd, Mode, OpenFlags},
};

use crate::{
    alloc::{alloc as arch_alloc, dealloc as arch_dealloc},
    sys::{FUTEX_PRIVATE_FLAG, FUTEX_WAIT, FUTEX_WAKE, futex},
    time::Instant,
};
#[cfg(target_os = "linux")]
use crate::{
    sys::{Timespec, gettid, openat},
    time::realtime,
};

// The net/thread fd-op backends are Linux-only kernel-ABI surface (the same
// gate as `arch::net`/`arch::thread`); freestanding targets have no socket or
// `clone` floor. The vtable shape is identical on every target — only the slot
// IMPLEMENTATIONS differ (real adapters on Linux, `ENOSYS`-returning stubs on
// freestanding) — so a `#[repr(C)]` table built either way stays ABI-compatible.
#[cfg(target_os = "linux")]
use crate::sys::net::{AF_UNIX, SockaddrUn, UNIX_PATH_MAX};
#[cfg(target_os = "linux")]
use crate::sys::{
    AT_FDCWD, Errno, accept as sys_accept, bind as sys_bind, close as sys_close,
    connect as sys_connect, listen as sys_listen, read as sys_read, send_nosignal,
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

// ── net + thread fd-op adapters (SP05) ──────────────────────────────────────
//
// Linux-only: the socket + `clone` floor exists only on Linux (same gate as
// `arch::net`/`arch::thread`). The `#[cfg(not(target_os = "linux"))]` stub
// adapters at the end of this section fill the identical vtable slots with
// `-ENOSYS` returns so the `#[repr(C)]` table is one shape on every target.

/// `EINVAL` errno code, the negative-return used when a caller's path is
/// malformed (empty, no NUL, or longer than `UNIX_PATH_MAX - 1`).
#[cfg(target_os = "linux")]
const EINVAL_CODE: i32 = 22;
/// `ENOMEM` errno code, the negative-return used when a thread stack or its
/// shared block cannot be mapped/allocated.
#[cfg(target_os = "linux")]
const ENOMEM_CODE: i32 = 12;

/// Encodes an arch [`Errno`] as the negative-errno `i64` the fd-op slots
/// return: `-code`.
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
unsafe extern "C" fn unix_accept(listener_fd: Fd) -> i64 {
    // A valid accepted fd fits in i32; the narrowing cannot wrap. The canonical
    // `Fd` newtype maps to the raw `i32` arch's `accept` syscall wrapper takes.
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
#[cfg(target_os = "linux")]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
unsafe extern "C" fn fd_read(fd: Fd, buf: *mut u8, len: usize) -> i64 {
    // SAFETY: caller guarantees `buf` is valid for `len` writable bytes.
    let slice = unsafe { core::slice::from_raw_parts_mut(buf, len) };
    // A byte count never exceeds `isize::MAX`, so the cast cannot wrap. The
    // canonical `Fd` maps to the raw `i32` arch's `read` wrapper takes.
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
#[cfg(target_os = "linux")]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
unsafe extern "C" fn fd_write(fd: Fd, buf: *const u8, len: usize) -> i64 {
    // SAFETY: caller guarantees `buf` is valid for `len` readable bytes.
    let slice = unsafe { core::slice::from_raw_parts(buf, len) };
    // A byte count never exceeds `isize::MAX`, so the cast cannot wrap. The
    // canonical `Fd` maps to the raw `i32` arch's `send` wrapper takes.
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
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
unsafe extern "C" fn fd_close(fd: Fd) -> i64 {
    // The canonical `Fd` maps to the raw `i32` arch's `close` wrapper takes.
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
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
struct SpawnPair {
    entry: unsafe extern "C" fn(*mut u8),
    arg: *mut u8,
}

/// Detached-thread page size (`x86_64`/`aarch64` 4 KiB base pages).
#[cfg(target_os = "linux")]
const SPAWN_PAGE_SIZE: usize = 4096;
/// Usable detached-thread stack (1 MiB), excluding the guard page.
#[cfg(target_os = "linux")]
const SPAWN_STACK_SIZE: usize = 1024 * 1024;
/// Total mapped detached-thread region: guard page + usable stack.
#[cfg(target_os = "linux")]
const SPAWN_MAP_SIZE: usize = SPAWN_PAGE_SIZE + SPAWN_STACK_SIZE;

/// The `clone` flag set for a detached TLS-free worker thread.
///
/// `CLONE_VM|FS|FILES|SIGHAND|THREAD|SYSVSEM` make a real thread sharing the
/// process address space, fd table, signal handlers and `SysV` sem-undo list.
/// Unlike `thread::spawn`, NO `CLONE_PARENT_SETTID`/`CLONE_CHILD_CLEARTID`: a
/// detached thread has no join word, so the kernel neither sets nor clears a
/// ctid — the parent gets the tid as the `clone` return and never waits on it.
#[cfg(target_os = "linux")]
const SPAWN_CLONE_FLAGS: usize = crate::sys::CLONE_VM
    | crate::sys::CLONE_FS
    | crate::sys::CLONE_FILES
    | crate::sys::CLONE_SIGHAND
    | crate::sys::CLONE_THREAD
    | crate::sys::CLONE_SYSVSEM;

/// Maps a guarded stack, boxes the `(entry, arg)` pair, and `clone`s a detached
/// child into [`spawn_trampoline`]. Returns the child tid on success, or a
/// positive errno code on failure (the caller negates it for the slot return).
///
/// # Safety
///
/// `entry`/`arg` must satisfy the [`thread_spawn`] contract: `entry` is a valid
/// function safe to call with `arg`, and `arg` lives for `entry`'s execution.
#[cfg(target_os = "linux")]
unsafe fn spawn_detached(entry: unsafe extern "C" fn(*mut u8), arg: *mut u8) -> Result<i32, i32> {
    use crate::sys::{
        MAP_ANONYMOUS, MAP_PRIVATE, PROT_NONE, PROT_READ, PROT_WRITE, clone_into, mmap, mprotect,
        munmap,
    };

    // Map the stack: a low guard page (PROT_NONE) below the usable stack, so a
    // stack overflow traps on the guard rather than corrupting an adjacent map.
    let stack_base =
        mmap(0, SPAWN_MAP_SIZE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0)
            .map_err(|_| ENOMEM_CODE)?;
    if mprotect(stack_base, SPAWN_PAGE_SIZE, PROT_NONE).is_err() {
        let _ = munmap(stack_base, SPAWN_MAP_SIZE);
        return Err(ENOMEM_CODE);
    }

    // Box the (entry, arg) pair through arch's allocator.
    let pair_layout = Layout::new::<SpawnPair>();
    let Ok(pair) = arch_alloc(pair_layout) else {
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
                arch_dealloc(pair.cast(), pair_layout);
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
#[cfg(target_os = "linux")]
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
        arch_dealloc(core::ptr::NonNull::new_unchecked(pair.cast()), Layout::new::<SpawnPair>());
    }
    // Run the user closure trampoline.
    // SAFETY: `entry`/`arg` satisfy the `thread_spawn` contract (the caller's
    // obligation); `entry` is safe to call with `arg`.
    unsafe { entry(arg) }
    // Exit the THREAD only (not the process). No ctid to clear (detached).
    crate::sys::exit(0)
}

// ── time + file + thread-identity adapters (SP05) ────────────────────────────

/// The wall-clock adapter: reads arch's real-time clock and returns whole
/// nanoseconds since the Unix epoch, matching the contract's `RealtimeFn` ABI.
///
/// `arch::time::realtime` returns a [`Timespec`]; this flattens it to a single
/// `i64` nanos value (no `Timespec` repr crosses the contract — the slot is a
/// scalar return).
///
/// # Safety
///
/// `unsafe extern "C"` to share the vtable's one ABI shape; the underlying
/// `realtime` read has no precondition, so this is sound to call anytime.
#[cfg(target_os = "linux")]
unsafe extern "C" fn realtime_adapter() -> i64 {
    let Timespec { tv_sec, tv_nsec } = realtime();
    // tv_sec/tv_nsec are i64; the year-2262 horizon keeps this in range.
    tv_sec * 1_000_000_000 + tv_nsec
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
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
unsafe extern "C" fn thread_id() -> i64 {
    i64::from(gettid())
}

// ── freestanding stub adapters (non-Linux) ───────────────────────────────────
//
// Freestanding targets have no socket/`clone` floor. The slots still exist (the
// vtable is one `#[repr(C)]` shape on every target); they report `-ENOSYS` so a
// consumer that somehow reaches them on a freestanding build gets a typed error
// rather than a link failure. `ENOSYS` = 38 (Linux errno space, the shared floor
// vocabulary).

/// `ENOSYS` errno code — "function not implemented" — the freestanding stubs'
/// negative return.
#[cfg(not(target_os = "linux"))]
const ENOSYS_CODE: i64 = 38;

#[cfg(not(target_os = "linux"))]
const unsafe extern "C" fn unix_connect(_path: *const u8, _path_len: usize) -> i64 {
    -ENOSYS_CODE
}
#[cfg(not(target_os = "linux"))]
const unsafe extern "C" fn unix_listen(_path: *const u8, _path_len: usize) -> i64 {
    -ENOSYS_CODE
}
#[cfg(not(target_os = "linux"))]
const unsafe extern "C" fn unix_accept(_listener_fd: Fd) -> i64 {
    -ENOSYS_CODE
}
#[cfg(not(target_os = "linux"))]
const unsafe extern "C" fn fd_read(_fd: Fd, _buf: *mut u8, _len: usize) -> i64 {
    -ENOSYS_CODE
}
#[cfg(not(target_os = "linux"))]
const unsafe extern "C" fn fd_write(_fd: Fd, _buf: *const u8, _len: usize) -> i64 {
    -ENOSYS_CODE
}
#[cfg(not(target_os = "linux"))]
const unsafe extern "C" fn fd_close(_fd: Fd) -> i64 {
    -ENOSYS_CODE
}
#[cfg(not(target_os = "linux"))]
unsafe extern "C" fn thread_spawn(_entry: unsafe extern "C" fn(*mut u8), _arg: *mut u8) -> i64 {
    -ENOSYS_CODE
}

/// Freestanding wall-clock stub: returns `0`, the Unix-epoch sentinel. A
/// bare-metal target has no real-time clock, so the wall anchor reads as the
/// epoch (the consumer only uses it as a human-readable origin, never for
/// ordering).
#[cfg(not(target_os = "linux"))]
const unsafe extern "C" fn realtime_adapter() -> i64 {
    0
}
/// Freestanding file-open stub: `-ENOSYS` (no filesystem floor on bare metal),
/// the same fallible shape as the other fd stubs.
#[cfg(not(target_os = "linux"))]
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
#[cfg(not(target_os = "linux"))]
const unsafe extern "C" fn thread_id() -> i64 {
    0
}
/// Freestanding file-write stub: `-ENOSYS` (no filesystem/stdio floor on bare
/// metal), the same fallible shape as the other fd stubs.
#[cfg(not(target_os = "linux"))]
const unsafe extern "C" fn file_write(_fd: Fd, _buf: *const u8, _len: usize) -> i64 {
    -ENOSYS_CODE
}

/// arch's platform vtable: a `static` of const function pointers (zero heap to
/// build), the table the boot path installs.
///
/// The slot order matches [`PlatformVtable`]'s append-only (AB3) declaration.
static PLATFORM_VTABLE: PlatformVtable = PlatformVtable {
    clock,
    alloc,
    dealloc,
    park,
    unpark,
    unpark_all,
    // SP05 net/thread slots: real adapters on Linux, `-ENOSYS` stubs on
    // freestanding (the items above are cfg-selected to one impl per target).
    unix_connect,
    unix_listen,
    unix_accept,
    fd_read,
    fd_write,
    fd_close,
    thread_spawn,
    // SP05 time/file/thread-identity slots: real adapters on Linux, `-ENOSYS`
    // (or epoch/id-0 sentinels) on freestanding, cfg-selected to one impl per
    // target.
    realtime: realtime_adapter,
    file_open,
    thread_id,
    file_write,
};

/// Installs arch's platform vtable as the process-wide handle (write-once).
///
/// The boot path (`rust_entry`, the arch-owned entry in `start.rs`) calls this
/// once, after the allocator is up (it inits heap-free, so it is already
/// usable) and the `static` vtable is built (it is `const`, so it is built at
/// compile time). The first install wins; a second returns
/// [`InstallError::AlreadyInstalled`] (AB12), mirroring the `arch::panic`
/// write-once seams.
///
/// # Errors
///
/// Returns [`InstallError::AlreadyInstalled`] if a handle is already installed
/// — a double-boot bug, surfaced rather than silently overwritten.
pub fn install_platform() -> Result<(), InstallError> {
    install(&PLATFORM_VTABLE)
}

// L12 layout: tests live in the sibling file `platform_tests.rs`, declared as a
// `#[path]` child so `super::` reaches `PLATFORM_VTABLE` and the adapters.
#[cfg(feature = "selftest")]
#[path = "platform_tests.rs"]
mod tests;
