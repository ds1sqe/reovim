//! `reovim-kabi-platform` — the down-face platform contract.
//!
//! `uapi/` is the up-face (what the kernel exposes upward); `kabi/` is the
//! down-face (what the kernel requires downward). This crate declares the
//! single mechanism every platform provider implements: a `#[repr(C)]`
//! [`PlatformVtable`] of effectful primitive function pointers, plus the
//! process-global write-once handle that names the installed table.
//!
//! ## The airlock direction (master invariant 6 — the cardinal rule)
//!
//! `kabi/platform` depends on **nothing impl-side**. It is a leaf. The
//! contract NEVER names its implementor: `arch → kabi` is the only permitted
//! edge, and `kabi → arch` is forbidden. A provider (`arch`) builds a
//! `static` [`PlatformVtable`] from its own primitives and installs it; a
//! consumer (`lib/ds`, kernels) reads the installed handle and calls through
//! it. Neither side names the other directly.
//!
//! ## Boot-order prerequisite (the no-read-before-install invariant)
//!
//! The handle is **write-once**: the boot path installs exactly one table
//! ([`install`]), and every later read ([`handle`]) sees it. Reading the
//! handle before install is a boot-stage bug, surfaced by construction —
//! [`handle`] panics on an unset handle rather than returning a sentinel.
//! This mirrors the arch panic-handler precedent (`arch::panic`'s five
//! write-once hook seams, 6.2 §5.2): a provider installs once at boot, and
//! a double-install or a pre-install read is a programming error, not a
//! recoverable runtime state. No consumer may construct a `lib/ds` data
//! structure before the boot path (`rust_entry`) installs the handle (Bootstrap
//! resolution).
//!
//! ## Layout freeze (AB15) + append-only slots (AB3)
//!
//! [`PlatformVtable`] is `#[repr(C)]`, so its layout is frozen and ABI-stable
//! across the provider/consumer boundary. The slot order is **append-only**:
//! new primitives are added at the end, never reordered or removed, so a
//! provider compiled against an older slot set stays binary-compatible.
//!
//! ## Why `unsafe` is allowed here (the floor is `{arch, kabi}`)
//!
//! `arch` (the provider) owns OS FFI; `kabi` (this contract) owns the
//! `#[repr(C)]` FFI vtable that names it. Reading the installed handle
//! ([`handle`]) dereferences a raw pointer published across threads — an
//! irreducible unsafe operation at the provider/consumer boundary. `kabi`
//! *encapsulates* that unsafe behind a safe `install`/`handle` API so its
//! consumers (`lib/ds`, kernels) stay safe: the contract tier is the
//! safe/unsafe boundary as well as the Math/World one. The workspace lint
//! stays `warn`, so any crate ABOVE the floor that introduces `unsafe` still
//! flags; the allow is scoped to this floor-contract crate, mirroring
//! `arch`'s crate-scoped allow.
#![no_std]
// SAFETY (lint): kabi/platform is the down-face contract half of the floor —
// it declares the #[repr(C)] FFI vtable and dereferences the installed handle
// pointer in `handle()`. unsafe cannot be expressed away at the FFI boundary;
// it is encapsulated here so consumers above the floor stay safe. The
// workspace lint stays `warn` so crates above the floor still flag unsafe.
#![allow(unsafe_code)]

use core::sync::atomic::{
    AtomicPtr,
    Ordering::{Acquire, Release},
};

/// Failure to allocate through the platform's allocator primitive.
///
/// The canonical alloc-seam error. It lives in `kabi` (not `arch`) because it
/// is part of the [`PlatformVtable`] alloc-slot's Rust-facing contract: a
/// consumer that maps the C-ABI null return of [`PlatformVtable::alloc`] back
/// to a typed `Result` names `kabi::AllocError`, with no edge to any
/// implementor (master invariant 2: the global handle static and `AllocError`
/// live in `kabi`, never `arch`).
///
/// It is a unit struct: the only failure the seam reports is "the request was
/// not satisfied" — the floor's allocator distinguishes no further (a refused
/// `mmap`, a malformed layout, and an overflowed size all collapse to this).
///
/// ```rust
/// use reovim_kabi_platform::AllocError;
///
/// assert_eq!(AllocError, AllocError);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocError;

/// The clock primitive: reads a monotonic time source, returning whole
/// nanoseconds since an unspecified epoch.
///
/// Monotonic (never steps backward), so the difference of two readings is a
/// non-negative duration. The SP02 proof slot — the one primitive wired
/// end-to-end through the handle (provider → vtable → consumer) to prove the
/// mechanism works before SP03 routes alloc/park through it.
///
/// # Safety
///
/// The provider's backing implementation reads a system clock; the function
/// is `unsafe extern "C"` to share one ABI shape with the effectful slots
/// below. A correct provider's clock read has no precondition, so a caller
/// upholds nothing beyond the handle being installed.
pub type ClockFn = unsafe extern "C" fn() -> i64;

