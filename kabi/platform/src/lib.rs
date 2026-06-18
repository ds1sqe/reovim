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

use core::{
    alloc::Layout,
    ptr::NonNull,
    sync::atomic::{
        AtomicPtr, AtomicU32,
        Ordering::{Acquire, Release},
    },
};

use reovim_uapi_posix::{Errno, Fd, Mode, OpenFlags};

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

// ── Boot information (push-at-entry static data) ───────────────────────────────

/// The kind of a physical memory range in a [`BootInfo`] memory map.
///
/// Mirrors the E820 memory-type vocabulary a PC bootloader reports, narrowed to
/// the classes the floor distinguishes. ARM's mailbox RAM query yields a single
/// [`Usable`](MemoryKind::Usable) range; an x86 bootloader's E820 map carries
/// several ranges of mixed kinds. `#[repr(u8)]` with explicit discriminants
/// freezes the tag values so a range built from firmware data keeps its meaning.
///
/// ```rust
/// use reovim_kabi_platform::MemoryKind;
///
/// assert_eq!(MemoryKind::Usable as u8, 1);
/// ```
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryKind {
    /// RAM free for the allocator to use.
    Usable = 1,
    /// Firmware- or MMIO-reserved; not available as general RAM.
    Reserved = 2,
    /// ACPI tables (reclaimable after parsing; treated as reserved by the floor).
    Acpi = 3,
    /// Bad or otherwise unusable RAM.
    Unusable = 4,
}

/// One physical address range in a [`BootInfo`] memory map.
///
/// `base` and `len` are byte address and byte size (`u64` so a memory map above
/// 4 GiB is representable on either arch). `#[repr(C)]` freezes the field layout
/// so the range matches the firmware-data shape it is built from.
///
/// ```rust
/// use reovim_kabi_platform::{MemoryKind, MemoryRange};
///
/// let r = MemoryRange { base: 0, len: 0x4000_0000, kind: MemoryKind::Usable };
/// assert_eq!(r.len, 1 << 30);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRange {
    /// Physical base address of the range, in bytes.
    pub base: u64,
    /// Length of the range, in bytes.
    pub len: u64,
    /// What the range may be used for.
    pub kind: MemoryKind,
}

/// Hardware facts discovered at runtime and pushed into the kernel at entry.
///
/// The kernel is platform-neutral — it knows nothing about the board, the core
/// count, or how much RAM is installed, and none of that is knowable at compile
/// time (one image runs on machines with different hardware). A platform
/// provider discovers these facts at boot (firmware mailbox / CPUID / E820) and
/// hands a `BootInfo` to `Init::new` through the launcher args. This is one-shot
/// *static data*, not a runtime-services callback, so it is delivered
/// push-at-entry rather than through the [`PlatformVtable`].
///
/// An empty `BootInfo` (the [`Default`]) is valid: a hosted build that has no
/// firmware to query carries an empty memory map and zeroed CPU fields, and the
/// kernel reports only what is present.
///
/// ```rust
/// use reovim_kabi_platform::BootInfo;
///
/// // The hosted / no-firmware default: empty map, zeroed CPU.
/// let info = BootInfo::default();
/// assert!(info.memory.is_empty());
/// assert_eq!(info.cpu_count, 0);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct BootInfo {
    /// Physical memory map. E820-style on x86 (several ranges); ARM supplies a
    /// single [`Usable`](MemoryKind::Usable) range. Backed by storage that lives
    /// for the program's lifetime (the provider's boot arena), hence `'static`.
    pub memory: &'static [MemoryRange],
    /// CPU clock frequency in Hz (`CNTFRQ_EL0` on ARM; TSC-via-CPUID on x86), or
    /// `0` when unknown.
    pub cpu_freq_hz: u64,
    /// CPU identification: `MIDR_EL1` on ARM narrowed to its low 32 bits (the
    /// implementer/variant/part fields; bits 63:32 are architecturally RES0, so
    /// the narrowing loses nothing), CPUID leaf 1 `eax` on x86. `0` when unknown.
    pub cpu_id: u32,
    /// Logical CPU count (`1` on the current single-core floor; `0` when unknown).
    pub cpu_count: u32,
}

