//! Tests for `boot_info_x86.rs`, compiled into the lib under `selftest`.
//!
//! L12 layout: declared in `boot_info_x86.rs` via
//! `#[cfg(feature = "selftest")] #[path = "boot_info_x86_tests.rs"] mod tests;`,
//! so `super::` reaches the migrated mmap parser + the type classifier; the
//! boot-stashed Multiboot pointer is read through the floor's accessor.
//!
//! Two tiers: the `parse_*`/`mmap_*`/`kind_*` cases drive the parser over
//! synthetic byte blobs — deterministic, no hardware. The `boot_info_*` cases
//! run on the real QEMU machine to prove the stash + live discovery end to end
//! through the floor's raw-fact accessors.

use {
    super::{FLAG_MMAP, collect_boot_info, kind_of, mmap_region, parse_entries},
    reovim_arch_sys_none_x86_64 as backend,
    reovim_kabi_platform::{MemoryKind, MemoryRange},
    reovim_testrt::{self as testrt, arch_test},
};

/// Writes one Multiboot1 mmap entry (`size` excludes itself; stride = size + 4)
/// into a 24-byte window.
fn write_entry(buf: &mut [u8], size: u32, base: u64, len: u64, ty: u32) {
    buf[0..4].copy_from_slice(&size.to_le_bytes());
    buf[4..12].copy_from_slice(&base.to_le_bytes());
    buf[12..20].copy_from_slice(&len.to_le_bytes());
    buf[20..24].copy_from_slice(&ty.to_le_bytes());
}

/// A zeroed `MemoryRange` to initialize fixed-size output arrays.
fn blank() -> MemoryRange {
    MemoryRange {
        base: 0,
        len: 0,
        kind: MemoryKind::Reserved,
    }
}

arch_test!(boot_info_parse_two_entries, {
    // Two standard 20-byte entries: usable low RAM + reserved high range.
    let mut buf = [0u8; 48];
    write_entry(&mut buf[0..24], 20, 0x0, 0x0009_F000, 1);
    write_entry(&mut buf[24..48], 20, 0x10_0000, 0x0700_0000, 2);
    let mut out = [blank(); 8];
    // SAFETY: `buf` is a readable 48-byte blob owned by this frame.
    let n = unsafe { parse_entries(buf.as_ptr(), buf.len(), &mut out) };
    testrt::check_eq(n, 2usize);
    testrt::check_eq(out[0].base, 0u64);
    testrt::check_eq(out[0].len, 0x0009_F000u64);
    testrt::check(out[0].kind == MemoryKind::Usable, "type 1 -> Usable");
    testrt::check_eq(out[1].base, 0x10_0000u64);
    testrt::check(out[1].kind == MemoryKind::Reserved, "type 2 -> Reserved");
});

arch_test!(boot_info_parse_kind_mapping, {
    // type 3 -> Acpi, type 5 -> Unusable (the non-Usable/Reserved arms).
    let mut buf = [0u8; 48];
    write_entry(&mut buf[0..24], 20, 0x0, 0x1000, 3);
    write_entry(&mut buf[24..48], 20, 0x2000, 0x1000, 5);
    let mut out = [blank(); 4];
    // SAFETY: readable 48-byte blob owned by this frame.
    let n = unsafe { parse_entries(buf.as_ptr(), buf.len(), &mut out) };
    testrt::check_eq(n, 2usize);
    testrt::check(out[0].kind == MemoryKind::Acpi, "type 3 -> Acpi");
    testrt::check(out[1].kind == MemoryKind::Unusable, "type 5 -> Unusable");
});

arch_test!(boot_info_parse_counts_without_writing, {
    // Empty `out`: the first sizing pass counts every entry, writes none.
    let mut buf = [0u8; 48];
    write_entry(&mut buf[0..24], 20, 0x0, 0x1000, 1);
    write_entry(&mut buf[24..48], 20, 0x2000, 0x1000, 1);
    // SAFETY: readable 48-byte blob owned by this frame.
    let n = unsafe { parse_entries(buf.as_ptr(), buf.len(), &mut []) };
    testrt::check_eq(n, 2usize);
});

