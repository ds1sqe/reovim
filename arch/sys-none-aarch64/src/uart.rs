//! PL011 UART byte output, polled.
//!
//! The BCM2711 (Pi 4) maps the primary PL011 at physical `0xFE20_1000` in
//! low-peripheral mode; QEMU's `raspi4b` machine emulates the same layout.
//! Boot firmware (and QEMU's `-serial`) leaves UART0 usable, so the floor
//! needs only the data path: poll the flag register until the transmit FIFO
//! has room, then store the byte to the data register. No init sequence, no
//! interrupts, no RX — the selftest payload's output channel is the whole
//! requirement.

use core::ptr::{read_volatile, write_volatile};

/// PL011 UART0 base on BCM2711 (low-peripheral mode).
const UART0_BASE: usize = 0xFE20_1000;
/// `UARTDR` — data register offset.
const DR: usize = 0x00;
/// `UARTFR` — flag register offset.
const FR: usize = 0x18;
/// `UARTFR` bit 5: transmit FIFO full.
const FR_TXFF: u32 = 1 << 5;

/// Writes every byte of `buf` to the UART, polling for FIFO space per byte.
///
/// Polled MMIO cannot fail: the registers are always present on the target
/// machine, so unlike the fd-based `write` path there is no error to report.
pub fn write_bytes(buf: &[u8]) {
    for &byte in buf {
        // SAFETY: `UART0_BASE + FR`/`+ DR` are the PL011 MMIO registers of
        // the target machine (BCM2711 / QEMU raspi4b). Volatile accesses of
        // the architected register width are the defined way to reach device
        // registers, and no Rust-managed memory is aliased by these
        // addresses.
        unsafe {
            while read_volatile((UART0_BASE + FR) as *const u32) & FR_TXFF != 0 {
                core::hint::spin_loop();
            }
            write_volatile((UART0_BASE + DR) as *mut u32, u32::from(byte));
        }
    }
}
