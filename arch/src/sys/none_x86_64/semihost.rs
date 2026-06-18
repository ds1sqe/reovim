//! QEMU `isa-debug-exit` exit channel.
//!
//! QEMU's `isa-debug-exit` device (added with `-device isa-debug-exit`)
//! watches a small I/O port; a write to it makes the emulator exit with
//! status `(value << 1) | 1`, where `value` is the word written. This
//! backend wires the device at the conventional port `0xf4` with the
//! default 4-byte access size, so a 32-bit `out` carries the exit code —
//! which is how the harness's exit-code contract survives onto bare metal
//! (the harness inverts the `(code << 1) | 1` transform).
//!
//! Without the device present (real hardware, no QEMU) the `out` is a
//! discarded port write rather than an exit, so the function parks in the
//! halt loop below instead of returning.

use core::arch::asm;

/// `isa-debug-exit` I/O port (the conventional `iobase`).
const DEBUG_EXIT_PORT: u16 = 0xf4;

/// Reports `code` to the QEMU debug-exit device and never returns.
pub fn exit(code: i32) -> ! {
    // SAFETY: `out dx, eax` is the architected 32-bit port-write
    // instruction; `0xf4` is the `isa-debug-exit` device port of the target
    // machine and the write touches no Rust-managed memory. QEMU consumes
    // the value and terminates before control could ever resume.
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") DEBUG_EXIT_PORT,
            in("eax") code.cast_unsigned(),
            options(nomem, nostack, preserves_flags),
        );
    }
    // No debug-exit device took the write: there is nothing to return to on
    // bare metal.
    loop {
        // SAFETY: `hlt` only halts the core until the next interrupt; no
        // state is touched.
        unsafe {
            asm!("hlt", options(nomem, nostack));
        }
    }
}
