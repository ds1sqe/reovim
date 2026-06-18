//! Time-stamp-counter clock source.
//!
//! `RDTSC` reads the CPU's free-running 64-bit time-stamp counter — the
//! x86 equivalent of the ARM generic timer's `CNTPCT_EL0`. Its tick rate
//! is the invariant-TSC frequency, which (unlike `CNTFRQ_EL0`) is not a
//! single architected register: it is derived from CPUID leaf `0x15` (the
//! TSC/crystal ratio) when the firmware populates it, with CPUID leaf
//! `0x16` (the processor base frequency) as a fallback and a documented 1
//! GHz constant as the last resort, so the floor's clock realization is one
//! counter read plus a one-shot frequency probe.

use core::arch::asm;

/// 1 GHz fallback when neither CPUID leaf reports a usable frequency. A
/// concrete nonzero value keeps the `clock_gettime` division well-defined;
/// it makes the clock's tick-to-nanosecond scale approximate on such a CPU,
/// but never undefined.
const FALLBACK_HZ: u64 = 1_000_000_000;

/// Reads the free-running time-stamp counter.
#[must_use]
pub fn counter() -> u64 {
    let lo: u32;
    let hi: u32;
    // SAFETY: `rdtsc` reads the time-stamp counter into edx:eax; it touches
    // no memory and has no side effects beyond writing those registers.
    unsafe {
        asm!(
            "rdtsc",
            out("eax") lo,
            out("edx") hi,
            options(nomem, nostack, preserves_flags),
        );
    }
    (u64::from(hi) << 32) | u64::from(lo)
}

/// Executes `cpuid` for `leaf` (with sub-leaf 0) and returns
/// `(eax, ebx, ecx, edx)`.
///
/// `cpuid` clobbers all four of eax/ebx/ecx/edx, but the x86-64 LLVM
/// backend reserves rbx for its own use and rejects it as an operand, so
/// rbx is saved into the scratch register r11 around the instruction and
/// the result is moved back out — the standard freestanding `cpuid`
/// idiom.
fn cpuid(leaf: u32) -> (u32, u32, u32, u32) {
    let eax: u32;
    let ebx: u32;
    let ecx: u32;
    let edx: u32;
    // SAFETY: `cpuid` is a pure CPU-identification instruction; it touches
    // no memory and has no side effects beyond its four output registers.
    // rbx is preserved by routing it through r11, so the reserved register
    // is restored before the block returns.
    unsafe {
        asm!(
            "mov r11, rbx",
            "cpuid",
            "mov {ebx:e}, ebx",
            "mov rbx, r11",
            ebx = lateout(reg) ebx,
            inout("eax") leaf => eax,
            inout("ecx") 0u32 => ecx,
            lateout("edx") edx,
            lateout("r11") _,
            options(nomem, nostack, preserves_flags),
        );
    }
    (eax, ebx, ecx, edx)
}

/// Reads the time-stamp-counter frequency in Hz.
///
/// Tries CPUID leaf `0x15` first: with a known crystal frequency in `ecx`
/// and a nonzero TSC/crystal ratio (`ebx`/`eax`) the TSC frequency is
/// `ecx * ebx / eax`. Falls back to CPUID leaf `0x16`'s processor base
/// frequency in MHz, then to [`FALLBACK_HZ`]. Never returns zero, so the
/// `clock_gettime` division is always well-defined.
#[must_use]
pub fn frequency() -> u64 {
    // Leaf 0x15: eax = TSC/crystal denominator, ebx = numerator, ecx = crystal Hz.
    let (denominator, numerator, crystal_hz, _) = cpuid(0x15);
    // Leaf 0x16: eax = processor base frequency in MHz.
    let (base_mhz, _, _, _) = cpuid(0x16);
    freq_from_cpuid(denominator, numerator, crystal_hz, base_mhz)
}

/// Derives the TSC frequency in Hz from the CPUID leaf `0x15`/`0x16` values.
///
/// Split out from [`frequency`] (which performs the `cpuid` reads) so the
/// three-way selection — leaf-`0x15` ratio, leaf-`0x16` base frequency, then
/// the [`FALLBACK_HZ`] constant — is a pure function exercisable without a CPU.
/// Never returns zero: the constant fallback is the floor.
fn freq_from_cpuid(denominator: u32, numerator: u32, crystal_hz: u32, base_mhz: u32) -> u64 {
    if denominator != 0 && numerator != 0 && crystal_hz != 0 {
        return u64::from(crystal_hz) * u64::from(numerator) / u64::from(denominator);
    }
    if base_mhz != 0 {
        return u64::from(base_mhz) * 1_000_000;
    }
    FALLBACK_HZ
}

#[cfg(feature = "selftest")]
#[path = "timer_tests.rs"]
mod tests;
