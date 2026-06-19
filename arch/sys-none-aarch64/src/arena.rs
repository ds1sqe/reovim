//! The static page arena behind `mmap`.
//!
//! A freestanding target has no MMU-backed mapping machinery, so the floor's
//! page source is a fixed `.bss` arena handed out by a monotonic bump
//! offset. Pages are never reclaimed — `munmap` is a no-op by design, which
//! mirrors the slab allocator above: it never returns small-class pages
//! either, because floor allocations are long-lived. A freed large mapping
//! leaks its pages; the leak is bounded by the arena itself.
//!
//! Size: 1 MiB. The selftest payload's live high-water is a handful of slab
//! pages plus the panic/test-runner buffers — far below the arena — and a
//! genuine exhaustion surfaces as `ENOMEM` through the same error arm the
//! Linux backends take on kernel OOM, so the failure mode is honest, not
//! silent.

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::errno::{ENOMEM, Errno};

/// The hand-out granularity, matching the slab allocator's page size.
const PAGE_SIZE: usize = 4096;
/// Total arena capacity.
const ARENA_BYTES: usize = 1 << 20;

/// Page-aligned arena storage. The `UnsafeCell` makes writes through the
/// handed-out addresses defined behavior; the alignment lets every hand-out
/// be page-aligned by construction.
#[repr(C, align(4096))]
struct Arena(UnsafeCell<[u8; ARENA_BYTES]>);

// SAFETY: the storage is only ever reached through addresses returned by
// `alloc_pages`, and the monotonic bump never hands the same byte range out
// twice, so no two owners alias.
unsafe impl Sync for Arena {}

/// The arena storage. Zero-initialized, so it lives in `.bss` — the boot
/// entry's BSS clear is what guarantees the anonymous-mapping zero-fill
/// contract `mmap` callers rely on.
static ARENA: Arena = Arena(UnsafeCell::new([0; ARENA_BYTES]));

/// Next unallocated byte offset into [`ARENA`].
static NEXT: AtomicUsize = AtomicUsize::new(0);

/// Total capacity of the page arena in bytes.
///
/// A one-shot *static* fact — the arena's fixed `.bss` size, not its live
/// used/free, which is mutable runtime state the floor reports through a
/// different seam. The boot-info collector reads this to report heap capacity.
#[must_use]
pub const fn capacity() -> usize {
    ARENA_BYTES
}

/// Hands out `len` bytes (rounded up to whole pages), page-aligned.
///
/// # Errors
///
/// `ENOMEM` when the arena cannot fit the request — the same errno the
/// Linux backends surface on kernel OOM, so callers' refusal arms behave
/// identically across targets.
pub fn alloc_pages(len: usize) -> Result<usize, Errno> {
    let len = len.checked_next_multiple_of(PAGE_SIZE).ok_or(ENOMEM)?;
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
        next.checked_add(len).filter(|&end| end <= ARENA_BYTES)
    })
    .map(|offset| ARENA.0.get().addr() + offset)
    .map_err(|_| ENOMEM)
}
