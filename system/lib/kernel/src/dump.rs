//! Dump status, artifact encoding, and explicit diagnostic-storage sync.
//!
//! The live status path summarizes retained in-memory state. Persistent proof is
//! claimed only when an explicit diagnostic block target is installed and a dump
//! sync writes, flushes, reads back, byte-compares, and parses the artifact.
//! Targets without that durability/read-back boundary fail closed.

use {
    crate::{block, exec, klog, mm, proc, sched, service, syscall},
    core::{
        cell::UnsafeCell,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    reovim_uapi_panic::{Disposition, PanicRecord},
    reovim_uapi_system::{BootInfo, DeviceClass, DeviceEntry},
};

/// Maximum bytes needed by the current dump snapshot header.
pub const MAX_SNAPSHOT_HEADER_BYTES: usize = 1536;
/// Maximum bytes in one bounded dump artifact.
pub const MAX_DUMP_ARTIFACT_BYTES: usize = 256 * 1024;
/// Maximum device inventory rows retained in one bounded dump artifact.
pub const MAX_DUMP_DEVICE_RECORDS: usize = 16;

const EMPTY_DEVICE_ENTRY: DeviceEntry = DeviceEntry {
    class: DeviceClass::Unknown,
    mmio_base: 0,
    mmio_len: 0,
    irq: u32::MAX,
    capacity_bytes: 0,
    compatible: "",
};

const PANIC_RECORD_EMPTY: usize = 0;
const PANIC_RECORD_PRESENT: usize = 1 << 0;
const PANIC_RECORD_RECOVER: usize = 1 << 1;
const PANIC_RECORD_ROLLBACK_FAILED: usize = 1 << 2;

static PANIC_RECORD: AtomicUsize = AtomicUsize::new(PANIC_RECORD_EMPTY);

/// Panic record retained for the next dump snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DumpPanicRecord {
    /// Panic disposition selected by the fault floor.
    pub disposition: Disposition,
    /// Whether the panic happened while cleanup was in progress.
    pub rollback_failed: bool,
}

/// Records panic state for later dump snapshots.
///
/// This is the system-kernel persistence hook target used by the panic bridge.
/// It is intentionally atomic-only so it remains usable from the panic path.
pub fn record_panic_state(record: PanicRecord) {
    let mut bits = PANIC_RECORD_PRESENT;
    if matches!(record.disposition, Disposition::Recover) {
        bits |= PANIC_RECORD_RECOVER;
    }
    if record.rollback_failed {
        bits |= PANIC_RECORD_ROLLBACK_FAILED;
    }
    PANIC_RECORD.store(bits, Ordering::Release);
}

fn snapshot_panic_record() -> Option<DumpPanicRecord> {
    let bits = PANIC_RECORD.load(Ordering::Acquire);
    if bits & PANIC_RECORD_PRESENT == 0 {
        return None;
    }
    let disposition = if bits & PANIC_RECORD_RECOVER != 0 {
        Disposition::Recover
    } else {
        Disposition::Halt
    };
    Some(DumpPanicRecord {
        disposition,
        rollback_failed: bits & PANIC_RECORD_ROLLBACK_FAILED != 0,
    })
}

/// Clears retained panic state for isolated no_std selftests.
#[cfg(feature = "selftest")]
pub fn reset_panic_record_for_tests() {
    PANIC_RECORD.store(PANIC_RECORD_EMPTY, Ordering::Release);
}

/// Encoded dump snapshot header bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotHeader {
    /// Backing bytes.
    pub bytes: [u8; MAX_SNAPSHOT_HEADER_BYTES],
    /// Used byte count.
    pub len: usize,
}

impl SnapshotHeader {
    /// Returns the encoded header bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// Encoded dump artifact bytes.
pub struct DumpArtifact {
    /// Backing bytes.
    pub bytes: [u8; MAX_DUMP_ARTIFACT_BYTES],
    /// Used byte count.
    pub len: usize,
}

impl DumpArtifact {
    /// Returns the encoded artifact bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// Current dump facility status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DumpStatus {
    /// Dump format version for host-side analyzers.
    pub format_version: usize,
    /// Boot/session identity for this snapshot.
    pub identity: klog::DiagnosticIdentity,
    /// Boot image identity for host-side analyzer matching.
    pub image: DumpImageIdentity,
    /// Boot facts captured for post-poweroff diagnostics.
    pub boot_info: BootInfo,
    /// Bounded device inventory captured for post-poweroff diagnostics.
    pub devices: [DeviceEntry; MAX_DUMP_DEVICE_RECORDS],
    /// Number of valid `devices` entries.
    pub device_records: usize,
    /// Proof checklist state represented in this snapshot.
    pub proof_state: &'static str,
    /// Panic record state represented in this snapshot.
    pub panic_state: &'static str,
    /// Number of retained panic records.
    pub panic_records: usize,
    /// Last retained panic record, if any.
    pub panic_record: Option<DumpPanicRecord>,
    /// Whether a persistent sink is available.
    pub persistent_available: bool,
    /// Stable storage target label.
    pub storage: &'static str,
    /// Total bytes exposed by the persistent storage target, or zero.
    pub storage_capacity_bytes: usize,
    /// Last dump sync attempt retained in memory.
    pub last_sync: DumpSyncStatus,
    /// Retained kernel-log metadata.
    pub klog: klog::Stats,
    /// Retained structured kernel event records.
    pub event_records: usize,
    /// Retained process records.
    pub process_records: usize,
    /// Retained init-service records.
    pub service_records: usize,
    /// Retained executable load/admission records.
    pub exec_load_records: usize,
    /// Retained pending executable invocation records.
    pub pending_exec_records: usize,
    /// Retained process wait records.
    pub wait_records: usize,
    /// Retained scheduler task records.
    pub task_records: usize,
    /// Retained typed syscall dispatch records.
    pub syscall_records: usize,
    /// Active retained raw syscall continuations.
    pub syscall_continuation_records: usize,
}

/// Boot image identity copied into dump snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DumpImageIdentity {
    /// Cargo package name for the boot image.
    pub package: &'static str,
    /// Cargo package version for the boot image.
    pub version: &'static str,
    /// Rust target triple used to build the image.
    pub target: &'static str,
    /// Profile selected for this boot.
    pub selected_profile: &'static str,
    /// Build-time profile request before fallback policy.
    pub profile_request: &'static str,
    /// Whether `REOVIM_OS_BOOTLINE` was compiled into the image.
    pub bootline: &'static str,
    /// Whether the launch-profile feature was enabled at build time.
    pub launch_profile_feature: &'static str,
}

impl DumpImageIdentity {
    /// Builds an image identity row-set for dump snapshots.
    #[must_use]
    pub const fn new(
        package: &'static str,
        version: &'static str,
        target: &'static str,
        selected_profile: &'static str,
        profile_request: &'static str,
        bootline: &'static str,
        launch_profile_feature: &'static str,
    ) -> Self {
        Self {
            package,
            version,
            target,
            selected_profile,
            profile_request,
            bootline,
            launch_profile_feature,
        }
    }

    /// Placeholder identity for isolated kernel selftests without rootd image facts.
    #[must_use]
    pub const fn unknown() -> Self {
        Self::new("unknown", "unknown", "unknown", "unknown", "unknown", "unknown", "unknown")
    }
}

/// Result from an attempted persistent dump flush.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DumpSyncStatus {
    /// Whether this status came from an actual sync attempt.
    pub attempted: bool,
    /// Whether a persistent sink was available.
    pub persistent_available: bool,
    /// Stable storage target label.
    pub storage: &'static str,
    /// Total bytes exposed by the persistent storage target, or zero.
    pub storage_capacity_bytes: usize,
    /// Whether bytes were written.
    pub written: bool,
    /// Number of bytes written and read back.
    pub bytes_written: usize,
    /// Snapshot checksum verified after read-back.
    pub checksum: u32,
    /// Whether read-back verification succeeded.
    pub verified: bool,
    /// Stable failure or success reason.
    pub reason: &'static str,
}

/// Persistent dump storage target.
pub type DumpSink = block::BlockDevice;

/// Error while encoding a bounded dump snapshot header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DumpEncodeError {
    /// The fixed output buffer is too small for the current header.
    BufferTooSmall,
}

/// Error while parsing a bounded dump snapshot header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DumpParseError {
    /// The `reovim-dump-v1` header is missing or unsupported.
    BadHeader,
    /// A required `key=value` row is missing or has the wrong key.
    BadLine,
    /// A numeric value could not be decoded.
    BadNumber,
    /// The checksum row is missing.
    MissingChecksum,
    /// The checksum does not match the preceding bytes.
    ChecksumMismatch,
}