/// Maps a fd-op slot's `i64` return (`>= 0` success, `< 0` is `-errno`) to a
/// typed `Result`. The success value is narrowed to the caller's `usize`
/// (a byte count or a non-negative fd both fit).
///
/// The error half is the canonical [`Errno`] from `uapi/posix`: the fd-op slots
/// report a negative-errno return (the Linux raw-syscall convention, FFI-safe
/// across the `extern "C"` boundary), and the safe wrappers recover the
/// positive code into the POSIX-vocabulary newtype so consumers match on a
/// positive code rather than re-deriving the sign.
const fn map_fd_ret(ret: i64) -> Result<usize, Errno> {
    if ret < 0 {
        // A negative return encodes `-errno`; recover the positive code. The
        // cast cannot truncate meaningfully: errno values are small positives.
        #[allow(clippy::cast_possible_truncation)]
        Err(Errno::from_code((-ret) as i32))
    } else {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        Ok(ret as usize)
    }
}

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

/// The wake-all primitive: wakes **every** thread parked on `word` via
/// [`ParkFn`], not just one.
///
/// [`UnparkFn`] wakes a single waiter — enough for a mutex hand-off, but a
/// `Condvar::notify_all` and an `RwLock` writer-release-to-readers must wake
/// the whole cohort. This is a distinct primitive (the futex wake count is
/// `i32::MAX`, not `1`), appended as a trailing slot (AB3) rather than folded
/// into [`UnparkFn`] so a provider compiled against the older slot set stays
/// ABI-compatible.
///
/// # Safety
///
/// `word` must point to a live, aligned `u32`; waking a `word` no thread is
/// parked on is a no-op (not an error).
pub type UnparkAllFn = unsafe extern "C" fn(word: *const u32);

/// The Unix-domain connect primitive.
///
/// Connects a stream socket to the NUL-terminated pathname `path` (`path_len`
/// bytes including the NUL) and returns the connected fd (`>= 0`), or a
/// negative errno on failure.
///
/// fd-based syscall surface (the SP05 net stratum): the contract carries raw
/// fds as `i32`, paths as `*const u8` + `usize`, and reports errors as a
/// negative errno return — the Linux raw-syscall convention — rather than a
/// non-FFI-safe `Result`. The consumer (`lib/ds`'s `UnixStream`) maps a
/// negative return back to a typed error and wraps the fd in a Math type.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `path` must point to `path_len`
/// readable bytes containing a NUL-terminated pathname; the provider reads no
/// further. A malformed path maps to a negative-errno return, not UB.
pub type UnixConnectFn = unsafe extern "C" fn(path: *const u8, path_len: usize) -> i64;

/// The Unix-domain listen primitive.
///
/// Binds a stream socket to the NUL-terminated pathname `path` and marks it
/// passive, returning the listening fd (`>= 0`) or a negative errno. A stale
/// socket file at `path` is unlinked before bind so a repeated bind (a
/// restarted server) succeeds (the
/// `panic = "abort"` profile means a listener's Drop-unlink may not run; the
/// pre-bind unlink is the robust cleanup).
///
/// # Safety
///
/// As [`UnixConnectFn`]: `path` must point to `path_len` readable bytes with a
/// NUL-terminated pathname.
pub type UnixListenFn = unsafe extern "C" fn(path: *const u8, path_len: usize) -> i64;

/// The Unix-domain accept primitive: blocks until a connection arrives on the
/// listening `listener_fd`, returning the connected fd (`>= 0`) or a negative
/// errno.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `listener_fd` is a plain scalar; an
/// invalid fd maps to a negative errno, not UB.
pub type UnixAcceptFn = unsafe extern "C" fn(listener_fd: Fd) -> i64;

/// The fd-read primitive: reads up to `len` bytes from `fd` into `buf`,
/// returning the byte count (`0` at end-of-stream, `> 0` otherwise) or a
/// negative errno.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `buf` must point to `len` writable
/// bytes; the provider writes at most the returned count. A bad fd maps to a
/// negative errno.
pub type FdReadFn = unsafe extern "C" fn(fd: Fd, buf: *mut u8, len: usize) -> i64;

