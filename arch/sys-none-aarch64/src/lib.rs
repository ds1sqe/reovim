//! aarch64 bare-metal raw mechanism (`reovim-arch-sys-none-aarch64`).
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
//! identical to the Linux backends'. There is no `raw` module: raw syscalls
//! are kernel-ABI-class surface, and this target has no kernel ABI to expose.
//! `reovim-arch` re-exports this crate's surface through its `sys` facade so
//! consumers compile unchanged.

#![no_std]
#![allow(unsafe_code)]

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod arena;
// Raw hardware-discovery facts (asm register reads + the mailbox/arena-backed
// memory map). The device-neutral `BootInfo` assembly that decodes them lifted
// into `reovim-system-kernel` (SP04 04a); the raw mechanism stays here.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod boot_info;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub mod errno;
// The VideoCore mailbox framebuffer (raw MMIO) STAYS; the `Framebuffer` type is
// the Q2 seam the system-kernel console consumes by value (SP04 04a). The
// console / escape parser / color model / fonts that rendered through it lifted
// into `reovim-system-kernel` along with the FDT reader/enumerator.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub mod framebuffer;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod semihost;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod timer;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod uart;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod wrap;

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use errno::{
    EADDRINUSE, EAGAIN, EBADF, ECONNREFUSED, EFAULT, EINVAL, ENOENT, ENOMEM, ENOTTY, EOPNOTSUPP,
    EPIPE, EWOULDBLOCK, Errno, from_ret,
};

/// The firmware-provided flattened-device-tree (DTB) physical address.
///
/// Relocated into this raw-mechanism crate (SP01 edge inversion): the boot
/// pointer is a raw hardware fact and belongs with the machine layer. The
/// `_start` asm in `reovim-arch-floor-none-aarch64` stashes the
/// firmware-provided value here via a cross-crate
/// `sym reovim_arch_sys_none_aarch64::DTB_PTR` operand, after the BSS clear
/// and before calling Rust entry (SP03 floor split). The system kernel's
/// device-inventory assembly reads it through the [`dtb_ptr`] accessor (the FDT
/// parse + enumerate lifted up in SP04 04a) to walk the DTB. Zero until stashed,
/// and on any entry with no DTB (QEMU without `-dtb`), which the reader treats as
/// "no device tree".
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub static DTB_PTR: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

// ---- raw-fact provider surface for the system kernel's boot-info assembly ----
//
// The device-neutral `BootInfo` assembly lifted into `reovim-system-kernel`
// (SP04 04a). The asm register reads + the mailbox/arena raw mechanism it
// decodes STAY here (invariant #4, asm confinement); the system kernel reads
// them through these crate-root accessors (the §11 system-kernel → arch-sys-none
// impl edge) and maps them into the neutral struct.

/// Reads `CLIDR_EL1`, the cache-level-id register (NATIVE u64).
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use boot_info::clidr;
/// Reads `CTR_EL0`, the cache-type register (NATIVE u64).
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use boot_info::ctr;
/// Queries the `VideoCore` mailbox for the ARM RAM region and stores it as a
/// `'static` arena-backed memory map (raw MMIO + arena, both stay below).
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use boot_info::discover_memory;
/// Reads `MIDR_EL1`, the CPU main-id register (NATIVE u64).
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use boot_info::midr;
/// Reads `MPIDR_EL1`, the multiprocessor-affinity register (NATIVE u64).
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use boot_info::mpidr;
/// Selects and reads the `CCSIDR_EL1` cache-size register for one cache (the raw
/// `asm!` selector write + read); the system kernel decodes the returned value.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use boot_info::read_ccsidr;

/// Reads the generic-timer frequency (`CNTFRQ_EL0`) in Hz — the
/// `BootInfo.timer_freq_hz` fact the system kernel reads across the impl edge.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use timer::frequency as timer_frequency;

/// Queries the `VideoCore` mailbox for the SDRAM clock rate in Hz, or `None`
/// when the firmware does not report it.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use framebuffer::sdram_clock_hz;

/// Total capacity of the static page arena in bytes (the heap-capacity fact the
/// boot-info assembly reports).
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use arena::capacity as arena_capacity;

/// Hands out `len` bytes (rounded up to whole pages), page-aligned, from the
/// static boot arena.
///
/// The device enumerator (lifted into the system kernel) allocates its
/// `'static` `DeviceEntry` storage through this accessor; the arena itself
/// (raw `.bss` bump mechanism) stays below.
///
/// # Errors
///
/// `ENOMEM` when the arena cannot fit the request.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub fn arena_alloc_pages(len: usize) -> Result<usize, Errno> {
    arena::alloc_pages(len)
}

/// The firmware DTB physical address captured by `_start` at boot, or `0` when
/// no DTB was passed. Reads the atomic cell the boot asm stashes
/// ([`DTB_PTR`]) before calling Rust entry.
///
/// Public so the system kernel's device-inventory assembly reads the boot fact
/// through an accessor rather than naming the [`DTB_PTR`] static across the
/// upward impl edge.
#[cfg(all(target_os = "none", target_arch = "aarch64", feature = "runtime"))]
#[must_use]
pub fn dtb_ptr() -> u64 {
    DTB_PTR.load(core::sync::atomic::Ordering::Relaxed)
}

/// Returns `0` on non-runtime configurations (hosted builds, tests without
/// the boot-asm path). The caller treats a zero pointer as "no DTB".
#[cfg(all(target_os = "none", target_arch = "aarch64", not(feature = "runtime")))]
#[must_use]
pub fn dtb_ptr() -> u64 {
    0
}

// The wrapper-level floor this backend owes the facade, realized over
// hardware (Linux backends satisfy it from the shared `wrap.rs`).
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
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
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub const unsafe fn clone_into(
    _flags: usize,
    _stack: usize,
    _join_word: usize,
    _entry: usize,
) -> Result<usize, Errno> {
    Err(EAGAIN)
}