arch_test!(boot_info_parse_short_buffer_is_empty, {
    // Fewer than one full entry's bytes: zero ranges, no out-of-bounds read.
    let buf = [0u8; 10];
    let mut out = [blank(); 4];
    // SAFETY: readable 10-byte blob owned by this frame.
    let n = unsafe { parse_entries(buf.as_ptr(), buf.len(), &mut out) };
    testrt::check_eq(n, 0usize);
});

arch_test!(boot_info_parse_zero_size_halts, {
    // A zero-size entry would be a non-advancing stride; the walk must stop
    // after the first valid entry rather than spin.
    let mut buf = [0u8; 48];
    write_entry(&mut buf[0..24], 20, 0x0, 0x1000, 1);
    write_entry(&mut buf[24..48], 0, 0x2000, 0x1000, 1); // size = 0 (malformed)
    let mut out = [blank(); 4];
    // SAFETY: readable 48-byte blob owned by this frame.
    let n = unsafe { parse_entries(buf.as_ptr(), buf.len(), &mut out) };
    testrt::check_eq(n, 1usize);
});

arch_test!(boot_info_mmap_region_reads_header, {
    // A header with the mmap flag set yields (addr, len); the flag clear, or a
    // null addr, yields None.
    let mut hdr = [0u8; 52];
    hdr[0..4].copy_from_slice(&FLAG_MMAP.to_le_bytes()); // flags: bit 6 set
    hdr[44..48].copy_from_slice(&48u32.to_le_bytes()); // mmap_length
    hdr[48..52].copy_from_slice(&0x1000u32.to_le_bytes()); // mmap_addr
    // SAFETY: `hdr` is a readable 52-byte structure owned by this frame.
    let region = unsafe { mmap_region(hdr.as_ptr()) };
    testrt::check(region == Some((0x1000usize, 48usize)), "flag set -> region");

    hdr[0..4].copy_from_slice(&0u32.to_le_bytes()); // clear the mmap flag
    // SAFETY: as above.
    let none = unsafe { mmap_region(hdr.as_ptr()) };
    testrt::check(none.is_none(), "flag clear -> None");
});

arch_test!(boot_info_kind_of_maps_all, {
    testrt::check(kind_of(1) == MemoryKind::Usable, "1 -> Usable");
    testrt::check(kind_of(2) == MemoryKind::Reserved, "2 -> Reserved");
    testrt::check(kind_of(3) == MemoryKind::Acpi, "3 -> Acpi");
    testrt::check(kind_of(5) == MemoryKind::Unusable, "5 -> Unusable");
    testrt::check(kind_of(99) == MemoryKind::Reserved, "unknown -> Reserved");
});

arch_test!(boot_info_multiboot_ptr_stashed, {
    // End-to-end: QEMU's Multiboot1 loader passes a non-null info pointer in
    // EBX, which `_start` must have stashed (survives the long-mode climb).
    testrt::check(backend::multiboot_ptr() != 0, "multiboot ptr stashed by _start");
});

arch_test!(boot_info_discovers_real_ram, {
    // End-to-end on the real q35 machine: the live mmap parses into >=1 usable
    // range with nonzero total, and CPUID reports a processor id.
    let bi = collect_boot_info();
    testrt::check(!bi.memory.is_empty(), "real mmap parsed >=1 range");
    let usable: u64 = bi
        .memory
        .iter()
        .filter(|r| r.kind == MemoryKind::Usable)
        .map(|r| r.len)
        .sum();
    testrt::check(usable > 0, "nonzero usable RAM");
    testrt::check(bi.cpu_id != 0, "cpu_id from CPUID leaf 1");
    testrt::check_eq(bi.cpu_count, 1u32);
});
