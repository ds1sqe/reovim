//! Panic-mirror region — the `'static` append-time flush buffer (9.5 §9.1).
//!
//! ## ONE-log-buffer rule (LOG1, LOG6)
//!
//! This region is **not** a second log buffer. It is the §9.1-mandated panic
//! mirror of the one kernel log ring: every byte written here is a rendered
//! copy of a byte already in the ring. The region exists solely because the
//! panic handler may not allocate or take a lock the panicking thread could be
//! holding; it must find the rendered bytes already in place when it fires.
//!
//! ## Append-time maintenance (9.5 §9.1)
//!
//! On every ring append, `LogRing::push_event` calls [`update_after_push`]
//! **before** the ring push — the ring lock is not yet held at call time. That
//! function serialises all mirror writers under `FLUSH_LOCK`, copies the
//! rendered LOG2 bytes into `FLUSH_BUF`, then stores the new cursor length with
//! `Release` ordering. The ring-tail provider [`ring_tail`] loads the cursor
//! with `Acquire` and returns exactly that prefix. By the Release/Acquire
//! ordering the bytes are visible before the length, so the provider always
//! returns a coherent prefix. No allocation, no lock in the provider path.
//!
//! ## Capacity and overflow handling
//!
//! The region is a fixed-capacity static BSS array of [`FLUSH_BUF_CAPACITY`]
//! bytes (default 1 MiB — the `log-ring-bytes` spec default, LOG6). BSS is
//! demand-paged; the 1 MiB reservation costs nothing until first written.
//!
//! A runtime-sized region requires the config service; the static default
//! applies until then. Config-driven size follows when the config service exists.
//!
//! When `update_after_push` finds the buffer would overflow, it calls
//! [`compact_into_buf`] to restart the region from the live ring contents.
//! Compaction calls `ring.for_each` which takes the ring lock; this is safe
//! because `update_after_push` is called BEFORE the ring push — the ring lock
//! is not held when we enter. Lock order is `FLUSH_LOCK` → ring `Mutex`:
//! compaction may take the ring lock while holding the flush lock; no path
//! takes them in the other order. The compaction rebuilds the mirror from the
//! ring's current contents (entries from all previous pushes); the current
//! event's `line` is then appended after compaction. Whole LOG2 lines are
//! always preserved.
//!
//! ## `unsafe` policy (§9.1 single-writer + Release/Acquire)
//!
//! `FLUSH_BUF` is a `static mut` array. Access is bounded by two invariants:
//!
//! - **Single-writer**: `update_after_push` is the only write path, and it
//!   serialises all writers under `FLUSH_LOCK` — `emit()` is lock-free, so
//!   two threads CAN reach `push_event` concurrently; nothing upstream
//!   serialises them. Lock order is `FLUSH_LOCK` → ring `Mutex` (compaction
//!   iterates the ring while holding the flush lock); no path acquires them
//!   in the other order. The panic-time reader (`ring_tail`) never takes
//!   `FLUSH_LOCK` — it is the Acquire-prefix read §9.1 mandates.
//! - **Release/Acquire cursor**: the cursor `FLUSH_CURSOR` is written with
//!   `Release` after bytes are placed; `ring_tail` loads it with `Acquire`.
//!   Any thread that observes the new cursor also observes all bytes at
//!   indices `0..cursor`. This is the §9.1 ordering discipline from arch/panic.
//!
//! This mirrors how `arch/src/panic.rs` structures its own write-once statics,
//! with the SAFETY comments making the invariants explicit.

// `unsafe` is required for the `static mut FLUSH_BUF` raw-pointer writes and
// reads that implement the §9.1 Release/Acquire single-writer discipline.
// SAFETY invariants are documented at every `unsafe` block. This is the
// minimum unsafe surface: one `static mut` array + one `AtomicUsize` cursor.
#![allow(unsafe_code)]

use core::sync::atomic::{AtomicUsize, Ordering};

use reovim_lib_ds::Mutex;

use crate::log::ring::LogRing;

// ── Capacity constant ─────────────────────────────────────────────────────────

