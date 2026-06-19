//! Raw aarch64-linux syscall primitives.
//!
//! The `aarch64` Linux syscall convention: the syscall number goes in `x8`;
//! arguments in `x0`, `x1`, `x2`, `x3`, `x4`, `x5`; the `svc #0` instruction
//! returns its result in `x0` and — unlike the `x86_64` `syscall`, which
//! clobbers `rcx`/`r11` — the kernel preserves every register except the
//! `x0` return, so no scratch-clobber operands are needed. A return value in
//! the range `-4095..=-1` is a negated errno; mapping is done by the wrappers
//! (see [`super::errno`]).

use core::arch::asm;

/// aarch64-linux syscall numbers — the minimal floor (1.2 §10).
///
/// These are the `asm-generic` unistd numbers, verified against
/// `/usr/include/asm-generic/unistd.h` (the kernel uapi header aarch64
/// inherits via `asm/unistd.h`). `MMAP` is `__NR3264_mmap` (222), which the
/// header aliases to `__NR_mmap` on 64-bit targets.
///
/// Callers should prefer the typed wrappers in [`crate::sys`] (e.g.
/// [`crate::sys::write`], [`crate::sys::mmap`]) which handle the
/// errno-mapping automatically. These constants are exposed for the rare
/// inline-asm consumer (such as the thread `clone` path) that needs the raw
/// number.
pub mod nr {
    /// `read(fd, buf, count)`.
    ///
    /// See [`crate::sys::read`] for the typed wrapper.
    pub const READ: usize = 63;
    /// `write(fd, buf, count)`.
    ///
    /// See [`crate::sys::write`] for the typed wrapper.
    pub const WRITE: usize = 64;
    /// `close(fd)`.
    ///
    /// See [`crate::sys::close`] for the typed wrapper.
    pub const CLOSE: usize = 57;
    /// `mmap(addr, len, prot, flags, fd, off)`.
    ///
    /// See [`crate::sys::mmap`] for the typed wrapper.
    pub const MMAP: usize = 222;
    /// `mprotect(addr, len, prot)`.
    ///
    /// See [`crate::sys::mprotect`] for the typed wrapper.
    pub const MPROTECT: usize = 226;
    /// `munmap(addr, len)`.
    ///
    /// See [`crate::sys::munmap`] for the typed wrapper.
    pub const MUNMAP: usize = 215;
    /// `clone(flags, stack, ptid, tls, ctid)`.
    ///
    /// Note the arg order differs from `x86_64`: on the `asm-generic` ABI the
    /// `tls` and `ctid` arguments are swapped relative to the `x86_64`
    /// `CLONE_BACKWARDS` ABI. See [`super::clone_into`] for the consumer.
    ///
    /// See [`crate::thread::spawn`] for the typed thread-creation wrapper.
    pub const CLONE: usize = 220;
    /// `gettid()`.
    ///
    /// See [`crate::sys::gettid`] for the typed wrapper.
    pub const GETTID: usize = 178;
    /// `futex(uaddr, op, val, timeout, uaddr2, val3)`.
    ///
    /// See [`crate::sys::futex`] for the typed wrapper.
    pub const FUTEX: usize = 98;
    /// `exit(status)` — terminates the calling thread only (cf.
    /// [`EXIT_GROUP`], which terminates the whole process).
    ///
    /// See [`crate::sys::exit`] for the typed wrapper.
    pub const EXIT: usize = 93;
    /// `clock_gettime(clockid, tp)`.
    ///
    /// See [`crate::sys::clock_gettime`] for the typed wrapper.
    pub const CLOCK_GETTIME: usize = 113;
    /// `exit_group(status)`.
    ///
    /// See [`crate::sys::exit_group`] for the typed wrapper.
    pub const EXIT_GROUP: usize = 94;
    /// `openat(dirfd, path, flags, mode)`.
    ///
    /// See [`crate::sys::openat`] for the typed wrapper.
    pub const OPENAT: usize = 56;

