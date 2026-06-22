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

/// File-open flags word carried by the down-face platform contract.
///
/// This is a `#[repr(transparent)]` wrapper over the Linux ABI `i32` flag word.
/// It lives in `kabi/platform` so providers and the system-kernel bridge do not
/// import up-face POSIX vocabulary directly.
///
/// ```rust
/// use reovim_kabi_platform::OpenFlags;
///
/// assert_eq!(OpenFlags(0o100).bits(), 0o100);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct OpenFlags(pub i32);

impl OpenFlags {
    /// Returns the raw flag bit pattern.
    ///
    /// ```rust
    /// use reovim_kabi_platform::OpenFlags;
    ///
    /// assert_eq!(OpenFlags(0o2_000_000).bits(), 0o2_000_000);
    /// ```
    #[must_use]
    pub const fn bits(self) -> i32 {
        self.0
    }
}

impl core::ops::BitOr for OpenFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Open for reading only.
pub const O_RDONLY: OpenFlags = OpenFlags(0);

/// Open for writing only.
pub const O_WRONLY: OpenFlags = OpenFlags(0o1);

/// Create the file if it does not exist.
pub const O_CREAT: OpenFlags = OpenFlags(0o100);

/// Truncate the file to zero length on open.
pub const O_TRUNC: OpenFlags = OpenFlags(0o1000);

/// Append writes to the end of the file.
pub const O_APPEND: OpenFlags = OpenFlags(0o2000);

/// Close the descriptor on exec.
pub const O_CLOEXEC: OpenFlags = OpenFlags(0o2_000_000);

/// File creation mode carried by the down-face platform contract.
///
/// ```rust
/// use reovim_kabi_platform::Mode;
///
/// assert_eq!(Mode(0o644).bits(), 0o644);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Mode(pub u32);

impl Mode {
    /// Returns the raw permission bit pattern.
    ///
    /// ```rust
    /// use reovim_kabi_platform::Mode;
    ///
    /// assert_eq!(Mode(0o600).bits(), 0o600);
    /// ```
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

/// File descriptor scalar carried by the down-face platform contract.
///
/// ```rust
/// use reovim_kabi_platform::Fd;
///
/// assert_eq!(Fd(3).as_i32(), 3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Fd(pub i32);

impl Fd {
    /// Returns the raw file descriptor value.
    ///
    /// ```rust
    /// use reovim_kabi_platform::Fd;
    ///
    /// assert_eq!(Fd(5).as_i32(), 5);
    /// ```
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self.0
    }
}

/// Positive errno code carried by the down-face platform contract.
///
/// The vtable ABI returns negative `-errno`; safe wrappers convert that to this
/// positive diagnostic code.
///
/// ```rust
/// use reovim_kabi_platform::Errno;
///
/// assert_eq!(Errno::from_code(2).code(), 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Errno(pub i32);

impl Errno {
    /// Builds an errno from a positive code.
    ///
    /// ```rust
    /// use reovim_kabi_platform::Errno;
    ///
    /// assert_eq!(Errno::from_code(22).code(), 22);
    /// ```
    #[must_use]
    pub const fn from_code(code: i32) -> Self {
        Self(code)
    }

    /// Returns the positive errno code.
    ///
    /// ```rust
    /// use reovim_kabi_platform::Errno;
    ///
    /// assert_eq!(Errno(9).code(), 9);
    /// ```
    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }
}

const _: () = assert!(core::mem::size_of::<OpenFlags>() == core::mem::size_of::<i32>());
const _: () = assert!(core::mem::align_of::<OpenFlags>() == core::mem::align_of::<i32>());
const _: () = assert!(core::mem::size_of::<Mode>() == core::mem::size_of::<u32>());
const _: () = assert!(core::mem::align_of::<Mode>() == core::mem::align_of::<u32>());
const _: () = assert!(core::mem::size_of::<Fd>() == core::mem::size_of::<i32>());
const _: () = assert!(core::mem::align_of::<Fd>() == core::mem::align_of::<i32>());
const _: () = assert!(core::mem::size_of::<Errno>() == core::mem::size_of::<i32>());
const _: () = assert!(core::mem::align_of::<Errno>() == core::mem::align_of::<i32>());

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

