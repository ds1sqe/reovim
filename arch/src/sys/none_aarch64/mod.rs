//! aarch64 bare-metal (freestanding) backend for `arch/src/sys/`.
//!
//! Targets `aarch64-unknown-none` on the Raspberry Pi 4 machine (BCM2711) —
//! QEMU `-M raspi4b` and real hardware. There is no kernel below this code:
//! the stable boundary is the hardware/firmware interface (the DAG6
//! freestanding class), so the floor wrappers are realized directly over
//! MMIO and system registers instead of syscalls:
//!
//! - byte I/O: the PL011 UART, polled (`uart`)
//! - clock: the ARM generic timer (`timer`)
//! - pages: a static bump arena (`arena`)
//! - exit: QEMU semihosting `SYS_EXIT` (`semihost`)
//!
//! Services the hardware does not provide fail through the floor's existing
//! error paths — `clone_into` (no thread floor) and `openat` (no
//! filesystem) return `Err` — so the wrapper-level floor surface is
//! identical to the Linux backends' and callers above `sys/` compile
//! unchanged. There is no `raw` module: raw syscalls are kernel-ABI-class
//! surface, and this target has no kernel ABI to expose.

mod arena;
mod boot_info;
pub mod console;
pub mod framebuffer;
mod semihost;
mod timer;
mod uart;
mod wrap;

// Runtime hardware discovery (RAM / CPU) assembled into a `BootInfo` and pushed
// into the kernel at entry. Freestanding-only — the Linux backends carry an
// empty default; the bare-metal payload discovers real facts.
pub use boot_info::collect_boot_info;

// The wrapper-level floor this backend owes `sys/mod.rs`, realized over
// hardware (Linux backends satisfy it from the shared `wrap.rs`).
pub use wrap::{
    AT_FDCWD, CLOCK_MONOTONIC, CLOCK_REALTIME, CLONE_CHILD_CLEARTID, CLONE_FILES, CLONE_FS,
    CLONE_PARENT_SETTID, CLONE_SIGHAND, CLONE_SYSVSEM, CLONE_THREAD, CLONE_VM, FUTEX_PRIVATE_FLAG,
    FUTEX_WAIT, FUTEX_WAKE, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, O_CLOEXEC, O_CREAT, O_RDONLY,
    O_TRUNC, O_WRONLY, PROT_NONE, PROT_READ, PROT_WRITE, Timespec, clock_gettime, close, exit,
    exit_group, futex, gettid, mmap, mprotect, munmap, openat, read, write,
};

use super::errno::{EAGAIN, Errno};

/// The spawn primitive, which this target cannot realize: there is no
/// kernel to schedule a second thread and the boot entry parks every
/// secondary core. Returns `EAGAIN` — the errno Linux `clone` reports at
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
pub const unsafe fn clone_into(
    _flags: usize,
    _stack: usize,
    _join_word: usize,
    _entry: usize,
) -> Result<usize, Errno> {
    Err(EAGAIN)
}