/// The allocation primitive: returns `size` bytes aligned to `align`, or null
/// on failure (the canonical `malloc`-shaped C ABI).
///
/// Shaped to the C ABI (`*mut u8`, null = failure) rather than carrying a Rust
/// `Result<NonNull<u8>, AllocError>` across the `extern "C"` boundary, which is
/// not FFI-safe. The consumer (`lib/ds`, SP03) maps the null return back to
/// [`AllocError`]. `align` is always a power of two; `size` is non-zero.
///
/// # Safety
///
/// `align` must be a power of two and `size` non-zero (the consumer's
/// `Layout` guarantees both). The returned pointer, if non-null, owns `size`
/// bytes valid for reads/writes until passed to [`DeallocFn`] with the same
/// `(size, align)`.
pub type AllocFn = unsafe extern "C" fn(size: usize, align: usize) -> *mut u8;

/// The deallocation primitive: frees a block previously returned by
/// [`AllocFn`] for the same `(size, align)`.
///
/// # Safety
///
/// `ptr` must come from a prior [`AllocFn`] call with the identical
/// `(size, align)` and must not have been freed already; after this call the
/// block is invalid.
pub type DeallocFn = unsafe extern "C" fn(ptr: *mut u8, size: usize, align: usize);

/// The park primitive: blocks the calling thread while `*word == expected`,
/// returning when woken by [`UnparkFn`] or spuriously.
///
/// Futex-shaped (the low-level wait the provider exposes): the kernel
/// re-checks `*word` against `expected` before sleeping, closing the
/// lost-wake window. Spurious wakes are permitted — the caller re-checks its
/// own condition in a loop. SP03 consumes this through the handle to build the
/// `lib/ds` `Mutex`/`Condvar`; SP02 only declares it.
///
/// # Safety
///
/// `word` must point to a live, aligned `u32` that stays valid across the
/// (possibly blocking) call; the provider reads `*word` and may block the
/// calling thread.
pub type ParkFn = unsafe extern "C" fn(word: *const u32, expected: u32);

/// The unpark primitive: wakes one thread parked on `word` via [`ParkFn`].
///
/// # Safety
///
/// `word` must point to a live, aligned `u32`; waking a `word` no thread is
/// parked on is a no-op (not an error).
pub type UnparkFn = unsafe extern "C" fn(word: *const u32);

/// The platform handle: a `#[repr(C)]` vtable of effectful primitive function
/// pointers the provider builds and the boot path installs.
///
/// One stable contract, N co-located implementations, one bound at
/// composition (the nouveau/nvidia model): every provider exposes this exact
/// layout; the boot path installs whichever provider's table is live. The
/// vtable is built as a `static` from const function pointers — zero heap to
/// construct, so it is available before any allocation occurs (the bootstrap
/// chicken-egg dissolves: the allocator inits heap-free, then this table is
/// built, then installed).
///
/// ## Slot order is append-only (AB3) and frozen (AB15)
///
/// `#[repr(C)]` freezes the field layout; the order — `clock`, `alloc`,
/// `dealloc`, `park`, `unpark` — is append-only. A future primitive is a new
/// trailing field, never a reorder or removal, so a binary built against an
/// older slot set stays ABI-compatible. In SP02 only `clock` is wired
/// end-to-end; `alloc`/`dealloc`/`park`/`unpark` are declared and pointed at
/// real backends but consumed by no caller until SP03.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PlatformVtable {
    /// Monotonic clock read (the SP02 end-to-end proof slot).
    pub clock: ClockFn,
    /// Allocate `(size, align)` bytes; null on failure (SP03-consumed).
    pub alloc: AllocFn,
    /// Free a block from [`PlatformVtable::alloc`] (SP03-consumed).
    pub dealloc: DeallocFn,
    /// Block while `*word == expected` (SP03-consumed).
    pub park: ParkFn,
    /// Wake one thread parked on `word` (SP03-consumed).
    pub unpark: UnparkFn,
}

// `PlatformVtable` is `Sync` automatically: it holds only `unsafe extern "C"`
// function pointers, which are `Send + Sync` (a code address shared read-only
// across threads carries no interior state). No explicit `unsafe impl Sync` is
// needed — the auto-derived bound suffices for the `'static HANDLE` static and
// the `&'static` install path.

/// The error a second [`install`] returns: the handle is already set.
///
/// Write-once (AB12): the first installer wins and this call changed nothing.
/// Mirrors `arch::panic::SetError` — a double-install is a boot-stage bug,
/// surfaced as a typed error rather than silently overwriting the live table.
///
/// ```rust
/// use reovim_kabi_platform::InstallError;
///
/// assert_eq!(InstallError::AlreadyInstalled, InstallError::AlreadyInstalled);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallError {
    /// A platform handle was already installed; the first install wins and
    /// this call changed nothing (AB12 write-once).
    AlreadyInstalled,
}

