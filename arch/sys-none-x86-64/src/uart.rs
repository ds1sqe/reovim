//! 16550 COM1 UART byte output, polled.
//!
//! The PC platform maps the first serial port (COM1) at I/O port base
//! `0x3F8`; QEMU's `q35`/`pc` machines emulate a 16550-compatible device
//! there and route it to `-serial`. The arch-owned `_start` programs the baud
//! divisor and line/FIFO control in asm before any Rust runs (the boot banner
//! needs COM1 up before the runtime exists), so this data path mirrors the
//! Pi's PL011 surface exactly: poll the line-status register until the
//! transmit holding register is empty before writes, and poll until receive
//! data is ready before reads. No init and no interrupts.

use core::arch::asm;

/// COM1 I/O port base.
const COM1_BASE: u16 = 0x3F8;
/// Transmit holding register (DLAB=0): the byte sink.
const THR: u16 = COM1_BASE;
/// Receiver buffer register (DLAB=0): the byte source.
const RBR: u16 = COM1_BASE;
/// Line status register; bit 5 (`0x20`) is "transmit holding register empty".
const LSR: u16 = COM1_BASE + 5;
/// `LSR` bit 0: receive data ready.
const LSR_DR: u8 = 1;
/// `LSR` bit 5: the transmit holding register is empty (room for a byte).
const LSR_THRE: u8 = 1 << 5;

/// Reads a byte from I/O port `port`.
#[inline]
fn inb(port: u16) -> u8 {
    let val: u8;
    // SAFETY: `in al, dx` is the architected port-read instruction; the COM1
    // ports are device registers of the target machine and touch no
    // Rust-managed memory.
    unsafe {
        asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    val
}

/// Writes byte `val` to I/O port `port`.
#[inline]
fn outb(port: u16, val: u8) {
    // SAFETY: `out dx, al` is the architected port-write instruction; the
    // COM1 ports are device registers of the target machine and touch no
    // Rust-managed memory.
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
    }
}

/// Writes every byte of `buf` to the UART, polling for THR space per byte.
///
/// Polled port I/O cannot fail: the registers are always present on the
/// target machine, so unlike the fd-based `write` path there is no error to
/// report.
pub fn write_bytes(buf: &[u8]) {
    for &byte in buf {
        while inb(LSR) & LSR_THRE == 0 {
            core::hint::spin_loop();
        }
        outb(THR, byte);
    }
}

/// Reads one byte from COM1 when a byte is already pending.
#[must_use]
pub fn try_read_byte() -> Option<u8> {
    if inb(LSR) & LSR_DR == 0 {
        return None;
    }
    Some(inb(RBR))
}

/// Blocks until one byte is received from COM1.
#[must_use]
pub fn read_byte() -> u8 {
    loop {
        if let Some(byte) = try_read_byte() {
            return byte;
        }
        core::hint::spin_loop();
    }
}
