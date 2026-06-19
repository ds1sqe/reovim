//! The floor's write-sink registry: a write-once `fn(&[u8])` callback the
//! floor `write` fans fd 1/2 to, in addition to the UART.
//!
//! Symmetric with the aarch64 backend's registry (SP04 04b): the system kernel
//! installs a write-sink trampoline *downward* through [`install_write_sink`],
//! and the floor `write` calls it *from below* through [`fan_out`], so no
//! reverse arch-sys-none → system-kernel edge exists (invariant #2). x86-none
//! has no framebuffer console, so at runtime the sink stays `None` and fd 1/2
//! are UART-only; the registry exists so the floor `write` path is uniform
//! across both freestanding arches and any future install just works.

use core::cell::UnsafeCell;

/// The write-sink callback the floor `write` fans fd 1/2 to.
type SinkFn = fn(&[u8]);

/// The write-once holder for the installed [`SinkFn`] callback.
struct WriteSink(UnsafeCell<Option<SinkFn>>);

// SAFETY: the freestanding floor is a single thread of control — no preemption
// and no interrupt-driven writers — so the cell is only ever reached from that
// one thread, exactly as the page arena's storage is (`arena.rs`).
unsafe impl Sync for WriteSink {}

/// The installed callback, if any. `None` until [`install_write_sink`] runs;
/// stays `None` on x86-none, which has no display.
static SINK: WriteSink = WriteSink(UnsafeCell::new(None));

/// Installs `f` as the floor's write sink, **once**.
///
/// A second call after one has succeeded is ignored, so an already-installed
/// sink is never silently displaced (mirroring the kabi/panic write-once
/// intent). x86-none has no display and installs no sink at runtime, but the
/// entry point exists so the registry is symmetric with the aarch64 backend.
///
/// # Example
///
/// ```ignore
/// reovim_arch_sys_none_x86_64::install_write_sink(render_to_console);
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
/// The floor `write` calls this for fd 1/2 after the UART write. On x86-none the
/// sink is `None`, so this does nothing and fd 1/2 stay UART-only.
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