/// The fd-write primitive.
///
/// Writes up to `len` bytes from `buf` to `fd`, returning the byte count
/// written or a negative errno. The provider suppresses `SIGPIPE` (a peer that
/// closed mid-stream surfaces as a negative errno, not a process-killing
/// signal).
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `buf` must point to `len` readable
/// bytes. A bad fd or a closed peer maps to a negative errno.
pub type FdWriteFn = unsafe extern "C" fn(fd: Fd, buf: *const u8, len: usize) -> i64;

/// The fd-close primitive: closes `fd`, returning `0` or a negative errno.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `fd` is a plain scalar; closing an
/// already-closed fd maps to a negative errno, not UB.
pub type FdCloseFn = unsafe extern "C" fn(fd: Fd) -> i64;

/// The wall-clock primitive: reads a real-time clock, returning whole
/// nanoseconds since the Unix epoch (1970-01-01 00:00:00 UTC).
///
/// Distinct from [`ClockFn`]: that slot is MONOTONIC (never steps, unspecified
/// epoch, for ordering durations); this is WALL-CLOCK (can step under NTP/manual
/// set, Unix epoch, for a human-readable timestamp anchor). A flat `i64` nanos
/// return — not a `(secs, nanos)` struct — keeps the FFI return an ABI-trivial
/// scalar with no layout freeze (AB15) for zero benefit; the i64-nanos horizon
/// reaches year ~2262.
///
/// # Safety
///
/// `unsafe extern "C"` to share the vtable's one ABI shape with the effectful
/// slots; the underlying clock read has no precondition, so a caller upholds
/// nothing beyond the handle being installed.
pub type RealtimeFn = unsafe extern "C" fn() -> i64;

/// The file-open primitive.
///
/// Opens the NUL-terminated pathname `path` (`path_len` bytes) with the Linux
/// open `flags` and creation `mode`, returning the new fd (`>= 0`) or a negative
/// errno. The directory base is the provider's current working directory
/// (`AT_FDCWD`-relative); a consumer passes only an absolute or cwd-relative
/// path.
///
/// fd-based syscall surface, the sibling of [`UnixConnectFn`] for files: the
/// contract carries the path as `*const u8` + `usize`, the open `flags`/`mode`
/// as the Linux-ABI `i32`/`u32` scalars, and reports errors as a negative-errno
/// return. The `flags` values are Linux open-flag constants (an ABI vocabulary,
/// not internal policy), so exporting them across `#[repr(C)]` is correct. The
/// consumer (`lib/ds`'s `File`) maps a negative return to a typed error and
/// wraps the fd; write/read/close then route through the existing fd-op slots.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `path` must point to `path_len`
/// readable bytes containing a NUL-terminated pathname; the provider reads no
/// further. A malformed path or a refused open maps to a negative-errno return,
/// not UB.
pub type FileOpenFn =
    unsafe extern "C" fn(path: *const u8, path_len: usize, flags: OpenFlags, mode: Mode) -> i64;

/// The thread-id primitive: returns the calling thread's kernel thread id
/// (`>= 0`).
///
/// The platform identity syscall (`gettid`-shaped) the kernel uses to stamp a
/// service row with its owning thread. A flat `i64` return matches the other
/// scalar slots; a valid tid is a small positive integer well within `i32`.
///
/// # Safety
///
/// `unsafe extern "C"` to share the vtable's one ABI shape; the underlying
/// thread-id read has no precondition and cannot fail, so a caller upholds
/// nothing beyond the handle being installed.
pub type ThreadIdFn = unsafe extern "C" fn() -> i64;

