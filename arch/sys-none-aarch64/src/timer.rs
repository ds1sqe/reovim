//! ARM generic-timer clock source.
//!
//! `CNTPCT_EL0` is the architecture's free-running monotonic counter and
//! `CNTFRQ_EL0` its firmware-programmed frequency (54 MHz on the Pi 4).
//! Both are readable at EL1 with no further setup on the target machine —
//! QEMU's `raspi4b` model and real BCM2711 firmware both leave the counter
//! accessible — so the floor's clock realization is two register reads.

use core::arch::asm;

/// Reads the free-running counter.
///
/// The `isb` is the architecturally required barrier before a counter read:
/// without it the CPU may satisfy the `mrs` speculatively ahead of program
/// order, letting a later call observe an earlier counter value.
#[must_use]
pub fn counter() -> u64 {
    let value: u64;
    // SAFETY: `cntpct_el0` is a read-only system register accessible at EL1;
    // the read touches no memory and has no side effects.
    unsafe {
        asm!(
            "isb",
            "mrs {value}, cntpct_el0",
            value = out(reg) value,
            options(nomem, nostack, preserves_flags),
        );
    }
    value
}

/// Reads the counter frequency in Hz.
#[must_use]
pub fn frequency() -> u64 {
    let value: u64;
    // SAFETY: `cntfrq_el0` is a read-only system register accessible at EL1;
    // the read touches no memory and has no side effects.
    unsafe {
        asm!(
            "mrs {value}, cntfrq_el0",
            value = out(reg) value,
            options(nomem, nostack, preserves_flags),
        );
    }
    value
}