// ── Device inventory (push-at-entry static data) ──────────────────────────────

/// The coarse device class assigned to a statically-enumerated DTB node.
///
/// This is a *Declared* / *Present* classification — the class the firmware
/// device tree claims the device is. It is not a driver-lifecycle state: the
/// kernel receives this once at entry and holds it without modifying it.
/// Driver-specific details (interrupt routing, device-specific register maps,
/// power domains) are driver concerns and are not encoded here.
///
/// ```rust
/// use reovim_kabi_platform::DeviceClass;
///
/// assert_eq!(DeviceClass::Uart, DeviceClass::Uart);
/// ```
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    /// A UART serial port (e.g. `arm,pl011`).
    Uart = 0,
    /// A generic interrupt controller (e.g. `arm,gic-400`).
    Interrupt = 1,
    /// A firmware mailbox channel (e.g. `brcm,bcm2835-mbox`).
    Mailbox = 2,
    /// A block storage device.
    Block = 3,
    /// A USB host controller or device.
    Usb = 4,
    /// A device whose compatible string was not recognised by the enumerator.
    Unknown = 255,
}

/// One statically-enumerated device node: the one-shot, static facts the
/// enumerator extracts from the firmware device tree at boot.
///
/// These are *Declared* / *Present* facts — what the device tree says the
/// device is, not what a driver has discovered at runtime. Fields like `irq`
/// and `capacity_bytes` carry honest "absent" sentinels (`u32::MAX` / `0`)
/// where only a driver (or further firmware interrogation) could fill them in.
/// The kernel stores this inventory for later driver-discovery phases.
///
/// ```rust
/// use reovim_kabi_platform::{DeviceClass, DeviceEntry};
///
/// let entry = DeviceEntry {
///     class: DeviceClass::Uart,
///     mmio_base: 0x7e20_1000,
///     mmio_len: 0x200,
///     irq: u32::MAX,
///     capacity_bytes: 0,
///     compatible: "arm,pl011",
/// };
/// assert_eq!(entry.compatible, "arm,pl011");
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DeviceEntry {
    /// Coarse device class, derived from the caller-supplied compatible-string
    /// classifier.
    pub class: DeviceClass,
    /// MMIO base address from the node's `reg` property; `0` when absent.
    pub mmio_base: u64,
    /// MMIO region length in bytes from the node's `reg` property; `0` when absent.
    pub mmio_len: u64,
    /// Primary interrupt number — the first cell of the `interrupts` property.
    /// `u32::MAX` when the property is absent or this enumerator does not decode
    /// it yet.
    pub irq: u32,
    /// Block device capacity in bytes. `0` for non-block devices and for block
    /// devices where the capacity is not derivable from the DTB alone (a driver
    /// must query the hardware).
    pub capacity_bytes: u64,
    /// The first NUL-separated string from the `compatible` property, backed by
    /// the DTB byte slice (process-lifetime storage). Empty string `""` when the
    /// property was absent or contained non-UTF-8 bytes.
    pub compatible: &'static str,
}