    // ---- networking syscalls -----------------------------------------------
    //
    // aarch64 inherits `asm-generic/unistd.h` directly — no socketcall
    // multiplexer.  Verified against
    // `include/uapi/asm-generic/unistd.h` in the Linux kernel source tree.

    /// `socket(domain, type, protocol)`.
    ///
    /// Source: Linux `include/uapi/asm-generic/unistd.h` `__NR_socket` = 198.
    /// See [`crate::sys::socket`] for the typed wrapper.
    pub const SOCKET: usize = 198;
    /// `bind(sockfd, addr, addrlen)`.
    ///
    /// Source: Linux `include/uapi/asm-generic/unistd.h` `__NR_bind` = 200.
    /// See [`crate::sys::bind`] for the typed wrapper.
    pub const BIND: usize = 200;
    /// `listen(sockfd, backlog)`.
    ///
    /// Source: Linux `include/uapi/asm-generic/unistd.h` `__NR_listen` = 201.
    /// See [`crate::sys::listen`] for the typed wrapper.
    pub const LISTEN: usize = 201;
    /// `accept(sockfd, addr, addrlen)`.
    ///
    /// Source: Linux `include/uapi/asm-generic/unistd.h` `__NR_accept` = 202.
    /// See [`crate::sys::accept`] for the typed wrapper.
    pub const ACCEPT: usize = 202;
    /// `connect(sockfd, addr, addrlen)`.
    ///
    /// Source: Linux `include/uapi/asm-generic/unistd.h` `__NR_connect` = 203.
    /// See [`crate::sys::connect`] for the typed wrapper.
    pub const CONNECT: usize = 203;
    /// `sendto(2)` — socket send; with a NULL address it is `send(2)`.
    /// Used for stream writes so `MSG_NOSIGNAL` can suppress `SIGPIPE`.
    pub const SENDTO: usize = 206;

    // ---- device-control syscall --------------------------------------------

    /// `ioctl(fd, request, ...)` — used for termios `TCGETS`/`TCSETS`.
    ///
    /// Source: Linux `include/uapi/asm-generic/unistd.h` `__NR_ioctl` = 29.
    /// See [`crate::sys::ioctl`] for the typed wrapper.
    pub const IOCTL: usize = 29;

    // ---- UDS path unlink --------------------------------------------------

    /// `unlinkat(dirfd, path, flags)` — used to remove the UDS socket path on
    /// listener teardown.
    ///
    /// Source: Linux `include/uapi/asm-generic/unistd.h` `__NR_unlinkat` = 35.
    /// See [`crate::sys::unlinkat`] for the typed wrapper.
    pub const UNLINKAT: usize = 35;
}

/// Issues a syscall with no arguments.
///
/// Returns the raw `x0` result; a value in `-4095..=-1` is a negated
/// errno (see [`super::errno::from_ret`]).
///
/// # Safety
///
/// The caller must ensure `nr` names a syscall whose contract is satisfied
/// by zero arguments and whose effects are sound in the current context.
///
/// ```ignore
/// // Direct use requires knowing the ABI number and effects; prefer the typed
/// // wrappers in `crate::sys` (e.g. `crate::sys::gettid()`).
/// use reovim_arch_sys_linux_aarch64::{syscall0, from_ret};
/// use reovim_arch_sys_linux_aarch64::raw::nr;
/// // SAFETY: GETTID takes no arguments and always succeeds.
/// let raw = unsafe { syscall0(nr::GETTID) };
/// assert!(from_ret(raw).is_ok());
/// ```
#[must_use]
pub unsafe fn syscall0(nr: usize) -> isize {
    let ret: isize;
    // SAFETY: the `svc #0` instruction reads `nr` from x8 and writes its
    // result to x0; the kernel preserves all other registers per the aarch64
    // Linux ABI, so the only out operand is x0. memory is clobbered because a
    // syscall may read or write through any pointer argument. The caller
    // upholds the per-syscall contract for `nr` (this primitive takes no
    // pointer arguments itself).
    unsafe {
        asm!(
            "svc #0",
            in("x8") nr,
            out("x0") ret,
            options(nostack),
        );
    }
    ret
}

