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
pub mod errno;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod semihost;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod timer;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod uart;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod wrap;

// The sibling `*_tests.rs` files (`arena_tests.rs`, `errno_tests.rs`,
// `timer_tests.rs`) are declared next to their source modules (the `#[path]`
// child pattern), reaching the test runtime through the `reovim-testrt` leaf
// rather than an upward arch-sys -> arch edge (SP07). The `boot_info_tests.rs`
// moved up with the whole boot-info assembly into `reovim-system-kernel`
// (SP04 04a).

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
/// `_start` asm in `reovim-arch-floor-none-x86-64` stashes it here in the
/// 32-bit prologue (after the BSS clear, before the COM1 banner loop reuses
/// `rbx`/`bl`) via a cross-crate
/// `sym reovim_arch_sys_none_x86_64::MULTIBOOT_INFO_PTR` operand (SP03 floor
/// split). The system kernel's x86 boot-info assembly reads it through the
/// [`multiboot_ptr`] accessor (the boot-info parse lifted up in SP04 04a) to
/// parse the firmware memory map. Zero until stashed, and on any non-Multiboot
/// entry, which the reader treats as "no boot info" (empty map).
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub static MULTIBOOT_INFO_PTR: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);

// ---- raw-fact provider surface for the system kernel's boot-info assembly ----
//
// The device-neutral `BootInfo` assembly (the Multiboot1 mmap parser, the
// memory-type classifier, the neutral-struct mapping) lifted into
// `reovim-system-kernel` (SP04 04a). What STAYS here is the raw mechanism it
// reads through: the boot-stashed Multiboot pointer ([`MULTIBOOT_INFO_PTR`]),
// the static arena the parsed map is backed by, and the `CPUID`/`RDTSC` asm —
// the system kernel reaches them via these crate-root accessors (the §11
// system-kernel → arch-sys-none impl edge) and does the parse + field mapping.

/// The Multiboot1 information-structure pointer stashed by `_start`, or `0` on a
/// non-Multiboot entry. The system kernel parses the firmware memory map from
/// the structure this points at.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[must_use]
pub fn multiboot_ptr() -> u32 {
    MULTIBOOT_INFO_PTR.load(core::sync::atomic::Ordering::Relaxed)
}

/// The processor identifier from `CPUID` leaf 1 (eax — family / model / stepping
/// / type), the NATIVE value the boot-info assembly maps to `BootInfo.cpu_id`.
///
/// The `cpuid` asm stays below; only the leaf-1 eax fact crosses the seam.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[must_use]
pub fn cpu_id_native() -> u32 {
    timer::cpuid(1).0
}

/// The time-stamp-counter frequency in Hz (the `BootInfo.cpu_freq_hz` fact),
/// derived from the `CPUID` leaf `0x15`/`0x16` reads that stay below.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use timer::frequency as timer_frequency;

/// Hands out `len` bytes (rounded up to whole pages), page-aligned, from the
/// static boot arena.
///
/// The boot-info assembly backs its parsed `'static` `MemoryRange` map through
/// this accessor; the arena (raw `.bss` bump) stays below.
///
/// # Errors
///
/// `ENOMEM` when the arena cannot fit the request.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub fn arena_alloc_pages(len: usize) -> Result<usize, Errno> {
    arena::alloc_pages(len)
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