/// The static device inventory discovered from the firmware device tree and
/// pushed into the kernel at entry alongside [`BootInfo`].
///
/// Like [`BootInfo`], this is one-shot *static data* delivered push-at-entry.
/// The `devices` slice is backed by the boot arena (process-lifetime storage).
/// The empty default is valid: a hosted build or a freestanding target without
/// a DTB carries no inventory, and the kernel reports only what is present.
///
/// ```rust
/// use reovim_kabi_platform::DeviceInventory;
///
/// assert!(DeviceInventory::default().devices.is_empty());
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DeviceInventory {
    /// The enumerated device entries. Backed by arena storage that lives for
    /// the process lifetime, hence `'static`. Empty on hosted builds and on
    /// freestanding targets without a DTB.
    pub devices: &'static [DeviceEntry],
}

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
    ///
    /// The CPU *vendor* is not a separate field: it is the implementer byte of
    /// `cpu_id` (`MIDR_EL1[31:24]`), decoded where it is reported.
    pub cpu_id: u32,
    /// Logical CPU count (`1` on the current single-core floor; `0` when unknown).
    pub cpu_count: u32,
    /// Minimum cache line size in bytes (`CTR_EL0` on ARM), or `0` when unknown.
    /// The smallest line across the cache levels — the granule cache-maintenance
    /// and false-sharing reasoning are sized against.
    pub cache_line_bytes: u32,
    /// L1 data cache size in bytes (`sets × ways × line`, from `CCSIDR_EL1` for
    /// the level-1 data cache selected via `CSSELR_EL1`), or `0` when absent or
    /// unknown.
    pub l1d_bytes: u32,
    /// L1 instruction cache size in bytes (same derivation as [`l1d_bytes`] for
    /// the level-1 instruction cache), or `0` when absent or unknown.
    ///
    /// [`l1d_bytes`]: BootInfo::l1d_bytes
    pub l1i_bytes: u32,
    /// Unified L2 cache size in bytes (same derivation for the level-2 cache),
    /// or `0` when absent or unknown.
    pub l2_bytes: u32,
    /// CPU affinity register `MPIDR_EL1` (the `Aff0..Aff3` topology fields). Bit
    /// 31 is RES1 on `ARMv8`, so a value with bit 31 clear signals an unreadable
    /// register. `0` when unknown.
    pub cpu_affinity: u64,
    /// Memory (SDRAM) clock frequency in Hz, from the firmware mailbox
    /// `GET_CLOCK_RATE` on ARM, or `0` when the firmware does not report it.
    pub mem_freq_hz: u64,
    /// Total capacity in bytes of the floor's page arena — a one-shot *static*
    /// fact (the arena's fixed size), not live used/free, which is mutable
    /// runtime-services state. `0` when unknown.
    pub heap_total_bytes: u64,
}

/// Maps a fd-op slot's `i64` return (`>= 0` success, `< 0` is `-errno`) to a
/// typed `Result`. The success value is narrowed to the caller's `usize`
/// (a byte count or a non-negative fd both fit).
///
/// The error half is the canonical down-face [`Errno`]: the fd-op slots report
/// a negative-errno return (the Linux raw-syscall convention, FFI-safe across
/// the `extern "C"` boundary), and the safe wrappers recover the positive code
/// into the contract newtype so consumers match on a positive code rather than
/// re-deriving the sign.
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
/// non-negative duration. This was the initial slot wired end-to-end through the
/// handle (provider → vtable → consumer) before allocation and parking joined the
/// same mechanism.
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
/// not FFI-safe. The `lib/ds` consumer maps the null return back to
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
/// own condition in a loop. `lib/ds` consumes this through the handle to build
/// `Mutex` and `Condvar`.
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
/// The fd-based syscall surface carries raw fds as `i32`, paths as
/// `*const u8` + `usize`, and reports errors as a
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
/// pipe, or a tty (the tui's stdout). The two write paths split in the
/// system-kernel bridge: socket writes use [`FdWriteFn`], file/stdout writes
/// use this slot.
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
/// The consumer (`system/lib/kernel::sched`) boxes a Rust closure into a
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

/// The set-raw-mode primitive: puts `fd` into raw (cbreak, no-echo) mode and
/// returns a saved-state token (`>= 0`), or a negative errno (`-ENOTTY` when
/// `fd` is not a terminal).
///
/// The narrow, policy-free half of the termios contract (the other half is
/// [`TermRestoreFn`]). The provider owns ALL mechanism: it reads the current
/// termios, decides which flags to clear (the raw-mode policy), writes the raw
/// termios, and stashes the saved state keyed by the returned token. No
/// `Termios` layout and no flag bit crosses the contract — only `(fd) → token`
/// (master invariant 3: canonicalization/mechanism lives in the provider).
///
/// The token is fd-keyed (the token sub-decision): the provider holds the saved
/// state in a fixed fd→state table and returns the fd as the token, so
/// [`TermRestoreFn`] restores by fd alone. `-ENOTTY` is the not-a-tty signal a
/// consumer matches to take its no-raw-mode fallback arm (a pipe/`/dev/null`
/// stdin, not a terminal).
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `fd` is a plain scalar; the slot
/// dereferences no memory the caller passes. A non-tty or bad fd maps to a
/// negative errno, not UB.
pub type TermSetRawFn = unsafe extern "C" fn(fd: i32) -> i64;