/// The plain byte-write primitive: writes up to `len` bytes from `buf` to `fd`
/// with `write(2)` semantics, returning the byte count or a negative errno.
///
/// The file/stdio companion to [`FileOpenFn`], distinct from [`FdWriteFn`]:
/// that slot is a stream `send` that suppresses `SIGPIPE` for the socket
/// carrier, and `send` is valid only on a socket fd. This slot is the ordinary
/// `write(2)` that works on any fd — a regular file (the kernel log sink), a
/// pipe, or a tty (the tui's stdout). The two write paths split on the
/// `lib/ds` module boundary: `net` uses [`FdWriteFn`], `fs` uses this slot.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `buf` must point to `len` readable
/// bytes; the provider reads at most that many.
pub type FileWriteFn = unsafe extern "C" fn(fd: Fd, buf: *const u8, len: usize) -> i64;

/// The thread-spawn primitive: starts a detached thread that calls
/// `entry(arg)`, returning the new thread id (`>= 0`) or a negative errno.
///
/// pthread_create-shaped: a single C-ABI entry taking one type-erased argument.
/// The consumer (`lib/ds`'s thread-spawn helper) boxes a Rust closure into a
/// provider-allocated block, hands the thin block pointer as `arg`, and
/// supplies a monomorphized `extern "C"` trampoline as `entry` that reconstructs
/// and runs the closure. The thread is **detached**: the provider owns the
/// thread's stack and frees nothing on exit (the only floor consumer — the
/// server accept/connection loops — never joins; a joinable handle is deferred
/// until a real joiner appears, rule of three).
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `entry` must be a valid function
/// that runs to completion (or exits the thread) given `arg`; `arg` must be
/// valid for the duration of `entry`'s execution. The provider transfers
/// ownership of `arg` to the new thread.
pub type ThreadSpawnFn =
    unsafe extern "C" fn(entry: unsafe extern "C" fn(*mut u8), arg: *mut u8) -> i64;

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
/// `dealloc`, `park`, `unpark`, `unpark_all`, then the SP05 net/thread slots
/// — is append-only. A future primitive is a new trailing field, never a
/// reorder or removal, so a binary built against an older slot set stays
/// ABI-compatible. SP02 wired `clock` end-to-end; SP03 wires
/// `alloc`/`dealloc`/`park`/`unpark`/`unpark_all` through the safe wrappers
/// below (consumed by `lib/ds`). SP05 appends the net fd-op slots
/// (`unix_connect`/`unix_listen`/`unix_accept`/`fd_read`/`fd_write`/`fd_close`)
/// and `thread_spawn` (AB3 trailing append) so `lib/ds` can build
/// `UnixStream`/`UnixListener` Math types and a detached thread-spawn helper
/// without naming `arch`. SP05 also appends `realtime` (wall-clock anchor),
/// `file_open` (open a file by path), and `thread_id` (the calling thread's
/// tid) so the kernel's clock/log-sink/service-registration and the tui's
/// stdio reach time + file + thread-identity backends through the handle.
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
    /// Wake every thread parked on `word` (SP03-consumed; AB3 trailing append).
    pub unpark_all: UnparkAllFn,
    /// Connect a Unix-domain stream socket (SP05; AB3 trailing append).
    pub unix_connect: UnixConnectFn,
    /// Bind + listen a Unix-domain stream socket (SP05; AB3 trailing append).
    pub unix_listen: UnixListenFn,
    /// Accept a connection on a listening fd (SP05; AB3 trailing append).
    pub unix_accept: UnixAcceptFn,
    /// Read bytes from an fd (SP05; AB3 trailing append).
    pub fd_read: FdReadFn,
    /// Write bytes to an fd, SIGPIPE-suppressed (SP05; AB3 trailing append).
    pub fd_write: FdWriteFn,
    /// Close an fd (SP05; AB3 trailing append).
    pub fd_close: FdCloseFn,
    /// Spawn a detached thread running a C-ABI entry (SP05; AB3 trailing
    /// append).
    pub thread_spawn: ThreadSpawnFn,
    /// Read the wall clock in Unix-epoch nanoseconds (SP05; AB3 trailing
    /// append).
    pub realtime: RealtimeFn,
    /// Open a file, returning its fd (SP05; AB3 trailing append).
    pub file_open: FileOpenFn,
    /// Read the calling thread's kernel thread id (SP05; AB3 trailing append).
    pub thread_id: ThreadIdFn,
    /// Plain `write(2)` to any fd — file or stdio (SP05; AB3 trailing append).
    pub file_write: FileWriteFn,
}

