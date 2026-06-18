//! Tests for `timer.rs`, compiled into the lib under `selftest`.
//!
//! L12 layout: declared in `timer.rs` via
//! `#[cfg(feature = "selftest")] #[path = "timer_tests.rs"] mod tests;`, so
//! `super::` reaches the private frequency-derivation helper and the fallback
//! constant.

use {
    super::{FALLBACK_HZ, freq_from_cpuid, frequency},
    crate::{arch_test, testrt},
};

arch_test!(timer_freq_leaf15_computes_crystal_ratio, {
    // Leaf 0x15 complete: freq = crystal(ecx) * numerator(ebx) / denominator(eax).
    // 100 MHz crystal * 24 / 2 = 1.2 GHz.
    testrt::check_eq(freq_from_cpuid(2, 24, 100_000_000, 0), 1_200_000_000u64);
});

arch_test!(timer_freq_leaf16_used_when_leaf15_incomplete, {
    // Leaf 0x15 reports no usable ratio, so leaf 0x16's base MHz is scaled to Hz.
    testrt::check_eq(freq_from_cpuid(0, 0, 0, 2400), 2_400_000_000u64);
});

arch_test!(timer_freq_constant_fallback_when_no_leaf, {
    // Neither leaf usable: the documented constant fallback, never zero.
    testrt::check_eq(freq_from_cpuid(0, 0, 0, 0), FALLBACK_HZ);
});

arch_test!(timer_frequency_is_never_zero, {
    // Whatever this QEMU CPU reports, the live probe yields a usable divisor
    // for clock_gettime (the constant fallback guarantees it).
    testrt::check(frequency() != 0, "frequency() must never be zero");
});