/// The restore-cooked-mode primitive: restores the saved termios for `fd` (the
/// `term_set_raw` counterpart) and clears the provider's saved-state entry.
///
/// **Idempotent**: restoring a `fd` with no live saved entry (already restored,
/// or never raw) is a best-effort no-op, never an error. This is what makes the
/// tui panic hook's [`RawMode::restore_fd`] sound even though it runs after the
/// guard's `Drop` has already restored — a double-restore is harmless because
/// the second call finds the entry cleared and does nothing.
///
/// Fd-keyed (the token sub-decision): `fd` IS the token, so the panic hook
/// (which fires after the guard is gone) restores without needing the guard's
/// state — the provider's fd-keyed table still holds the entry.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `fd` is a plain scalar; the slot
/// dereferences no memory the caller passes. An unknown fd is a no-op, not UB.
pub type TermRestoreFn = unsafe extern "C" fn(fd: i32);

/// The path-unlink primitive: removes the NUL-terminated pathname `path`
/// (`path_len` bytes), returning `0` on success or a negative errno.
///
/// The contract deliberately carries only the path. Directory-fd and flag
/// vocabulary stay below the bridge; the current provider maps this to
/// `AT_FDCWD` + no flags for socket/file cleanup.
///
/// # Safety
///
/// `unsafe extern "C"` per the vtable ABI. `path` must point to `path_len`
/// readable bytes containing a NUL-terminated pathname; the provider reads no
/// further. A missing path maps to a negative errno, not UB.
///
/// ```rust
/// use reovim_kabi_platform::PathUnlinkFn;
///
/// unsafe extern "C" fn unlink_stub(path: *const u8, len: usize) -> i64 {
///     let _ = (path, len);
///     0
/// }
///
/// let _f: PathUnlinkFn = unlink_stub;
/// ```
pub type PathUnlinkFn = unsafe extern "C" fn(path: *const u8, path_len: usize) -> i64;

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
/// `#[repr(C)]` freezes the field layout; the order is append-only. A future
/// primitive is a new trailing field, never a reorder or removal, so a binary
/// built against an older slot set stays ABI-compatible. The base slots cover
/// monotonic time, allocation, and parking. The trailing fd-op slots
/// (`unix_connect`/`unix_listen`/`unix_accept`/`fd_read`/`fd_write`/`fd_close`)
/// and `thread_spawn` let the system-kernel bridge build UDS and detached
/// thread-spawn services without naming `arch`. `realtime`, `file_open`,
/// `thread_id`, and `file_write` let kernel clock/log/service-registration and
/// tui stdio reach time, file, thread-identity, and plain-write backends through
/// the handle. The two termios slots (`term_set_raw`/`term_restore`) let the
/// tui's raw-mode entry/restore reach a terminal backend through the handle:
/// the contract carries only `(fd) → token`/`(fd) → ()`, with the `Termios`
/// layout and flag policy staying in the provider (master invariant 3).
/// `path_unlink` is another trailing slot used by the system-kernel fs bridge
/// for app-level socket cleanup without exposing `unlinkat` above the bridge.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PlatformVtable {
    /// Monotonic clock read.
    pub clock: ClockFn,
    /// Allocate `(size, align)` bytes; null on failure.
    pub alloc: AllocFn,
    /// Free a block from [`PlatformVtable::alloc`].
    pub dealloc: DeallocFn,
    /// Block while `*word == expected`.
    pub park: ParkFn,
    /// Wake one thread parked on `word`.
    pub unpark: UnparkFn,
    /// Wake every thread parked on `word` (AB3 trailing append).
    pub unpark_all: UnparkAllFn,
    /// Connect a Unix-domain stream socket (AB3 trailing append).
    pub unix_connect: UnixConnectFn,
    /// Bind + listen a Unix-domain stream socket (AB3 trailing append).
    pub unix_listen: UnixListenFn,
    /// Accept a connection on a listening fd (AB3 trailing append).
    pub unix_accept: UnixAcceptFn,
    /// Read bytes from an fd (AB3 trailing append).
    pub fd_read: FdReadFn,
    /// Write bytes to an fd, SIGPIPE-suppressed (AB3 trailing append).
    pub fd_write: FdWriteFn,
    /// Close an fd (AB3 trailing append).
    pub fd_close: FdCloseFn,
    /// Spawn a detached thread running a C-ABI entry (AB3 trailing append).
    pub thread_spawn: ThreadSpawnFn,
    /// Read the wall clock in Unix-epoch nanoseconds (AB3 trailing append).
    pub realtime: RealtimeFn,
    /// Open a file, returning its fd (AB3 trailing append).
    pub file_open: FileOpenFn,
    /// Read the calling thread's kernel thread id (AB3 trailing append).
    pub thread_id: ThreadIdFn,
    /// Plain `write(2)` to any fd — file or stdio (AB3 trailing append).
    pub file_write: FileWriteFn,
    /// Put an fd into raw mode, returning a saved-state token (AB3
    /// trailing append).
    pub term_set_raw: TermSetRawFn,
    /// Restore an fd's saved termios, idempotently (AB3 trailing append).
    pub term_restore: TermRestoreFn,
    /// Unlink a path relative to the provider's current working directory
    /// (AB3 trailing append).
    pub path_unlink: PathUnlinkFn,
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
    /// slot. The CALLER (`system/lib/kernel::sched`) is responsible for
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

    /// Puts `fd` into raw (cbreak, no-echo) mode, returning the saved-state
    /// token (the fd itself, per the fd-keyed design).
    ///
    /// The safe Rust face of the [`term_set_raw`](PlatformVtable::term_set_raw)
    /// slot: the provider does all the termios mechanism (read, flag policy,
    /// write, stash the saved state) and returns a token; this maps a
    /// negative-errno return to [`Errno`]. [`RawMode::enter`] is the RAII
    /// consumer; products stay safe — the `unsafe extern "C"` call is
    /// encapsulated here.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] when raw mode cannot be entered — most importantly
    /// [`ENOTTY`] when `fd` is not a terminal (the not-a-tty signal a consumer
    /// matches to take its no-raw-mode fallback arm).
    pub fn term_set_raw(&self, fd: i32) -> Result<usize, Errno> {
        // SAFETY: `fd` is a plain scalar; the slot dereferences no caller
        // memory. A non-tty/bad fd maps to a negative errno, recovered below.
        let ret = unsafe { (self.term_set_raw)(fd) };
        map_fd_ret(ret)
    }

    /// Restores `fd`'s saved termios (the
    /// [`term_set_raw`](PlatformVtable::term_set_raw) counterpart), idempotently.
    ///
    /// The safe Rust face of the [`term_restore`](PlatformVtable::term_restore)
    /// slot. Restoring an `fd` with no live saved entry is a best-effort no-op
    /// (never an error), which is why both [`RawMode`]'s `Drop` and the panic
    /// hook's [`RawMode::restore_fd`] can call it without coordinating: a
    /// double-restore is harmless.
    pub fn term_restore(&self, fd: i32) {
        // SAFETY: `fd` is a plain scalar; the slot dereferences no caller
        // memory. An unknown fd is a no-op (the idempotency contract).
        unsafe { (self.term_restore)(fd) }
    }

    /// Unlinks the NUL-terminated `path`.
    ///
    /// The safe Rust face of the [`path_unlink`](PlatformVtable::path_unlink)
    /// slot. The provider decides the native mechanism; the current Linux
    /// provider maps this to `unlinkat(AT_FDCWD, path, 0)`.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] when the unlink fails (missing path, permission denied,
    /// malformed pathname).
    ///
    /// ```rust
    /// # use reovim_kabi_platform::{install, Fd, Mode, OpenFlags, PlatformVtable};
    /// # unsafe extern "C" fn clock() -> i64 { 0 }
    /// # unsafe extern "C" fn alloc(_: usize, _: usize) -> *mut u8 { core::ptr::null_mut() }
    /// # unsafe extern "C" fn dealloc(_: *mut u8, _: usize, _: usize) {}
    /// # unsafe extern "C" fn park(_: *const u32, _: u32) {}
    /// # unsafe extern "C" fn unpark(_: *const u32) {}
    /// # unsafe extern "C" fn connect(_: *const u8, _: usize) -> i64 { -1 }
    /// # unsafe extern "C" fn accept(_: Fd) -> i64 { -1 }
    /// # unsafe extern "C" fn read(_: Fd, _: *mut u8, _: usize) -> i64 { -1 }
    /// # unsafe extern "C" fn write(_: Fd, _: *const u8, _: usize) -> i64 { -1 }
    /// # unsafe extern "C" fn close(_: Fd) -> i64 { 0 }
    /// # unsafe extern "C" fn spawn(_: unsafe extern "C" fn(*mut u8), _: *mut u8) -> i64 { -1 }
    /// # unsafe extern "C" fn realtime() -> i64 { 0 }
    /// # unsafe extern "C" fn file_open(_: *const u8, _: usize, _: OpenFlags, _: Mode) -> i64 { -1 }
    /// # unsafe extern "C" fn thread_id() -> i64 { 1 }
    /// # unsafe extern "C" fn term_set_raw(_: i32) -> i64 { -25 }
    /// # unsafe extern "C" fn term_restore(_: i32) {}
    /// unsafe extern "C" fn path_unlink(path: *const u8, len: usize) -> i64 {
    ///     let _ = (path, len);
    ///     0
    /// }
    ///
    /// static TABLE: PlatformVtable = PlatformVtable {
    ///     clock,
    ///     alloc,
    ///     dealloc,
    ///     park,
    ///     unpark,
    ///     unpark_all: unpark,
    ///     unix_connect: connect,
    ///     unix_listen: connect,
    ///     unix_accept: accept,
    ///     fd_read: read,
    ///     fd_write: write,
    ///     fd_close: close,
    ///     thread_spawn: spawn,
    ///     realtime,
    ///     file_open,
    ///     thread_id,
    ///     file_write: write,
    ///     term_set_raw,
    ///     term_restore,
    ///     path_unlink,
    /// };
    ///
    /// install(&TABLE).expect("first install succeeds");
    /// assert_eq!(reovim_kabi_platform::handle().path_unlink(b"/tmp/x\0"), Ok(()));
    /// ```
    pub fn path_unlink(&self, path: &[u8]) -> Result<(), Errno> {
        // SAFETY: `path` is a live slice borrowed for the call, so the pointer
        // is valid for `path.len()` readable bytes; the provider reads no
        // further and maps malformed paths to a negative errno.
        let ret = unsafe { (self.path_unlink)(path.as_ptr(), path.len()) };
        map_fd_ret(ret).map(|_| ())
    }
}

