//! Raw x86_64-linux syscall primitives.
//!
//! The `x86_64` Linux syscall convention: the syscall number goes in `rax`;
//! arguments in `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9` (note `r10`, not
//! `rcx`, for the fourth argument); the `syscall` instruction returns its
//! result in `rax` and clobbers `rcx` and `r11`. A return value in the
//! range `-4095..=-1` is a negated errno; mapping is done by the wrappers
//! (see [`super::errno`]).

use core::arch::asm;

/// x86_64-linux syscall numbers — the minimal floor (1.2 §10).
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
    pub const READ: usize = 0;
    /// `write(fd, buf, count)`.
    ///
    /// See [`crate::sys::write`] for the typed wrapper.
    pub const WRITE: usize = 1;
    /// `close(fd)`.
    ///
    /// See [`crate::sys::close`] for the typed wrapper.
    pub const CLOSE: usize = 3;
    /// `mmap(addr, len, prot, flags, fd, off)`.
    ///
    /// See [`crate::sys::mmap`] for the typed wrapper.
    pub const MMAP: usize = 9;
    /// `mprotect(addr, len, prot)`.
    ///
    /// See [`crate::sys::mprotect`] for the typed wrapper.
    pub const MPROTECT: usize = 10;
    /// `munmap(addr, len)`.
    ///
    /// See [`crate::sys::munmap`] for the typed wrapper.
    pub const MUNMAP: usize = 11;
    /// `clone(flags, stack, ptid, ctid, tls)`.
    ///
    /// See [`crate::thread::spawn`] for the typed thread-creation wrapper.
    pub const CLONE: usize = 56;
    /// `gettid()`.
    ///
    /// See [`crate::sys::gettid`] for the typed wrapper.
    pub const GETTID: usize = 186;
    /// `futex(uaddr, op, val, timeout, uaddr2, val3)`.
    ///
    /// See [`crate::sys::futex`] for the typed wrapper.
    pub const FUTEX: usize = 202;
    /// `exit(status)` — terminates the calling thread only (cf.
    /// [`EXIT_GROUP`], which terminates the whole process).
    ///
    /// See [`crate::sys::exit`] for the typed wrapper.
    pub const EXIT: usize = 60;
    /// `clock_gettime(clockid, tp)`.
    ///
    /// See [`crate::sys::clock_gettime`] for the typed wrapper.
    pub const CLOCK_GETTIME: usize = 228;
    /// `exit_group(status)`.
    ///
    /// See [`crate::sys::exit_group`] for the typed wrapper.
    pub const EXIT_GROUP: usize = 231;
    /// `openat(dirfd, path, flags, mode)`.
    ///
    /// See [`crate::sys::openat`] for the typed wrapper.
    pub const OPENAT: usize = 257;

    // ---- networking syscalls -----------------------------------------------

    /// `socket(domain, type, protocol)`.
    ///
    /// Source: Linux `arch/x86/include/generated/uapi/asm/unistd_64.h` line 41.
    /// See [`crate::sys::socket`] for the typed wrapper.
    pub const SOCKET: usize = 41;
    /// `connect(sockfd, addr, addrlen)`.
    ///
    /// Source: Linux `arch/x86/include/generated/uapi/asm/unistd_64.h` line 42.
    /// See [`crate::sys::connect`] for the typed wrapper.
    pub const CONNECT: usize = 42;
    /// `accept(sockfd, addr, addrlen)`.
    ///
    /// Source: Linux `arch/x86/include/generated/uapi/asm/unistd_64.h` line 43.
    /// See [`crate::sys::accept`] for the typed wrapper.
    pub const ACCEPT: usize = 43;
    /// `sendto(2)` — socket send; with a NULL address it is `send(2)`.
    /// Used for stream writes so `MSG_NOSIGNAL` can suppress `SIGPIPE`.
    pub const SENDTO: usize = 44;
    /// `bind(sockfd, addr, addrlen)`.
    ///
    /// Source: Linux `arch/x86/include/generated/uapi/asm/unistd_64.h` line 49.
    /// See [`crate::sys::bind`] for the typed wrapper.
    pub const BIND: usize = 49;
    /// `listen(sockfd, backlog)`.
    ///
    /// Source: Linux `arch/x86/include/generated/uapi/asm/unistd_64.h` line 50.
    /// See [`crate::sys::listen`] for the typed wrapper.
    pub const LISTEN: usize = 50;

    // ---- device-control syscall --------------------------------------------

    /// `ioctl(fd, request, ...)` — used for termios `TCGETS`/`TCSETS`.
    ///
    /// Source: Linux `arch/x86/include/generated/uapi/asm/unistd_64.h` line 16.
    /// See [`crate::sys::ioctl`] for the typed wrapper.
    pub const IOCTL: usize = 16;

    // ---- UDS path unlink --------------------------------------------------

    /// `unlinkat(dirfd, path, flags)` — used to remove the UDS socket path on
    /// listener teardown.
    ///
    /// Source: Linux `arch/x86/include/generated/uapi/asm/unistd_64.h` line 263.
    /// See [`crate::sys::unlinkat`] for the typed wrapper.
    pub const UNLINKAT: usize = 263;
}