// `PlatformVtable` is `Sync` automatically: it holds only `unsafe extern "C"`
// function pointers, which are `Send + Sync` (a code address shared read-only
// across threads carries no interior state). No explicit `unsafe impl Sync` is
// needed — the auto-derived bound suffices for the `'static HANDLE` static and
// the `&'static` install path.

impl PlatformVtable {
    /// Allocates `layout.size()` bytes aligned to `layout.align()`, mapping the
    /// raw C-ABI null return back to [`AllocError`].
    ///
    /// The safe Rust face of the [`alloc`](PlatformVtable::alloc) slot: it
    /// builds the `(size, align)` pair from `layout`, calls the provider's
    /// primitive, and converts the FFI-shaped `*mut u8` (null = failure) into a
    /// `Result<NonNull<u8>, AllocError>`. Consumers (`lib/ds`) stay safe — the
    /// `unsafe extern "C"` call is encapsulated here, in the floor-contract
    /// tier.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] when the provider returns null (a refused
    /// allocation, or a layout the provider's backend cannot represent).
    pub fn alloc(&self, layout: Layout) -> Result<NonNull<u8>, AllocError> {
        // SAFETY: the `alloc` slot was installed from a provider's
        // `unsafe extern "C"` allocator; `layout.align()` is a power of two and
        // `layout.size()` is the requested size. A null return is the
        // contract's "allocation refused" signal, mapped to `AllocError` below.
        let ptr = unsafe { (self.alloc)(layout.size(), layout.align()) };
        NonNull::new(ptr).ok_or(AllocError)
    }

    /// Frees a block previously returned by [`alloc`](PlatformVtable::alloc) for
    /// the same `layout`.
    ///
    /// # Panics
    ///
    /// Does not panic. `ptr`/`layout` must name a live allocation from a prior
    /// [`alloc`](PlatformVtable::alloc) with the identical `layout`; the safe
    /// wrapper takes a `NonNull<u8>` so a null can never reach the slot.
    pub fn dealloc(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: by the caller's contract `ptr`/`layout` are the exact pair a
        // prior `alloc` returned and the block is still live; the provider's
        // `dealloc` upholds the rest. `NonNull` guarantees a non-null pointer.
        unsafe { (self.dealloc)(ptr.as_ptr(), layout.size(), layout.align()) }
    }

    /// Blocks the calling thread while `word` still holds `expected`, returning
    /// when woken by [`unpark`](PlatformVtable::unpark)/
    /// [`unpark_all`](PlatformVtable::unpark_all) or spuriously.
    ///
    /// Takes a shared `&AtomicU32` so the address handed to the futex primitive
    /// is always a live, aligned `u32`. Spurious returns are permitted — the
    /// caller re-checks its own condition in a loop.
    pub fn park(&self, word: &AtomicU32, expected: u32) {
        let addr = core::ptr::from_ref(word).cast::<u32>();
        // SAFETY: `word` is a live, aligned `&AtomicU32` borrowed for the call,
        // so `addr` points at a valid `u32` across the (possibly blocking)
        // park. The provider only reads `*addr` and may block.
        unsafe { (self.park)(addr, expected) }
    }

    /// Wakes one thread parked on `word` (the matched
    /// [`park`](PlatformVtable::park) counterpart). A no-op when no thread is
    /// parked.
    pub fn unpark(&self, word: &AtomicU32) {
        let addr = core::ptr::from_ref(word).cast::<u32>();
        // SAFETY: `word` is a live, aligned `&AtomicU32`; the provider reads no
        // value beyond the futex key and waking an empty key is a no-op.
        unsafe { (self.unpark)(addr) }
    }

    /// Wakes every thread parked on `word`, for the wake-all sync paths
    /// (`Condvar::notify_all`, `RwLock` release). A no-op when none are parked.
    pub fn unpark_all(&self, word: &AtomicU32) {
        let addr = core::ptr::from_ref(word).cast::<u32>();
        // SAFETY: as `unpark`: `word` is a live, aligned `&AtomicU32`; the
        // provider wakes the whole cohort and an empty key is a no-op.
        unsafe { (self.unpark_all)(addr) }
    }

