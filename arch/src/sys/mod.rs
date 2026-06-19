//! Platform seam facade: `arch::sys` re-exports the raw per-target mechanism
//! from the `arch-sys-{target}` crate selected by the same cfg gates the old
//! in-crate backend selection used.
//!
//! The raw mechanism (syscalls, MMIO, asm, errno vocabulary, the freestanding
//! console/framebuffer/font surface) moved out of `reovim-arch` into per-target
//! `reovim-arch-sys-*` crates (SP01 carve-out). This module keeps the exact
//! `arch::sys::*` public surface so no consumer — product or internal — changes
//! an import: every name below was previously re-exported from
//! `arch/src/sys/mod.rs`, now sourced from the per-target crate via a `pub use`.
//!
//! Each `arch-sys-{target}` is a downward `arch -> arch-sys` Cargo edge (a
//! target-cfg-gated path dep in `arch/Cargo.toml`); there is no upward edge.
//! The boot-pointer statics (`DTB_PTR`, `MULTIBOOT_INFO_PTR`) live in the
//! `arch-sys-none-*` crates and the `_start` asm in the matching
//! `arch-floor-none-{aarch64,x86-64}` crate names them by cross-crate `sym`
//! path (SP03 floor split).

// ---- backend selection (per-target arch-sys crate) ---------------------------

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_sys_linux_x86_64 as target;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_sys_linux_aarch64 as target;

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use reovim_arch_sys_none_aarch64 as target;

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use reovim_arch_sys_none_x86_64 as target;

// The VideoCore mailbox framebuffer is freestanding-only hardware surface
// (no Linux backend has one), exposed so the bare-metal payloads can drive it.
// Same target gate as the backend it lives in. The `Framebuffer` type STAYS in
// the raw-mechanism crate (the SP04 Q2 seam) — the bootcore / splash fixtures
// still reach it through this facade by value.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use target::framebuffer;

// The device-neutral console / fonts / color device model + the boot-info /
// device-inventory assembly lifted out of arch-sys-none into
// `reovim-system-kernel` (SP04 04a). The fixtures name those through the system
// kernel directly (the §11 system-kernel → arch-sys-none impl edge); the facade
// no longer re-exports them. The full neutral per-target seam replacement is
// SP06; this narrowing tracks the device-neutral surface leaving arch-sys-none.

#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "none", target_arch = "aarch64"),
    all(target_os = "none", target_arch = "x86_64"),
)))]
compile_error!(
    "no arch sys backend for this target; see the arch/sys-{target}/ crates for the per-target backend convention"
);

// ---- sockets / termios kernel-ABI surface ------------------------------------

// Sockets and termios are kernel-ABI surface (DAG6 1.2 §10): Linux-only,
// like `wrap.rs` — freestanding backends have neither.
#[cfg(target_os = "linux")]
pub use target::net;
#[cfg(target_os = "linux")]
pub use target::term;

// ---- floor surface re-exports ------------------------------------------------

/// The fused clone asm primitive: issues `clone` and in the child branches
/// straight to `entry` on the new stack. `arch::thread` consumes it in-crate,
/// and the `platform-linux-native` provider names it through this facade for the
/// detached-thread spawn adapter (SP02), so it is `pub` on the floor surface.
pub use target::clone_into;

pub use target::{EAGAIN, EBADF, EFAULT, EINVAL, ENOENT, ENOMEM, EWOULDBLOCK, Errno, from_ret};

// Raw syscalls are kernel-ABI-class surface (DAG6 1.2 §10): only targets
// whose stable boundary IS the Linux syscall ABI may expose them.
#[cfg(target_os = "linux")]
pub use target::{syscall0, syscall1, syscall2, syscall3, syscall4, syscall6};

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
pub use target::{
    AT_REMOVEDIR, MSG_NOSIGNAL, accept, bind, connect, ioctl, listen, send_nosignal, socket,
    unix_stream_socket, unlinkat,
};

// Errno constants used by the net/term wrappers — keep them in the same
// re-export tier as the existing errno constants.
pub use target::{EADDRINUSE, ECONNREFUSED, ENOTTY, EOPNOTSUPP, EPIPE};