/// Parsed bounded dump snapshot header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedSnapshotHeader<'a> {
    /// Dump format version.
    pub format_version: usize,
    /// Boot identifier.
    pub boot_id: usize,
    /// Session identifier.
    pub session_id: usize,
    /// Identity source.
    pub identity_source: &'a str,
    /// Cargo package name for the boot image.
    pub package: &'a str,
    /// Cargo package version for the boot image.
    pub version: &'a str,
    /// Rust target triple used to build the image.
    pub target: &'a str,
    /// Profile selected for this boot.
    pub selected_profile: &'a str,
    /// Build-time profile request before fallback policy.
    pub profile_request: &'a str,
    /// Whether `REOVIM_OS_BOOTLINE` was compiled into the image.
    pub bootline: &'a str,
    /// Whether the launch-profile feature was enabled at build time.
    pub launch_profile_feature: &'a str,
    /// Boot memory-map range count.
    pub boot_memory_ranges: usize,
    /// Boot usable memory bytes.
    pub boot_memory_usable_bytes: usize,
    /// Boot CPU count.
    pub boot_cpu_count: usize,
    /// Boot heap capacity bytes.
    pub boot_heap_total_bytes: usize,
    /// Retained device rows in the artifact.
    pub device_records: usize,
    /// Proof checklist state represented in this snapshot.
    pub proof_state: &'a str,
    /// Panic record state represented in this snapshot.
    pub panic_state: &'a str,
    /// Number of retained panic records.
    pub panic_records: usize,
    /// Whether persistence was available for the captured image.
    pub persistent_available: bool,
    /// Stable storage target label.
    pub storage: &'a str,
    /// Total bytes exposed by the persistent storage target, or zero.
    pub storage_capacity_bytes: usize,
    /// Retained kernel-log bytes.
    pub klog_retained_bytes: usize,
    /// Dropped kernel-log bytes.
    pub klog_dropped_bytes: usize,
    /// Next event sequence.
    pub klog_next_event_seq: usize,
    /// Structured event records retained.
    pub event_records: usize,
    /// Process records retained.
    pub process_records: usize,
    /// Init-service records retained.
    pub service_records: usize,
    /// Executable load/admission records retained.
    pub exec_load_records: usize,
    /// Pending executable invocation records retained.
    pub pending_exec_records: usize,
    /// Wait records retained.
    pub wait_records: usize,
    /// Task records retained.
    pub task_records: usize,
    /// Syscall records retained.
    pub syscall_records: usize,
    /// Active retained syscall continuation records.
    pub syscall_continuation_records: usize,
    /// Whether a dump sync has been attempted before this snapshot.
    pub last_sync_attempted: bool,
    /// Whether persistence was available during the last sync attempt.
    pub last_sync_persistent_available: bool,
    /// Stable storage target label for the last sync attempt.
    pub last_sync_storage: &'a str,
    /// Total bytes exposed by the last sync target, or zero.
    pub last_sync_storage_capacity_bytes: usize,
    /// Whether the last sync wrote bytes.
    pub last_sync_written: bool,
    /// Bytes written by the last sync attempt.
    pub last_sync_bytes_written: usize,
    /// Header checksum reported by the last sync attempt.
    pub last_sync_checksum: u32,
    /// Whether the last sync read-back verification succeeded.
    pub last_sync_verified: bool,
    /// Stable reason reported by the last sync attempt.
    pub last_sync_reason: &'a str,
    /// Header checksum.
    pub checksum: u32,
}

const NEVER_SYNCED_STATUS: DumpSyncStatus = DumpSyncStatus {
    attempted: false,
    persistent_available: false,
    storage: "none",
    storage_capacity_bytes: 0,
    written: false,
    bytes_written: 0,
    checksum: 0,
    verified: false,
    reason: "never-synced",
};

struct LastSyncCell(UnsafeCell<DumpSyncStatus>);

// SAFETY: access is serialized by `LAST_SYNC_LOCK`.
unsafe impl Sync for LastSyncCell {}

static LAST_SYNC: LastSyncCell = LastSyncCell(UnsafeCell::new(NEVER_SYNCED_STATUS));
static LAST_SYNC_LOCK: AtomicBool = AtomicBool::new(false);

struct ReadbackCell(UnsafeCell<[u8; MAX_DUMP_ARTIFACT_BYTES]>);

// SAFETY: access is serialized by `READBACK_LOCK`.
unsafe impl Sync for ReadbackCell {}

static READBACK: ReadbackCell = ReadbackCell(UnsafeCell::new([0u8; MAX_DUMP_ARTIFACT_BYTES]));
static READBACK_LOCK: AtomicBool = AtomicBool::new(false);

struct ArtifactCell(UnsafeCell<DumpArtifact>);

// SAFETY: access is serialized by `ARTIFACT_LOCK`.
unsafe impl Sync for ArtifactCell {}

static ARTIFACT: ArtifactCell = ArtifactCell(UnsafeCell::new(DumpArtifact {
    bytes: [0u8; MAX_DUMP_ARTIFACT_BYTES],
    len: 0,
}));
static ARTIFACT_LOCK: AtomicBool = AtomicBool::new(false);

struct LastSyncGuard;