    /// Connects a Unix-domain stream socket to the NUL-terminated `path`,
    /// returning the connected fd.
    ///
    /// The safe Rust face of the [`unix_connect`](PlatformVtable::unix_connect)
    /// slot: it passes the path slice's pointer + length to the provider and
    /// maps a negative-errno return to [`Errno`]. `lib/ds`'s `UnixStream`
    /// stays safe — the `unsafe extern "C"` call is encapsulated here.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] when the connect fails (no listener, missing path,
    /// malformed address).
    pub fn unix_connect(&self, path: &[u8]) -> Result<i32, Errno> {
        // SAFETY: `path` is a live slice borrowed for the call, so the pointer
        // is valid for `path.len()` bytes; the provider reads no further and
        // dereferences nothing else.
        let ret = unsafe { (self.unix_connect)(path.as_ptr(), path.len()) };
        map_fd_ret(ret).map(narrow_fd)
    }

    /// Binds + listens a Unix-domain stream socket at the NUL-terminated
    /// `path`, returning the listening fd.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] when the bind or listen fails (malformed path,
    /// permission denied).
    pub fn unix_listen(&self, path: &[u8]) -> Result<i32, Errno> {
        // SAFETY: as `unix_connect`: `path` is a live slice; the pointer is
        // valid for `path.len()` bytes and the provider reads no further.
        let ret = unsafe { (self.unix_listen)(path.as_ptr(), path.len()) };
        map_fd_ret(ret).map(narrow_fd)
    }

    /// Accepts one connection on the listening `listener_fd`, returning the
    /// connected fd. Blocks until a peer connects.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] when accept fails (the listener fd was closed, the
    /// socket is not listening).
    pub fn unix_accept(&self, listener_fd: i32) -> Result<i32, Errno> {
        // SAFETY: `listener_fd` is a scalar (passed as the canonical `Fd`
        // newtype, ABI-identical to `i32`); the slot dereferences no memory.
        let ret = unsafe { (self.unix_accept)(Fd(listener_fd)) };
        map_fd_ret(ret).map(narrow_fd)
    }

