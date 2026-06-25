//! Minimal block-device service for kernel diagnostics.
//!
//! This is the first system-kernel block boundary used by persistent dump and
//! executable source-media and executable-bundle work. It deliberately models
//! only explicit callback targets today; real media enumeration, partitions,
//! filesystems, barriers, and removable-device policy belong in later cuts.

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

/// Callback-backed block device used for diagnostic artifact storage.
#[derive(Clone, Copy)]
pub struct BlockDevice {
    /// Stable device label used in operator and dump evidence.
    pub label: &'static str,
    /// Total writable bytes exposed by this diagnostic target.
    pub capacity_bytes: usize,
    /// Write bytes at byte offset.
    pub write: fn(offset: usize, bytes: &[u8]) -> bool,
    /// Read bytes at byte offset into `out`, returning copied bytes.
    pub read: fn(offset: usize, out: &mut [u8]) -> usize,
}

impl BlockDevice {
    /// Builds a callback-backed block device.
    #[must_use]
    pub const fn new(
        label: &'static str,
        capacity_bytes: usize,
        write: fn(offset: usize, bytes: &[u8]) -> bool,
        read: fn(offset: usize, out: &mut [u8]) -> usize,
    ) -> Self {
        Self {
            label,
            capacity_bytes,
            write,
            read,
        }
    }
}

/// Public diagnostic block status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockStatus {
    /// Stable device label.
    pub label: &'static str,
    /// Total writable bytes exposed by this target.
    pub capacity_bytes: usize,
}

/// Result of one diagnostic block operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockIoResult {
    /// Whether a diagnostic block target was installed.
    pub available: bool,
    /// Stable target label, or `none`.
    pub storage: &'static str,
    /// Total writable bytes exposed by the target, or zero.
    pub capacity_bytes: usize,
    /// Bytes copied by this operation.
    pub bytes: usize,
    /// Whether the operation succeeded.
    pub ok: bool,
    /// Stable result reason.
    pub reason: &'static str,
}

const NO_DEVICE_RESULT: BlockIoResult = BlockIoResult {
    available: false,
    storage: "none",
    capacity_bytes: 0,
    bytes: 0,
    ok: false,
    reason: "no-diagnostic-block-device",
};

struct DeviceCell(UnsafeCell<Option<BlockDevice>>);

// SAFETY: access is serialized by `DEVICE_LOCK`.
unsafe impl Sync for DeviceCell {}

static DIAGNOSTIC_DEVICE: DeviceCell = DeviceCell(UnsafeCell::new(None));
static SOURCE_MEDIA_DEVICE: DeviceCell = DeviceCell(UnsafeCell::new(None));
static EXEC_BUNDLE_DEVICE: DeviceCell = DeviceCell(UnsafeCell::new(None));
static DEVICE_LOCK: AtomicBool = AtomicBool::new(false);

struct DeviceGuard;