/// Narrows a non-negative `usize` (a fd or tid the slot returned) to the `i32`
/// the fd-op ABI uses. A valid fd / tid is always `< 2^31`, so the cast cannot
/// wrap; the `#[allow]` documents the proven-in-range narrowing.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
const fn narrow_fd(v: usize) -> i32 {
    v as i32
}

// ── termios contract ──────────────────────────────────────────────────

/// `ENOTTY` — "not a typewriter": the errno a terminal-control ioctl returns
/// when the fd is not a terminal.
///
/// The not-a-tty signal of the termios contract:
/// [`term_set_raw`](PlatformVtable::term_set_raw) /
/// [`RawMode::enter`] return `Err(ENOTTY)` when raw mode is requested on a
/// non-terminal fd (a pipe or `/dev/null` stdin), and a consumer matches this
/// arm to continue without raw mode. It lives in `kabi/platform` (not `arch`)
/// because it is part of the termios slots' Rust-facing contract — the one
/// errno a consumer must name to discriminate the fallback — mirroring how
/// [`AllocError`] lives here for the alloc slot. The code is `25` (the Linux
/// errno-space value shared across the floor's `arch-sys-*` crates).
pub const ENOTTY: Errno = Errno::from_code(25);

/// A RAII guard that places a terminal fd into raw (cbreak, no-echo) mode and
/// restores the original settings on `Drop`.
///
/// The `kabi/platform`-owned terminal guard the tui names (it no longer reaches
/// `arch::term`). [`enter`](RawMode::enter) calls the
/// [`term_set_raw`](PlatformVtable::term_set_raw) slot — the provider does all
/// the termios mechanism and stashes the saved state keyed by fd — and `Drop`
/// calls [`term_restore`](PlatformVtable::term_restore). The saved `Termios`
/// never crosses the contract: the guard holds only the `fd` (which is also the
/// fd-keyed token).
///
/// ## The panic-hook restore path
///
/// Under `panic = "abort"` (the workspace profile) `Drop` does not run on a
/// panic, so a panicking tui would leave the terminal raw. The pre-exit hook
/// path restores via [`restore_fd`](RawMode::restore_fd) — a fd-keyed restore
/// that does not need a live guard, because the provider's saved-state table is
/// keyed by fd. The restore is idempotent, so the hook calling it after a normal
/// `Drop` already restored is a harmless no-op.
///
/// ```no_run
/// // Entering raw mode requires a real tty (stdin fd 0) — no_run.
/// use reovim_kabi_platform::RawMode;
///
/// let _raw = RawMode::enter(0).expect("stdin is a tty");
/// // Read raw bytes, paint frames, etc. On drop, cooked mode is restored.
/// ```
pub struct RawMode {
    /// The fd placed into raw mode; also the fd-keyed restore token.
    fd: i32,
}