/// Capacity of the panic-mirror flush buffer in bytes (9.5 §9.1).
///
/// Fixed at the `log-ring-bytes` default (1 MiB, LOG6). A runtime-sized region
/// requires the config service; the 9.5 §9.1 static-default rule applies until
/// then. Config-driven size follows when the config service exists.
///
/// BSS is demand-paged; reserving 1 MiB costs nothing until written.
///
/// ```rust
/// use reovim_kernel::log::flush::FLUSH_BUF_CAPACITY;
/// assert_eq!(FLUSH_BUF_CAPACITY, 1024 * 1024);
/// ```
pub const FLUSH_BUF_CAPACITY: usize = 1024 * 1024; // 1 MiB

// ── Static byte region (single-writer + Release/Acquire) ─────────────────────

/// The process-global flush-buffer backing array (9.5 §9.1).
///
/// SAFETY: all writes go through `update_after_push` / `compact_into_buf`, both
/// of which uphold the single-writer + Release/Acquire discipline documented in
/// this module's top comment. Reads go through `ring_tail` (Acquire load of the
/// cursor, then a `'static` slice of the already-visible prefix). Accessed only
/// through raw pointers (`&raw mut` / `&raw const`) to avoid the
/// `static_mut_refs` lint, following the same pattern as `arch/src/start.rs`.
static mut FLUSH_BUF: [u8; FLUSH_BUF_CAPACITY] = [0u8; FLUSH_BUF_CAPACITY];

/// Cursor: the number of valid (written) bytes in `FLUSH_BUF`.
///
/// Written with `Release` after bytes are placed; read with `Acquire` by
/// `ring_tail` (§9.1 ordering).
static FLUSH_CURSOR: AtomicUsize = AtomicUsize::new(0);

/// Serialises the mirror's writers (`update_after_push`, and through it
/// `compact_into_buf`). The panic-time reader stays lock-free per §9.1.
static FLUSH_LOCK: Mutex<()> = Mutex::new(());

// ── Provider (panic-path: no alloc, no lock) ─────────────────────────────────

/// Returns the pre-rendered LOG2 ring mirror as a contiguous `'static` slice.
///
/// This is the `RingTailProvider` (`fn() -> &'static [u8]`) registered with
/// `arch::panic::set_ring_tail_provider` during boot. `arch` writes the returned
/// slice verbatim ahead of the panic line and never parses entries (9.5 §9.1).
///
/// The cursor is loaded with `Acquire`: by the Release/Acquire discipline the
/// bytes at `0..len` are already visible. No allocation, no lock.
///
/// ```rust,no_run
/// // no_run: mutates process-global state — parallel doctests share the process.
/// use reovim_kernel::log::flush::ring_tail;
///
/// // Before any ring appends the region is empty.
/// let tail = ring_tail();
/// assert!(tail.is_empty());
/// ```
pub fn ring_tail() -> &'static [u8] {
    let len = FLUSH_CURSOR.load(Ordering::Acquire);
    // SAFETY: `len` was stored with `Release` by `update_after_push` /
    // `compact_into_buf` after writing exactly `len` bytes into
    // `FLUSH_BUF[0..len]`. By `Acquire` ordering those bytes are visible here.
    // `FLUSH_BUF` is a fixed static array; the slice is valid for `'static`.
    // Using `&raw const` per `arch/src/start.rs` convention to avoid
    // `static_mut_refs`.
    unsafe {
        let ptr: *const [u8; FLUSH_BUF_CAPACITY] = &raw const FLUSH_BUF;
        // Cast to a slice pointer; `len <= FLUSH_BUF_CAPACITY` is guaranteed
        // by the store discipline in `update_after_push` / `compact_into_buf`.
        core::slice::from_raw_parts(ptr.cast::<u8>(), len)
    }
}

// ── Append-time writer ────────────────────────────────────────────────────────