impl DeviceGuard {
    fn acquire() -> Self {
        while DEVICE_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for DeviceGuard {
    fn drop(&mut self) {
        DEVICE_LOCK.store(false, Ordering::Release);
    }
}

fn with_device<R>(cell: &DeviceCell, f: impl FnOnce(&mut Option<BlockDevice>) -> R) -> R {
    let _guard = DeviceGuard::acquire();
    // SAFETY: `DEVICE_LOCK` serializes access to the device slot.
    let slot = unsafe { &mut *cell.0.get() };
    f(slot)
}

fn with_diagnostic_device<R>(f: impl FnOnce(&mut Option<BlockDevice>) -> R) -> R {
    with_device(&DIAGNOSTIC_DEVICE, f)
}

fn with_source_media_device<R>(f: impl FnOnce(&mut Option<BlockDevice>) -> R) -> R {
    with_device(&SOURCE_MEDIA_DEVICE, f)
}

fn with_exec_bundle_device<R>(f: impl FnOnce(&mut Option<BlockDevice>) -> R) -> R {
    with_device(&EXEC_BUNDLE_DEVICE, f)
}

fn installed_diagnostic_device() -> Option<BlockDevice> {
    with_diagnostic_device(|slot| *slot)
}

fn installed_source_media_device() -> Option<BlockDevice> {
    with_source_media_device(|slot| *slot)
}

fn installed_exec_bundle_device() -> Option<BlockDevice> {
    with_exec_bundle_device(|slot| *slot)
}

/// Installs the diagnostic block target used by persistent dump sync.
pub fn install_diagnostic_device(device: BlockDevice) {
    with_diagnostic_device(|slot| *slot = Some(device));
}

/// Installs the source-media block target used by executable source loading.
pub fn install_source_media_device(device: BlockDevice) {
    with_source_media_device(|slot| *slot = Some(device));
}

/// Installs the executable-bundle block target used by exec artifact loading.
pub fn install_exec_bundle_device(device: BlockDevice) {
    with_exec_bundle_device(|slot| *slot = Some(device));
}

/// Clears the diagnostic block target for isolated no_std selftests.
#[cfg(feature = "selftest")]
pub fn clear_diagnostic_device_for_tests() {
    with_diagnostic_device(|slot| *slot = None);
}

/// Clears the source-media block target for isolated no_std selftests.
#[cfg(feature = "selftest")]
pub fn clear_source_media_device_for_tests() {
    with_source_media_device(|slot| *slot = None);
}

/// Clears the executable-bundle block target for isolated no_std selftests.
#[cfg(feature = "selftest")]
pub fn clear_exec_bundle_device_for_tests() {
    with_exec_bundle_device(|slot| *slot = None);
}

/// Returns the currently installed diagnostic block target status.
#[must_use]
pub fn diagnostic_status() -> Option<BlockStatus> {
    installed_diagnostic_device().map(|device| BlockStatus {
        label: device.label,
        capacity_bytes: device.capacity_bytes,
    })
}

/// Returns the currently installed executable source-media target status.
#[must_use]
pub fn source_media_status() -> Option<BlockStatus> {
    installed_source_media_device().map(|device| BlockStatus {
        label: device.label,
        capacity_bytes: device.capacity_bytes,
    })
}

/// Returns the currently installed executable-bundle target status.
#[must_use]
pub fn exec_bundle_status() -> Option<BlockStatus> {
    installed_exec_bundle_device().map(|device| BlockStatus {
        label: device.label,
        capacity_bytes: device.capacity_bytes,
    })
}

fn write_artifact(device: Option<BlockDevice>, bytes: &[u8]) -> BlockIoResult {
    let Some(device) = device else {
        return NO_DEVICE_RESULT;
    };
    if bytes.len() > device.capacity_bytes {
        return BlockIoResult {
            available: true,
            storage: device.label,
            capacity_bytes: device.capacity_bytes,
            bytes: 0,
            ok: false,
            reason: "write-out-of-range",
        };
    }
    if !(device.write)(0, bytes) {
        return BlockIoResult {
            available: true,
            storage: device.label,
            capacity_bytes: device.capacity_bytes,
            bytes: 0,
            ok: false,
            reason: "write-failed",
        };
    }
    BlockIoResult {
        available: true,
        storage: device.label,
        capacity_bytes: device.capacity_bytes,
        bytes: bytes.len(),
        ok: true,
        reason: "write-ok",
    }
}

fn read_artifact_at(device: Option<BlockDevice>, offset: usize, out: &mut [u8]) -> BlockIoResult {
    let Some(device) = device else {
        return NO_DEVICE_RESULT;
    };
    if offset >= device.capacity_bytes {
        return BlockIoResult {
            available: true,
            storage: device.label,
            capacity_bytes: device.capacity_bytes,
            bytes: 0,
            ok: false,
            reason: "read-out-of-range",
        };
    }
    let read_len = (device.read)(offset, out);
    if read_len > device.capacity_bytes - offset || read_len > out.len() {
        return BlockIoResult {
            available: true,
            storage: device.label,
            capacity_bytes: device.capacity_bytes,
            bytes: read_len,
            ok: false,
            reason: "read-out-of-range",
        };
    }
    BlockIoResult {
        available: true,
        storage: device.label,
        capacity_bytes: device.capacity_bytes,
        bytes: read_len,
        ok: true,
        reason: "read-ok",
    }
}

/// Writes a complete diagnostic artifact at offset zero.
#[must_use]
pub fn write_diagnostic_artifact(bytes: &[u8]) -> BlockIoResult {
    write_artifact(installed_diagnostic_device(), bytes)
}

/// Reads the last diagnostic artifact from offset zero.
#[must_use]
pub fn read_diagnostic_artifact(out: &mut [u8]) -> BlockIoResult {
    read_artifact_at(installed_diagnostic_device(), 0, out)
}

/// Reads one executable source artifact from the installed source-media target.
#[must_use]
pub fn read_source_media_artifact(out: &mut [u8]) -> BlockIoResult {
    read_source_media_artifact_at(0, out)
}

/// Reads executable source-media bytes from a byte offset.
#[must_use]
pub fn read_source_media_artifact_at(offset: usize, out: &mut [u8]) -> BlockIoResult {
    read_artifact_at(installed_source_media_device(), offset, out)
}

/// Reads one executable bundle artifact from the installed exec-bundle target.
#[must_use]
pub fn read_exec_bundle_artifact(out: &mut [u8]) -> BlockIoResult {
    read_exec_bundle_artifact_at(0, out)
}

/// Reads executable bundle bytes from a byte offset.
#[must_use]
pub fn read_exec_bundle_artifact_at(offset: usize, out: &mut [u8]) -> BlockIoResult {
    read_artifact_at(installed_exec_bundle_device(), offset, out)
}

#[cfg(feature = "selftest")]
#[path = "block_tests.rs"]
mod tests;
