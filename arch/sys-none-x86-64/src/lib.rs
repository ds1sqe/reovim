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
// The write-once `fn(&[u8])` write-sink registry, symmetric with the aarch64
// backend (SP04 04b): the floor `write` fans fd 1/2 to an installed callback
// (in addition to the UART). x86-none has no console, so the sink stays `None`
// at runtime — the registry exists for a uniform floor `write` path across both
// freestanding arches.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod sink;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod timer;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod uart;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
mod wrap;

// The sibling `*_tests.rs` files are declared next to their source modules (the
// `#[path]` child pattern), reaching the test runtime through the
// `reovim-testrt` leaf rather than an upward arch-sys -> arch edge (SP07).

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use errno::{
    EADDRINUSE, EAGAIN, EBADF, ECONNREFUSED, EFAULT, EINVAL, ENOENT, ENOMEM, ENOTTY, EOPNOTSUPP,
    EPIPE, EWOULDBLOCK, Errno, from_ret,
};

/// Installs a `fn(&[u8])` callback as the floor's write sink (once).
///
/// Symmetric with the aarch64 backend's §11 impl-edge accessor; x86-none has no
/// display, so nothing installs a sink at runtime, but the entry point exists so
/// the registry surface is uniform across both freestanding arches.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use sink::install_write_sink;

/// Reads one pending stdin byte without blocking.
///
/// OS-mode composition roots use this to keep the root console input loop
/// shaped like the aarch64 USB-keyboard/UART merge path.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use uart::try_read_byte as try_read_stdin_byte;

/// The Multiboot1 information-structure pointer the bootloader leaves in `EBX`
/// at entry.
///
/// Relocated into this raw-mechanism crate (SP01 edge inversion): the boot
/// pointer is a raw hardware fact and belongs with the machine layer. The
/// `_start` asm in `reovim-arch-floor-none-x86-64` stashes it here in the
/// 32-bit prologue (after the BSS clear, before the COM1 banner loop reuses
/// `rbx`/`bl`) via a cross-crate
/// `sym reovim_arch_sys_none_x86_64::MULTIBOOT_INFO_PTR` operand (SP03 floor
/// split). Target-side boot-info parsing reads it through the
/// [`multiboot_ptr`] accessor before the composition root hands neutral memory
/// facts to the system-kernel bridge. Zero until stashed, and on any
/// non-Multiboot entry, which the reader treats as "no boot info" (empty map).
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub static MULTIBOOT_INFO_PTR: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);

// ---- boot-fact provider surface for the boot composition root ----------------
//
// The Multiboot1 pointer walk, E820 memory-type interpretation, and CPU/timer
// raw mechanism STAY here (invariant #4, asm confinement); boot composition
// roots gather neutral facts through these crate-root accessors and pass them
// into the system-kernel bridge.

/// Empty `MemoryRange` value for static storage initialization.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use boot_info::EMPTY_MEMORY_RANGE;

/// Default number of `MemoryRange` slots a composition root should reserve for
/// Multiboot/E820 parsing.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use boot_info::MEMORY_STORAGE_ENTRIES;

/// Neutral memory-map entry type used by the parsed Multiboot/E820 map.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use boot_info::MemoryRange;

/// Parses the bootloader memory map into caller-owned static storage.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use boot_info::discover_memory;

/// The Multiboot1 information-structure pointer stashed by `_start`, or `0` on
/// a non-Multiboot entry.
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

/// The neutral CPU identifier fact for `BootInfo`.
///
/// ```rust,ignore
/// // Target-only: reads CPUID leaf 1 through the x86_64 floor.
/// let cpu = reovim_arch_sys_none_x86_64::cpu_id();
/// let _stepping = cpu & 0xf;
/// ```
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[must_use]
pub fn cpu_id() -> u32 {
    cpu_id_native()
}

/// The time-stamp-counter frequency in Hz (the `BootInfo.cpu_freq_hz` fact),
/// derived from the `CPUID` leaf `0x15`/`0x16` reads that stay below.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use timer::frequency as timer_frequency;

/// Hands out `len` bytes (rounded up to whole pages), page-aligned, from the
/// static boot arena.
///
/// Boot composition roots may allocate process-lifetime storage through this
/// accessor; the arena itself (raw `.bss` bump mechanism) stays below.
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