/// The process-global installed handle. `null` until the boot path installs a
/// table. Stored as a raw pointer to a `'static PlatformVtable` the provider
/// owns (its `static` table), set once via CAS from the null sentinel.
static HANDLE: AtomicPtr<PlatformVtable> = AtomicPtr::new(core::ptr::null_mut());

/// Installs `vtable` as the process-wide platform handle (write-once, AB12).
///
/// The boot path (`rust_entry`, sequenced in arch's `start.rs`) calls this
/// exactly once, after the allocator is up and the provider's `static`
/// [`PlatformVtable`] is built. The first install wins; a second is rejected
/// with [`InstallError::AlreadyInstalled`] and changes nothing. This mirrors
/// the `arch::panic` write-once seams: `compare_exchange` from the null
/// sentinel with `Release` ordering, so a later [`handle`] read (paired with
/// `Acquire`) observes every write the provider made before installing.
///
/// `vtable` is `'static` because a provider's table is a `static`; the handle
/// borrows it for the process lifetime.
///
/// # Errors
///
/// Returns [`InstallError::AlreadyInstalled`] if a handle is already installed.
///
/// ```no_run
/// use reovim_kabi_platform::{install, PlatformVtable};
///
/// unsafe extern "C" fn clock_stub() -> i64 { 0 }
/// unsafe extern "C" fn alloc_stub(size: usize, align: usize) -> *mut u8 {
///     let _ = (size, align);
///     core::ptr::null_mut()
/// }
/// unsafe extern "C" fn dealloc_stub(ptr: *mut u8, size: usize, align: usize) {
///     let _ = (ptr, size, align);
/// }
/// unsafe extern "C" fn park_stub(word: *const u32, expected: u32) {
///     let _ = (word, expected);
/// }
/// unsafe extern "C" fn unpark_stub(word: *const u32) { let _ = word; }
///
/// static TABLE: PlatformVtable = PlatformVtable {
///     clock:   clock_stub,
///     alloc:   alloc_stub,
///     dealloc: dealloc_stub,
///     park:    park_stub,
///     unpark:  unpark_stub,
/// };
///
/// // Boot path: install the platform table once.
/// install(&TABLE).expect("first install always succeeds");
/// ```
pub fn install(vtable: &'static PlatformVtable) -> Result<(), InstallError> {
    // CAS from the null sentinel: the first writer wins (AB12). `Release` so a
    // consumer that later observes the non-null handle also observes the
    // provider's writes that built the table; `Acquire` on the failure load
    // pairs symmetrically. The cast drops only the outer shared-ref-to-`*mut`
    // for the atomic; the table is never written through the pointer.
    let ptr = core::ptr::from_ref(vtable).cast_mut();
    HANDLE
        .compare_exchange(core::ptr::null_mut(), ptr, Release, Acquire)
        .map(|_| ())
        .map_err(|_| InstallError::AlreadyInstalled)
}

/// Returns the installed platform handle.
///
/// # Panics
///
/// Panics if no handle is installed yet. This is a by-construction boot
/// prerequisite, not a runtime guard: the boot path installs the handle before
/// any consumer runs (the no-read-before-install invariant in the module
/// docs), so a panic here means a consumer ran ahead of the boot-path install
/// (`rust_entry`) — a
/// boot-ordering bug. The precedent is the `arch::panic` handler, which treats
/// an unregistered seam as a boot-stage error rather than a recoverable state.
///
/// ```no_run
/// use reovim_kabi_platform::handle;
///
/// // After the boot path has called install(), handle() returns the
/// // installed table. Calling it before install() panics — no_run here
/// // so the doctest does not execute and panic at test time.
/// let _vtable = handle();
/// ```
#[must_use]
pub fn handle() -> &'static PlatformVtable {
    let ptr = HANDLE.load(Acquire);
    // SAFETY: a non-null `HANDLE` was published by `install` from a
    // `&'static PlatformVtable`, so it points at a live, immutable `'static`
    // table; the `Acquire` load pairs with the installer's `Release`.
    unsafe { ptr.as_ref() }.expect("platform handle read before the boot path installed it")
}

// L12 layout: tests live in the sibling file `tests.rs`, declared as a
// `#[path]` child so `super::` reaches `HANDLE` for install/read assertions.
// `kabi/platform` is a plain library crate (not the arch no_std selftest
// runtime), so its unit tests run under the ordinary `cargo test` harness —
// the write-once contract is process-global, so the dedicated test resets are
// unnecessary here (one install per test process is the only path exercised).
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
