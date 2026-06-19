//! Tests for `reader.rs`, compiled into the lib under the `selftest` feature.
//!
//! L12 layout: declared in `reader.rs` via
//! `#[cfg(feature = "selftest")] #[path = "reader_tests.rs"] mod tests;`, so
//! `super::` reaches the private reader types and helpers.
//!
//! The happy-path tests embed a real BCM2711 fixture DTB and assert exact
//! decoded values. The error tests use small hand-crafted blobs to verify that
//! malformed input returns typed errors rather than panicking.

use {
    super::{Fdt, FdtError},
    crate::{arch_test, testrt},
};

// ── Fixture ───────────────────────────────────────────────────────────────────

/// Minimal BCM2711 device tree, original to this repo. Covers the nodes the
/// Phase 2 enumerator will consume.
static FIXTURE: &[u8] = include_bytes!("testdata/reovim-bcm2711.dtb");

// ── Header tests ─────────────────────────────────────────────────────────────

arch_test!(fdt_header_magic_and_version, {
    let fdt = Fdt::parse(FIXTURE).unwrap_or_else(|e| panic!("parse: {e:?}"));
    testrt::check_eq(fdt.version(), 17u32);
});

// ── /memory reg (integration smoke: header → struct-walk → reg decode) ────────
//
// Root declares #address-cells=2, #size-cells=1.  memory@0 has:
//   reg = <0x00 0x00 0x40000000>   →   base=0x0, len=0x40000000.

arch_test!(fdt_memory_reg_decodes_correctly, {
    let fdt = Fdt::parse(FIXTURE).unwrap_or_else(|e| panic!("parse: {e:?}"));
    let root = fdt.root().unwrap_or_else(|e| panic!("root: {e:?}"));
    let mem = root
        .find_child("memory")
        .unwrap_or_else(|| panic!("no memory node"));
    let (base, len) = mem.reg().unwrap_or_else(|| panic!("memory: no reg"));
    testrt::check_eq(base, 0x0000_0000_0000_0000u64);
    testrt::check_eq(len, 0x4000_0000u64);
    // Confirm device_type while we're here.
    let dt = mem
        .prop_bytes("device_type")
        .unwrap_or_else(|| panic!("no device_type"));
    // NUL-terminated "memory\0"
    testrt::check(dt.starts_with(b"memory"), "device_type starts with 'memory'");
});

// ── /soc/serial@7e201000 (proves /soc #address-cells=1 #size-cells=1) ─────────

arch_test!(fdt_serial_compatible_and_reg, {
    let fdt = Fdt::parse(FIXTURE).unwrap_or_else(|e| panic!("parse: {e:?}"));
    let root = fdt.root().unwrap_or_else(|e| panic!("root: {e:?}"));
    let soc = root
        .find_child("soc")
        .unwrap_or_else(|| panic!("no soc node"));
    let serial = soc
        .find_child_exact("serial@7e201000")
        .unwrap_or_else(|| panic!("no serial@7e201000"));
    // First compatible string must be "arm,pl011".
    let compat = serial
        .first_compatible()
        .unwrap_or_else(|| panic!("serial: no compatible"));
    testrt::check(compat == "arm,pl011", "serial compatible is arm,pl011");
    // /soc declares #address-cells=1 #size-cells=1, so reg = <0x7e201000 0x200>.
    let (base, len) = serial.reg().unwrap_or_else(|| panic!("serial: no reg"));
    testrt::check_eq(base, 0x7e20_1000u64);
    testrt::check_eq(len, 0x200u64);
});

// ── /soc/mailbox@7e00b880 ─────────────────────────────────────────────────────

arch_test!(fdt_mailbox_compatible_and_reg, {
    let fdt = Fdt::parse(FIXTURE).unwrap_or_else(|e| panic!("parse: {e:?}"));
    let root = fdt.root().unwrap_or_else(|e| panic!("root: {e:?}"));
    let soc = root
        .find_child("soc")
        .unwrap_or_else(|| panic!("no soc node"));
    let mbox = soc
        .find_child_exact("mailbox@7e00b880")
        .unwrap_or_else(|| panic!("no mailbox@7e00b880"));
    let compat = mbox
        .first_compatible()
        .unwrap_or_else(|| panic!("mailbox: no compatible"));
    testrt::check(compat == "brcm,bcm2835-mbox", "mailbox compatible");
    let (base, len) = mbox.reg().unwrap_or_else(|| panic!("mailbox: no reg"));
    testrt::check_eq(base, 0x7e00_b880u64);
    testrt::check_eq(len, 0x40u64);
});

