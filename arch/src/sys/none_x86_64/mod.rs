//! x86-64 bare-metal (freestanding) backend for `arch/src/sys/`.
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
//! identical to the Linux backends' and callers above `sys/` compile
//! unchanged. There is no `raw` module: raw syscalls are kernel-ABI-class
//! surface, and this target has no kernel ABI to expose.

mod arena;
mod semihost;
mod timer;
mod uart;
mod wrap;

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
