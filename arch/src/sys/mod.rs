//! Platform seam: `sys/` is the single boundary between target-neutral Rust
//! and per-target asm.
//!
//! Backends are cfg-selected per target with identical floor signatures; all
//! target asm lives in `sys/<target>/`. `errno.rs` is target-neutral floor
//! vocabulary — the `Errno` type and value mapping every backend's wrappers
//! return. `wrap.rs` is Linux-kernel-ABI family code shared by Linux backends
//! only — freestanding backends bring their own wrap-level implementations
//! behind the same floor signatures. Each backend re-exports the wrapper
//! floor it owes; the single `pub use target::{…}` list below is the drift
//! guard every backend must satisfy.

// ---- backend selection -------------------------------------------------------

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux_x86_64;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use linux_x86_64 as target;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod linux_aarch64;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use linux_aarch64 as target;

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod none_aarch64;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use none_aarch64 as target;

// The VideoCore mailbox framebuffer is freestanding-only hardware surface
// (no Linux backend has one), exposed so the bare-metal splash payload can
// drive it. Same target gate as the backend it lives in.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use none_aarch64::framebuffer;

// The coverage-blended text console layered over the framebuffer, so the
// bare-metal boot log renders on the HDMI surface and not only the UART, plus
// the selectable embedded fonts it blits through. Same target gate as the
// framebuffer they draw on.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use none_aarch64::console;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use none_aarch64::fonts;
// The terminal color model the console pen resolves through (truecolor +
// indexed palette). Same target gate as the console that consumes it.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use none_aarch64::color;

#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "none", target_arch = "aarch64"),
)))]
compile_error!(
    "no arch sys backend for this target; see arch/src/sys/ for the per-target backend convention"
);

// ---- target-neutral floor vocabulary ------------------------------------------

mod errno;

#[cfg(feature = "selftest")]
mod errno_tests;

// ---- shared Linux-family wrappers ----------------------------------------------

#[cfg(target_os = "linux")]
mod wrap;

#[cfg(all(target_os = "linux", feature = "selftest"))]
mod wrap_tests;

// Sockets and termios are kernel-ABI surface (DAG6 1.2 §10): Linux-only,
// like `wrap.rs` — freestanding backends have neither.
#[cfg(target_os = "linux")]
pub mod net;
#[cfg(target_os = "linux")]
pub mod term;

#[cfg(all(target_os = "linux", feature = "selftest"))]
mod net_tests;
#[cfg(all(target_os = "linux", feature = "selftest"))]
mod term_tests;

// ---- floor surface re-exports ------------------------------------------------

/// The fused clone asm primitive: issues `clone` and in the child branches
/// straight to `entry` on the new stack. Target-internal; only `thread`
/// consumes it.
pub(crate) use target::clone_into;

pub use errno::{EAGAIN, EBADF, EFAULT, EINVAL, ENOENT, ENOMEM, EWOULDBLOCK, Errno, from_ret};

// Raw syscalls are kernel-ABI-class surface (DAG6 1.2 §10): only targets
// whose stable boundary IS the Linux syscall ABI may expose them.
#[cfg(target_os = "linux")]
pub use target::raw::{syscall0, syscall1, syscall2, syscall3, syscall4, syscall6};

pub use target::{
    AT_FDCWD, CLOCK_MONOTONIC, CLOCK_REALTIME, CLONE_CHILD_CLEARTID, CLONE_FILES, CLONE_FS,
    CLONE_PARENT_SETTID, CLONE_SIGHAND, CLONE_SYSVSEM, CLONE_THREAD, CLONE_VM, FUTEX_PRIVATE_FLAG,
    FUTEX_WAIT, FUTEX_WAKE, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, O_CLOEXEC, O_CREAT, O_RDONLY,
    O_TRUNC, O_WRONLY, PROT_NONE, PROT_READ, PROT_WRITE, Timespec, clock_gettime, close, exit,
    exit_group, futex, gettid, mmap, mprotect, munmap, openat, read, write,
};

// Socket/termios/unlink wrappers are Linux-only kernel-ABI surface, same
// class gate as `wrap.rs` itself (no freestanding realization exists).
#[cfg(target_os = "linux")]
pub use wrap::{
    AT_REMOVEDIR, MSG_NOSIGNAL, accept, bind, connect, ioctl, listen, send_nosignal, socket,
    unix_stream_socket, unlinkat,
};

// Errno constants used by the net/term wrappers — keep them in the same
// re-export tier as the existing errno constants.
pub use errno::{EADDRINUSE, ECONNREFUSED, ENOTTY, EOPNOTSUPP, EPIPE};
