//! The floor's write-sink registry: a write-once `fn(&[u8])` callback the
//! floor `write` fans fd 1/2 to, in addition to the UART.
//!
//! The framebuffer console that used to be the on-screen sink lifted one tier
//! up into `reovim-system-kernel` (SP04 04a), so the floor `write` (down here)
//! can no longer name it — that would be a forbidden arch-sys-none →
//! system-kernel upward edge (invariant #2). This registry is the acyclic
//! decoupling: the boot composition root installs a system-kernel
//! console-renderer callback through [`install_write_sink`] at boot, and the
//! floor `write` calls it from below through [`fan_out`]. No Cargo edge reverses
//! — the same shape as the kabi/panic write-once atoms.
//!
//! A boot with no display (the exit-code selftest fixtures) never installs a
//! sink; [`fan_out`] is then a no-op and fd 1/2 stay UART-only.

use core::cell::UnsafeCell;

/// The on-screen write-sink callback the floor `write` fans fd 1/2 to.
type SinkFn = fn(&[u8]);

/// The write-once holder for the installed [`SinkFn`] callback.
struct WriteSink(UnsafeCell<Option<SinkFn>>);

// SAFETY: the freestanding floor is a single thread of control — no preemption
// and no interrupt-driven writers — so the cell is only ever reached from that
// one thread, exactly as the page arena's storage is (`arena.rs`).
unsafe impl Sync for WriteSink {}

/// The installed callback, if any. `None` until [`install_write_sink`] runs.
static SINK: WriteSink = WriteSink(UnsafeCell::new(None));

/// Installs `f` as the floor's on-screen write sink, **once**.
///
/// The boot composition root calls this at boot to register a system-kernel
/// console-renderer callback before the kernel's first `write(2,...)`; the
/// floor `write` then mirrors every fd 1/2 byte to `f` in addition to the UART.
/// The install is write-once: a second call after one has succeeded is ignored,
/// so an already-installed console is never silently displaced (mirroring the
/// kabi/panic write-once intent).
///
/// # Example
///
/// ```ignore
/// // A boot composition root registers the console callback at boot:
/// reovim_arch_sys_none_aarch64::install_write_sink(render_to_console);
/// ```
pub fn install_write_sink(f: fn(&[u8])) {
    // SAFETY: single thread of control; no live reference into the cell exists
    // across this read-then-store. The guard installs only into an empty slot,
    // so the store cannot race a prior install's reader either.
    unsafe {
        let slot = &mut *SINK.0.get();
        if slot.is_none() {
            *slot = Some(f);
        }
    }
}

/// Fans `buf` to the installed sink, if one is present; a no-op otherwise.
///
/// The floor `write` calls this for fd 1/2 after the UART write. With no sink
/// installed (a no-display boot) it does nothing, so UART-only boots are
/// unaffected.
// pub(crate): called by the sibling `wrap` module, not external API — keeping it
// crate-internal avoids over-exposing the floor's write mechanism (the `pub`
// form the nursery lint suggests would also trip the public-doctest gate).
#[allow(clippy::redundant_pub_crate)]
pub(crate) fn fan_out(buf: &[u8]) {
    // SAFETY: single thread of control; the copied `fn` pointer borrows nothing
    // from the cell, so the borrow ends before the call — the callback may
    // itself reach back through the floor `write` without aliasing this read.
    let installed = unsafe { *SINK.0.get() };
    if let Some(f) = installed {
        f(buf);
    }
}
