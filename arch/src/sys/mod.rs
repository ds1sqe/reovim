//! Platform seam: `sys/` is the single boundary between target-neutral Rust
//! and per-target asm.
//!
//! Backends are cfg-selected per target with identical floor signatures; all
//! target asm lives in `sys/<target>/`. `errno.rs` and `wrap.rs` are
//! Linux-kernel-ABI family code shared by Linux backends only — freestanding
//! backends bring their own wrap-level implementations behind the same floor
//! signatures.

// ---- backend selection -------------------------------------------------------

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux_x86_64;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use linux_x86_64 as target;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod linux_aarch64;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use linux_aarch64 as target;

#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
)))]
compile_error!(
    "no arch sys backend for this target; see arch/src/sys/ for the per-target backend convention"
);

// ---- shared Linux-family modules ---------------------------------------------

mod errno;
pub mod net;
pub mod term;
mod wrap;

#[cfg(feature = "selftest")]
mod errno_tests;
#[cfg(feature = "selftest")]
mod net_tests;
#[cfg(feature = "selftest")]
mod term_tests;
#[cfg(feature = "selftest")]
mod wrap_tests;

// ---- floor surface re-exports ------------------------------------------------

/// The fused clone asm primitive: issues `clone` and in the child branches
/// straight to `entry` on the new stack. Target-internal; only `thread`
/// consumes it.
pub(crate) use target::clone_into;

pub use {
    errno::{EAGAIN, EBADF, EFAULT, EINVAL, ENOENT, ENOMEM, EWOULDBLOCK, Errno, from_ret},
    target::raw::{syscall0, syscall1, syscall2, syscall3, syscall4, syscall6},
    wrap::{
        AT_FDCWD, AT_REMOVEDIR, CLOCK_MONOTONIC, CLOCK_REALTIME, CLONE_CHILD_CLEARTID, CLONE_FILES,
        CLONE_FS, CLONE_PARENT_SETTID, CLONE_SIGHAND, CLONE_SYSVSEM, CLONE_THREAD, CLONE_VM,
        FUTEX_PRIVATE_FLAG, FUTEX_WAIT, FUTEX_WAKE, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE,
        MSG_NOSIGNAL, O_CLOEXEC, O_CREAT, O_RDONLY, O_TRUNC, O_WRONLY, PROT_NONE, PROT_READ,
        PROT_WRITE, Timespec, accept, bind, clock_gettime, close, connect, exit, exit_group, futex,
        gettid, ioctl, listen, mmap, mprotect, munmap, openat, read, send_nosignal, socket,
        unix_stream_socket, unlinkat, write,
    },
};

// Errno constants used by the net/term wrappers — keep them in the same
// re-export tier as the existing errno constants.
pub use errno::{EADDRINUSE, ECONNREFUSED, ENOTTY, EOPNOTSUPP, EPIPE};