/// Issues a syscall with no arguments.
///
/// Returns the raw `rax` result; a value in `-4095..=-1` is a negated
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
/// use reovim_arch::sys::{syscall0, from_ret};
/// use reovim_arch::sys::raw::nr;
/// // SAFETY: GETTID takes no arguments and always succeeds.
/// let raw = unsafe { syscall0(nr::GETTID) };
/// assert!(from_ret(raw).is_ok());
/// ```
#[must_use]
pub unsafe fn syscall0(nr: usize) -> isize {
    let ret: isize;
    // SAFETY: the `syscall` instruction reads `nr` from rax and writes its
    // result to rax; it clobbers rcx and r11 per the x86_64 Linux ABI, both
    // declared lateout. memory is clobbered because a syscall may read or
    // write through any pointer argument. The caller upholds the per-syscall
    // contract for `nr` (this primitive takes no pointer arguments itself).
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

/// Issues a syscall with one argument.
///
/// Returns the raw `rax` result; a value in `-4095..=-1` is a negated
/// errno.
///
/// # Safety
///
/// The caller must ensure `nr`/`a1` form a valid call: any pointer-shaped
/// argument must reference memory live and correctly typed for the syscall.
///
/// ```ignore
/// // Prefer the typed wrappers in `crate::sys`. Direct use:
/// use reovim_arch::sys::{syscall1, from_ret};
/// use reovim_arch::sys::raw::nr;
/// // SAFETY: close(-1) — EBADF is a valid error outcome, no memory dereferenced.
/// let raw = unsafe { syscall1(nr::CLOSE, usize::MAX) }; // -1 as usize
/// assert!(from_ret(raw).is_err()); // EBADF
/// ```
#[must_use]
pub unsafe fn syscall1(nr: usize, a1: usize) -> isize {
    let ret: isize;
    // SAFETY: as `syscall0`; `a1` is passed in rdi per the ABI. The caller
    // upholds the syscall's argument contract (e.g. a valid pointer/length).
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a1,
            lateout("rcx") _,
            lateout("r11") _,
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
            "syscall",
            in("rax") nr,
            in("rdi") a1,
            options(noreturn, nostack),
        );
    }
}

/// Issues a syscall with two arguments.
///
/// Returns the raw `rax` result; a value in `-4095..=-1` is a negated
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
/// use reovim_arch::sys::{syscall2, from_ret};
/// use reovim_arch::sys::raw::nr;
/// // SAFETY: munmap(0, 0) — EINVAL for zero/misaligned address, no memory dereferenced.
/// let raw = unsafe { syscall2(nr::MUNMAP, 0, 0) };
/// assert!(from_ret(raw).is_err()); // EINVAL
/// ```
#[must_use]
pub unsafe fn syscall2(nr: usize, a1: usize, a2: usize) -> isize {
    let ret: isize;
    // SAFETY: as `syscall0`; arguments are passed in rdi, rsi per the ABI.
    // The caller upholds the syscall's argument contract.
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a1,
            in("rsi") a2,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

/// Issues a syscall with three arguments.
///
/// Returns the raw `rax` result; a value in `-4095..=-1` is a negated
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
/// use reovim_arch::sys::{syscall3, from_ret};
/// use reovim_arch::sys::raw::nr;
/// // SAFETY: close(-1) via 3-arg trampoline — EBADF, no memory dereferenced.
/// // (WRITE with fd=-1 gives EBADF, demonstrating a 3-arg call.)
/// let raw = unsafe { syscall3(nr::WRITE, usize::MAX, 0, 0) };
/// assert!(from_ret(raw).is_err()); // EBADF or EFAULT
/// ```
#[must_use]
pub unsafe fn syscall3(nr: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let ret: isize;
    // SAFETY: as `syscall0`; arguments are passed in rdi, rsi, rdx per the
    // ABI. The caller upholds the syscall's argument contract.
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

/// Issues a syscall with four arguments.
///
/// Returns the raw `rax` result; a value in `-4095..=-1` is a negated
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
/// use reovim_arch::sys::{syscall4, from_ret};
/// use reovim_arch::sys::raw::nr;
/// // SAFETY: openat with a null path pointer → EFAULT; demonstrates a 4-arg call.
/// let raw = unsafe { syscall4(nr::OPENAT, usize::MAX, 0, 0, 0) };
/// assert!(from_ret(raw).is_err());
/// ```
#[must_use]
pub unsafe fn syscall4(nr: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> isize {
    let ret: isize;
    // SAFETY: as `syscall0`; the fourth argument is passed in r10 (not rcx)
    // per the x86_64 Linux ABI. The caller upholds the argument contract.
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

/// Issues a syscall with six arguments.
///
/// Returns the raw `rax` result; a value in `-4095..=-1` is a negated
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
/// use reovim_arch::sys::{syscall6, from_ret};
/// use reovim_arch::sys::raw::nr;
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
    // SAFETY: as `syscall0`; arguments are passed in rdi, rsi, rdx, r10, r8,
    // r9 per the ABI. The caller upholds the syscall's argument contract.
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            in("r8") a5,
            in("r9") a6,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}