impl LastSyncGuard {
    fn acquire() -> Self {
        while LAST_SYNC_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for LastSyncGuard {
    fn drop(&mut self) {
        LAST_SYNC_LOCK.store(false, Ordering::Release);
    }
}

fn with_last_sync<R>(f: impl FnOnce(&mut DumpSyncStatus) -> R) -> R {
    let _guard = LastSyncGuard::acquire();
    // SAFETY: `LAST_SYNC_LOCK` serializes access to the retained status.
    let status = unsafe { &mut *LAST_SYNC.0.get() };
    f(status)
}

fn snapshot_last_sync() -> DumpSyncStatus {
    with_last_sync(|status| *status)
}

fn record_last_sync(status: DumpSyncStatus) {
    with_last_sync(|slot| *slot = status);
}

struct ReadbackGuard;

impl ReadbackGuard {
    fn acquire() -> Self {
        while READBACK_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for ReadbackGuard {
    fn drop(&mut self) {
        READBACK_LOCK.store(false, Ordering::Release);
    }
}

fn with_readback_buffer<R>(f: impl FnOnce(&mut [u8; MAX_DUMP_ARTIFACT_BYTES]) -> R) -> R {
    let _guard = ReadbackGuard::acquire();
    // SAFETY: `READBACK_LOCK` serializes access to the reusable readback buffer.
    let bytes = unsafe { &mut *READBACK.0.get() };
    f(bytes)
}

struct ArtifactGuard;

impl ArtifactGuard {
    fn acquire() -> Self {
        while ARTIFACT_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for ArtifactGuard {
    fn drop(&mut self) {
        ARTIFACT_LOCK.store(false, Ordering::Release);
    }
}

fn with_artifact_buffer<R>(f: impl FnOnce(&mut DumpArtifact) -> R) -> R {
    let _guard = ArtifactGuard::acquire();
    // SAFETY: `ARTIFACT_LOCK` serializes access to the reusable artifact buffer.
    let artifact = unsafe { &mut *ARTIFACT.0.get() };
    f(artifact)
}

fn finish_sync(status: DumpSyncStatus) -> DumpSyncStatus {
    record_last_sync(status);
    status
}

/// Clears retained sync status for isolated no_std selftests.
#[cfg(feature = "selftest")]
pub fn reset_last_sync_for_tests() {
    record_last_sync(NEVER_SYNCED_STATUS);
}

/// Installs a persistent dump sink.
pub fn install_sink(sink: DumpSink) {
    block::install_diagnostic_device(sink);
}

/// Clears the persistent dump sink for isolated no_std selftests.
#[cfg(feature = "selftest")]
pub fn clear_sink_for_tests() {
    block::clear_diagnostic_device_for_tests();
    reset_last_sync_for_tests();
}

/// Returns the current in-memory dump status.
#[must_use]
pub fn status() -> DumpStatus {
    status_with_image(DumpImageIdentity::unknown())
}

/// Returns the current in-memory dump status with caller-supplied image identity.
#[must_use]
pub fn status_with_image(image: DumpImageIdentity) -> DumpStatus {
    status_with_context(image, BootInfo::default(), &[])
}

/// Returns the current in-memory dump status with boot/device context.
#[must_use]
pub fn status_with_context(
    image: DumpImageIdentity,
    boot_info: BootInfo,
    devices: &[DeviceEntry],
) -> DumpStatus {
    let mut processes = [proc::EMPTY_PROCESS_RECORD; proc::MAX_PROCESSES];
    let mut waits = [proc::EMPTY_WAIT_RECORD; proc::MAX_WAITS];
    let mut tasks = [sched::EMPTY_KERNEL_TASK_RECORD; sched::MAX_KERNEL_TASKS];
    let mut events = [klog::EMPTY_EVENT_RECORD; klog::MAX_EVENTS];
    let mut exec_loads = [exec::EMPTY_EXEC_LOAD_RECORD; exec::MAX_EXEC_LOAD_RECORDS];
    let mut pending_execs = [exec::EMPTY_PENDING_EXEC_RECORD; exec::MAX_PENDING_EXEC_RECORDS];
    let mut services = [service::EMPTY_SERVICE_RECORD; service::MAX_SERVICES];
    let mut syscalls = [syscall::EMPTY_SYSCALL_RECORD; syscall::MAX_SYSCALL_RECORDS];
    let mut continuations =
        [syscall::EMPTY_SYSCALL_CONTINUATION_RECORD; syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let klog_stats = klog::stats();
    let sink = block::diagnostic_status();
    let panic_record = snapshot_panic_record();
    let mut device_records = [EMPTY_DEVICE_ENTRY; MAX_DUMP_DEVICE_RECORDS];
    let mut device_count = 0usize;
    while device_count < devices.len() && device_count < device_records.len() {
        device_records[device_count] = devices[device_count];
        device_count += 1;
    }
    DumpStatus {
        format_version: 1,
        identity: klog_stats.identity,
        image,
        boot_info,
        devices: device_records,
        device_records: device_count,
        proof_state: "operator-required",
        panic_state: if panic_record.is_some() {
            "recorded"
        } else {
            "none"
        },
        panic_records: if panic_record.is_some() { 1 } else { 0 },
        panic_record,
        persistent_available: sink.is_some(),
        storage: sink.map_or("none", |sink| sink.label),
        storage_capacity_bytes: sink.map_or(0, |sink| sink.capacity_bytes),
        last_sync: snapshot_last_sync(),
        klog: klog_stats,
        event_records: klog::snapshot_events(&mut events),
        process_records: proc::snapshot(&mut processes),
        service_records: service::snapshot(&mut services),
        exec_load_records: exec::snapshot_loads(&mut exec_loads),
        pending_exec_records: exec::snapshot_pending(&mut pending_execs),
        wait_records: proc::snapshot_waits(&mut waits),
        task_records: sched::snapshot_kernel_tasks(&mut tasks),
        syscall_records: syscall::snapshot_syscalls(&mut syscalls),
        syscall_continuation_records: syscall::snapshot_syscall_continuations(&mut continuations),
    }
}

/// Attempts to flush the in-memory dump to persistent storage.
#[must_use]
pub fn sync() -> DumpSyncStatus {
    sync_with_image(DumpImageIdentity::unknown())
}

/// Attempts to flush the in-memory dump to persistent storage with image identity.
#[must_use]
pub fn sync_with_image(image: DumpImageIdentity) -> DumpSyncStatus {
    sync_with_context(image, BootInfo::default(), &[])
}

/// Attempts to flush the in-memory dump with boot/device context.
#[must_use]
pub fn sync_with_context(
    image: DumpImageIdentity,
    boot_info: BootInfo,
    devices: &[DeviceEntry],
) -> DumpSyncStatus {
    let Some(sink_device) = block::diagnostic_device_snapshot() else {
        return finish_sync(DumpSyncStatus {
            attempted: true,
            persistent_available: false,
            storage: "none",
            storage_capacity_bytes: 0,
            written: false,
            bytes_written: 0,
            checksum: 0,
            verified: false,
            reason: "no-persistent-dump-sink",
        });
    };
    let sink = block::BlockStatus {
        label: sink_device.label,
        capacity_bytes: sink_device.capacity_bytes,
    };

    with_artifact_buffer(|artifact| {
        let checksum = match encode_context_artifact_into(sink, image, boot_info, devices, artifact)
        {
            Ok(checksum) => checksum,
            Err(status) => return finish_sync(status),
        };
        let first_sync = write_and_verify_artifact(sink_device, artifact, checksum);
        if !first_sync.written || !first_sync.verified {
            return finish_sync(first_sync);
        }

        record_last_sync(first_sync);
        let checksum = match encode_context_artifact_into(sink, image, boot_info, devices, artifact)
        {
            Ok(checksum) => checksum,
            Err(status) => return finish_sync(status),
        };
        finish_sync(write_and_verify_artifact(sink_device, artifact, checksum))
    })
}

fn encode_context_artifact_into(
    sink: block::BlockStatus,
    image: DumpImageIdentity,
    boot_info: BootInfo,
    devices: &[DeviceEntry],
    artifact: &mut DumpArtifact,
) -> Result<u32, DumpSyncStatus> {
    let mut status = status_with_context(image, boot_info, devices);
    status.persistent_available = true;
    status.storage = sink.label;
    status.storage_capacity_bytes = sink.capacity_bytes;
    encode_status_artifact_into(status, artifact).map_err(|_| DumpSyncStatus {
        attempted: true,
        persistent_available: true,
        storage: sink.label,
        storage_capacity_bytes: sink.capacity_bytes,
        written: false,
        bytes_written: 0,
        checksum: 0,
        verified: false,
        reason: "encode-error",
    })?;
    let checksum = parse_snapshot_header_prefix(artifact.as_bytes())
        .map_err(|_| DumpSyncStatus {
            attempted: true,
            persistent_available: true,
            storage: sink.label,
            storage_capacity_bytes: sink.capacity_bytes,
            written: false,
            bytes_written: 0,
            checksum: 0,
            verified: false,
            reason: "encode-error",
        })?
        .checksum;
    Ok(checksum)
}

fn write_and_verify_artifact(
    sink: block::BlockDevice,
    artifact: &DumpArtifact,
    checksum: u32,
) -> DumpSyncStatus {
    let write = block::write_selected_diagnostic_artifact(sink, artifact.as_bytes());
    if !write.ok {
        return DumpSyncStatus {
            attempted: true,
            persistent_available: true,
            storage: write.storage,
            storage_capacity_bytes: write.capacity_bytes,
            written: false,
            bytes_written: write.bytes,
            checksum,
            verified: false,
            reason: write.reason,
        };
    }

    with_readback_buffer(|readback| {
        let read = block::read_selected_diagnostic_artifact(sink, readback);
        if !read.ok {
            return DumpSyncStatus {
                attempted: true,
                persistent_available: true,
                storage: read.storage,
                storage_capacity_bytes: read.capacity_bytes,
                written: false,
                bytes_written: write.bytes,
                checksum,
                verified: false,
                reason: read.reason,
            };
        }
        if read.bytes != artifact.len {
            return DumpSyncStatus {
                attempted: true,
                persistent_available: true,
                storage: read.storage,
                storage_capacity_bytes: read.capacity_bytes,
                written: false,
                bytes_written: write.bytes,
                checksum,
                verified: false,
                reason: "readback-size-mismatch",
            };
        }
        let read_len = read.bytes;
        if &readback[..read_len] != artifact.as_bytes() {
            return DumpSyncStatus {
                attempted: true,
                persistent_available: true,
                storage: read.storage,
                storage_capacity_bytes: read.capacity_bytes,
                written: false,
                bytes_written: write.bytes,
                checksum,
                verified: false,
                reason: "readback-mismatch",
            };
        }
        if parse_snapshot_header_prefix(&readback[..read_len]).is_err() {
            return DumpSyncStatus {
                attempted: true,
                persistent_available: true,
                storage: read.storage,
                storage_capacity_bytes: read.capacity_bytes,
                written: false,
                bytes_written: write.bytes,
                checksum,
                verified: false,
                reason: "readback-parse-failed",
            };
        }

        DumpSyncStatus {
            attempted: true,
            persistent_available: true,
            storage: read.storage,
            storage_capacity_bytes: read.capacity_bytes,
            written: true,
            bytes_written: write.bytes,
            checksum,
            verified: true,
            reason: "written-readback-ok",
        }
    })
}

/// Encodes the current in-memory dump snapshot header.
pub fn encode_snapshot_header() -> Result<SnapshotHeader, DumpEncodeError> {
    encode_status_header(status())
}

/// Encodes `status` as a bounded parseable dump snapshot header.
pub fn encode_status_header(status: DumpStatus) -> Result<SnapshotHeader, DumpEncodeError> {
    let mut header = SnapshotHeader {
        bytes: [0u8; MAX_SNAPSHOT_HEADER_BYTES],
        len: 0,
    };
    {
        let mut writer = TextWriter::new(&mut header.bytes);
        writer.bytes(b"reovim-dump-v")?;
        writer.usize(status.format_version)?;
        writer.nl()?;
        writer.key_usize(b"boot_id", status.identity.boot_id)?;
        writer.key_usize(b"session_id", status.identity.session_id)?;
        writer.key_str(b"identity_source", status.identity.identity_source)?;
        writer.key_str(b"package", status.image.package)?;
        writer.key_str(b"version", status.image.version)?;
        writer.key_str(b"target", status.image.target)?;
        writer.key_str(b"selected_profile", status.image.selected_profile)?;
        writer.key_str(b"profile_request", status.image.profile_request)?;
        writer.key_str(b"bootline", status.image.bootline)?;
        writer.key_str(b"launch_profile_feature", status.image.launch_profile_feature)?;
        writer.key_usize(b"boot_memory_ranges", status.boot_info.memory.range_count())?;
        writer.key_u64(b"boot_memory_usable_bytes", status.boot_info.memory.usable_bytes())?;
        writer.key_usize(b"boot_cpu_count", status.boot_info.cpu_count as usize)?;
        writer.key_u64(b"boot_heap_total_bytes", status.boot_info.heap_total_bytes)?;
        writer.key_usize(b"device_records", status.device_records)?;
        writer.key_str(b"proof_state", status.proof_state)?;
        writer.key_str(b"panic_state", status.panic_state)?;
        writer.key_usize(b"panic_records", status.panic_records)?;
        writer.key_str(
            b"persistent",
            if status.persistent_available {
                "available"
            } else {
                "unavailable"
            },
        )?;
        writer.key_str(b"storage", status.storage)?;
        writer.key_usize(b"storage_capacity_bytes", status.storage_capacity_bytes)?;
        writer.key_bool(b"last_sync_attempted", status.last_sync.attempted)?;
        writer.key_str(
            b"last_sync_persistent",
            if status.last_sync.persistent_available {
                "available"
            } else {
                "unavailable"
            },
        )?;
        writer.key_str(b"last_sync_storage", status.last_sync.storage)?;
        writer.key_usize(
            b"last_sync_storage_capacity_bytes",
            status.last_sync.storage_capacity_bytes,
        )?;
        writer.key_str(
            b"last_sync_status",
            if status.last_sync.written {
                "written"
            } else {
                "not-written"
            },
        )?;
        writer.key_usize(b"last_sync_bytes", status.last_sync.bytes_written)?;
        writer.key_usize(b"last_sync_checksum", status.last_sync.checksum as usize)?;
        writer.key_bool(b"last_sync_verified", status.last_sync.verified)?;
        writer.key_str(b"last_sync_reason", status.last_sync.reason)?;
        writer.key_usize(b"klog_retained_bytes", status.klog.retained_bytes)?;
        writer.key_usize(b"klog_dropped_bytes", status.klog.dropped_bytes)?;
        writer.key_usize(b"klog_next_event_seq", status.klog.next_event_seq)?;
        writer.key_usize(b"event_records", status.event_records)?;
        writer.key_usize(b"process_records", status.process_records)?;
        writer.key_usize(b"service_records", status.service_records)?;
        writer.key_usize(b"exec_load_records", status.exec_load_records)?;
        writer.key_usize(b"pending_exec_records", status.pending_exec_records)?;
        writer.key_usize(b"wait_records", status.wait_records)?;
        writer.key_usize(b"task_records", status.task_records)?;
        writer.key_usize(b"syscall_records", status.syscall_records)?;
        writer.key_usize(b"syscall_continuation_records", status.syscall_continuation_records)?;
        header.len = writer.finish();
    }
    let checksum = checksum32(header.as_bytes());
    {
        let mut writer = TextWriter::with_len(&mut header.bytes, header.len);
        writer.key_usize(b"checksum", checksum as usize)?;
        header.len = writer.finish();
    }
    Ok(header)
}

/// Encodes the current in-memory dump snapshot artifact.
pub fn encode_snapshot_artifact() -> Result<DumpArtifact, DumpEncodeError> {
    encode_status_artifact(status())
}

/// Encodes `status` plus retained diagnostic tables as a bounded dump artifact.
pub fn encode_status_artifact(status: DumpStatus) -> Result<DumpArtifact, DumpEncodeError> {
    let mut artifact = DumpArtifact {
        bytes: [0u8; MAX_DUMP_ARTIFACT_BYTES],
        len: 0,
    };
    encode_status_artifact_into(status, &mut artifact)?;
    Ok(artifact)
}

fn encode_status_artifact_into(
    status: DumpStatus,
    artifact: &mut DumpArtifact,
) -> Result<(), DumpEncodeError> {
    let header = encode_status_header(status)?;
    artifact.len = 0;
    artifact.bytes[..header.len].copy_from_slice(header.as_bytes());
    artifact.len = header.len;
    {
        let mut writer = TextWriter::with_len(&mut artifact.bytes, artifact.len);
        write_boot_table(&mut writer, status)?;
        write_device_table(&mut writer, status)?;
        write_proof_table(&mut writer, status)?;
        write_dump_sync_table(&mut writer, status)?;
        write_panic_table(&mut writer, status)?;
        write_event_table(&mut writer)?;
        write_process_table(&mut writer)?;
        write_address_space_table(&mut writer)?;
        write_address_space_page_table_table(&mut writer)?;
        write_address_space_page_table_entry_table(&mut writer)?;
        write_address_space_object_table(&mut writer)?;
        write_service_table(&mut writer)?;
        write_exec_load_table(&mut writer)?;
        write_pending_exec_table(&mut writer)?;
        write_scheduler_state(&mut writer)?;
        write_syscall_table(&mut writer)?;
        write_syscall_continuation_table(&mut writer)?;
        write_task_table(&mut writer)?;
        write_wait_table(&mut writer)?;
        artifact.len = writer.finish();
    }
    Ok(())
}

/// Parses and verifies a bounded dump snapshot header.
pub fn parse_snapshot_header(bytes: &[u8]) -> Result<ParsedSnapshotHeader<'_>, DumpParseError> {
    let (parsed, next) = parse_snapshot_header_inner(bytes)?;
    if next < bytes.len()
        && bytes[next..]
            .iter()
            .any(|byte| *byte != b'\n' && *byte != b'\r')
    {
        return Err(DumpParseError::BadLine);
    }
    Ok(parsed)
}

/// Parses and verifies a bounded dump snapshot header at the start of an artifact.
pub fn parse_snapshot_header_prefix(
    bytes: &[u8],
) -> Result<ParsedSnapshotHeader<'_>, DumpParseError> {
    parse_snapshot_header_inner(bytes).map(|(parsed, _)| parsed)
}

fn parse_snapshot_header_inner(
    bytes: &[u8],
) -> Result<(ParsedSnapshotHeader<'_>, usize), DumpParseError> {
    let mut offset = 0usize;
    let (line, next) = next_line(bytes, offset).ok_or(DumpParseError::BadHeader)?;
    offset = next;
    let Some(version_bytes) = line.strip_prefix(b"reovim-dump-v") else {
        return Err(DumpParseError::BadHeader);
    };
    let format_version = parse_usize(version_bytes)?;
    if format_version != 1 {
        return Err(DumpParseError::BadHeader);
    }

    let (boot_id, next) = parse_next_usize(bytes, offset, b"boot_id")?;
    offset = next;
    let (session_id, next) = parse_next_usize(bytes, offset, b"session_id")?;
    offset = next;
    let (identity_source, next) = parse_next_str(bytes, offset, b"identity_source")?;
    offset = next;
    let (package, next) = parse_next_str(bytes, offset, b"package")?;
    offset = next;
    let (version, next) = parse_next_str(bytes, offset, b"version")?;
    offset = next;
    let (target, next) = parse_next_str(bytes, offset, b"target")?;
    offset = next;
    let (selected_profile, next) = parse_next_str(bytes, offset, b"selected_profile")?;
    offset = next;
    let (profile_request, next) = parse_next_str(bytes, offset, b"profile_request")?;
    offset = next;
    let (bootline, next) = parse_next_str(bytes, offset, b"bootline")?;
    offset = next;
    let (launch_profile_feature, next) = parse_next_str(bytes, offset, b"launch_profile_feature")?;
    offset = next;
    let (boot_memory_ranges, next) = parse_next_usize(bytes, offset, b"boot_memory_ranges")?;
    offset = next;
    let (boot_memory_usable_bytes, next) =
        parse_next_usize(bytes, offset, b"boot_memory_usable_bytes")?;
    offset = next;
    let (boot_cpu_count, next) = parse_next_usize(bytes, offset, b"boot_cpu_count")?;
    offset = next;
    let (boot_heap_total_bytes, next) = parse_next_usize(bytes, offset, b"boot_heap_total_bytes")?;
    offset = next;
    let (device_records, next) = parse_next_usize(bytes, offset, b"device_records")?;
    offset = next;
    let (proof_state, next) = parse_next_str(bytes, offset, b"proof_state")?;
    if proof_state.is_empty() {
        return Err(DumpParseError::BadLine);
    }
    offset = next;
    let (panic_state, next) = parse_next_str(bytes, offset, b"panic_state")?;
    if panic_state.is_empty() {
        return Err(DumpParseError::BadLine);
    }
    offset = next;
    let (panic_records, next) = parse_next_usize(bytes, offset, b"panic_records")?;
    offset = next;
    let (persistent, next) = parse_next_str(bytes, offset, b"persistent")?;
    offset = next;
    let persistent_available = match persistent {
        "available" => true,
        "unavailable" => false,
        _ => return Err(DumpParseError::BadLine),
    };
    let (storage, next) = parse_next_str(bytes, offset, b"storage")?;
    if storage.is_empty() {
        return Err(DumpParseError::BadLine);
    }
    offset = next;
    let (storage_capacity_bytes, next) =
        parse_next_usize(bytes, offset, b"storage_capacity_bytes")?;
    offset = next;
    let (last_sync_attempted, next) = parse_next_bool(bytes, offset, b"last_sync_attempted")?;
    offset = next;
    let (last_sync_persistent, next) = parse_next_str(bytes, offset, b"last_sync_persistent")?;
    offset = next;
    let last_sync_persistent_available = match last_sync_persistent {
        "available" => true,
        "unavailable" => false,
        _ => return Err(DumpParseError::BadLine),
    };
    let (last_sync_storage, next) = parse_next_str(bytes, offset, b"last_sync_storage")?;
    if last_sync_storage.is_empty() {
        return Err(DumpParseError::BadLine);
    }
    offset = next;
    let (last_sync_storage_capacity_bytes, next) =
        parse_next_usize(bytes, offset, b"last_sync_storage_capacity_bytes")?;
    offset = next;
    let (last_sync_status, next) = parse_next_str(bytes, offset, b"last_sync_status")?;
    offset = next;
    let last_sync_written = match last_sync_status {
        "written" => true,
        "not-written" => false,
        _ => return Err(DumpParseError::BadLine),
    };
    let (last_sync_bytes_written, next) = parse_next_usize(bytes, offset, b"last_sync_bytes")?;
    offset = next;
    let (last_sync_checksum, next) = parse_next_usize(bytes, offset, b"last_sync_checksum")?;
    offset = next;
    let (last_sync_verified, next) = parse_next_bool(bytes, offset, b"last_sync_verified")?;
    offset = next;
    let (last_sync_reason, next) = parse_next_str(bytes, offset, b"last_sync_reason")?;
    if last_sync_reason.is_empty() {
        return Err(DumpParseError::BadLine);
    }
    offset = next;
    let (klog_retained_bytes, next) = parse_next_usize(bytes, offset, b"klog_retained_bytes")?;
    offset = next;
    let (klog_dropped_bytes, next) = parse_next_usize(bytes, offset, b"klog_dropped_bytes")?;
    offset = next;
    let (klog_next_event_seq, next) = parse_next_usize(bytes, offset, b"klog_next_event_seq")?;
    offset = next;
    let (event_records, next) = parse_next_usize(bytes, offset, b"event_records")?;
    offset = next;
    let (process_records, next) = parse_next_usize(bytes, offset, b"process_records")?;
    offset = next;
    let (service_records, next) = parse_next_usize(bytes, offset, b"service_records")?;
    offset = next;
    let (exec_load_records, next) = parse_next_usize(bytes, offset, b"exec_load_records")?;
    offset = next;
    let (pending_exec_records, next) = parse_next_usize(bytes, offset, b"pending_exec_records")?;
    offset = next;
    let (wait_records, next) = parse_next_usize(bytes, offset, b"wait_records")?;
    offset = next;
    let (task_records, next) = parse_next_usize(bytes, offset, b"task_records")?;
    offset = next;
    let (syscall_records, next) = parse_next_usize(bytes, offset, b"syscall_records")?;
    offset = next;
    let (syscall_continuation_records, next) =
        parse_next_usize(bytes, offset, b"syscall_continuation_records")?;
    offset = next;

    let checksum_offset = offset;
    let (checksum, next) = parse_next_usize(bytes, offset, b"checksum")
        .map_err(|_| DumpParseError::MissingChecksum)?;
    if checksum32(&bytes[..checksum_offset]) != checksum as u32 {
        return Err(DumpParseError::ChecksumMismatch);
    }

    Ok((
        ParsedSnapshotHeader {
            format_version,
            boot_id,
            session_id,
            identity_source,
            package,
            version,
            target,
            selected_profile,
            profile_request,
            bootline,
            launch_profile_feature,
            boot_memory_ranges,
            boot_memory_usable_bytes,
            boot_cpu_count,
            boot_heap_total_bytes,
            device_records,
            proof_state,
            panic_state,
            panic_records,
            persistent_available,
            storage,
            storage_capacity_bytes,
            klog_retained_bytes,
            klog_dropped_bytes,
            klog_next_event_seq,
            event_records,
            process_records,
            service_records,
            exec_load_records,
            pending_exec_records,
            wait_records,
            task_records,
            syscall_records,
            syscall_continuation_records,
            last_sync_attempted,
            last_sync_persistent_available,
            last_sync_storage,
            last_sync_storage_capacity_bytes,
            last_sync_written,
            last_sync_bytes_written,
            last_sync_checksum: last_sync_checksum as u32,
            last_sync_verified,
            last_sync_reason,
            checksum: checksum as u32,
        },
        next,
    ))
}

struct TextWriter<'a> {
    bytes: &'a mut [u8],
    len: usize,
}

impl<'a> TextWriter<'a> {
    fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, len: 0 }
    }

    fn with_len(bytes: &'a mut [u8], len: usize) -> Self {
        Self { bytes, len }
    }

    fn finish(self) -> usize {
        self.len
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), DumpEncodeError> {
        if self.len + bytes.len() > self.bytes.len() {
            return Err(DumpEncodeError::BufferTooSmall);
        }
        let end = self.len + bytes.len();
        self.bytes[self.len..end].copy_from_slice(bytes);
        self.len = end;
        Ok(())
    }

    fn nl(&mut self) -> Result<(), DumpEncodeError> {
        self.bytes(b"\n")
    }

    fn key_str(&mut self, key: &[u8], value: &str) -> Result<(), DumpEncodeError> {
        self.bytes(key)?;
        self.bytes(b"=")?;
        self.bytes(value.as_bytes())?;
        self.nl()
    }

    fn key_usize(&mut self, key: &[u8], value: usize) -> Result<(), DumpEncodeError> {
        self.bytes(key)?;
        self.bytes(b"=")?;
        self.usize(value)?;
        self.nl()
    }

    fn key_bool(&mut self, key: &[u8], value: bool) -> Result<(), DumpEncodeError> {
        self.key_str(key, if value { "true" } else { "false" })
    }

    fn key_u64(&mut self, key: &[u8], value: u64) -> Result<(), DumpEncodeError> {
        self.bytes(key)?;
        self.bytes(b"=")?;
        self.u64(value)?;
        self.nl()
    }

    fn usize(&mut self, value: usize) -> Result<(), DumpEncodeError> {
        let mut buf = [0u8; 20];
        let len = usize_to_dec(value, &mut buf);
        self.bytes(&buf[..len])
    }

    fn u64(&mut self, value: u64) -> Result<(), DumpEncodeError> {
        let mut buf = [0u8; 20];
        let len = u64_to_dec(value, &mut buf);
        self.bytes(&buf[..len])
    }

    fn u64_hex(&mut self, value: u64) -> Result<(), DumpEncodeError> {
        let mut buf = [0u8; 18];
        buf[0] = b'0';
        buf[1] = b'x';
        let len = u64_to_hex(value, &mut buf[2..]);
        self.bytes(&buf[..2 + len])
    }
}

fn write_boot_table(
    writer: &mut TextWriter<'_>,
    status: DumpStatus,
) -> Result<(), DumpEncodeError> {
    writer.bytes(b"boot:\n")?;
    writer.bytes(b"memory_ranges=")?;
    writer.usize(status.boot_info.memory.range_count())?;
    writer.bytes(b"\nmemory_usable_bytes=")?;
    writer.u64(status.boot_info.memory.usable_bytes())?;
    writer.bytes(b"\ncpu_count=")?;
    writer.usize(status.boot_info.cpu_count as usize)?;
    writer.bytes(b"\nheap_total_bytes=")?;
    writer.u64(status.boot_info.heap_total_bytes)?;
    writer.bytes(b"\ncpu_freq_hz=")?;
    writer.u64(status.boot_info.cpu_freq_hz)?;
    writer.bytes(b"\nmem_freq_hz=")?;
    writer.u64(status.boot_info.mem_freq_hz)?;
    writer.bytes(b"\ncache_line_bytes=")?;
    writer.usize(status.boot_info.cache_line_bytes as usize)?;
    writer.nl()
}

fn write_device_table(
    writer: &mut TextWriter<'_>,
    status: DumpStatus,
) -> Result<(), DumpEncodeError> {
    writer.bytes(b"devices:\n")?;
    let mut index = 0usize;
    while index < status.device_records {
        let device = status.devices[index];
        writer.bytes(b"- index=")?;
        writer.usize(index)?;
        writer.bytes(b" class=")?;
        writer.bytes(device_class_name(device.class).as_bytes())?;
        writer.bytes(b" compat=")?;
        writer.bytes(device.compatible.as_bytes())?;
        writer.bytes(b" mmio_base=")?;
        writer.u64_hex(device.mmio_base)?;
        writer.bytes(b" mmio_len=")?;
        writer.u64_hex(device.mmio_len)?;
        writer.bytes(b" irq=")?;
        writer.usize(device.irq as usize)?;
        writer.bytes(b" capacity_bytes=")?;
        writer.u64(device.capacity_bytes)?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_proof_table(
    writer: &mut TextWriter<'_>,
    status: DumpStatus,
) -> Result<(), DumpEncodeError> {
    writer.bytes(b"proof:\nstate=")?;
    writer.bytes(status.proof_state.as_bytes())?;
    writer.bytes(b"\nphysical_usb_keyboard=required\n")?;
    writer.bytes(b"zero_dropped_logs=required\n")?;
    writer.bytes(b"dump_sync=")?;
    writer.bytes(if status.persistent_available {
        b"available"
    } else {
        b"unavailable"
    })?;
    writer.bytes(b"\ndump_storage=")?;
    writer.bytes(status.storage.as_bytes())?;
    writer.bytes(b"\ndump_storage_capacity_bytes=")?;
    writer.usize(status.storage_capacity_bytes)?;
    writer.bytes(b"\nlast_dump_sync_attempted=")?;
    writer.bytes(if status.last_sync.attempted {
        b"true"
    } else {
        b"false"
    })?;
    writer.bytes(b"\nlast_dump_sync_status=")?;
    writer.bytes(if status.last_sync.written {
        b"written"
    } else {
        b"not-written"
    })?;
    writer.bytes(b"\nlast_dump_sync_reason=")?;
    writer.bytes(status.last_sync.reason.as_bytes())?;
    writer.nl()
}

fn write_dump_sync_table(
    writer: &mut TextWriter<'_>,
    status: DumpStatus,
) -> Result<(), DumpEncodeError> {
    writer.bytes(b"dump-sync:\n")?;
    writer.bytes(b"attempted=")?;
    writer.bytes(if status.last_sync.attempted {
        b"true"
    } else {
        b"false"
    })?;
    writer.bytes(b"\npersistent=")?;
    writer.bytes(if status.last_sync.persistent_available {
        b"available"
    } else {
        b"unavailable"
    })?;
    writer.bytes(b"\nstorage=")?;
    writer.bytes(status.last_sync.storage.as_bytes())?;
    writer.bytes(b"\nstorage_capacity_bytes=")?;
    writer.usize(status.last_sync.storage_capacity_bytes)?;
    writer.bytes(b"\nstatus=")?;
    writer.bytes(if status.last_sync.written {
        b"written"
    } else {
        b"not-written"
    })?;
    writer.bytes(b"\nbytes=")?;
    writer.usize(status.last_sync.bytes_written)?;
    writer.bytes(b"\nsync_checksum=")?;
    writer.usize(status.last_sync.checksum as usize)?;
    writer.bytes(b"\nverified=")?;
    writer.bytes(if status.last_sync.verified {
        b"true"
    } else {
        b"false"
    })?;
    writer.bytes(b"\nreason=")?;
    writer.bytes(status.last_sync.reason.as_bytes())?;
    writer.nl()
}

fn write_panic_table(
    writer: &mut TextWriter<'_>,
    status: DumpStatus,
) -> Result<(), DumpEncodeError> {
    writer.bytes(b"panic:\nstate=")?;
    writer.bytes(status.panic_state.as_bytes())?;
    writer.bytes(b"\nrecords=")?;
    writer.usize(status.panic_records)?;
    writer.nl()?;
    if let Some(record) = status.panic_record {
        writer.bytes(b"- disposition=")?;
        writer.bytes(disposition_name(record.disposition).as_bytes())?;
        writer.bytes(b" rollback_failed=")?;
        writer.bytes(if record.rollback_failed {
            b"true"
        } else {
            b"false"
        })?;
        writer.nl()
    } else {
        writer.bytes(b"record=none\n")
    }
}

fn write_event_table(writer: &mut TextWriter<'_>) -> Result<(), DumpEncodeError> {
    let mut records = [klog::EMPTY_EVENT_RECORD; klog::MAX_EVENTS];
    let count = klog::snapshot_events(&mut records);
    writer.bytes(b"events:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- seq=")?;
        writer.usize(record.seq)?;
        writer.bytes(b" boot=")?;
        writer.usize(record.boot_id)?;
        writer.bytes(b" session=")?;
        writer.usize(record.session_id)?;
        writer.bytes(b" source=")?;
        writer.bytes(record.source.as_bytes())?;
        writer.bytes(b" component=")?;
        writer.bytes(record.component.as_bytes())?;
        writer.bytes(b" severity=")?;
        writer.bytes(record.severity.as_bytes())?;
        writer.bytes(b" kind=")?;
        writer.bytes(record.kind.as_bytes())?;
        writer.bytes(b" pid=")?;
        writer.usize(record.process_id)?;
        writer.bytes(b" task=")?;
        writer.usize(record.task_id)?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

const fn device_class_name(class: DeviceClass) -> &'static str {
    match class {
        DeviceClass::Uart => "uart",
        DeviceClass::Interrupt => "interrupt",
        DeviceClass::Mailbox => "mailbox",
        DeviceClass::Block => "block",
        DeviceClass::Usb => "usb",
        DeviceClass::Bus => "bus",
        DeviceClass::Unknown => "unknown",
    }
}

const fn disposition_name(disposition: Disposition) -> &'static str {
    match disposition {
        Disposition::Recover => "recover",
        Disposition::Halt => "halt",
    }
}

fn write_process_table(writer: &mut TextWriter<'_>) -> Result<(), DumpEncodeError> {
    let mut records = [proc::EMPTY_PROCESS_RECORD; proc::MAX_PROCESSES];
    let count = proc::snapshot(&mut records);
    writer.bytes(b"processes:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- pid=")?;
        writer.usize(record.pid)?;
        writer.bytes(b" ppid=")?;
        writer.usize(record.parent_pid)?;
        writer.bytes(b" task=")?;
        writer.usize(record.task_id)?;
        writer.bytes(b" state=")?;
        writer.bytes(record.state.as_str().as_bytes())?;
        writer.bytes(b" path=")?;
        writer.bytes(record.program_path.as_bytes())?;
        writer.bytes(b" exit=")?;
        writer.usize(record.exit_code as usize)?;
        writer.bytes(b" loader=")?;
        writer.bytes(record.loader.as_bytes())?;
        writer.bytes(b" entry_fn=")?;
        writer.bytes(record.entry_name.as_bytes())?;
        writer.bytes(b" block=")?;
        writer.bytes(record.block_reason.as_str().as_bytes())?;
        writer.bytes(b" argc=")?;
        writer.usize(record.argc)?;
        writer.bytes(b" argv0=")?;
        writer.bytes(record.argv0().as_bytes())?;
        writer.bytes(b" argv0_truncated=")?;
        writer.bytes(if record.argv_was_truncated(0) {
            b"true"
        } else {
            b"false"
        })?;
        writer.bytes(b" argv1=")?;
        writer.bytes(record.argv1().as_bytes())?;
        writer.bytes(b" argv1_truncated=")?;
        writer.bytes(if record.argv_was_truncated(1) {
            b"true"
        } else {
            b"false"
        })?;
        let mut argv_index = 2usize;
        while argv_index < record.argc && argv_index < proc::MAX_PROCESS_ARGS {
            writer.bytes(b" argv")?;
            writer.usize(argv_index)?;
            writer.bytes(b"=")?;
            writer.bytes(record.argv(argv_index).as_bytes())?;
            writer.bytes(b" argv")?;
            writer.usize(argv_index)?;
            writer.bytes(b"_truncated=")?;
            writer.bytes(if record.argv_was_truncated(argv_index) {
                b"true"
            } else {
                b"false"
            })?;
            argv_index += 1;
        }
        writer.bytes(b" envc=")?;
        writer.usize(record.envc)?;
        let mut env_index = 0usize;
        while env_index < record.envc && env_index < proc::MAX_PROCESS_ENVS {
            writer.bytes(b" env")?;
            writer.usize(env_index)?;
            writer.bytes(b"_name=")?;
            writer.bytes(record.env_name(env_index).as_bytes())?;
            writer.bytes(b" env")?;
            writer.usize(env_index)?;
            writer.bytes(b"_value=")?;
            writer.bytes(record.env_value(env_index).as_bytes())?;
            writer.bytes(b" env")?;
            writer.usize(env_index)?;
            writer.bytes(b"_truncated=")?;
            writer.bytes(if record.env_was_truncated(env_index) {
                b"true"
            } else {
                b"false"
            })?;
            env_index += 1;
        }
        writer.bytes(b" address_space=")?;
        writer.usize(record.address_space_id)?;
        let address_space = mm::address_space(record.address_space_id);
        writer.bytes(b" address_space_state=")?;
        let address_space_state = address_space.map_or("missing", |space| space.state.as_str());
        writer.bytes(address_space_state.as_bytes())?;
        writer.bytes(b" address_space_source=")?;
        writer.bytes(
            address_space
                .map_or("", |space| space.source_path)
                .as_bytes(),
        )?;
        writer.bytes(b" address_space_text_bytes=")?;
        writer.usize(address_space.map_or(0, |space| space.text_bytes))?;
        writer.bytes(b" address_space_text_checksum=")?;
        writer.usize(address_space.map_or(0, |space| space.text_checksum as usize))?;
        writer.bytes(b" address_space_stack_bytes=")?;
        writer.usize(address_space.map_or(0, |space| space.stack_bytes))?;
        writer.bytes(b" address_space_regions=")?;
        writer.usize(address_space.map_or(0, |space| space.region_count))?;
        writer.bytes(b" address_space_page_table=")?;
        writer.usize(address_space.map_or(0, |space| space.page_table_id))?;
        writer.bytes(b" address_space_mapped_pages=")?;
        writer.usize(address_space.map_or(0, |space| space.mapped_pages))?;
        writer.bytes(b" address_space_text_pages=")?;
        writer.usize(address_space.map_or(0, |space| space.text_pages))?;
        writer.bytes(b" address_space_stack_pages=")?;
        writer.usize(address_space.map_or(0, |space| space.stack_pages))?;
        writer.bytes(b" address_space_text_start=")?;
        writer.usize(address_space.map_or(0, |space| space.text_start))?;
        writer.bytes(b" address_space_text_end=")?;
        writer.usize(address_space.map_or(0, |space| space.text_end))?;
        writer.bytes(b" address_space_text_flags=")?;
        writer.usize(address_space.map_or(0, |space| space.text_flags as usize))?;
        writer.bytes(b" address_space_stack_start=")?;
        writer.usize(address_space.map_or(0, |space| space.stack_start))?;
        writer.bytes(b" address_space_stack_end=")?;
        writer.usize(address_space.map_or(0, |space| space.stack_end))?;
        writer.bytes(b" address_space_stack_flags=")?;
        writer.usize(address_space.map_or(0, |space| space.stack_flags as usize))?;
        writer.bytes(b" artifact_body_format=")?;
        writer.bytes(record.artifact_body_format.as_str().as_bytes())?;
        writer.bytes(b" artifact_body_inner=")?;
        writer.bytes(record.artifact_body_inner_format.as_str().as_bytes())?;
        writer.bytes(b" image_generation=")?;
        writer.usize(record.image_generation)?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_address_space_table(writer: &mut TextWriter<'_>) -> Result<(), DumpEncodeError> {
    let mut records = [mm::EMPTY_ADDRESS_SPACE_RECORD; mm::MAX_ADDRESS_SPACE_RECORDS];
    let count = mm::snapshot_address_spaces(&mut records);
    writer.bytes(b"address-spaces:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- id=")?;
        writer.usize(record.id)?;
        writer.bytes(b" owner_pid=")?;
        writer.usize(record.owner_pid)?;
        writer.bytes(b" image_generation=")?;
        writer.usize(record.image_generation)?;
        writer.bytes(b" state=")?;
        writer.bytes(record.state.as_str().as_bytes())?;
        writer.bytes(b" path=")?;
        writer.bytes(record.program_path.as_bytes())?;
        writer.bytes(b" loader=")?;
        writer.bytes(record.loader.as_bytes())?;
        writer.bytes(b" entry_fn=")?;
        writer.bytes(record.entry_name.as_bytes())?;
        writer.bytes(b" source=")?;
        writer.bytes(record.source_path.as_bytes())?;
        writer.bytes(b" text_bytes=")?;
        writer.usize(record.text_bytes)?;
        writer.bytes(b" text_checksum=")?;
        writer.usize(record.text_checksum as usize)?;
        writer.bytes(b" stack_bytes=")?;
        writer.usize(record.stack_bytes)?;
        writer.bytes(b" regions=")?;
        writer.usize(record.region_count)?;
        writer.bytes(b" text_start=")?;
        writer.usize(record.text_start)?;
        writer.bytes(b" text_end=")?;
        writer.usize(record.text_end)?;
        writer.bytes(b" text_flags=")?;
        writer.usize(record.text_flags as usize)?;
        writer.bytes(b" stack_start=")?;
        writer.usize(record.stack_start)?;
        writer.bytes(b" stack_end=")?;
        writer.usize(record.stack_end)?;
        writer.bytes(b" stack_flags=")?;
        writer.usize(record.stack_flags as usize)?;
        writer.bytes(b" page_table=")?;
        writer.usize(record.page_table_id)?;
        writer.bytes(b" mapped_pages=")?;
        writer.usize(record.mapped_pages)?;
        writer.bytes(b" text_pages=")?;
        writer.usize(record.text_pages)?;
        writer.bytes(b" stack_pages=")?;
        writer.usize(record.stack_pages)?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_address_space_page_table_table(
    writer: &mut TextWriter<'_>,
) -> Result<(), DumpEncodeError> {
    let mut records =
        [mm::EMPTY_ADDRESS_SPACE_PAGE_TABLE_RECORD; mm::MAX_ADDRESS_SPACE_PAGE_TABLE_RECORDS];
    let count = mm::snapshot_address_space_page_tables(&mut records);
    writer.bytes(b"address-space-page-tables:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- id=")?;
        writer.usize(record.id)?;
        writer.bytes(b" address_space=")?;
        writer.usize(record.address_space_id)?;
        writer.bytes(b" owner_pid=")?;
        writer.usize(record.owner_pid)?;
        writer.bytes(b" image_generation=")?;
        writer.usize(record.image_generation)?;
        writer.bytes(b" state=")?;
        writer.bytes(record.state.as_str().as_bytes())?;
        writer.bytes(b" root_table=")?;
        writer.usize(record.root_table_id)?;
        writer.bytes(b" source=")?;
        writer.bytes(record.source_path.as_bytes())?;
        writer.bytes(b" mapped_pages=")?;
        writer.usize(record.mapped_pages)?;
        writer.bytes(b" text_pages=")?;
        writer.usize(record.text_pages)?;
        writer.bytes(b" stack_pages=")?;
        writer.usize(record.stack_pages)?;
        writer.bytes(b" text_flags=")?;
        writer.usize(record.text_flags as usize)?;
        writer.bytes(b" stack_flags=")?;
        writer.usize(record.stack_flags as usize)?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_address_space_page_table_entry_table(
    writer: &mut TextWriter<'_>,
) -> Result<(), DumpEncodeError> {
    let mut records = [mm::EMPTY_ADDRESS_SPACE_PAGE_RECORD; mm::MAX_ADDRESS_SPACE_PAGE_RECORDS];
    let count = mm::snapshot_address_space_pages(&mut records);
    writer.bytes(b"address-space-pages:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- id=")?;
        writer.usize(record.id)?;
        writer.bytes(b" address_space=")?;
        writer.usize(record.address_space_id)?;
        writer.bytes(b" page_table=")?;
        writer.usize(record.page_table_id)?;
        writer.bytes(b" owner_pid=")?;
        writer.usize(record.owner_pid)?;
        writer.bytes(b" image_generation=")?;
        writer.usize(record.image_generation)?;
        writer.bytes(b" state=")?;
        writer.bytes(record.state.as_str().as_bytes())?;
        writer.bytes(b" kind=")?;
        writer.bytes(record.kind.as_str().as_bytes())?;
        writer.bytes(b" backing=")?;
        writer.bytes(record.backing.as_str().as_bytes())?;
        writer.bytes(b" source=")?;
        writer.bytes(record.source_path.as_bytes())?;
        writer.bytes(b" page_index=")?;
        writer.usize(record.page_index)?;
        writer.bytes(b" virtual_start=")?;
        writer.usize(record.virtual_start)?;
        writer.bytes(b" virtual_end=")?;
        writer.usize(record.virtual_end)?;
        writer.bytes(b" bytes=")?;
        writer.usize(record.bytes)?;
        writer.bytes(b" source_offset=")?;
        writer.usize(record.source_offset)?;
        writer.bytes(b" flags=")?;
        writer.usize(record.flags as usize)?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_address_space_object_table(writer: &mut TextWriter<'_>) -> Result<(), DumpEncodeError> {
    let mut records = [mm::EMPTY_ADDRESS_SPACE_OBJECT_RECORD; mm::MAX_ADDRESS_SPACE_OBJECT_RECORDS];
    let count = mm::snapshot_address_space_objects(&mut records);
    writer.bytes(b"address-space-objects:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- id=")?;
        writer.usize(record.id)?;
        writer.bytes(b" address_space=")?;
        writer.usize(record.address_space_id)?;
        writer.bytes(b" owner_pid=")?;
        writer.usize(record.owner_pid)?;
        writer.bytes(b" image_generation=")?;
        writer.usize(record.image_generation)?;
        writer.bytes(b" state=")?;
        writer.bytes(record.state.as_str().as_bytes())?;
        writer.bytes(b" kind=")?;
        writer.bytes(record.kind.as_str().as_bytes())?;
        writer.bytes(b" backing=")?;
        writer.bytes(record.backing.as_str().as_bytes())?;
        writer.bytes(b" source=")?;
        writer.bytes(record.source_path.as_bytes())?;
        writer.bytes(b" start=")?;
        writer.usize(record.start)?;
        writer.bytes(b" end=")?;
        writer.usize(record.end)?;
        writer.bytes(b" bytes=")?;
        writer.usize(record.bytes)?;
        writer.bytes(b" checksum=")?;
        writer.usize(record.checksum as usize)?;
        writer.bytes(b" flags=")?;
        writer.usize(record.flags as usize)?;
        writer.bytes(b" page_count=")?;
        writer.usize(record.page_count)?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_service_table(writer: &mut TextWriter<'_>) -> Result<(), DumpEncodeError> {
    let mut records = [service::EMPTY_SERVICE_RECORD; service::MAX_SERVICES];
    let count = service::snapshot(&mut records);
    writer.bytes(b"services:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- seq=")?;
        writer.usize(record.seq)?;
        writer.bytes(b" name=")?;
        writer.bytes(record.name.as_bytes())?;
        writer.bytes(b" target=")?;
        writer.bytes(record.target.as_bytes())?;
        writer.bytes(b" state=")?;
        writer.bytes(record.state.as_str().as_bytes())?;
        writer.bytes(b" reason=")?;
        writer.bytes(record.reason.as_str().as_bytes())?;
        writer.bytes(b" owner_pid=")?;
        writer.usize(record.owner_pid)?;
        writer.bytes(b" owner_task=")?;
        writer.usize(record.owner_task_id)?;
        writer.bytes(b" service_pid=")?;
        writer.usize(record.service_pid)?;
        writer.bytes(b" service_task=")?;
        writer.usize(record.service_task_id)?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_exec_load_table(writer: &mut TextWriter<'_>) -> Result<(), DumpEncodeError> {
    let mut records = [exec::EMPTY_EXEC_LOAD_RECORD; exec::MAX_EXEC_LOAD_RECORDS];
    let count = exec::snapshot_loads(&mut records);
    writer.bytes(b"execs:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- seq=")?;
        writer.usize(record.seq)?;
        writer.bytes(b" argv0=")?;
        writer.bytes(record.argv0().as_bytes())?;
        writer.bytes(b" status=")?;
        writer.bytes(record.status.as_str().as_bytes())?;
        writer.bytes(b" reason=")?;
        writer.bytes(record.reason.as_str().as_bytes())?;
        writer.bytes(b" path=")?;
        writer.bytes(record.path.as_bytes())?;
        writer.bytes(b" source=")?;
        writer.bytes(record.source_path.as_bytes())?;
        writer.bytes(b" loader=")?;
        writer.bytes(record.loader.as_bytes())?;
        writer.bytes(b" entry_fn=")?;
        writer.bytes(record.entry_name.as_bytes())?;
        writer.bytes(b" truncated=")?;
        writer.bytes(if record.argv0_truncated {
            b"true"
        } else {
            b"false"
        })?;
        writer.bytes(b" kind=")?;
        writer.bytes(record.kind.as_str().as_bytes())?;
        writer.bytes(b" origin=")?;
        writer.bytes(record.origin.as_str().as_bytes())?;
        writer.bytes(b" artifact_format=")?;
        writer.bytes(record.artifact_format.as_str().as_bytes())?;
        writer.bytes(b" artifact_body_format=")?;
        writer.bytes(record.artifact_body_format.as_str().as_bytes())?;
        writer.bytes(b" artifact_body_inner=")?;
        writer.bytes(record.artifact_body_inner_format.as_str().as_bytes())?;
        writer.bytes(b" artifact_bytes=")?;
        writer.usize(record.artifact_bytes_len)?;
        writer.bytes(b" artifact_body_bytes=")?;
        writer.usize(record.artifact_body_bytes_len)?;
        writer.bytes(b" artifact_checksum=")?;
        writer.usize(record.artifact_checksum as usize)?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_pending_exec_table(writer: &mut TextWriter<'_>) -> Result<(), DumpEncodeError> {
    let mut records = [exec::EMPTY_PENDING_EXEC_RECORD; exec::MAX_PENDING_EXEC_RECORDS];
    let count = exec::snapshot_pending(&mut records);
    writer.bytes(b"pending:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- pid=")?;
        writer.usize(record.pid)?;
        writer.bytes(b" ppid=")?;
        writer.usize(record.parent_pid)?;
        writer.bytes(b" task=")?;
        writer.usize(record.task_id)?;
        writer.bytes(b" path=")?;
        writer.bytes(record.path.as_bytes())?;
        writer.bytes(b" loader=")?;
        writer.bytes(record.loader.as_bytes())?;
        writer.bytes(b" entry_fn=")?;
        writer.bytes(record.entry_name.as_bytes())?;
        writer.bytes(b" kind=")?;
        writer.bytes(record.kind.as_str().as_bytes())?;
        writer.bytes(b" artifact_body_format=")?;
        writer.bytes(record.artifact_body_format.as_str().as_bytes())?;
        writer.bytes(b" artifact_body_inner=")?;
        writer.bytes(record.artifact_body_inner_format.as_str().as_bytes())?;
        writer.bytes(b" stdin_bytes=")?;
        writer.usize(record.stdin_len)?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_scheduler_state(writer: &mut TextWriter<'_>) -> Result<(), DumpEncodeError> {
    let snapshot = sched::snapshot_scheduler();
    writer.bytes(b"scheduler:\ncurrent_task=")?;
    writer.usize(snapshot.current_task_id)?;
    writer.bytes(b"\ncurrent_pid=")?;
    writer.usize(snapshot.current_process_id)?;
    writer.bytes(b"\nready_queue_len=")?;
    writer.usize(snapshot.ready_len)?;
    writer.bytes(b"\nnext_ready_task=")?;
    writer.usize(snapshot.next_ready_task_id())?;
    writer.bytes(b"\nnext_ready_pid=")?;
    writer.usize(snapshot.next_ready_process_id())?;
    writer.bytes(b"\ndispatch_count=")?;
    writer.usize(snapshot.dispatch_count)?;
    writer.bytes(b"\nyield_count=")?;
    writer.usize(snapshot.yield_count)?;
    writer.bytes(b"\ntick_count=")?;
    writer.usize(snapshot.tick_count)?;
    writer.bytes(b"\nready_queue:\n")?;
    let mut index = 0usize;
    while index < snapshot.ready_len {
        writer.bytes(b"- task=")?;
        writer.usize(snapshot.ready_queue[index])?;
        writer.bytes(b" pid=")?;
        writer.usize(snapshot.ready_process_queue[index])?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_syscall_table(writer: &mut TextWriter<'_>) -> Result<(), DumpEncodeError> {
    let mut records = [syscall::EMPTY_SYSCALL_RECORD; syscall::MAX_SYSCALL_RECORDS];
    let count = syscall::snapshot_syscalls(&mut records);
    writer.bytes(b"syscalls:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- seq=")?;
        writer.usize(record.seq)?;
        writer.bytes(b" pid=")?;
        writer.usize(record.process_id)?;
        writer.bytes(b" task=")?;
        writer.usize(record.task_id)?;
        writer.bytes(b" path=")?;
        writer.bytes(record.program_path.as_bytes())?;
        writer.bytes(b" op=")?;
        writer.bytes(record.op.as_str().as_bytes())?;
        writer.bytes(b" status=")?;
        writer.bytes(record.status.as_str().as_bytes())?;
        writer.bytes(b" loader=")?;
        writer.bytes(record.loader.as_bytes())?;
        writer.bytes(b" entry_fn=")?;
        writer.bytes(record.entry_name.as_bytes())?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_syscall_continuation_table(writer: &mut TextWriter<'_>) -> Result<(), DumpEncodeError> {
    let mut records =
        [syscall::EMPTY_SYSCALL_CONTINUATION_RECORD; syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let count = syscall::snapshot_syscall_continuations(&mut records);
    writer.bytes(b"continuations:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- pid=")?;
        writer.usize(record.process_id)?;
        writer.bytes(b" task=")?;
        writer.usize(record.task_id)?;
        writer.bytes(b" path=")?;
        writer.bytes(record.program_path.as_bytes())?;
        writer.bytes(b" nr=")?;
        writer.usize(record.nr.raw() as usize)?;
        writer.bytes(b" op=")?;
        writer.bytes(record.op.as_str().as_bytes())?;
        writer.bytes(b" memory=")?;
        writer.bytes(record.memory.as_str().as_bytes())?;
        writer.bytes(b" a0=")?;
        writer.usize(record.args.a0)?;
        writer.bytes(b" a1=")?;
        writer.usize(record.args.a1)?;
        writer.bytes(b" a2=")?;
        writer.usize(record.args.a2)?;
        writer.bytes(b" a3=")?;
        writer.usize(record.args.a3)?;
        writer.bytes(b" a4=")?;
        writer.usize(record.args.a4)?;
        writer.bytes(b" a5=")?;
        writer.usize(record.args.a5)?;
        writer.bytes(b" loader=")?;
        writer.bytes(record.loader.as_bytes())?;
        writer.bytes(b" entry_fn=")?;
        writer.bytes(record.entry_name.as_bytes())?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_task_table(writer: &mut TextWriter<'_>) -> Result<(), DumpEncodeError> {
    let mut records = [sched::EMPTY_KERNEL_TASK_RECORD; sched::MAX_KERNEL_TASKS];
    let count = sched::snapshot_kernel_tasks(&mut records);
    writer.bytes(b"tasks:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- task=")?;
        writer.usize(record.task_id)?;
        writer.bytes(b" pid=")?;
        writer.usize(record.process_id)?;
        writer.bytes(b" parent_task=")?;
        writer.usize(record.parent_task_id)?;
        writer.bytes(b" state=")?;
        writer.bytes(record.state.as_str().as_bytes())?;
        writer.bytes(b" entry=")?;
        writer.bytes(record.entry.as_bytes())?;
        writer.bytes(b" block=")?;
        writer.bytes(record.block_reason.as_str().as_bytes())?;
        writer.bytes(b" wake_tick=")?;
        writer.usize(record.wake_tick)?;
        writer.bytes(b" runs=")?;
        writer.usize(record.run_count)?;
        writer.bytes(b" ticks=")?;
        writer.usize(record.runtime_ticks)?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn write_wait_table(writer: &mut TextWriter<'_>) -> Result<(), DumpEncodeError> {
    let mut records = [proc::EMPTY_WAIT_RECORD; proc::MAX_WAITS];
    let count = proc::snapshot_waits(&mut records);
    writer.bytes(b"waits:\n")?;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        writer.bytes(b"- parent_pid=")?;
        writer.usize(record.parent_pid)?;
        writer.bytes(b" child_pid=")?;
        writer.usize(record.child_pid)?;
        writer.bytes(b" child_state=")?;
        writer.bytes(record.child_state.as_str().as_bytes())?;
        writer.bytes(b" exit=")?;
        writer.usize(record.exit_code as usize)?;
        writer.bytes(b" completed=")?;
        writer.bytes(if record.completed { b"true" } else { b"false" })?;
        writer.nl()?;
        index += 1;
    }
    Ok(())
}

fn next_line(bytes: &[u8], offset: usize) -> Option<(&[u8], usize)> {
    if offset >= bytes.len() {
        return None;
    }
    let mut end = offset;
    while end < bytes.len() && bytes[end] != b'\n' && bytes[end] != b'\r' {
        end += 1;
    }
    let mut next = end;
    while next < bytes.len() && (bytes[next] == b'\n' || bytes[next] == b'\r') {
        next += 1;
    }
    Some((&bytes[offset..end], next))
}

fn parse_next_usize(
    bytes: &[u8],
    offset: usize,
    key: &[u8],
) -> Result<(usize, usize), DumpParseError> {
    let (line, next) = next_line(bytes, offset).ok_or(DumpParseError::BadLine)?;
    let value = line_value(line, key)?;
    Ok((parse_usize(value)?, next))
}

fn parse_next_str<'a>(
    bytes: &'a [u8],
    offset: usize,
    key: &[u8],
) -> Result<(&'a str, usize), DumpParseError> {
    let (line, next) = next_line(bytes, offset).ok_or(DumpParseError::BadLine)?;
    let value = line_value(line, key)?;
    let value = core::str::from_utf8(value).map_err(|_| DumpParseError::BadLine)?;
    Ok((value, next))
}

fn parse_next_bool(
    bytes: &[u8],
    offset: usize,
    key: &[u8],
) -> Result<(bool, usize), DumpParseError> {
    let (value, next) = parse_next_str(bytes, offset, key)?;
    match value {
        "true" => Ok((true, next)),
        "false" => Ok((false, next)),
        _ => Err(DumpParseError::BadLine),
    }
}

fn line_value<'a>(line: &'a [u8], key: &[u8]) -> Result<&'a [u8], DumpParseError> {
    if line.len() <= key.len() || &line[..key.len()] != key || line[key.len()] != b'=' {
        return Err(DumpParseError::BadLine);
    }
    Ok(&line[key.len() + 1..])
}

fn parse_usize(bytes: &[u8]) -> Result<usize, DumpParseError> {
    if bytes.is_empty() {
        return Err(DumpParseError::BadNumber);
    }
    let mut value = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if !byte.is_ascii_digit() {
            return Err(DumpParseError::BadNumber);
        }
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add((byte - b'0') as usize))
            .ok_or(DumpParseError::BadNumber)?;
        index += 1;
    }
    Ok(value)
}

fn usize_to_dec(mut value: usize, out: &mut [u8]) -> usize {
    if value == 0 {
        out[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 20];
    let mut len = 0usize;
    while value > 0 {
        tmp[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
    }
    let mut index = 0usize;
    while len > 0 {
        len -= 1;
        out[index] = tmp[len];
        index += 1;
    }
    index
}

fn u64_to_dec(mut value: u64, out: &mut [u8]) -> usize {
    if value == 0 {
        out[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 20];
    let mut len = 0usize;
    while value > 0 {
        tmp[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
    }
    let mut i = 0usize;
    while i < len {
        out[i] = tmp[len - 1 - i];
        i += 1;
    }
    len
}

fn u64_to_hex(mut value: u64, out: &mut [u8]) -> usize {
    if value == 0 {
        out[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 16];
    let mut len = 0usize;
    while value > 0 {
        let digit = (value & 0xf) as u8;
        tmp[len] = match digit {
            0..=9 => b'0' + digit,
            _ => b'a' + (digit - 10),
        };
        len += 1;
        value >>= 4;
    }
    let mut i = 0usize;
    while i < len {
        out[i] = tmp[len - 1 - i];
        i += 1;
    }
    len
}

fn checksum32(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c9dc5u32;
    let mut index = 0usize;
    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(16_777_619);
        index += 1;
    }
    hash
}

#[cfg(feature = "selftest")]
#[path = "dump_tests.rs"]
mod tests;
