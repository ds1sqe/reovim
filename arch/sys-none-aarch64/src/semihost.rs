//! QEMU semihosting exit channel.
//!
//! Semihosting `SYS_EXIT` (operation `0x18`) reports guest termination to a
//! semihosting host. A64 encodes the call as `hlt #0xF000` with the
//! operation number in `w0` and a parameter-block address in `x1`; for
//! `SYS_EXIT` the block is `{ADP_Stopped_ApplicationExit, exit_code}`, and
//! QEMU started with `-semihosting` terminates the emulator with that code —
//! which is how the harness's exit-code contract survives onto bare metal.
//!
//! Without a semihosting host (real hardware, no debugger) `hlt` raises an
//! exception instead of returning; real-hardware runs signal completion over
//! the serial end-marker and park in the halt loop below if the trap is not
//! taken.

use core::arch::asm;

/// Semihosting operation number for `SYS_EXIT`.
const SYS_EXIT: u32 = 0x18;
/// `ADP_Stopped_ApplicationExit` — the "normal application exit" reason
/// code the host expects in the parameter block's first field.
const ADP_STOPPED_APPLICATION_EXIT: u64 = 0x20026;

/// Reports `code` to the semihosting host and never returns.
pub fn exit(code: i32) -> ! {
    let block: [u64; 2] = [
        ADP_STOPPED_APPLICATION_EXIT,
        u64::from(code.cast_unsigned()),
    ];
    // SAFETY: `hlt #0xF000` is the architected A64 semihosting trap; `w0`
    // and `x1` carry the documented operation/parameter-block contract, and
    // `block` is live across the instruction (the host consumes it before
    // control could ever resume).
    unsafe {
        asm!(
            "hlt #0xf000",
            in("w0") SYS_EXIT,
            in("x1") block.as_ptr(),
            options(nostack, preserves_flags),
        );
    }
    // No host took the trap: there is nothing to return to on bare metal.
    loop {
        // SAFETY: `wfe` only waits for an event; no state is touched.
        unsafe {
            asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}
