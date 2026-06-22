//! Target-neutral up-face `BootInfo` shaping over lower boot facts.
//!
//! Raw register reads, firmware pointer walking, memory-map interpretation, and
//! target-specific defaults stay below this bridge in provider/composition
//! code. This module accepts already-neutral facts and assembles the up-face
//! [`BootInfo`] record consumed by upper diagnostics.

use {
    reovim_kabi_platform::{MemoryKind as KabiMemoryKind, MemoryRange as KabiMemoryRange},
    reovim_uapi_system::{BootInfo, MemorySummary},
};

/// Neutral boot facts gathered below the system-kernel bridge.
#[derive(Debug, Clone, Copy)]
pub struct BootFacts {
    /// Lower memory-map ranges already normalized into down-face memory kinds.
    pub memory: &'static [KabiMemoryRange],
    /// CPU clock frequency in Hz, or `0` when unknown.
    pub cpu_freq_hz: u64,
    /// Target-native CPU identification value, or `0` when unknown.
    pub cpu_id: u32,
    /// Logical CPU count, or `0` when unknown.
    pub cpu_count: u32,
    /// Minimum cache line size in bytes, or `0` when unknown.
    pub cache_line_bytes: u32,
    /// L1 data cache size in bytes, or `0` when unknown.
    pub l1d_bytes: u32,
    /// L1 instruction cache size in bytes, or `0` when unknown.
    pub l1i_bytes: u32,
    /// Unified L2 cache size in bytes, or `0` when unknown.
    pub l2_bytes: u32,
    /// Target-native CPU affinity value, or `0` when unknown.
    pub cpu_affinity: u64,
    /// Memory clock frequency in Hz, or `0` when unknown.
    pub mem_freq_hz: u64,
    /// Total heap/page-arena capacity in bytes, or `0` when unknown.
    pub heap_total_bytes: u64,
}

impl Default for BootFacts {
    fn default() -> Self {
        Self {
            memory: &[],
            cpu_freq_hz: 0,
            cpu_id: 0,
            cpu_count: 0,
            cache_line_bytes: 0,
            l1d_bytes: 0,
            l1i_bytes: 0,
            l2_bytes: 0,
            cpu_affinity: 0,
            mem_freq_hz: 0,
            heap_total_bytes: 0,
        }
    }
}

/// Assembles caller-supplied neutral boot facts into [`BootInfo`].
#[must_use]
pub fn collect_boot_info(facts: BootFacts) -> BootInfo {
    BootInfo {
        memory: memory_summary(facts.memory),
        cpu_freq_hz: facts.cpu_freq_hz,
        cpu_id: facts.cpu_id,
        cpu_count: facts.cpu_count,
        cache_line_bytes: facts.cache_line_bytes,
        l1d_bytes: facts.l1d_bytes,
        l1i_bytes: facts.l1i_bytes,
        l2_bytes: facts.l2_bytes,
        cpu_affinity: facts.cpu_affinity,
        mem_freq_hz: facts.mem_freq_hz,
        heap_total_bytes: facts.heap_total_bytes,
    }
}

fn memory_summary(memory: &[KabiMemoryRange]) -> MemorySummary {
    let usable = memory
        .iter()
        .filter(|range| matches!(range.kind, KabiMemoryKind::Usable))
        .map(|range| range.len)
        .sum();
    MemorySummary::new(memory.len(), usable)
}

// Bridge-level smoke: declared here so `super::` reaches `collect_boot_info`;
// reaches the test runtime through the reovim-testrt leaf.
#[cfg(feature = "selftest")]
#[path = "boot_info_tests.rs"]
mod tests;
