//! arch's implementation of the down-face platform contract.
//!
//! `arch` is the platform provider: this module builds a `static`
//! [`PlatformVtable`] from arch's real primitives (`time`, `alloc`, `sync`/
//! `thread` futex) and installs it write-once at boot (AB12). This is the
//! `arch → kabi` edge — arch implementing the contract `kabi` declares.
//!
//! ## SP02 scope: clock wired, the rest declared
//!
//! Only the `clock` slot is consumed end-to-end in SP02 (the boot selftest
//! reads it through the installed handle). The `alloc`/`dealloc`/`park`/
//! `unpark` slots are real — they point at arch's genuine backends — but no
//! caller routes through them yet; SP03 (`lib/ds`) consumes them through the
//! handle. Pointing them at the real backends now (rather than stubs) keeps
//! the vtable honest: the install proves the whole table, not just one slot.
//!
//! ## Zero heap to build
//!
//! The vtable is a `static` of const function pointers, so constructing it
//! allocates nothing. arch's allocator inits heap-free (it is the heap source),
//! so the install can happen at boot before any allocation, dissolving the
//! bootstrap chicken-egg (master plan, Bootstrap resolution).

use core::alloc::Layout;

use reovim_kabi_platform::{InstallError, PlatformVtable, install};

use crate::{
    alloc::{alloc as arch_alloc, dealloc as arch_dealloc},
    sys::{FUTEX_WAIT, FUTEX_WAKE, futex},
    time::Instant,
};

/// The clock adapter: reads arch's monotonic clock and returns whole
/// nanoseconds, matching the contract's `ClockFn` ABI.
///
/// # Safety
///
/// `unsafe extern "C"` to share the vtable's one ABI shape; the underlying
/// `Instant::now` read has no precondition, so this is sound to call anytime.
unsafe extern "C" fn clock() -> i64 {
    Instant::now().as_nanos()
}

/// The allocation adapter: reconstructs a `Layout` from `(size, align)` and
/// calls arch's allocator, returning the raw pointer (null on failure) the
/// contract's `AllocFn` ABI expects.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `align` must be a power of two and
/// `size` non-zero (the consumer's `Layout` guarantees both); a malformed
/// `(size, align)` that `Layout::from_size_align` rejects maps to a null
/// return, the same "allocation refused" signal as a kernel OOM.
unsafe extern "C" fn alloc(size: usize, align: usize) -> *mut u8 {
    // A layout the front cannot represent, and a refused allocation, both report
    // failure as null — the same channel a refused mmap uses; the consumer maps
    // null back to `AllocError`.
    let null = core::ptr::null_mut();
    Layout::from_size_align(size, align)
        .map_or(null, |layout| arch_alloc(layout).map_or(null, core::ptr::NonNull::as_ptr))
}

/// The deallocation adapter: reconstructs the `Layout` and frees through arch.
///
/// # Safety
///
/// `ptr` must come from a prior [`alloc`] call with the identical
/// `(size, align)` and not have been freed; `align` is a power of two and
/// `size` non-zero. A null `ptr` (or a `(size, align)` `Layout` rejects) is a
/// no-op — the consumer never frees a null, but guarding keeps the adapter
/// total against a malformed pair.
unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize, align: usize) {
    let Some(nn) = core::ptr::NonNull::new(ptr) else {
        return;
    };
    let Ok(layout) = Layout::from_size_align(size, align) else {
        return;
    };
    // SAFETY: by the contract `nn`/`layout` are the exact pair a prior `alloc`
    // returned and the block is still live; arch's `dealloc` upholds the rest.
    unsafe { arch_dealloc(nn, layout) }
}

/// The park adapter: futex-waits on `*word` while it equals `expected`.
///
/// # Safety
///
/// `word` must point to a live, aligned `u32` valid across the (possibly
/// blocking) call. Spurious/early returns are permitted — the caller loops on
/// its own condition. The futex result is ignored: `EAGAIN` (the word already
/// changed) and a genuine wake both mean "re-check", which the caller does.
unsafe extern "C" fn park(word: *const u32, expected: u32) {
    let addr = word as usize;
    // No FUTEX_PRIVATE_FLAG here: the contract leaves wake-key policy to the
    // consumer's matched unpark; SP03 wires the matched pair. Shared-key keeps
    // park/unpark on the same bucket without a flag-coordination assumption.
    let _ = futex(addr, FUTEX_WAIT, expected, 0, 0, 0);
}

/// The unpark adapter: futex-wakes one thread parked on `word`.
///
/// # Safety
///
/// `word` must point to a live, aligned `u32`. Waking a `word` no thread is
/// parked on is a no-op.
unsafe extern "C" fn unpark(word: *const u32) {
    let addr = word as usize;
    // Wake exactly one waiter (the matched `park`'s counterpart). The result
    // is ignored: zero woken (no waiter) is a valid no-op.
    let _ = futex(addr, FUTEX_WAKE, 1, 0, 0, 0);
}

/// arch's platform vtable: a `static` of const function pointers (zero heap to
/// build), the table the boot path installs.
///
/// The slot order matches [`PlatformVtable`]'s append-only (AB3) declaration.
static PLATFORM_VTABLE: PlatformVtable = PlatformVtable {
    clock,
    alloc,
    dealloc,
    park,
    unpark,
};

/// Installs arch's platform vtable as the process-wide handle (write-once).
///
/// The boot path (`rust_entry`, the arch-owned entry in `start.rs`) calls this
/// once, after the allocator is up (it inits heap-free, so it is already
/// usable) and the `static` vtable is built (it is `const`, so it is built at
/// compile time). The first install wins; a second returns
/// [`InstallError::AlreadyInstalled`] (AB12), mirroring the `arch::panic`
/// write-once seams.
///
/// # Errors
///
/// Returns [`InstallError::AlreadyInstalled`] if a handle is already installed
/// — a double-boot bug, surfaced rather than silently overwritten.
pub fn install_platform() -> Result<(), InstallError> {
    install(&PLATFORM_VTABLE)
}

// L12 layout: tests live in the sibling file `platform_tests.rs`, declared as a
// `#[path]` child so `super::` reaches `PLATFORM_VTABLE` and the adapters.
#[cfg(feature = "selftest")]
#[path = "platform_tests.rs"]
mod tests;