    /// Reads up to `buf.len()` bytes from `fd` into `buf`, returning the byte
    /// count (`0` at end-of-stream).
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on a read failure (bad fd, interrupted call the
    /// provider did not retry).
    pub fn fd_read(&self, fd: i32, buf: &mut [u8]) -> Result<usize, Errno> {
        // SAFETY: `buf` is a live mutable slice; its pointer is valid for
        // `buf.len()` writable bytes, and the provider writes at most the
        // returned count. `fd` is passed as the canonical `Fd` newtype.
        let ret = unsafe { (self.fd_read)(Fd(fd), buf.as_mut_ptr(), buf.len()) };
        map_fd_ret(ret)
    }

    /// Writes up to `buf.len()` bytes from `buf` to `fd`, returning the byte
    /// count written (SIGPIPE-suppressed by the provider).
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on a write failure (bad fd, closed peer → the
    /// provider's `EPIPE`-class error).
    pub fn fd_write(&self, fd: i32, buf: &[u8]) -> Result<usize, Errno> {
        // SAFETY: `buf` is a live slice; its pointer is valid for `buf.len()`
        // readable bytes. `fd` is passed as the canonical `Fd` newtype.
        let ret = unsafe { (self.fd_write)(Fd(fd), buf.as_ptr(), buf.len()) };
        map_fd_ret(ret)
    }

    /// Closes `fd`.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] when the close fails (already-closed fd).
    pub fn fd_close(&self, fd: i32) -> Result<(), Errno> {
        // SAFETY: `fd` is a scalar (passed as the canonical `Fd` newtype); the
        // slot dereferences no memory.
        let ret = unsafe { (self.fd_close)(Fd(fd)) };
        map_fd_ret(ret).map(|_| ())
    }

    /// Spawns a detached thread that calls `entry(arg)`, returning the new
    /// thread id.
    ///
    /// The safe Rust face of the [`thread_spawn`](PlatformVtable::thread_spawn)
    /// slot. The CALLER (`lib/ds`'s thread-spawn helper) is responsible for
    /// `entry`/`arg` validity — `entry` must be a valid function that runs to
    /// completion given `arg`, and the new thread takes ownership of `arg`.
    /// This method is itself `unsafe` because those obligations cannot be
    /// expressed in the type system: a wrong `entry`/`arg` pairing is UB inside
    /// the new thread.
    ///
    /// # Safety
    ///
    /// `entry` must be a valid `extern "C"` function safe to call with `arg`,
    /// and `arg` must be valid for the whole of `entry`'s execution. Ownership
    /// of `arg` transfers to the spawned thread.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] when the spawn fails (the underlying `clone` was
    /// refused, or a stack could not be mapped).
    pub unsafe fn thread_spawn(
        &self,
        entry: unsafe extern "C" fn(*mut u8),
        arg: *mut u8,
    ) -> Result<i32, Errno> {
        // SAFETY: the caller upholds `entry`/`arg` validity (this method's
        // contract); the slot starts a thread and returns a tid or -errno.
        let ret = unsafe { (self.thread_spawn)(entry, arg) };
        map_fd_ret(ret).map(narrow_fd)
    }

    /// Reads the monotonic clock, returning whole nanoseconds since an
    /// unspecified epoch.
    ///
    /// The safe Rust face of the [`clock`](PlatformVtable::clock) slot — the
    /// monotonic counterpart to [`realtime`](PlatformVtable::realtime). The
    /// reading never steps backward, so the difference of two readings is a
    /// non-negative duration; use it for ordering and durations, not for a
    /// human-readable timestamp. (A field named `clock` and this method named
    /// `clock` coexist: the field is the raw fn pointer, the method is its safe
    /// face.)
    #[must_use]
    pub fn clock(&self) -> i64 {
        // SAFETY: the `clock` slot was installed from a provider's
        // `unsafe extern "C"` monotonic read, which has no precondition.
        unsafe { (self.clock)() }
    }

    /// Reads the wall clock, returning whole nanoseconds since the Unix epoch.
    ///
    /// The safe Rust face of the [`realtime`](PlatformVtable::realtime) slot.
    /// The reading can step (NTP, manual clock set), so it is a human-readable
    /// timestamp anchor, never a basis for ordering — use the monotonic
    /// [`clock`](PlatformVtable::clock) slot for durations.
    #[must_use]
    pub fn realtime(&self) -> i64 {
        // SAFETY: the `realtime` slot was installed from a provider's
        // `unsafe extern "C"` clock read, which has no precondition.
        unsafe { (self.realtime)() }
    }

    /// Opens the NUL-terminated `path` with the Linux open `flags` and creation
    /// `mode`, returning the new fd.
    ///
    /// The safe Rust face of the [`file_open`](PlatformVtable::file_open) slot:
    /// it passes the path slice's pointer + length and the `flags`/`mode`
    /// values to the provider and maps a negative-errno return to [`Errno`].
    /// `lib/ds`'s `File` stays safe — the `unsafe extern "C"` call is
    /// encapsulated here. `flags`/`mode` are the canonical POSIX
    /// [`OpenFlags`]/[`Mode`] the consumer supplies.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] when the open fails (missing path, permission
    /// denied, a malformed pathname).
    pub fn file_open(&self, path: &[u8], flags: OpenFlags, mode: Mode) -> Result<i32, Errno> {
        // SAFETY: `path` is a live slice borrowed for the call, so the pointer
        // is valid for `path.len()` bytes; the provider reads no further.
        let ret = unsafe { (self.file_open)(path.as_ptr(), path.len(), flags, mode) };
        map_fd_ret(ret).map(narrow_fd)
    }

    /// Returns the calling thread's kernel thread id.
    ///
    /// The safe Rust face of the [`thread_id`](PlatformVtable::thread_id) slot.
    /// The id cannot fail, so it returns a plain `i32` (a tid is a small
    /// positive integer well within `i32`).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn thread_id(&self) -> i32 {
        // SAFETY: the `thread_id` slot was installed from a provider's
        // `unsafe extern "C"` thread-id read, which has no precondition and
        // cannot fail.
        let ret = unsafe { (self.thread_id)() };
        // A tid is a small positive integer (< 2^31), so the narrowing to the
        // i32 the service-registration consumer uses cannot wrap.
        ret as i32
    }

    /// Writes up to `buf.len()` bytes from `buf` to `fd` with `write(2)`
    /// semantics, returning the byte count written.
    ///
    /// The safe Rust face of the [`file_write`](PlatformVtable::file_write)
    /// slot — the plain-write counterpart to
    /// [`fd_write`](PlatformVtable::fd_write) (a socket `send`). Use it for
    /// non-socket fds: a regular file, a pipe, or a tty. `lib/ds`'s `fs`
    /// surface routes here; `net` routes through `fd_write`.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on a write failure (bad fd, full disk, a broken
    /// pipe → the provider's `EPIPE`-class error).
    pub fn file_write(&self, fd: i32, buf: &[u8]) -> Result<usize, Errno> {
        // SAFETY: `buf` is a live slice; its pointer is valid for `buf.len()`
        // readable bytes.
        let ret = unsafe { (self.file_write)(Fd(fd), buf.as_ptr(), buf.len()) };
        map_fd_ret(ret)
    }
}

/// Narrows a non-negative `usize` (a fd or tid the slot returned) to the `i32`
/// the fd-op ABI uses. A valid fd / tid is always `< 2^31`, so the cast cannot
/// wrap; the `#[allow]` documents the proven-in-range narrowing.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
const fn narrow_fd(v: usize) -> i32 {
    v as i32
}

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
/// use reovim_uapi_posix::{Fd, Mode, OpenFlags};
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
/// unsafe extern "C" fn unpark_all_stub(word: *const u32) { let _ = word; }
/// unsafe extern "C" fn connect_stub(p: *const u8, n: usize) -> i64 { let _ = (p, n); -1 }
/// unsafe extern "C" fn listen_stub(p: *const u8, n: usize) -> i64 { let _ = (p, n); -1 }
/// unsafe extern "C" fn accept_stub(fd: Fd) -> i64 { let _ = fd; -1 }
/// unsafe extern "C" fn read_stub(fd: Fd, b: *mut u8, n: usize) -> i64 {
///     let _ = (fd, b, n);
///     0
/// }
/// unsafe extern "C" fn write_stub(fd: Fd, b: *const u8, n: usize) -> i64 {
///     let _ = (fd, b, n);
///     0
/// }
/// unsafe extern "C" fn close_stub(fd: Fd) -> i64 { let _ = fd; 0 }
/// unsafe extern "C" fn spawn_stub(entry: unsafe extern "C" fn(*mut u8), arg: *mut u8) -> i64 {
///     let _ = (entry, arg);
///     -1
/// }
/// unsafe extern "C" fn realtime_stub() -> i64 { 0 }
/// unsafe extern "C" fn file_open_stub(p: *const u8, n: usize, f: OpenFlags, m: Mode) -> i64 {
///     let _ = (p, n, f, m);
///     -1
/// }
/// unsafe extern "C" fn thread_id_stub() -> i64 { 1 }
/// unsafe extern "C" fn file_write_stub(fd: Fd, b: *const u8, n: usize) -> i64 {
///     let _ = (fd, b, n);
///     0
/// }
///
/// static TABLE: PlatformVtable = PlatformVtable {
///     clock:        clock_stub,
///     alloc:        alloc_stub,
///     dealloc:      dealloc_stub,
///     park:         park_stub,
///     unpark:       unpark_stub,
///     unpark_all:   unpark_all_stub,
///     unix_connect: connect_stub,
///     unix_listen:  listen_stub,
///     unix_accept:  accept_stub,
///     fd_read:      read_stub,
///     fd_write:     write_stub,
///     fd_close:     close_stub,
///     thread_spawn: spawn_stub,
///     realtime:     realtime_stub,
///     file_open:    file_open_stub,
///     thread_id:    thread_id_stub,
///     file_write:   file_write_stub,
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