impl RawMode {
    /// Enters raw mode on `fd`, returning a guard that restores the original
    /// settings on `Drop`.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] when raw mode cannot be entered — [`ENOTTY`] when `fd`
    /// is not a terminal (the consumer's fallback signal), or another errno
    /// (e.g. a bad fd) the provider's terminal read surfaced.
    pub fn enter(fd: i32) -> Result<Self, Errno> {
        handle().term_set_raw(fd)?;
        Ok(Self { fd })
    }

    /// Restores `fd`'s terminal to cooked mode without a live guard — the
    /// fd-keyed entry point the panic pre-exit hook calls.
    ///
    /// The hook fires after the [`RawMode`] guard's `Drop` has already run (or,
    /// on an unwind past the guard, before it), so it cannot route through a
    /// guard instance. Because the provider keys the saved state by fd, this
    /// thin wrapper over [`term_restore`](PlatformVtable::term_restore) restores
    /// correctly on its own. It is idempotent: a double-restore (guard `Drop`
    /// then hook) is a harmless no-op.
    pub fn restore_fd(fd: i32) {
        handle().term_restore(fd);
    }
}

impl Drop for RawMode {
    /// Restores the terminal to its saved cooked settings via the
    /// [`term_restore`](PlatformVtable::term_restore) slot.
    ///
    /// Best-effort: the provider's restore is idempotent and infallible from the
    /// guard's view. Under `panic = "abort"` this `Drop` does not run on a panic
    /// — the panic hook's [`restore_fd`](RawMode::restore_fd) covers that path.
    fn drop(&mut self) {
        handle().term_restore(self.fd);
    }
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
/// use reovim_kabi_platform::{Fd, Mode, OpenFlags, PlatformVtable, install};
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
/// unsafe extern "C" fn term_set_raw_stub(fd: i32) -> i64 { let _ = fd; -25 }
/// unsafe extern "C" fn term_restore_stub(fd: i32) { let _ = fd; }
/// unsafe extern "C" fn path_unlink_stub(p: *const u8, n: usize) -> i64 {
///     let _ = (p, n);
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
///     term_set_raw: term_set_raw_stub,
///     term_restore: term_restore_stub,
///     path_unlink:  path_unlink_stub,
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
