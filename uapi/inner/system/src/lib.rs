//! Product-facing system boot facts and device inventory.
//!
//! These are up-face data records consumed by editor/client layers. Lower
//! providers may gather richer or target-native facts, but those are bridged
//! through `system/lib/kernel` before reaching product code.

#![no_std]

use reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr};

/// Memory-map facts needed by boot diagnostics.
///
/// This intentionally carries the diagnostic view of memory, not the raw
/// firmware range slice. The system-kernel bridge computes it from lower
/// memory-map records so editor code does not parse down-face memory kinds.
///
/// ```rust
/// use reovim_uapi_system::MemorySummary;
///
/// let summary = MemorySummary::new(2, 1024 * 1024);
/// assert_eq!(summary.range_count(), 2);
/// assert!(!summary.is_empty());
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemorySummary {
    range_count: usize,
    usable_bytes: u64,
}

impl MemorySummary {
    /// Creates a memory summary from a firmware-map range count and usable RAM
    /// total.
    ///
    /// ```rust
    /// use reovim_uapi_system::MemorySummary;
    ///
    /// let summary = MemorySummary::new(1, 64 * 1024 * 1024);
    /// assert_eq!(summary.usable_bytes(), 64 * 1024 * 1024);
    /// ```
    #[must_use]
    pub const fn new(range_count: usize, usable_bytes: u64) -> Self {
        Self {
            range_count,
            usable_bytes,
        }
    }

    /// Returns the number of memory-map ranges discovered below the bridge.
    ///
    /// ```rust
    /// use reovim_uapi_system::MemorySummary;
    ///
    /// assert_eq!(MemorySummary::new(3, 0).range_count(), 3);
    /// ```
    #[must_use]
    pub const fn range_count(self) -> usize {
        self.range_count
    }

    /// Returns the number of usable RAM bytes summed below the bridge.
    ///
    /// ```rust
    /// use reovim_uapi_system::MemorySummary;
    ///
    /// assert_eq!(MemorySummary::new(1, 4096).usable_bytes(), 4096);
    /// ```
    #[must_use]
    pub const fn usable_bytes(self) -> u64 {
        self.usable_bytes
    }

    /// Returns whether no memory map was discovered.
    ///
    /// ```rust
    /// use reovim_uapi_system::MemorySummary;
    ///
    /// assert!(MemorySummary::default().is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.range_count == 0
    }
}

/// Hardware facts pushed into editor-core boot diagnostics.
///
/// ```rust
/// use reovim_uapi_system::{BootInfo, MemorySummary};
///
/// let info = BootInfo {
///     memory: MemorySummary::new(1, 512 * 1024 * 1024),
///     cpu_count: 4,
///     ..BootInfo::default()
/// };
/// assert_eq!(info.cpu_count, 4);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BootInfo {
    /// Diagnostic memory-map summary.
    pub memory: MemorySummary,
    /// CPU clock frequency in Hz, or `0` when unknown.
    pub cpu_freq_hz: u64,
    /// CPU identification register value, or `0` when unknown.
    pub cpu_id: u32,
    /// Logical CPU count, or `0` when unknown.
    pub cpu_count: u32,
    /// Minimum cache line size in bytes, or `0` when unknown.
    pub cache_line_bytes: u32,
    /// L1 data cache size in bytes, or `0` when unknown.
    pub l1d_bytes: u32,
    /// L1 instruction cache size in bytes, or `0` when unknown.
    pub l1i_bytes: u32,
    /// Unified L2 cache size in bytes, or `0` when unknown.
    pub l2_bytes: u32,
    /// CPU affinity register value, or `0` when unknown.
    pub cpu_affinity: u64,
    /// Memory clock frequency in Hz, or `0` when unknown.
    pub mem_freq_hz: u64,
    /// Total heap/page-arena capacity in bytes, or `0` when unknown.
    pub heap_total_bytes: u64,
}

/// Coarse static device class.
///
/// ```rust
/// use reovim_uapi_system::DeviceClass;
///
/// assert_eq!(DeviceClass::Uart, DeviceClass::Uart);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    /// UART serial port.
    Uart,
    /// Interrupt controller.
    Interrupt,
    /// Firmware mailbox channel.
    Mailbox,
    /// Block storage controller or device.
    Block,
    /// USB controller or device.
    Usb,
    /// Bus/root complex that can host child devices.
    Bus,
    /// Unrecognized device class.
    Unknown,
}