// ── Error cases ───────────────────────────────────────────────────────────────

// A 40-byte blob with the wrong magic word.
static BAD_MAGIC: [u8; 40] = {
    let mut b = [0u8; 40];
    // Magic bytes (big-endian): 0xdeadbeef instead of 0xd00dfeed.
    b[0] = 0xde;
    b[1] = 0xad;
    b[2] = 0xbe;
    b[3] = 0xef;
    // totalsize = 40 (just the header, so it won't trip Truncated first).
    b[4] = 0x00;
    b[5] = 0x00;
    b[6] = 0x00;
    b[7] = 0x28;
    b
};

arch_test!(fdt_bad_magic_returns_error, {
    let result = Fdt::parse(&BAD_MAGIC);
    testrt::check(matches!(result, Err(FdtError::BadMagic)), "bad magic → FdtError::BadMagic");
});

// A slice with only 8 bytes — shorter than the 40-byte minimum header.
static TOO_SHORT: [u8; 8] = [0xd0, 0x0d, 0xfe, 0xed, 0x00, 0x00, 0x00, 0x28];

arch_test!(fdt_truncated_header_returns_error, {
    let result = Fdt::parse(&TOO_SHORT);
    testrt::check(matches!(result, Err(FdtError::Truncated)), "short slice → FdtError::Truncated");
});

// A valid-magic header whose off_dt_struct points well past the end of the
// input slice.  totalsize = 64 but the input is only 40 bytes, so Truncated
// fires before we even inspect offsets — use totalsize=40 and push the struct
// offset past 40.
static OOB_STRUCT_OFFSET: [u8; 40] = {
    let mut b = [0u8; 40];
    // Magic: 0xd00dfeed
    b[0] = 0xd0;
    b[1] = 0x0d;
    b[2] = 0xfe;
    b[3] = 0xed;
    // totalsize = 40 (exactly the header; keeps the truncated check happy).
    b[4] = 0x00;
    b[5] = 0x00;
    b[6] = 0x00;
    b[7] = 0x28;
    // off_dt_struct = 0x0000_1000 — past the input.
    b[8] = 0x00;
    b[9] = 0x00;
    b[10] = 0x10;
    b[11] = 0x00;
    // off_dt_strings = 0x0000_1100 — also past the input.
    b[12] = 0x00;
    b[13] = 0x00;
    b[14] = 0x11;
    b[15] = 0x00;
    // off_mem_rsvmap = 0x28
    b[16] = 0x00;
    b[17] = 0x00;
    b[18] = 0x00;
    b[19] = 0x28;
    // version = 17
    b[20] = 0x00;
    b[21] = 0x00;
    b[22] = 0x00;
    b[23] = 0x11;
    // last_comp_version = 16
    b[24] = 0x00;
    b[25] = 0x00;
    b[26] = 0x00;
    b[27] = 0x10;
    // boot_cpuid_phys = 0
    // size_dt_strings = 0x10 (arbitrary small value)
    b[32] = 0x00;
    b[33] = 0x00;
    b[34] = 0x00;
    b[35] = 0x10;
    // size_dt_struct = 0x10 (arbitrary small value)
    b[36] = 0x00;
    b[37] = 0x00;
    b[38] = 0x00;
    b[39] = 0x10;
    b
};

arch_test!(fdt_oob_struct_offset_returns_error, {
    let result = Fdt::parse(&OOB_STRUCT_OFFSET);
    // The struct offset (0x1000) + size (0x10) exceeds totalsize (40), so the
    // parser must return OutOfBounds (not panic).
    testrt::check(
        matches!(result, Err(FdtError::OutOfBounds)),
        "out-of-bounds struct offset → FdtError::OutOfBounds",
    );
});
