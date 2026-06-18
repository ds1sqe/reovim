//! Runtime hardware discovery for the aarch64 bare-metal floor.
//!
//! The kernel is platform-neutral and learns the machine's facts only from the
//! [`BootInfo`] the floor hands it at entry. This module assembles that struct
//! from what the BCM2711 exposes at runtime:
//!
//! - RAM: the `VideoCore` mailbox "get ARM memory" tag (`framebuffer::arm_memory`).
//! - CPU clock: `CNTFRQ_EL0`, the generic-timer frequency (`timer::frequency`).
//! - CPU model: `MIDR_EL1`, the main-id register.
//!
//! Nothing here is compiled in or assumed — every value is read from hardware,
//! and a query that fails degrades to "unknown" (an empty map / zeroed field)
//! rather than reporting a wrong number.

use core::arch::asm;

use reovim_kabi_platform::{BootInfo, MemoryKind, MemoryRange};

use super::{arena, framebuffer, timer};

/// Reads `MIDR_EL1`, the CPU main-id register.
fn midr() -> u64 {
    let value: u64;
    // SAFETY: MIDR_EL1 is a read-only identification register readable at EL1
    // on this target; the read touches no memory and has no side effects.
    unsafe {
        asm!(
            "mrs {value}, midr_el1",
            value = out(reg) value,
            options(nomem, nostack, preserves_flags),
        );
    }
    value
}

/// Queries the mailbox for the ARM RAM region and stores it as a one-element
/// `'static` memory map backed by the boot arena.
///
/// Returns an empty map if the query fails or the arena cannot back it — the
/// kernel then reports memory as unknown rather than reporting a wrong range.
fn discover_memory() -> &'static [MemoryRange] {
    let Some((base, len)) = framebuffer::arm_memory() else {
        return &[];
    };
    let range = MemoryRange {
        base,
        len,
        kind: MemoryKind::Usable,
    };

    let Ok(addr) = arena::alloc_pages(core::mem::size_of::<MemoryRange>()) else {
        return &[];
    };
    let ptr = addr as *mut MemoryRange;
    // SAFETY: `addr` is a page-aligned hand-out from the static `ARENA`, whose
    // lifetime equals the process — the monotonic bump allocator never frees or
    // relocates it, so the `'static` lifetime on the returned slice is honest.
    // Page alignment (4096) exceeds `align_of::<MemoryRange>()`, and the
    // hand-out reserved a whole page, so writing one element and reading it back
    // as a one-element slice both stay within owned storage the bump never hands
    // out twice (no aliasing).
    unsafe {
        ptr.write(range);
        core::slice::from_raw_parts(ptr, 1)
    }
}

/// Discovers the machine's hardware facts and assembles them into a [`BootInfo`]
/// for `Init::new`.
///
/// All facts are read from hardware at call time; the floor parks every
/// secondary core at boot, so the CPU count is the single-core constant `1`.
#[must_use]
pub fn collect_boot_info() -> BootInfo {
    // MIDR_EL1's low 32 bits carry the implementer / variant / architecture /
    // part-num / revision fields; bits 63:32 are architecturally RES0, so
    // narrowing to the `cpu_id: u32` contract loses nothing.
    #[allow(clippy::cast_possible_truncation)]
    let cpu_id = midr() as u32;
    BootInfo {
        memory: discover_memory(),
        cpu_freq_hz: timer::frequency(),
        cpu_id,
        cpu_count: 1,
    }
}