/// One statically-enumerated device node.
///
/// ```rust
/// use reovim_uapi_system::{DeviceClass, DeviceEntry};
///
/// let entry = DeviceEntry {
///     class: DeviceClass::Uart,
///     mmio_base: 0x1000,
///     mmio_len: 0x100,
///     irq: u32::MAX,
///     capacity_bytes: 0,
///     compatible: "arm,pl011",
/// };
/// assert_eq!(entry.compatible, "arm,pl011");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceEntry {
    /// Coarse device class.
    pub class: DeviceClass,
    /// MMIO base address from firmware, or `0` when absent.
    pub mmio_base: u64,
    /// MMIO region length in bytes, or `0` when absent.
    pub mmio_len: u64,
    /// Primary interrupt number, or `u32::MAX` when absent.
    pub irq: u32,
    /// Block capacity in bytes, or `0` when absent/unknown.
    pub capacity_bytes: u64,
    /// First compatible string, or `""` when absent/invalid.
    pub compatible: &'static str,
}

/// Static device inventory pushed into editor-core launch arguments.
///
/// ```rust
/// use reovim_uapi_system::DeviceInventory;
///
/// assert!(DeviceInventory::default().devices.is_empty());
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeviceInventory {
    /// Enumerated device entries.
    pub devices: &'static [DeviceEntry],
}

/// Product-facing system-control refusal reason.
///
/// These errors describe Reovim system-control semantics, not POSIX errno
/// values. The bridge maps raw transport failures into this vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemError {
    /// This target or kernel build does not support the requested operation.
    Unsupported,
    /// The requested lower-provider probe target is unknown.
    UnknownTarget,
    /// The request arguments or state were invalid for the current operation.
    InvalidRequest,
    /// The operation requires a current process context and none exists.
    NoCurrentProcess,
    /// The requested system-control operation is already in progress.
    Busy,
    /// The bridge reported a non-specific failure.
    Failed,
}

/// System control backed by the raw Reovim syscall transport.
///
/// This adapter keeps system-control semantics in `uapi::system`; only this
/// wrapper packs raw syscall numbers and scalar arguments.
#[derive(Debug, Clone, Copy)]
pub struct SyscallSystemControl {
    raw: RawSyscall,
}

impl SyscallSystemControl {
    /// Creates a system-control adapter over the raw syscall transport.
    #[must_use]
    pub const fn new(raw: RawSyscall) -> Self {
        Self { raw }
    }

    /// Requests root/system shutdown for the current process.
    ///
    /// # Errors
    ///
    /// Returns [`SystemError`] when no current process context exists or the raw
    /// syscall transport rejects the halt request.
    pub fn halt(self) -> Result<(), SystemError> {
        self.raw
            .invoke(SyscallNr::SYSTEM_HALT, SyscallArgs::EMPTY)
            .decode()
            .map(|_| ())
            .map_err(system_error_from_syscall)
    }

    /// Runs a lower-provider probe target for diagnostics.
    ///
    /// This is a typed system-domain control over the raw transport. The
    /// provider owns the target catalog and output format below the bridge;
    /// callers only pass the target name and receive success or refusal.
    ///
    /// # Errors
    ///
    /// Returns [`SystemError`] when no current process context exists, the
    /// provider is unavailable, the target is unknown, or the raw transport
    /// rejects the request.
    pub fn probe(self, target: &str) -> Result<(), SystemError> {
        self.raw
            .invoke(
                SyscallNr::PROVIDER_PROBE,
                SyscallArgs::new([target.as_ptr().addr(), target.len(), 0, 0, 0, 0]),
            )
            .decode()
            .map(|_| ())
            .map_err(system_error_from_syscall)
    }
}

fn system_error_from_syscall(error: SyscallError) -> SystemError {
    match error {
        SyscallError::UNSUPPORTED => SystemError::Unsupported,
        SyscallError::NOT_FOUND => SystemError::UnknownTarget,
        SyscallError::INVALID_ARGUMENT => SystemError::InvalidRequest,
        SyscallError::NO_CURRENT_PROCESS => SystemError::NoCurrentProcess,
        SyscallError::BUSY => SystemError::Busy,
        _ => SystemError::Failed,
    }
}