/// Issues a syscall with one argument.
///
/// Returns the raw `x0` result; a value in `-4095..=-1` is a negated
/// errno.
///
/// # Safety
///
/// The caller must ensure `nr`/`a1` form a valid call: any pointer-shaped
/// argument must reference memory live and correctly typed for the syscall.
///
/// ```ignore
/// // Prefer the typed wrappers in `crate::sys`. Direct use:
/// use reovim_arch_sys_linux_aarch64::{syscall1, from_ret};
/// use reovim_arch_sys_linux_aarch64::raw::nr;
/// // SAFETY: close(-1) — EBADF is a valid error outcome, no memory dereferenced.
/// let raw = unsafe { syscall1(nr::CLOSE, usize::MAX) }; // -1 as usize
/// assert!(from_ret(raw).is_err()); // EBADF
/// ```
#[must_use]
pub unsafe fn syscall1(nr: usize, a1: usize) -> isize {
    let ret: isize;
    // SAFETY: as `syscall0`; `a1` is passed in x0 per the ABI, and x0 is also
    // the return register (`inlateout`). The caller upholds the syscall's
    // argument contract (e.g. a valid pointer/length).
    unsafe {
        asm!(
            "svc #0",
            in("x8") nr,
            inlateout("x0") a1 => ret,
            options(nostack),
        );
    }
    ret
}

/// Issues a single-argument syscall that never returns control
/// (`exit`, `exit_group`).
///
/// The `noreturn` option makes the asm block itself the function's tail:
/// there is no post-syscall code at all — no spin loop, no
/// `unreachable_unchecked` — so the never-returns contract is carried by
/// the asm shape, not by an unsafe hint.
///
/// # Safety
///
/// `nr` must name a syscall the kernel guarantees never returns to user
/// space for any argument value (on this floor: `EXIT`, `EXIT_GROUP`).
pub unsafe fn syscall1_noreturn(nr: usize, a1: usize) -> ! {
    // SAFETY: as `syscall1`; the caller guarantees the kernel never returns
    // from this syscall, which is exactly what `options(noreturn)` asserts.
    unsafe {
        asm!(
            "svc #0",
            in("x8") nr,
            in("x0") a1,
            options(noreturn, nostack),
        );
    }
}

/// Issues a syscall with two arguments.
///
/// Returns the raw `x0` result; a value in `-4095..=-1` is a negated
/// errno.
///
/// # Safety
///
/// The caller must ensure `nr`/`a1`/`a2` form a valid call: any
/// pointer-shaped argument must reference memory live and correctly typed
/// for the syscall.
///
/// ```ignore
/// // Prefer the typed wrappers in `crate::sys`. Direct use:
/// use reovim_arch_sys_linux_aarch64::{syscall2, from_ret};
/// use reovim_arch_sys_linux_aarch64::raw::nr;
/// // SAFETY: munmap(0, 0) — EINVAL for zero/misaligned address, no memory dereferenced.
/// let raw = unsafe { syscall2(nr::MUNMAP, 0, 0) };
/// assert!(from_ret(raw).is_err()); // EINVAL
/// ```
#[must_use]
pub unsafe fn syscall2(nr: usize, a1: usize, a2: usize) -> isize {
    let ret: isize;
    // SAFETY: as `syscall0`; arguments are passed in x0, x1 per the ABI.
    // The caller upholds the syscall's argument contract.
    unsafe {
        asm!(
            "svc #0",
            in("x8") nr,
            inlateout("x0") a1 => ret,
            in("x1") a2,
            options(nostack),
        );
    }
    ret
}

