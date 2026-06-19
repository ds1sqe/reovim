//! x86-64 bare-metal raw mechanism (`reovim-arch-sys-none-x86-64`).
//!
//! Targets `x86_64-unknown-none` under QEMU (the `q35`/`pc` machines).
//! There is no kernel below this code: the stable boundary is the
//! hardware/firmware interface (the DAG6 freestanding class), so the floor
//! wrappers are realized directly over port I/O and CPU instructions
//! instead of syscalls:
//!
//! - byte I/O: the 16550 COM1 UART, polled (`uart`)
//! - clock: the time-stamp counter (`timer`)
//! - pages: a static bump arena (`arena`)
//! - exit: QEMU's `isa-debug-exit` device (`semihost`)
//!
//! Services the hardware does not provide fail through the floor's existing
//! error paths — `clone_into` (no thread floor) and `openat` (no
//! filesystem) return `Err` — so the wrapper-level floor surface is
//! identical to the Linux backends'. There is no `raw` module: raw syscalls
//! are kernel-ABI-class surface, and this target has no kernel ABI to expose.
//! `reovim-arch` re-exports this crate's surface through its `sys` facade so
//! consumers compile unchanged.

#![no_std]
#![allow(unsafe_code)]

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod arena;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod boot_info;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub mod errno;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod semihost;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod timer;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod uart;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod wrap;

// The sibling `*_tests.rs` files (`arena_tests.rs`, `boot_info_tests.rs`,
// `errno_tests.rs`, `timer_tests.rs`) are declared next to their source modules
// (the `#[path]` child pattern), reaching the test runtime through the
// `reovim-testrt` leaf rather than an upward arch-sys -> arch edge (SP07).

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use errno::{
    EADDRINUSE, EAGAIN, EBADF, ECONNREFUSED, EFAULT, EINVAL, ENOENT, ENOMEM, ENOTTY, EOPNOTSUPP,
    EPIPE, EWOULDBLOCK, Errno, from_ret,
};

/// The Multiboot1 information-structure pointer the bootloader leaves in `EBX`
/// at entry.
///
/// Relocated into this raw-mechanism crate (SP01 edge inversion): the boot
/// pointer is a raw hardware fact and belongs with the machine layer. The
/// `arch::start` `_start` asm stashes it here in the 32-bit prologue (after the
/// BSS clear, before the COM1 banner loop reuses `rbx`/`bl`) via a cross-crate
/// `sym reovim_arch_sys_none_x86_64::MULTIBOOT_INFO_PTR` operand. The x86
/// boot-info provider ([`boot_info::collect_boot_info`]) reads it to parse the
/// firmware memory map. Zero until stashed, and on any non-Multiboot entry,
/// which the reader treats as "no boot info" (empty map).
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub static MULTIBOOT_INFO_PTR: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);

// Runtime hardware discovery (RAM / CPU) assembled into a `BootInfo` and pushed
// into the kernel at entry. Freestanding-only — the Linux backends carry an
// empty default; the bare-metal payload discovers real facts. x86 reads the
// firmware memory map from the Multiboot1 structure stashed by `_start`.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use boot_info::collect_boot_info;

/// x86 freestanding has no device tree; the device inventory is always empty.
///
/// Platform-ABI mirror of `none_aarch64::collect_device_inventory` so the
/// `sys/mod.rs` re-export table is symmetric across both freestanding arches.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[must_use]
pub fn collect_device_inventory() -> reovim_kabi_platform::DeviceInventory {
    reovim_kabi_platform::DeviceInventory::default()
}

// The wrapper-level floor this backend owes `sys/mod.rs`, realized over
// hardware (Linux backends satisfy it from the shared `wrap.rs`).
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use wrap::{
    AT_FDCWD, CLOCK_MONOTONIC, CLOCK_REALTIME, CLONE_CHILD_CLEARTID, CLONE_FILES, CLONE_FS,
    CLONE_PARENT_SETTID, CLONE_SIGHAND, CLONE_SYSVSEM, CLONE_THREAD, CLONE_VM, FUTEX_PRIVATE_FLAG,
    FUTEX_WAIT, FUTEX_WAKE, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, O_CLOEXEC, O_CREAT, O_RDONLY,
    O_TRUNC, O_WRONLY, PROT_NONE, PROT_READ, PROT_WRITE, Timespec, clock_gettime, close, exit,
    exit_group, futex, gettid, mmap, mprotect, munmap, openat, read, write,
};

/// The spawn primitive, which this target cannot realize.
///
/// There is no kernel to schedule a second thread and the boot entry parks
/// every secondary core. Returns `EAGAIN` — the errno Linux `clone` reports at
/// resource exhaustion — so the thread module's established spawn-failure
/// teardown runs unchanged.
///
/// # Errors
///
/// Always `EAGAIN`.
///
/// # Safety
///
/// Never dereferences its arguments. The contract is nominally the Linux
/// backends' (`stack` is a valid stack top with the trampoline argument at
/// `[stack]`, `join_word` is live, `entry` matches the argument's type) so
/// callers are identical across targets.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub const unsafe fn clone_into(
    _flags: usize,
    _stack: usize,
    _join_word: usize,
    _entry: usize,
) -> Result<usize, Errno> {
    Err(EAGAIN)
}