/// Updates the flush mirror with `line` bytes.
///
/// Called from `LogRing::push_event` before the ring push so no ring lock is
/// held. This means the ring lock is free and compaction via `for_each` is safe
/// (no lock re-entrancy). The ring does NOT yet contain the current `line`'s
/// entry at call time — the push comes after this call in `push_event`.
///
/// Normal path (no overflow): appends `line` to the current cursor position,
/// then stores the new cursor with `Release`.
///
/// Overflow path: calls [`compact_into_buf`] with `ring` to restart the mirror
/// from the live ring contents (ring entries from all PREVIOUS pushes), then
/// appends `line` (the current event). After compaction + append, if `line`
/// still does not fit (a single entry larger than the whole buffer), the entry
/// is silently dropped from the mirror — the ring is authoritative; the mirror
/// is best-effort.
///
/// ```rust,no_run
/// // no_run: mutates process-global state — parallel doctests share the process.
/// use reovim_kernel::log::flush::{ring_tail, update_after_push};
/// use reovim_kernel::log::ring::LogRing;
///
/// let ring = LogRing::try_new(1024 * 1024).unwrap();
/// let line = b"[    0.000001] kernel boot: ok\n";
/// update_after_push(line, &ring);
/// assert_eq!(ring_tail(), line.as_slice());
/// ```
pub fn update_after_push(line: &[u8], ring: &LogRing) {
    // Serialise writers: emit() is lock-free, so concurrent emits reach this
    // point in parallel. The flush lock is the single-writer guarantee §9.1's
    // region discipline rests on (the panic-time reader never takes it).
    let _writer = FLUSH_LOCK.lock();
    // Delegate to the capacity-parameterised inner function. Production always
    // passes `FLUSH_BUF_CAPACITY`; the selftest wrapper passes a smaller value
    // to exercise the overflow branches without filling 1 MiB.
    // SAFETY bound: `capacity <= FLUSH_BUF_CAPACITY` — the only caller here
    // passes `FLUSH_BUF_CAPACITY` exactly; the selftest wrapper is the only
    // other caller and it must satisfy the same bound.
    update_with_capacity(line, ring, FLUSH_BUF_CAPACITY);
}

/// Capacity-parameterised core of [`update_after_push`].
///
/// `capacity` MUST be `<= FLUSH_BUF_CAPACITY`. All writes stay within
/// `[0, capacity]` which is a subset of `[0, FLUSH_BUF_CAPACITY]`.
///
/// `FLUSH_LOCK` must already be held by the caller. This function never
/// acquires the lock itself, preventing any double-lock scenario.
///
/// `pub(crate)` so the selftest wrapper (`update_with_small_capacity`) can
/// call it directly with a reduced capacity for overflow-branch coverage
/// without needing to fill the full 1 MiB buffer.
pub(crate) fn update_with_capacity(line: &[u8], ring: &LogRing, capacity: usize) {
    debug_assert!(capacity <= FLUSH_BUF_CAPACITY, "capacity must not exceed FLUSH_BUF_CAPACITY");
    // Relaxed load: writes are serialised by `FLUSH_LOCK` (held by the caller).
    let cur = FLUSH_CURSOR.load(Ordering::Relaxed);

    if cur + line.len() > capacity {
        // Overflow: rebuild the mirror from all previous ring entries (ring lock
        // is free — we are called before the ring push). `compact_into_buf`
        // acquires the ring lock via `for_each`.
        let after_compact = compact_into_buf(ring, capacity);

        // After compaction, also append `line` itself (the current event, not
        // yet in the ring). If it still does not fit, drop it from the mirror
        // — the ring is authoritative; the mirror is best-effort.
        if after_compact + line.len() <= capacity {
            // SAFETY: `after_compact + line.len() <= capacity <= FLUSH_BUF_CAPACITY`.
            // Single writer (FLUSH_LOCK held); `compact_into_buf` already stored
            // the cursor. `&raw mut` avoids `static_mut_refs` per arch convention.
            unsafe {
                let ptr: *mut u8 = (&raw mut FLUSH_BUF).cast::<u8>();
                core::ptr::copy_nonoverlapping(line.as_ptr(), ptr.add(after_compact), line.len());
            }
            // Release: bytes visible before cursor.
            FLUSH_CURSOR.store(after_compact + line.len(), Ordering::Release);
        }
        // If the entry still does not fit after compaction, cursor stays at
        // `after_compact` (already stored by `compact_into_buf` with Release).
    } else {
        // Normal append.
        // SAFETY: `cur + line.len() <= capacity <= FLUSH_BUF_CAPACITY`. Single
        // writer (FLUSH_LOCK held); `&raw mut` avoids `static_mut_refs`.
        unsafe {
            let ptr: *mut u8 = (&raw mut FLUSH_BUF).cast::<u8>();
            core::ptr::copy_nonoverlapping(line.as_ptr(), ptr.add(cur), line.len());
        }
        // Release: bytes are visible before the cursor length is visible.
        FLUSH_CURSOR.store(cur + line.len(), Ordering::Release);
    }
}

