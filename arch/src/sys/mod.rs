//! x86_64-linux raw syscall layer — the only platform-specific module.
//!
//! This module hardcodes the x86_64-linux syscall ABI behind a single
//! boundary (rule of three: no multi-platform abstraction is built until a
//! second target is a real consumer; `arch/src/sys/` is the seam). It
//! exposes raw `syscall0..syscall6` primitives over the `syscall`
//! instruction, an [`Errno`] newtype with the kernel's negative-return
//! mapping, and thin typed wrappers returning `Result<usize, Errno>`.
//!
//! The syscall set is the minimal floor (1.2 §10 charter smoke): boot a
//! process, allocate, thread, time, log, and exit. New syscalls are added
//! when a concrete consumer needs them, never speculatively.

mod errno;
mod raw;
mod wrap;

#[cfg(feature = "selftest")]
mod errno_tests;
#[cfg(feature = "selftest")]
mod wrap_tests;

/// The raw syscall-number table, crate-internal: the typed wrappers are the
/// public seam, but fused-asm consumers (the thread module's `clone`) need
/// the number itself from the single inventory.
pub(crate) use raw::nr;

pub use {
    errno::{EAGAIN, EBADF, EFAULT, EINVAL, ENOENT, ENOMEM, EWOULDBLOCK, Errno, from_ret},
    raw::{syscall0, syscall1, syscall2, syscall3, syscall4, syscall6},
    wrap::{
        AT_FDCWD, CLOCK_MONOTONIC, CLOCK_REALTIME, CLONE_CHILD_CLEARTID, CLONE_FILES, CLONE_FS,
        CLONE_PARENT_SETTID, CLONE_SIGHAND, CLONE_SYSVSEM, CLONE_THREAD, CLONE_VM,
        FUTEX_PRIVATE_FLAG, FUTEX_WAIT, FUTEX_WAKE, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE,
        O_CLOEXEC, O_CREAT, O_RDONLY, O_TRUNC, O_WRONLY, PROT_NONE, PROT_READ, PROT_WRITE,
        Timespec, clock_gettime, close, exit, exit_group, futex, gettid, mmap, mprotect, munmap,
        openat, read, write,
    },
};
