//! Product-facing diagnostic dump control vocabulary.
//!
//! Dump status and snapshots are read through `/dump/*` filesystem objects.
//! This leaf owns the typed control needed to request a persistent dump sync;
//! it lowers through the raw syscall transport without turning the raw leaf
//! into a second semantic API.

#![no_std]

use {
    core::mem::size_of,
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr},
};

/// Maximum storage label bytes retained in a dump-sync report.
pub const DUMP_SYNC_STORAGE_BYTES: usize = 64;
/// Maximum reason bytes retained in a dump-sync report.
pub const DUMP_SYNC_REASON_BYTES: usize = 64;

/// Result details from one diagnostic dump sync request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DumpSyncReport {
    attempted: u8,
    persistent_available: u8,
    written: u8,
    verified: u8,
    storage_capacity_bytes: usize,
    bytes_written: usize,
    checksum: u32,
    storage_len: usize,
    storage: [u8; DUMP_SYNC_STORAGE_BYTES],
    reason_len: usize,
    reason: [u8; DUMP_SYNC_REASON_BYTES],
}

impl DumpSyncReport {
    /// Empty report value used before a sync request is issued.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            attempted: 0,
            persistent_available: 0,
            written: 0,
            verified: 0,
            storage_capacity_bytes: 0,
            bytes_written: 0,
            checksum: 0,
            storage_len: 0,
            storage: [0; DUMP_SYNC_STORAGE_BYTES],
            reason_len: 0,
            reason: [0; DUMP_SYNC_REASON_BYTES],
        }
    }

    /// Clears this report to the empty value.
    pub fn clear(&mut self) {
        *self = Self::empty();
    }

    /// Records whether the sync path was attempted.
    pub fn set_attempted(&mut self, value: bool) {
        self.attempted = u8::from(value);
    }

    /// Records whether persistent dump storage was available.
    pub fn set_persistent_available(&mut self, value: bool) {
        self.persistent_available = u8::from(value);
    }

    /// Records whether bytes were written.
    pub fn set_written(&mut self, value: bool) {
        self.written = u8::from(value);
    }

    /// Records whether read-back verification succeeded.
    pub fn set_verified(&mut self, value: bool) {
        self.verified = u8::from(value);
    }

    /// Records the storage capacity in bytes.
    pub fn set_storage_capacity_bytes(&mut self, value: usize) {
        self.storage_capacity_bytes = value;
    }

    /// Records the written byte count.
    pub fn set_bytes_written(&mut self, value: usize) {
        self.bytes_written = value;
    }

    /// Records the verified artifact checksum.
    pub fn set_checksum(&mut self, value: u32) {
        self.checksum = value;
    }

    /// Records the storage label, truncating to the report boundary.
    pub fn set_storage(&mut self, value: &str) {
        self.storage_len = copy_str(value, &mut self.storage);
    }

    /// Records the sync reason, truncating to the report boundary.
    pub fn set_reason(&mut self, value: &str) {
        self.reason_len = copy_str(value, &mut self.reason);
    }

    /// Returns whether the sync path was attempted.
    #[must_use]
    pub const fn attempted(self) -> bool {
        self.attempted != 0
    }

    /// Returns whether persistent dump storage was available.
    #[must_use]
    pub const fn persistent_available(self) -> bool {
        self.persistent_available != 0
    }

    /// Returns whether bytes were written.
    #[must_use]
    pub const fn written(self) -> bool {
        self.written != 0
    }

    /// Returns whether read-back verification succeeded.
    #[must_use]
    pub const fn verified(self) -> bool {
        self.verified != 0
    }

    /// Returns the storage capacity in bytes.
    #[must_use]
    pub const fn storage_capacity_bytes(self) -> usize {
        self.storage_capacity_bytes
    }

    /// Returns the written byte count.
    #[must_use]
    pub const fn bytes_written(self) -> usize {
        self.bytes_written
    }

    /// Returns the verified artifact checksum.
    #[must_use]
    pub const fn checksum(self) -> u32 {
        self.checksum
    }

    /// Returns the storage label bytes.
    #[must_use]
    pub fn storage_bytes(&self) -> &[u8] {
        &self.storage[..self.storage_len]
    }

    /// Returns the sync reason bytes.
    #[must_use]
    pub fn reason_bytes(&self) -> &[u8] {
        &self.reason[..self.reason_len]
    }
}

/// Product-facing dump-control error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DumpError {
    /// This target or kernel build does not support the requested operation.
    Unsupported,
    /// The request arguments or state were invalid for the current operation.
    InvalidRequest,
    /// The operation requires a current process context and none exists.
    NoCurrentProcess,
    /// The requested operation is already in progress.
    Busy,
    /// The bridge reported an I/O failure.
    Io,
    /// The bridge reported a non-specific failure.
    Failed,
}

/// Diagnostic dump control backed by the raw Reovim syscall transport.
#[derive(Debug, Clone, Copy)]
pub struct SyscallDumpControl {
    raw: RawSyscall,
}

impl SyscallDumpControl {
    /// Creates a dump-control adapter over the raw syscall transport.
    #[must_use]
    pub const fn new(raw: RawSyscall) -> Self {
        Self { raw }
    }

    /// Requests a persistent dump sync and fills `report` with the attempt result.
    ///
    /// # Errors
    ///
    /// Returns [`DumpError`] when no current process context exists or the raw
    /// transport rejects the request. Storage-unavailable and write-failed
    /// results are reported in `report`; they are not transport failures.
    pub fn sync(self, report: &mut DumpSyncReport) -> Result<(), DumpError> {
        report.clear();
        self.raw
            .invoke(
                SyscallNr::DUMP_SYNC,
                SyscallArgs::new([
                    core::ptr::from_mut(report).addr(),
                    size_of::<DumpSyncReport>(),
                    0,
                    0,
                    0,
                    0,
                ]),
            )
            .decode()
            .map(|_| ())
            .map_err(dump_error_from_syscall)
    }
}

fn copy_str(value: &str, out: &mut [u8]) -> usize {
    let bytes = value.as_bytes();
    let len = bytes.len().min(out.len());
    out[..len].copy_from_slice(&bytes[..len]);
    len
}

const fn dump_error_from_syscall(error: SyscallError) -> DumpError {
    match error {
        SyscallError::UNSUPPORTED => DumpError::Unsupported,
        SyscallError::INVALID_ARGUMENT => DumpError::InvalidRequest,
        SyscallError::NO_CURRENT_PROCESS => DumpError::NoCurrentProcess,
        SyscallError::BUSY => DumpError::Busy,
        SyscallError::IO => DumpError::Io,
        _ => DumpError::Failed,
    }
}