/// Selftest entry point for exercising the overflow branches of
/// [`update_with_capacity`] with a small `capacity` bound.
///
/// Takes the `FLUSH_LOCK` (the single-writer serialisation the outer
/// `update_after_push` normally provides) and calls `update_with_capacity`
/// with the caller-supplied `capacity`. This is the ONLY path that passes a
/// capacity smaller than `FLUSH_BUF_CAPACITY`; production code never uses it.
///
/// Only compiled under the `selftest` feature.
///
/// ```rust,no_run
/// // no_run: selftest-gated — not available in production builds.
/// ```
#[cfg(feature = "selftest")]
pub(crate) fn update_with_small_capacity(line: &[u8], ring: &LogRing, capacity: usize) {
    debug_assert!(capacity <= FLUSH_BUF_CAPACITY, "capacity must not exceed FLUSH_BUF_CAPACITY");
    let _writer = FLUSH_LOCK.lock();
    update_with_capacity(line, ring, capacity);
}

// ── Compaction helper ─────────────────────────────────────────────────────────

/// Restarts the flush buffer from `ring`'s current contents (oldest-first).
///
/// Iterates `ring.for_each` and copies rendered LOG2 bytes into `FLUSH_BUF`,
/// stopping when the next entry would exceed `capacity` (entries that fit in
/// `FLUSH_BUF` but not `capacity` are also dropped — `capacity` is the ceiling
/// for tests; production always passes `FLUSH_BUF_CAPACITY`).
///
/// The cursor is reset with `Release` to the number of bytes copied. The caller
/// does not need to store the cursor again after this call.
///
/// `pub(crate)` so `flush_tests.rs` can test compaction directly with a small
/// capacity bound without needing to fill 1 MiB of buffer.
///
/// ```rust,no_run
/// // no_run: pub(crate) — covered by flush_tests.rs.
/// ```
pub(crate) fn compact_into_buf(ring: &LogRing, capacity: usize) -> usize {
    let mut pos = 0usize;
    ring.for_each(|entry| {
        let bytes = entry.line.as_slice();
        if pos + bytes.len() <= capacity {
            // SAFETY: `pos + bytes.len() <= capacity <= FLUSH_BUF_CAPACITY`.
            // Single writer at append time. The compaction rebuilds from the
            // ring's current contents; whole LOG2 lines only (no partial lines).
            // `&raw mut` avoids `static_mut_refs` per arch convention.
            unsafe {
                let ptr: *mut u8 = (&raw mut FLUSH_BUF).cast::<u8>();
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.add(pos), bytes.len());
            }
            pos += bytes.len();
        }
        // Entries that do not fit (oldest first, then largest) are dropped from
        // the mirror. The ring is authoritative; the mirror is bounded.
    });
    // Release: bytes at 0..pos are visible before the cursor is visible.
    FLUSH_CURSOR.store(pos, Ordering::Release);
    pos
}

// ── Test reset (selftest-gated) ───────────────────────────────────────────────

/// Resets the flush buffer to the initial empty state.
///
/// Must be called at the start of every flush test so tests do not share
/// buffer state. The no_std selftest runner is single-threaded sequential, so
/// no external serialisation is needed.
///
/// Only available under the `selftest` feature (compile-time guard against
/// accidental production use).
///
/// ```rust,no_run
/// // no_run: selftest-gated — not available in production builds.
/// ```
#[cfg(feature = "selftest")]
pub fn reset_for_test() {
    FLUSH_CURSOR.store(0, Ordering::Relaxed);
    // SAFETY: single-threaded selftest runner; no concurrent reads. Zeroing
    // the buffer ensures that a subsequent `ring_tail()` before any writes
    // returns an empty slice, not stale bytes from a prior test.
    // `&raw mut` avoids `static_mut_refs` per arch convention.
    unsafe {
        let ptr: *mut u8 = (&raw mut FLUSH_BUF).cast::<u8>();
        core::ptr::write_bytes(ptr, 0, FLUSH_BUF_CAPACITY);
    }
}

// L12 layout: tests in sibling flush_tests.rs, declared in log/mod.rs.
