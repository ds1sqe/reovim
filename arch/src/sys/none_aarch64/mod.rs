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
pub mod color;
pub mod console;
pub mod escape;
mod fdt;
pub mod fonts;
pub mod framebuffer;
mod semihost;
mod timer;
mod uart;
mod wrap;

// Runtime hardware discovery (RAM / CPU) assembled into a `BootInfo` and pushed
// into the kernel at entry. Freestanding-only — the Linux backends carry an
// empty default; the bare-metal payload discovers real facts.
pub use boot_info::collect_boot_info;

/// The firmware DTB physical address captured by `_start` at boot, or `0` when
/// no DTB was passed. Reads the atomic cell the boot asm stashes
/// ([`crate::start::DTB_PTR`]) before calling Rust entry.
#[cfg(feature = "runtime")]
fn dtb_ptr() -> u64 {
    crate::start::DTB_PTR.load(core::sync::atomic::Ordering::Relaxed)
}

/// Returns `0` on non-runtime configurations (hosted builds, tests without
/// the boot-asm path). The caller treats a zero pointer as "no DTB".
#[cfg(not(feature = "runtime"))]
fn dtb_ptr() -> u64 {
    0
}

/// Reads the firmware DTB captured at boot and enumerates its devices into a
/// one-shot static [`reovim_kabi_platform::DeviceInventory`] pushed into the
/// kernel at entry.
///
/// Returns the empty default when:
/// - no DTB was passed (e.g. QEMU without `-dtb`),
/// - the pointer is zero or the FDT header is unreadable / oversized, or
/// - the FDT parse fails.
///
/// The returned inventory is backed by the boot arena and valid for the
/// process lifetime.
#[must_use]
pub fn collect_device_inventory() -> reovim_kabi_platform::DeviceInventory {
    const MAX_DTB_BYTES: usize = 1 << 20; // 1 MiB
    let p = dtb_ptr();
    if p == 0 {
        return reovim_kabi_platform::DeviceInventory::default();
    }
    let ptr = p as *const u8;
    // Read totalsize from the FDT header (big-endian u32 at byte offset 4) to
    // bound the slice before handing it to the parser.
    // SAFETY: the firmware DTB lives in identity-mapped RAM for the whole
    // process; the 8-byte header read is within it.
    let total = unsafe {
        let hdr = core::slice::from_raw_parts(ptr, 8);
        u32::from_be_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize
    };
    // Reject absurd sizes before forming the full slice. A valid FDT header is
    // at least 40 bytes; we cap well above any real DTB but within mapped RAM.
    if !(40..=MAX_DTB_BYTES).contains(&total) {
        return reovim_kabi_platform::DeviceInventory::default();
    }
    // SAFETY: as above — `total` bytes from the DTB base are within the
    // process-lifetime, identity-mapped firmware region. The `'static` lifetime
    // is honest: the firmware RAM is never unmapped and we never write through
    // this pointer. The enumerator's `compatible: &'static str` slices point
    // into this same byte range and inherit the same lifetime guarantee.
    let dtb: &'static [u8] = unsafe { core::slice::from_raw_parts(ptr, total) };
    let Ok(fdt) = fdt::reader::Fdt::parse(dtb) else {
        return reovim_kabi_platform::DeviceInventory::default();
    };
    fdt::enumerate::enumerate(&fdt)
}

// On-target integration smoke for `collect_device_inventory` (04 Phase 2/3).
// Runs in the aarch64 `arch-selftest` QEMU `raspi4b` pilot against the real
// firmware DTB supplied via `-dtb`.
#[cfg(feature = "selftest")]
#[path = "inventory_smoke_tests.rs"]
mod inventory_smoke_tests;

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