/// Issues a syscall with three arguments.
///
/// Returns the raw `x0` result; a value in `-4095..=-1` is a negated
/// errno.
///
/// # Safety
///
/// The caller must ensure the arguments form a valid call: any
/// pointer-shaped argument must reference memory live and correctly typed
/// for the syscall.
///
/// ```ignore
/// // Prefer the typed wrappers in `crate::sys`. Direct use:
/// use reovim_arch_sys_linux_aarch64::{syscall3, from_ret};
/// use reovim_arch_sys_linux_aarch64::raw::nr;
/// // SAFETY: close(-1) via 3-arg trampoline — EBADF, no memory dereferenced.
/// // (WRITE with fd=-1 gives EBADF, demonstrating a 3-arg call.)
/// let raw = unsafe { syscall3(nr::WRITE, usize::MAX, 0, 0) };
/// assert!(from_ret(raw).is_err()); // EBADF or EFAULT
/// ```
#[must_use]
pub unsafe fn syscall3(nr: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let ret: isize;
    // SAFETY: as `syscall0`; arguments are passed in x0, x1, x2 per the ABI.
    // The caller upholds the syscall's argument contract.
    unsafe {
        asm!(
            "svc #0",
            in("x8") nr,
            inlateout("x0") a1 => ret,
            in("x1") a2,
            in("x2") a3,
            options(nostack),
        );
    }
    ret
}

/// Issues a syscall with four arguments.
///
/// Returns the raw `x0` result; a value in `-4095..=-1` is a negated
/// errno.
///
/// # Safety
///
/// The caller must ensure the arguments form a valid call: any
/// pointer-shaped argument must reference memory live and correctly typed
/// for the syscall.
///
/// ```ignore
/// // Prefer the typed wrappers in `crate::sys`. Direct use:
/// use reovim_arch_sys_linux_aarch64::{syscall4, from_ret};
/// use reovim_arch_sys_linux_aarch64::raw::nr;
/// // SAFETY: openat with a null path pointer → EFAULT; demonstrates a 4-arg call.
/// let raw = unsafe { syscall4(nr::OPENAT, usize::MAX, 0, 0, 0) };
/// assert!(from_ret(raw).is_err());
/// ```
#[must_use]
pub unsafe fn syscall4(nr: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> isize {
    let ret: isize;
    // SAFETY: as `syscall0`; the fourth argument is passed in x3 (the aarch64
    // ABI passes args sequentially in x0..x5, with no register substitution).
    // The caller upholds the argument contract.
    unsafe {
        asm!(
            "svc #0",
            in("x8") nr,
            inlateout("x0") a1 => ret,
            in("x1") a2,
            in("x2") a3,
            in("x3") a4,
            options(nostack),
        );
    }
    ret
}

/// Issues a syscall with six arguments.
///
/// Returns the raw `x0` result; a value in `-4095..=-1` is a negated
/// errno.
///
/// # Safety
///
/// The caller must ensure the arguments form a valid call: any
/// pointer-shaped argument must reference memory live and correctly typed
/// for the syscall.
///
/// ```ignore
/// // Prefer the typed wrappers in `crate::sys`. Direct use:
/// use reovim_arch_sys_linux_aarch64::{syscall6, from_ret};
/// use reovim_arch_sys_linux_aarch64::raw::nr;
/// // SAFETY: mmap with zero length → EINVAL; demonstrates a 6-arg call.
/// let raw = unsafe { syscall6(nr::MMAP, 0, 0, 0, 0, usize::MAX, 0) };
/// assert!(from_ret(raw).is_err()); // EINVAL (zero length)
/// ```
#[must_use]
pub unsafe fn syscall6(
    nr: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
) -> isize {
    let ret: isize;
    // SAFETY: as `syscall0`; arguments are passed in x0, x1, x2, x3, x4, x5
    // per the ABI. The caller upholds the syscall's argument contract.
    unsafe {
        asm!(
            "svc #0",
            in("x8") nr,
            inlateout("x0") a1 => ret,
            in("x1") a2,
            in("x2") a3,
            in("x3") a4,
            in("x4") a5,
            in("x5") a6,
            options(nostack),
        );
    }
    ret
}
