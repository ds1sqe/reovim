//! Root daemon public surface for OS-mode shell execution.
//!
//! This module owns the kernel-only supervisor and payload launch registry.
//! It has no dependency on any provider, product crate, editor, server, or
//! client implementation.

use {
    crate::{
        dump,
        exec_body::{self, ExecBodyInnerFormat},
        klog, proc,
        program::{
            self, MAX_PROGRAM_ARG_BYTES, MAX_PROGRAM_STDIN_BYTES, ProgramArgvBuffer,
            ProgramDescriptor, ProgramEnvBuffer, ProgramStatus,
        },
        root_shell::{
            ProgramInvocation, RootShellSession, ShellLineError, execute_loaded_program_argv,
            parse_program_invocation, split_pipeline,
        },
        sched, service,
        source_store::ExecutableSourceStore,
        splash, syscall, vfs,
    },
    core::{
        cell::UnsafeCell,
        slice,
        sync::atomic::{AtomicBool, Ordering},
    },
    reovim_uapi_system::{BootInfo, DeviceClass, DeviceEntry},
};

const OK: &[u8] = b"\x1b[32m[  OK  ]\x1b[0m ";
const WARN: &[u8] = b"\x1b[33m[ WARN ]\x1b[0m ";
const TITLE: &[u8] = b"\x1b[38;2;90;210;255mReovim OS\x1b[0m boot report";
/// Maximum number of payload descriptors a profile can install.
pub const MAX_PAYLOADS: usize = 8;
/// Maximum number of provider-discovered payload descriptors retained from checked media.
pub const MAX_MEDIA_PAYLOADS: usize = 4;
const MEDIA_PAYLOAD_INDEX_BASE: usize = MAX_PAYLOADS;
const MAX_MEDIA_PAYLOAD_NAME_BYTES: usize = 32;
pub(crate) const MAX_MEDIA_PAYLOAD_PATH_BYTES: usize = MAX_MEDIA_PAYLOAD_NAME_BYTES + 9;
const MAX_MEDIA_PAYLOAD_ENTRY_BYTES: usize = MAX_MEDIA_PAYLOAD_NAME_BYTES + 8;
const MEDIA_PAYLOAD_SUMMARY: &str = "provider-discovered /payload program";
const MAX_SHELL_PIPELINE_CONTINUATIONS: usize = proc::MAX_PROCESSES;

/// Callback that emits output bytes through the active console/stdout sink.
pub type WriteFn = fn(&[u8]);

/// Callback that returns optional extra diagnostics appended after kernel log.
pub type DmesgSnapshot = fn() -> &'static str;

/// Input callback for the interactive root shell line reader.
pub type ReadLine = fn(&mut [u8]) -> usize;

/// Callback to stop the OS / return control to firmware.
pub type HaltKernel = fn();

/// Callback that runs after splash rendering and before checked boot output.
pub type PrepareShell = fn();

/// Callback that runs a lower-provider hardware probe for a named target.
pub type HardwareProbe = fn(&str, &[DeviceEntry], WriteFn) -> HardwareProbeResult;

/// Callback that derives a live console-input summary from the boot-time base.
pub type ConsoleInputStatus = fn(ConsoleInputSummary) -> ConsoleInputSummary;

/// Callback that renders one image-owned VFS pseudo-file through program syscalls.
pub type VfsFileWriter = for<'daemon, 'session, 'rootd> fn(
    vfs::File,
    &mut syscall::ProgramSyscalls<'daemon, 'session, 'rootd>,
);

/// Callback that renders image-owned `/bin/help` output.
pub type ProgramHelpWriter = for<'program_name, 'daemon, 'session, 'rootd> fn(
    Option<&'program_name str>,
    &mut syscall::ProgramSyscalls<'daemon, 'session, 'rootd>,
) -> ProgramStatus;

/// Bounded input buffer used by root-shell line reads.
pub const ROOT_LINE_BYTES: usize = 128;

#[derive(Clone, Copy, Debug)]
struct ShellPipelineContinuation {
    producer_pid: usize,
    read_fd: usize,
    consumer_argv: ProgramArgvBuffer,
    consumer_env: ProgramEnvBuffer,
}

impl ShellPipelineContinuation {
    const fn empty() -> Self {
        Self {
            producer_pid: 0,
            read_fd: 0,
            consumer_argv: ProgramArgvBuffer::empty(),
            consumer_env: ProgramEnvBuffer::empty(),
        }
    }

    const fn is_empty(self) -> bool {
        self.producer_pid == 0
    }
}

struct ShellPipelineContinuationTable {
    slots: [ShellPipelineContinuation; MAX_SHELL_PIPELINE_CONTINUATIONS],
}

impl ShellPipelineContinuationTable {
    const fn new() -> Self {
        Self {
            slots: [ShellPipelineContinuation::empty(); MAX_SHELL_PIPELINE_CONTINUATIONS],
        }
    }

    fn reset(&mut self) {
        self.slots = [ShellPipelineContinuation::empty(); MAX_SHELL_PIPELINE_CONTINUATIONS];
    }

    fn slot_index(&self, producer_pid: usize) -> Option<usize> {
        if producer_pid == 0 {
            return None;
        }
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].producer_pid == producer_pid {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn reclaim_stale(&mut self) {
        let mut index = 0usize;
        while index < self.slots.len() {
            let slot = self.slots[index];
            if !slot.is_empty()
                && !matches!(
                    proc::process(slot.producer_pid),
                    Some(process)
                        if matches!(
                            process.state,
                            proc::ProcessState::Ready
                                | proc::ProcessState::Running
                                | proc::ProcessState::Blocked
                        )
                )
            {
                self.slots[index] = ShellPipelineContinuation::empty();
            }
            index += 1;
        }
    }

    fn store(
        &mut self,
        producer_pid: usize,
        read_fd: usize,
        consumer_argv: ProgramArgvBuffer,
        consumer_env: ProgramEnvBuffer,
    ) -> bool {
        if producer_pid == 0 || self.slot_index(producer_pid).is_some() {
            return false;
        }
        self.reclaim_stale();
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].is_empty() {
                self.slots[index] = ShellPipelineContinuation {
                    producer_pid,
                    read_fd,
                    consumer_argv,
                    consumer_env,
                };
                return true;
            }
            index += 1;
        }
        false
    }

    fn take(&mut self, producer_pid: usize) -> Option<ShellPipelineContinuation> {
        let index = self.slot_index(producer_pid)?;
        let continuation = self.slots[index];
        self.slots[index] = ShellPipelineContinuation::empty();
        Some(continuation)
    }
}

struct ShellPipelineContinuationCell(UnsafeCell<ShellPipelineContinuationTable>);

// SAFETY: mutable access is serialized by `SHELL_PIPELINE_CONTINUATION_LOCK`.
unsafe impl Sync for ShellPipelineContinuationCell {}

static SHELL_PIPELINE_CONTINUATIONS: ShellPipelineContinuationCell =
    ShellPipelineContinuationCell(UnsafeCell::new(ShellPipelineContinuationTable::new()));
static SHELL_PIPELINE_CONTINUATION_LOCK: AtomicBool = AtomicBool::new(false);

struct ShellPipelineContinuationGuard;

impl ShellPipelineContinuationGuard {
    fn acquire() -> Self {
        while SHELL_PIPELINE_CONTINUATION_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for ShellPipelineContinuationGuard {
    fn drop(&mut self) {
        SHELL_PIPELINE_CONTINUATION_LOCK.store(false, Ordering::Release);
    }
}

fn with_shell_pipeline_continuations<R>(
    f: impl FnOnce(&mut ShellPipelineContinuationTable) -> R,
) -> R {
    let _guard = ShellPipelineContinuationGuard::acquire();
    // SAFETY: `SHELL_PIPELINE_CONTINUATION_LOCK` serializes access.
    let continuations = unsafe { &mut *SHELL_PIPELINE_CONTINUATIONS.0.get() };
    f(continuations)
}

pub(crate) fn reset_shell_pipeline_continuations() {
    with_shell_pipeline_continuations(ShellPipelineContinuationTable::reset);
}

fn store_shell_pipeline_continuation(
    producer_pid: usize,
    read_fd: usize,
    consumer_argv: ProgramArgvBuffer,
    consumer_env: ProgramEnvBuffer,
) -> bool {
    with_shell_pipeline_continuations(|continuations| {
        continuations.store(producer_pid, read_fd, consumer_argv, consumer_env)
    })
}

fn take_shell_pipeline_continuation(producer_pid: usize) -> Option<ShellPipelineContinuation> {
    with_shell_pipeline_continuations(|continuations| continuations.take(producer_pid))
}

/// Result of a boot-time check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootCheckState {
    /// The check completed normally.
    Ok,
    /// The check completed, but the path is degraded or already initialized.
    Warn,
}

/// Result from a lower-provider hardware probe callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareProbeResult {
    /// The provider recognized the target and wrote its report.
    Handled,
    /// The provider did not recognize this target.
    UnknownTarget,
}

/// Runtime-service installation checks captured before root-daemon entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeChecks {
    /// Heap allocator backend installation status.
    pub alloc_backend: BootCheckState,
    /// Synchronization backend installation status.
    pub sync_backend: BootCheckState,
}

impl RuntimeChecks {
    /// Builds a runtime check summary from individual statuses.
    #[must_use]
    pub const fn new(alloc_backend: BootCheckState, sync_backend: BootCheckState) -> Self {
        Self {
            alloc_backend,
            sync_backend,
        }
    }
}

/// Compile-time image identity supplied by the composition root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootImageSummary {
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

impl BootImageSummary {
    /// Builds static image identity for root-shell diagnostics.
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
}

/// Summary of the console input path selected by the composition root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsoleInputSummary {
    /// Provider-visible source name, for example `pl011-uart`.
    pub source: &'static str,
    /// Whether the selected source is live/manual or a scripted harness.
    pub mode: &'static str,
    /// Readiness of the selected source.
    pub source_state: BootCheckState,
    /// Readiness of the physical USB keyboard provider.
    pub usb_keyboard: BootCheckState,
    /// Decoded USB keyboard bytes waiting for the line discipline.
    pub usb_keyboard_pending_bytes: usize,
    /// Whether the composition root enabled USB keyboard probing.
    pub usb_keyboard_probe_enabled: bool,
    /// Minimum interval between lower USB keyboard hardware polls.
    pub usb_keyboard_poll_interval_ms: usize,
    /// Last lower USB keyboard poll/probe state visible to the composition root.
    pub usb_keyboard_last_poll: &'static str,
}

impl ConsoleInputSummary {
    /// Builds a fixed console-input summary for boot reporting.
    #[must_use]
    pub const fn new(
        source: &'static str,
        mode: &'static str,
        source_state: BootCheckState,
        usb_keyboard: BootCheckState,
    ) -> Self {
        Self::with_usb_pending_bytes(source, mode, source_state, usb_keyboard, 0)
    }

    /// Builds a console-input summary with live USB queue state.
    #[must_use]
    pub const fn with_usb_pending_bytes(
        source: &'static str,
        mode: &'static str,
        source_state: BootCheckState,
        usb_keyboard: BootCheckState,
        usb_keyboard_pending_bytes: usize,
    ) -> Self {
        Self::with_usb_state(
            source,
            mode,
            source_state,
            usb_keyboard,
            usb_keyboard_pending_bytes,
            false,
            0,
        )
    }

    /// Builds a console-input summary with live USB provider state.
    #[must_use]
    pub const fn with_usb_state(
        source: &'static str,
        mode: &'static str,
        source_state: BootCheckState,
        usb_keyboard: BootCheckState,
        usb_keyboard_pending_bytes: usize,
        usb_keyboard_probe_enabled: bool,
        usb_keyboard_poll_interval_ms: usize,
    ) -> Self {
        Self::with_usb_diagnostics(
            source,
            mode,
            source_state,
            usb_keyboard,
            usb_keyboard_pending_bytes,
            usb_keyboard_probe_enabled,
            usb_keyboard_poll_interval_ms,
            "not-polled",
        )
    }

    /// Builds a console-input summary with full live USB provider diagnostics.
    #[must_use]
    pub const fn with_usb_diagnostics(
        source: &'static str,
        mode: &'static str,
        source_state: BootCheckState,
        usb_keyboard: BootCheckState,
        usb_keyboard_pending_bytes: usize,
        usb_keyboard_probe_enabled: bool,
        usb_keyboard_poll_interval_ms: usize,
        usb_keyboard_last_poll: &'static str,
    ) -> Self {
        Self {
            source,
            mode,
            source_state,
            usb_keyboard,
            usb_keyboard_pending_bytes,
            usb_keyboard_probe_enabled,
            usb_keyboard_poll_interval_ms,
            usb_keyboard_last_poll,
        }
    }
}

/// Boot-time launch configuration for the root daemon shell.
pub struct RootBootConfig<'a> {
    /// Normalized boot summary for `boot` output and diagnostics.
    pub boot_info: BootInfo,
    /// Snapshot of discovered devices for inventory output.
    pub devices: &'a [DeviceEntry],
    /// Registered payload descriptors for `launch`.
    pub payloads: &'a [PayloadDescriptor],
    /// Payload source artifacts available to the executable loader.
    pub payload_sources: &'static [PayloadSourceArtifact],
    /// Registered `/bin` program descriptors for shell exec.
    pub programs: &'static [ProgramDescriptor],
    /// `/bin` source artifacts available to the executable loader.
    pub program_sources: &'static [crate::program::ProgramSourceArtifact],
    /// Image-owned VFS pseudo-file renderer.
    pub vfs_file_writer: VfsFileWriter,
    /// Image-owned `/bin/help` renderer.
    pub program_help_writer: ProgramHelpWriter,
    /// Optional extra diagnostics output.
    pub dmesg: Option<DmesgSnapshot>,
    /// Optional callback to stop firmware execution.
    pub halt: Option<HaltKernel>,
    /// Optional profile hook after splash rendering and before checked boot log.
    pub prepare_shell: Option<PrepareShell>,
    /// Optional lower-provider hardware probe callback for shell diagnostics.
    pub probe_hardware: Option<HardwareProbe>,
    /// Optional live console-input status callback for `/boot/profile`.
    pub console_input_status: Option<ConsoleInputStatus>,
    /// Line reader callback.
    pub read_line: ReadLine,
    /// Prompt bytes emitted before each input attempt.
    pub prompt: &'static str,
    /// Output bytes emitted to the active sink.
    pub write: WriteFn,
    /// Optional framebuffer/terminal geometry for splash rendering.
    pub splash_geometry: Option<(u32, u32)>,
    /// Compile-time image identity for `/boot/image`.
    pub boot_image: BootImageSummary,
    /// Root profile and launch policy.
    pub profile: ProfileSummary,
    /// Runtime-service installation checks.
    pub runtime_checks: RuntimeChecks,
    /// Selected console input source and USB-keyboard readiness.
    pub console_input: ConsoleInputSummary,
}

/// Descriptor for a launchable payload available to the root daemon.
#[derive(Clone, Copy, Debug)]
pub struct PayloadDescriptor {
    /// Canonical payload name, e.g. `editor-smoke` or `server-smoke`.
    pub name: &'static str,
    /// Process-visible payload path.
    pub path: &'static str,
    /// One-line payload description shown by `launch`.
    pub summary: &'static str,
    /// Stable name for the payload entry point.
    pub entry_name: &'static str,
    /// Executable payload body.
    pub image: PayloadImage,
}

impl PayloadDescriptor {
    /// Source kind for this payload image.
    #[must_use]
    pub const fn image_kind(&self) -> PayloadImageKind {
        self.image.kind()
    }
}

/// Failure while installing a media-discovered payload descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaPayloadInstallError {
    /// The media path is not a single-component `/payload/<name>` path.
    InvalidPath,
    /// The payload basename does not fit the bounded descriptor table.
    NameTooLong,
    /// All bounded media descriptor slots are occupied.
    NoSlot,
}

#[derive(Clone, Copy)]
struct MediaPayloadSlot {
    name: [u8; MAX_MEDIA_PAYLOAD_NAME_BYTES],
    name_len: usize,
    path: [u8; MAX_MEDIA_PAYLOAD_PATH_BYTES],
    path_len: usize,
    entry_name: [u8; MAX_MEDIA_PAYLOAD_ENTRY_BYTES],
    entry_name_len: usize,
    descriptor: PayloadDescriptor,
    occupied: bool,
}

impl MediaPayloadSlot {
    const fn empty() -> Self {
        Self {
            name: [0u8; MAX_MEDIA_PAYLOAD_NAME_BYTES],
            name_len: 0,
            path: [0u8; MAX_MEDIA_PAYLOAD_PATH_BYTES],
            path_len: 0,
            entry_name: [0u8; MAX_MEDIA_PAYLOAD_ENTRY_BYTES],
            entry_name_len: 0,
            descriptor: PayloadDescriptor {
                name: "",
                path: "",
                summary: "",
                entry_name: "",
                image: PayloadImage::SourcePath(""),
            },
            occupied: false,
        }
    }

    fn clear(&mut self) {
        *self = Self::empty();
    }

    fn write(&mut self, path: &str) -> Result<(), MediaPayloadInstallError> {
        let name = media_payload_name_from_path(path)?;
        if name.len() > self.name.len() || path.len() > self.path.len() {
            return Err(MediaPayloadInstallError::NameTooLong);
        }

        self.name_len = name.len();
        self.name[..self.name_len].copy_from_slice(name.as_bytes());
        self.path_len = path.len();
        self.path[..self.path_len].copy_from_slice(path.as_bytes());
        self.entry_name_len = write_media_payload_entry_name(name, &mut self.entry_name)?;

        let name = media_payload_slot_str(self.name.as_ptr(), self.name_len);
        let path = media_payload_slot_str(self.path.as_ptr(), self.path_len);
        let entry_name = media_payload_slot_str(self.entry_name.as_ptr(), self.entry_name_len);
        self.descriptor = PayloadDescriptor {
            name,
            path,
            summary: MEDIA_PAYLOAD_SUMMARY,
            entry_name,
            image: PayloadImage::SourcePath(path),
        };
        self.occupied = true;
        Ok(())
    }
}

struct MediaPayloadTable {
    slots: [MediaPayloadSlot; MAX_MEDIA_PAYLOADS],
}

impl MediaPayloadTable {
    const fn new() -> Self {
        Self {
            slots: [MediaPayloadSlot::empty(); MAX_MEDIA_PAYLOADS],
        }
    }

    fn reset(&mut self) {
        let mut index = 0usize;
        while index < self.slots.len() {
            self.slots[index].clear();
            index += 1;
        }
    }

    fn find_path(&self, path: &str) -> Option<usize> {
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].occupied && self.slots[index].descriptor.path == path {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn find_name(&self, name: &str) -> Option<usize> {
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].occupied && self.slots[index].descriptor.name == name {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn install(&mut self, path: &str) -> Result<usize, MediaPayloadInstallError> {
        if let Some(index) = self.find_path(path) {
            return Ok(index);
        }
        let mut index = 0usize;
        while index < self.slots.len() {
            if !self.slots[index].occupied {
                self.slots[index].write(path)?;
                return Ok(index);
            }
            index += 1;
        }
        Err(MediaPayloadInstallError::NoSlot)
    }
}

struct MediaPayloadCell(UnsafeCell<MediaPayloadTable>);

// SAFETY: mutable access is serialized by `MEDIA_PAYLOAD_LOCK`.
unsafe impl Sync for MediaPayloadCell {}

static MEDIA_PAYLOADS: MediaPayloadCell =
    MediaPayloadCell(UnsafeCell::new(MediaPayloadTable::new()));
static MEDIA_PAYLOAD_LOCK: AtomicBool = AtomicBool::new(false);

struct MediaPayloadGuard;

impl MediaPayloadGuard {
    fn acquire() -> Self {
        while MEDIA_PAYLOAD_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for MediaPayloadGuard {
    fn drop(&mut self) {
        MEDIA_PAYLOAD_LOCK.store(false, Ordering::Release);
    }
}

fn with_media_payloads<R>(f: impl FnOnce(&mut MediaPayloadTable) -> R) -> R {
    let _guard = MediaPayloadGuard::acquire();
    // SAFETY: `MEDIA_PAYLOAD_LOCK` serializes access to the media descriptor table.
    let table = unsafe { &mut *MEDIA_PAYLOADS.0.get() };
    f(table)
}

fn media_payload_descriptor(index: usize) -> &'static PayloadDescriptor {
    // SAFETY: media descriptor slots live for the program lifetime. Tests reset
    // between isolated cases; loaded payloads must not outlive a reset.
    unsafe { &(*MEDIA_PAYLOADS.0.get()).slots[index].descriptor }
}

fn media_payload_slot_str(ptr: *const u8, len: usize) -> &'static str {
    // SAFETY: callers only pass bytes previously copied from Rust `str` values
    // or ASCII-generated entry names into static media descriptor storage.
    unsafe { core::str::from_utf8_unchecked(slice::from_raw_parts(ptr, len)) }
}

fn media_payload_name_from_path(path: &str) -> Result<&str, MediaPayloadInstallError> {
    let Some(name) = path.strip_prefix("/payload/") else {
        return Err(MediaPayloadInstallError::InvalidPath);
    };
    if name.is_empty() || name.as_bytes().contains(&b'/') {
        return Err(MediaPayloadInstallError::InvalidPath);
    }
    let mut index = 0usize;
    let bytes = name.as_bytes();
    while index < bytes.len() {
        let byte = bytes[index];
        if !(byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_') {
            return Err(MediaPayloadInstallError::InvalidPath);
        }
        index += 1;
    }
    Ok(name)
}

fn write_media_payload_entry_name(
    name: &str,
    out: &mut [u8; MAX_MEDIA_PAYLOAD_ENTRY_BYTES],
) -> Result<usize, MediaPayloadInstallError> {
    if name.len() + 8 > out.len() {
        return Err(MediaPayloadInstallError::NameTooLong);
    }
    out[0] = b'p';
    out[1] = b'a';
    out[2] = b'y';
    out[3] = b'l';
    out[4] = b'o';
    out[5] = b'a';
    out[6] = b'd';
    out[7] = b'_';
    let mut index = 0usize;
    while index < name.len() {
        let byte = name.as_bytes()[index];
        out[index + 8] = if byte == b'-' { b'_' } else { byte };
        index += 1;
    }
    Ok(name.len() + 8)
}

/// Clears media-discovered payload descriptors.
pub fn reset_media_payloads() {
    with_media_payloads(MediaPayloadTable::reset);
}

/// Builds a checked `/payload/<name>` media path from a payload launch token.
pub fn media_payload_path_from_name<'a>(
    name: &str,
    out: &'a mut [u8; MAX_MEDIA_PAYLOAD_PATH_BYTES],
) -> Option<&'a str> {
    let len = if name.as_bytes().first() == Some(&b'/') {
        if name.len() > out.len() {
            return None;
        }
        out[..name.len()].copy_from_slice(name.as_bytes());
        name.len()
    } else {
        let prefix = b"/payload/";
        if prefix.len() + name.len() > out.len() {
            return None;
        }
        out[..prefix.len()].copy_from_slice(prefix);
        out[prefix.len()..prefix.len() + name.len()].copy_from_slice(name.as_bytes());
        prefix.len() + name.len()
    };
    // SAFETY: bytes were copied from Rust `str` values plus the ASCII `/payload/` prefix.
    let path = unsafe { core::str::from_utf8_unchecked(&out[..len]) };
    if media_payload_name_from_path(path).is_err() {
        return None;
    }
    Some(path)
}

/// Installs or returns a media-discovered payload descriptor.
pub fn install_media_payload(
    path: &str,
) -> Result<(usize, &'static PayloadDescriptor), MediaPayloadInstallError> {
    let index = with_media_payloads(|payloads| payloads.install(path))?;
    Ok((MEDIA_PAYLOAD_INDEX_BASE + index, media_payload_descriptor(index)))
}

/// Snapshots currently installed media-discovered `/payload` descriptors.
///
/// Profile descriptors remain supplied by the active boot profile. This helper
/// exposes the bounded runtime descriptor extension so `launch` and later
/// catalog surfaces do not hide provider-admitted payloads.
pub fn snapshot_media_payloads(out: &mut [Option<&'static PayloadDescriptor>]) -> usize {
    let count = with_media_payloads(|payloads| {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < payloads.slots.len() && written < out.len() {
            if payloads.slots[index].occupied {
                out[written] = Some(media_payload_descriptor(index));
                written += 1;
            }
            index += 1;
        }
        written
    });
    let mut index = count;
    while index < out.len() {
        out[index] = None;
        index += 1;
    }
    count
}

fn find_media_payload_by_name(name: &str) -> Option<(usize, &'static PayloadDescriptor)> {
    let index = with_media_payloads(|payloads| payloads.find_name(name))?;
    Some((MEDIA_PAYLOAD_INDEX_BASE + index, media_payload_descriptor(index)))
}

fn find_media_payload_by_path(path: &str) -> Option<(usize, &'static PayloadDescriptor)> {
    let index = with_media_payloads(|payloads| payloads.find_path(path))?;
    Some((MEDIA_PAYLOAD_INDEX_BASE + index, media_payload_descriptor(index)))
}

/// Finds a launch-profile or media-discovered payload descriptor by name/path.
#[must_use]
pub fn find_payload_by_name<'a>(
    payloads: &'a [PayloadDescriptor],
    name: &str,
) -> Option<(usize, &'a PayloadDescriptor)> {
    let mut index = 0usize;
    while index < payloads.len() {
        if payloads[index].name == name || payloads[index].path == name {
            return Some((index, &payloads[index]));
        }
        index += 1;
    }
    if name.as_bytes().first() == Some(&b'/') {
        find_media_payload_by_path(name)
    } else {
        find_media_payload_by_name(name)
    }
}

/// Source kind for a launchable payload image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayloadImageKind {
    /// Payload body is interpreted from a bounded Reovim source image.
    SourceImage,
    /// Payload body is a checked Reovim executable wrapper.
    ReovimExecBody,
}

impl PayloadImageKind {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceImage => "source-image",
            Self::ReovimExecBody => "reovim-exec-body",
        }
    }
}

/// Executable body for a launchable payload.
#[derive(Clone, Copy, Debug)]
pub enum PayloadImage {
    /// Payload source bytes are loaded from a source store path.
    SourcePath(&'static str),
}

impl PayloadImage {
    /// Source kind for this payload body.
    #[must_use]
    pub const fn kind(self) -> PayloadImageKind {
        match self {
            Self::SourcePath(_) => PayloadImageKind::SourceImage,
        }
    }

    /// Source-store path for this payload body.
    #[must_use]
    pub const fn source_path(self) -> &'static str {
        match self {
            Self::SourcePath(path) => path,
        }
    }
}

/// Payload source artifact available to the executable loader.
#[derive(Clone, Copy, Debug)]
pub struct PayloadSourceArtifact {
    /// Loader-visible artifact path.
    pub path: &'static str,
    /// Encoded source kind.
    pub kind: PayloadImageKind,
    /// Encoded payload source bytes.
    pub bytes: &'static [u8],
}

/// Source-store failure while loading a resolved payload descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayloadLoadError {
    /// Descriptor source path was not present in the source store.
    SourceNotFound,
    /// Source bytes failed loader validation.
    InvalidImage,
}

/// Loaded payload object retained while waiting for scheduler dispatch.
#[derive(Clone, Copy, Debug)]
pub struct LoadedPayloadProgram {
    /// Index in the root profile payload catalog.
    pub catalog_index: usize,
    /// Canonical payload name.
    pub name: &'static str,
    /// Process-visible payload path.
    pub path: &'static str,
    /// One-line payload description.
    pub summary: &'static str,
    /// Stable name for the payload entry point.
    pub entry_name: &'static str,
    /// Source kind for this payload image.
    pub image_kind: PayloadImageKind,
    /// Loader-visible artifact path that supplied the executable bytes.
    pub source_path: &'static str,
    source_bytes: &'static [u8],
}

impl LoadedPayloadProgram {
    pub(crate) fn from_descriptor(
        catalog_index: usize,
        descriptor: &PayloadDescriptor,
        source_store: ExecutableSourceStore,
    ) -> Result<Self, PayloadLoadError> {
        let source_path = descriptor.image.source_path();
        let Some(source) = source_store.find_payload(source_path) else {
            return Err(PayloadLoadError::SourceNotFound);
        };
        if source.kind != descriptor.image_kind() || !validate_payload_source_image(source.bytes) {
            return Err(PayloadLoadError::InvalidImage);
        }
        Ok(Self {
            catalog_index,
            name: descriptor.name,
            path: descriptor.path,
            summary: descriptor.summary,
            entry_name: descriptor.entry_name,
            image_kind: descriptor.image_kind(),
            source_path,
            source_bytes: source.bytes,
        })
    }

    pub(crate) fn from_descriptor_source_bytes(
        catalog_index: usize,
        descriptor: &PayloadDescriptor,
        source_bytes: &'static [u8],
    ) -> Result<Self, PayloadLoadError> {
        if descriptor.image_kind() != PayloadImageKind::SourceImage
            || !validate_payload_source_image(source_bytes)
        {
            return Err(PayloadLoadError::InvalidImage);
        }
        Ok(Self {
            catalog_index,
            name: descriptor.name,
            path: descriptor.path,
            summary: descriptor.summary,
            entry_name: descriptor.entry_name,
            image_kind: descriptor.image_kind(),
            source_path: descriptor.image.source_path(),
            source_bytes,
        })
    }

    pub(crate) fn from_descriptor_exec_body_bytes(
        catalog_index: usize,
        descriptor: &PayloadDescriptor,
        exec_body_bytes: &'static [u8],
    ) -> Result<Self, PayloadLoadError> {
        let Ok(body) = exec_body::parse_exec_body(exec_body_bytes) else {
            return Err(PayloadLoadError::InvalidImage);
        };
        if descriptor.image_kind() != PayloadImageKind::SourceImage
            || body.inner != ExecBodyInnerFormat::PayloadSourceImage
            || !validate_payload_source_image(body.bytes)
        {
            return Err(PayloadLoadError::InvalidImage);
        }
        Ok(Self {
            catalog_index,
            name: descriptor.name,
            path: descriptor.path,
            summary: descriptor.summary,
            entry_name: descriptor.entry_name,
            image_kind: PayloadImageKind::ReovimExecBody,
            source_path: descriptor.image.source_path(),
            source_bytes: exec_body_bytes,
        })
    }

    pub(crate) const fn source_bytes(&self) -> &'static [u8] {
        self.source_bytes
    }
}

/// Result from executing a payload source image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayloadLaunchResult {
    /// Launch path completed and payload is ready.
    Ready,
    /// Payload published readiness and remained resident as a blocked service process.
    Resident,
    /// Payload launch is not configured for the current profile or catalog.
    NotConfigured,
    /// Payload source execution failed.
    Failed,
    /// Payload returned a numeric Reovim exit code.
    ExitCode(u8),
}

impl PayloadLaunchResult {
    /// Stable text used in retained status rows.
    #[must_use]
    pub const fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Ready => b"payload.ready",
            Self::Resident => b"payload.resident",
            Self::NotConfigured => b"payload.not_configured",
            Self::Failed => b"payload.failed",
            Self::ExitCode(_) => b"payload.exit_code",
        }
    }

    /// Numeric process exit code represented by this payload result.
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Ready | Self::Resident => 0,
            Self::NotConfigured | Self::Failed => 1,
            Self::ExitCode(code) => code as i32,
        }
    }

    /// Whether this result counts as successful completion.
    #[must_use]
    pub const fn is_success(self) -> bool {
        match self {
            Self::Ready | Self::Resident => true,
            Self::NotConfigured | Self::Failed => false,
            Self::ExitCode(code) => code == 0,
        }
    }
}

impl core::fmt::Display for PayloadLaunchResult {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ready => out.write_str("payload.ready"),
            Self::Resident => out.write_str("payload.resident"),
            Self::NotConfigured => out.write_str("payload.not_configured"),
            Self::Failed => out.write_str("payload.failed"),
            Self::ExitCode(code) => {
                out.write_str("payload.exit_code(")?;
                core::fmt::Display::fmt(code, out)?;
                out.write_str(")")
            }
        }
    }
}

/// Installable payload-source exit status accepted by program syscalls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayloadSourceInstallStatus {
    /// Install a payload source that reports ready.
    Ready,
    /// Install a payload source that reports failed.
    Failed,
}

impl PayloadSourceInstallStatus {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    /// Encoded payload source image for this status.
    #[must_use]
    pub const fn source_bytes(self) -> &'static [u8] {
        match self {
            Self::Ready => PAYLOAD_SOURCE_READY_IMAGE,
            Self::Failed => PAYLOAD_SOURCE_FAILED_IMAGE,
        }
    }
}

const PAYLOAD_SOURCE_MAGIC: &[u8] = b"reovim-payload-source-v1";
const PAYLOAD_SOURCE_OP_EXIT_STATUS: &[u8] = b"exit-status ";
const PAYLOAD_SOURCE_OP_EXIT_CODE: &[u8] = b"exit-code ";
const PAYLOAD_SOURCE_OP_SERVICE_READY: &[u8] = b"service-ready ";
const PAYLOAD_SOURCE_OP_SERVICE_HOLD: &[u8] = b"service-hold";
const PAYLOAD_SOURCE_OP_EXEC_PAYLOAD: &[u8] = b"exec-payload ";
const PAYLOAD_SOURCE_OP_SPAWN_PAYLOAD: &[u8] = b"spawn-payload ";
const PAYLOAD_SOURCE_OP_SPAWN_WAIT_PAYLOAD: &[u8] = b"spawn-wait-payload ";
const PAYLOAD_SOURCE_OP_SPAWN_KILL_PAYLOAD: &[u8] = b"spawn-kill-payload ";
const PAYLOAD_SOURCE_OP_EXEC_BIN_STDIN_HEX: &[u8] = b"exec-bin-stdin-hex ";
const PAYLOAD_SOURCE_OP_EXEC_BIN: &[u8] = b"exec-bin ";
const PAYLOAD_SOURCE_OP_PIPE_BIN: &[u8] = b"pipe-bin ";
const PAYLOAD_SOURCE_OP_SPAWN_WAIT_BIN: &[u8] = b"spawn-wait-bin ";
const PAYLOAD_SOURCE_OP_SPAWN_KILL_BIN: &[u8] = b"spawn-kill-bin ";
const PAYLOAD_SOURCE_OP_SPAWN_BIN: &[u8] = b"spawn-bin ";
const PAYLOAD_SOURCE_OP_SLEEP_WAIT_BIN: &[u8] = b"sleep-wait-bin ";
const PAYLOAD_SOURCE_OP_SLEEP_BIN: &[u8] = b"sleep-bin ";
const PAYLOAD_SOURCE_OP_SCHEDULER_TICK: &[u8] = b"scheduler-tick";
const PAYLOAD_SOURCE_OP_YIELD_NOW: &[u8] = b"yield-now";
const PAYLOAD_SOURCE_OP_READ_TTY_LINE: &[u8] = b"read-tty-line";
const PAYLOAD_SOURCE_OP_WRITE_PROCESS_SELF: &[u8] = b"write-process-self";
const PAYLOAD_SOURCE_OP_WRITE_PROCESS_TABLE: &[u8] = b"write-process-table";
const PAYLOAD_SOURCE_OP_WRITE_SERVICE_TABLE: &[u8] = b"write-service-table";
const PAYLOAD_SOURCE_OP_WRITE_BOOT_PROFILE: &[u8] = b"write-boot-profile";
const PAYLOAD_SOURCE_OP_WRITE_DEVICE_TABLE: &[u8] = b"write-device-table";
const PAYLOAD_SOURCE_OP_WRITE_EXEC_TABLE: &[u8] = b"write-exec-table";
const PAYLOAD_SOURCE_OP_WRITE_PENDING_EXEC_TABLE: &[u8] = b"write-pending-exec-table";
const PAYLOAD_SOURCE_OP_WRITE_SOURCE_TABLE: &[u8] = b"write-source-table";
const PAYLOAD_SOURCE_OP_WRITE_SCHEDULER_STATE: &[u8] = b"write-scheduler-state";
const PAYLOAD_SOURCE_OP_WRITE_TASK_TABLE: &[u8] = b"write-task-table";
const PAYLOAD_SOURCE_OP_WRITE_WAIT_TABLE: &[u8] = b"write-wait-table";
const PAYLOAD_SOURCE_OP_WRITE_SYSCALL_TABLE: &[u8] = b"write-syscall-table";
const PAYLOAD_SOURCE_OP_WRITE_STDOUT_HEX: &[u8] = b"write-stdout-hex ";
const PAYLOAD_SOURCE_OP_WRITE_STDERR_HEX: &[u8] = b"write-stderr-hex ";
const PAYLOAD_SOURCE_OP_WRITE_VFS_FILE: &[u8] = b"write-vfs-file ";
const PAYLOAD_SOURCE_MAX_OPS: usize = 20;
const PAYLOAD_SOURCE_READY_IMAGE: &[u8] = b"reovim-payload-source-v1\nexit-status ready\n";
const PAYLOAD_SOURCE_FAILED_IMAGE: &[u8] = b"reovim-payload-source-v1\nexit-status failed\n";

/// One parsed operation from a payload source image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PayloadSourceOp {
    /// Empty line, accepted as a no-op for hand-edited payload images.
    Noop,
    /// Return a payload-level status.
    ExitStatus(PayloadLaunchResult),
    /// Return a numeric payload exit code.
    ExitCode(u8),
    /// Publish the current payload process as a running service.
    ServiceReady(&'static str),
    /// Block the current payload process as a resident service.
    ServiceHold,
    /// Launch a payload child through the current payload process context and wait.
    ExecPayload(ProgramArgvBuffer),
    /// Spawn a payload child from the current payload process without waiting.
    SpawnPayload(ProgramArgvBuffer),
    /// Spawn a payload child from the current payload process and wait by PID.
    SpawnWaitPayload(ProgramArgvBuffer),
    /// Spawn a payload child from the current payload process and kill it by PID.
    SpawnKillPayload(ProgramArgvBuffer),
    /// Execute a `/bin` program through the current payload process context.
    ExecBin(ProgramArgvBuffer),
    /// Execute a `/bin` program with decoded stdin through the current payload process context.
    ExecBinStdinHex {
        /// Hex-encoded stdin bytes admitted with the payload source image.
        stdin_hex: &'static [u8],
        /// Child `/bin` argv.
        argv: ProgramArgvBuffer,
    },
    /// Execute one `/bin` child, capture bounded stdout, and feed it to another.
    PipeBin {
        /// Producer child `/bin` argv.
        producer: ProgramArgvBuffer,
        /// Consumer child `/bin` argv.
        consumer: ProgramArgvBuffer,
    },
    /// Spawn a `/bin` program from the current payload process without waiting.
    SpawnBin(ProgramArgvBuffer),
    /// Spawn a `/bin` program from the current payload process and wait by PID.
    SpawnWaitBin(ProgramArgvBuffer),
    /// Spawn a `/bin` program from the current payload process and kill it by PID.
    SpawnKillBin(ProgramArgvBuffer),
    /// Spawn a `/bin` program and block it until a scheduler tick deadline.
    SleepBin {
        /// Number of explicit scheduler ticks before the child wakes.
        ticks: usize,
        /// Child `/bin` argv.
        argv: ProgramArgvBuffer,
    },
    /// Spawn a sleeping `/bin` program and wait for it with a tick deadline.
    SleepWaitBin {
        /// Number of explicit scheduler ticks before the child wakes and wait expires.
        ticks: usize,
        /// Child `/bin` argv.
        argv: ProgramArgvBuffer,
    },
    /// Record one scheduler tick against the current payload process.
    SchedulerTick,
    /// Cooperatively yield the current payload process through the scheduler.
    YieldNow,
    /// Read one interactive TTY line and write it through payload stdout.
    ReadTtyLine,
    /// Write the current payload process record through the typed process ABI.
    WriteProcessSelf,
    /// Write the retained process table by reading `/proc/processes`.
    WriteProcessTable,
    /// Write the retained service table by reading `/proc/services`.
    WriteServiceTable,
    /// Write selected boot-profile facts through the typed boot/profile ABI.
    WriteBootProfile,
    /// Write retained boot device inventory through the typed device ABI.
    WriteDeviceTable,
    /// Write retained executable load/admission rows by reading `/proc/execs`.
    WriteExecTable,
    /// Write retained pending executable rows by reading `/proc/pending`.
    WritePendingExecTable,
    /// Write executable source artifact rows by reading `/proc/sources`.
    WriteSourceTable,
    /// Write scheduler run-queue state by reading `/proc/scheduler`.
    WriteSchedulerState,
    /// Write retained scheduler task rows by reading `/proc/tasks`.
    WriteTaskTable,
    /// Write retained process wait rows by reading `/proc/waits`.
    WriteWaitTable,
    /// Write retained syscall trace rows by reading `/proc/syscalls`.
    WriteSyscallTable,
    /// Write decoded bytes through the current payload process stdout.
    WriteStdoutHex(&'static [u8]),
    /// Write decoded bytes through the current payload process stderr.
    WriteStderrHex(&'static [u8]),
    /// Stream a kernel VFS pseudo-file through the payload process fd path.
    WriteVfsFile(&'static str),
}

/// Result of reading one payload source-image operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PayloadSourceStep {
    /// No more source-image operations.
    End,
    /// The next source line was malformed.
    Invalid,
    /// Parsed one operation and advanced to the next byte offset.
    Op {
        /// Parsed source operation.
        op: PayloadSourceOp,
        /// Offset for the next call.
        next: usize,
    },
}

fn next_payload_source_line(bytes: &'static [u8], offset: usize) -> Option<(&'static [u8], usize)> {
    if offset >= bytes.len() {
        return None;
    }

    let mut end = offset;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }

    let mut next = end;
    if next < bytes.len() && bytes[next] == b'\n' {
        next += 1;
    }

    let mut line = &bytes[offset..end];
    if line.last() == Some(&b'\r') {
        line = &line[..line.len() - 1];
    }

    Some((line, next))
}

fn parse_payload_source_status(value: &[u8]) -> Option<PayloadLaunchResult> {
    match value {
        b"ready" => Some(PayloadLaunchResult::Ready),
        b"not-configured" => Some(PayloadLaunchResult::NotConfigured),
        b"failed" => Some(PayloadLaunchResult::Failed),
        _ => None,
    }
}

fn parse_payload_source_exit_code(value: &[u8]) -> Option<u8> {
    let source = core::str::from_utf8(value).ok()?;
    let code = parse_payload_source_usize(source)?;
    if code > u8::MAX as usize {
        return None;
    }
    Some(code as u8)
}

fn parse_payload_source_argv(value: &[u8]) -> Option<ProgramArgvBuffer> {
    let source = core::str::from_utf8(value).ok()?;
    if source.is_empty() || source.starts_with(' ') || source.ends_with(' ') {
        return None;
    }

    let mut argv = ProgramArgvBuffer::empty();
    for arg in source.split(' ') {
        if arg.is_empty() || argv.push(arg).is_err() {
            return None;
        }
    }

    if argv.argc() == 0 { None } else { Some(argv) }
}

fn parse_payload_source_service_name(value: &'static [u8]) -> Option<&'static str> {
    let name = core::str::from_utf8(value).ok()?;
    if name.is_empty() || name.len() > MAX_PROGRAM_ARG_BYTES {
        return None;
    }

    for byte in name.bytes() {
        if !(byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b'.') {
            return None;
        }
    }

    Some(name)
}

fn parse_payload_source_exec_bin_stdin_hex(value: &'static [u8]) -> Option<PayloadSourceOp> {
    let split = value.iter().position(|byte| *byte == b' ')?;
    let stdin_hex = &value[..split];
    let argv_text = &value[split + 1..];
    if stdin_hex.is_empty()
        || argv_text.is_empty()
        || stdin_hex.len() > MAX_PROGRAM_STDIN_BYTES * 2
        || !payload_source_hex_is_valid(stdin_hex)
    {
        return None;
    }

    parse_payload_source_argv(argv_text)
        .map(|argv| PayloadSourceOp::ExecBinStdinHex { stdin_hex, argv })
}

fn parse_payload_source_pipe_bin(value: &[u8]) -> Option<PayloadSourceOp> {
    let source = core::str::from_utf8(value).ok()?;
    let (producer_text, consumer_text) = source.split_once(" -- ")?;
    if producer_text.is_empty()
        || consumer_text.is_empty()
        || producer_text.starts_with(' ')
        || producer_text.ends_with(' ')
        || consumer_text.starts_with(' ')
        || consumer_text.ends_with(' ')
    {
        return None;
    }

    let producer = parse_payload_source_argv(producer_text.as_bytes())?;
    let consumer = parse_payload_source_argv(consumer_text.as_bytes())?;
    Some(PayloadSourceOp::PipeBin { producer, consumer })
}

fn parse_payload_source_usize(value: &str) -> Option<usize> {
    let mut parsed = 0usize;
    if value.is_empty() {
        return None;
    }

    for byte in value.bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        parsed = parsed
            .checked_mul(10)?
            .checked_add((byte - b'0') as usize)?;
    }

    Some(parsed)
}

fn parse_payload_source_sleep_bin(value: &[u8]) -> Option<PayloadSourceOp> {
    let source = core::str::from_utf8(value).ok()?;
    let (ticks_text, argv_text) = source.split_once(' ')?;
    if argv_text.is_empty() || argv_text.starts_with(' ') || argv_text.ends_with(' ') {
        return None;
    }

    let ticks = parse_payload_source_usize(ticks_text)?;
    if ticks == 0 {
        return None;
    }

    parse_payload_source_argv(argv_text.as_bytes())
        .map(|argv| PayloadSourceOp::SleepBin { ticks, argv })
}

fn parse_payload_source_sleep_wait_bin(value: &[u8]) -> Option<PayloadSourceOp> {
    let source = core::str::from_utf8(value).ok()?;
    let (ticks_text, argv_text) = source.split_once(' ')?;
    if argv_text.is_empty() || argv_text.starts_with(' ') || argv_text.ends_with(' ') {
        return None;
    }

    let ticks = parse_payload_source_usize(ticks_text)?;
    if ticks == 0 {
        return None;
    }

    parse_payload_source_argv(argv_text.as_bytes())
        .map(|argv| PayloadSourceOp::SleepWaitBin { ticks, argv })
}

fn parse_payload_source_vfs_path(value: &'static [u8]) -> Option<&'static str> {
    let path = core::str::from_utf8(value).ok()?;
    if path.is_empty() || path.as_bytes().contains(&0) {
        None
    } else {
        Some(path)
    }
}

fn parse_payload_source_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn payload_source_hex_is_valid(encoded: &[u8]) -> bool {
    if encoded.len() % 2 != 0 {
        return false;
    }

    let mut index = 0usize;
    while index < encoded.len() {
        if parse_payload_source_hex_digit(encoded[index]).is_none()
            || parse_payload_source_hex_digit(encoded[index + 1]).is_none()
        {
            return false;
        }
        index += 2;
    }

    true
}

fn parse_payload_source_op(line: &'static [u8]) -> Option<PayloadSourceOp> {
    if line.is_empty() {
        return Some(PayloadSourceOp::Noop);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_EXIT_STATUS) {
        return parse_payload_source_status(&line[PAYLOAD_SOURCE_OP_EXIT_STATUS.len()..])
            .map(PayloadSourceOp::ExitStatus);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_EXIT_CODE) {
        return parse_payload_source_exit_code(&line[PAYLOAD_SOURCE_OP_EXIT_CODE.len()..])
            .map(PayloadSourceOp::ExitCode);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_SERVICE_READY) {
        return parse_payload_source_service_name(&line[PAYLOAD_SOURCE_OP_SERVICE_READY.len()..])
            .map(PayloadSourceOp::ServiceReady);
    }

    if line == PAYLOAD_SOURCE_OP_SERVICE_HOLD {
        return Some(PayloadSourceOp::ServiceHold);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_EXEC_PAYLOAD) {
        return parse_payload_source_argv(&line[PAYLOAD_SOURCE_OP_EXEC_PAYLOAD.len()..])
            .map(PayloadSourceOp::ExecPayload);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_SPAWN_PAYLOAD) {
        return parse_payload_source_argv(&line[PAYLOAD_SOURCE_OP_SPAWN_PAYLOAD.len()..])
            .map(PayloadSourceOp::SpawnPayload);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_SPAWN_WAIT_PAYLOAD) {
        return parse_payload_source_argv(&line[PAYLOAD_SOURCE_OP_SPAWN_WAIT_PAYLOAD.len()..])
            .map(PayloadSourceOp::SpawnWaitPayload);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_SPAWN_KILL_PAYLOAD) {
        return parse_payload_source_argv(&line[PAYLOAD_SOURCE_OP_SPAWN_KILL_PAYLOAD.len()..])
            .map(PayloadSourceOp::SpawnKillPayload);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_EXEC_BIN_STDIN_HEX) {
        return parse_payload_source_exec_bin_stdin_hex(
            &line[PAYLOAD_SOURCE_OP_EXEC_BIN_STDIN_HEX.len()..],
        );
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_EXEC_BIN) {
        return parse_payload_source_argv(&line[PAYLOAD_SOURCE_OP_EXEC_BIN.len()..])
            .map(PayloadSourceOp::ExecBin);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_PIPE_BIN) {
        return parse_payload_source_pipe_bin(&line[PAYLOAD_SOURCE_OP_PIPE_BIN.len()..]);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_SPAWN_WAIT_BIN) {
        return parse_payload_source_argv(&line[PAYLOAD_SOURCE_OP_SPAWN_WAIT_BIN.len()..])
            .map(PayloadSourceOp::SpawnWaitBin);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_SPAWN_KILL_BIN) {
        return parse_payload_source_argv(&line[PAYLOAD_SOURCE_OP_SPAWN_KILL_BIN.len()..])
            .map(PayloadSourceOp::SpawnKillBin);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_SPAWN_BIN) {
        return parse_payload_source_argv(&line[PAYLOAD_SOURCE_OP_SPAWN_BIN.len()..])
            .map(PayloadSourceOp::SpawnBin);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_SLEEP_WAIT_BIN) {
        return parse_payload_source_sleep_wait_bin(
            &line[PAYLOAD_SOURCE_OP_SLEEP_WAIT_BIN.len()..],
        );
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_SLEEP_BIN) {
        return parse_payload_source_sleep_bin(&line[PAYLOAD_SOURCE_OP_SLEEP_BIN.len()..]);
    }

    if line == PAYLOAD_SOURCE_OP_SCHEDULER_TICK {
        return Some(PayloadSourceOp::SchedulerTick);
    }

    if line == PAYLOAD_SOURCE_OP_YIELD_NOW {
        return Some(PayloadSourceOp::YieldNow);
    }

    if line == PAYLOAD_SOURCE_OP_READ_TTY_LINE {
        return Some(PayloadSourceOp::ReadTtyLine);
    }

    if line == PAYLOAD_SOURCE_OP_WRITE_PROCESS_SELF {
        return Some(PayloadSourceOp::WriteProcessSelf);
    }

    if line == PAYLOAD_SOURCE_OP_WRITE_PROCESS_TABLE {
        return Some(PayloadSourceOp::WriteProcessTable);
    }

    if line == PAYLOAD_SOURCE_OP_WRITE_SERVICE_TABLE {
        return Some(PayloadSourceOp::WriteServiceTable);
    }

    if line == PAYLOAD_SOURCE_OP_WRITE_BOOT_PROFILE {
        return Some(PayloadSourceOp::WriteBootProfile);
    }

    if line == PAYLOAD_SOURCE_OP_WRITE_DEVICE_TABLE {
        return Some(PayloadSourceOp::WriteDeviceTable);
    }

    if line == PAYLOAD_SOURCE_OP_WRITE_EXEC_TABLE {
        return Some(PayloadSourceOp::WriteExecTable);
    }

    if line == PAYLOAD_SOURCE_OP_WRITE_PENDING_EXEC_TABLE {
        return Some(PayloadSourceOp::WritePendingExecTable);
    }

    if line == PAYLOAD_SOURCE_OP_WRITE_SOURCE_TABLE {
        return Some(PayloadSourceOp::WriteSourceTable);
    }

    if line == PAYLOAD_SOURCE_OP_WRITE_SCHEDULER_STATE {
        return Some(PayloadSourceOp::WriteSchedulerState);
    }

    if line == PAYLOAD_SOURCE_OP_WRITE_TASK_TABLE {
        return Some(PayloadSourceOp::WriteTaskTable);
    }

    if line == PAYLOAD_SOURCE_OP_WRITE_WAIT_TABLE {
        return Some(PayloadSourceOp::WriteWaitTable);
    }

    if line == PAYLOAD_SOURCE_OP_WRITE_SYSCALL_TABLE {
        return Some(PayloadSourceOp::WriteSyscallTable);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_WRITE_STDOUT_HEX) {
        let encoded = &line[PAYLOAD_SOURCE_OP_WRITE_STDOUT_HEX.len()..];
        if payload_source_hex_is_valid(encoded) {
            return Some(PayloadSourceOp::WriteStdoutHex(encoded));
        }
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_WRITE_STDERR_HEX) {
        let encoded = &line[PAYLOAD_SOURCE_OP_WRITE_STDERR_HEX.len()..];
        if payload_source_hex_is_valid(encoded) {
            return Some(PayloadSourceOp::WriteStderrHex(encoded));
        }
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_WRITE_VFS_FILE) {
        return parse_payload_source_vfs_path(&line[PAYLOAD_SOURCE_OP_WRITE_VFS_FILE.len()..])
            .map(PayloadSourceOp::WriteVfsFile);
    }

    None
}

pub(crate) fn payload_source_op_start(bytes: &'static [u8]) -> Option<usize> {
    let (header, offset) = next_payload_source_line(bytes, 0)?;
    if header == PAYLOAD_SOURCE_MAGIC {
        Some(offset)
    } else {
        None
    }
}

pub(crate) fn next_payload_source_op(bytes: &'static [u8], offset: usize) -> PayloadSourceStep {
    let Some((line, next)) = next_payload_source_line(bytes, offset) else {
        return PayloadSourceStep::End;
    };

    let Some(op) = parse_payload_source_op(line) else {
        return PayloadSourceStep::Invalid;
    };

    PayloadSourceStep::Op { op, next }
}

fn validate_payload_source_image(bytes: &'static [u8]) -> bool {
    let Some(mut offset) = payload_source_op_start(bytes) else {
        return false;
    };

    let mut ops = 0usize;
    loop {
        match next_payload_source_op(bytes, offset) {
            PayloadSourceStep::End => return true,
            PayloadSourceStep::Invalid => return false,
            PayloadSourceStep::Op { next, .. } => {
                ops += 1;
                if ops > PAYLOAD_SOURCE_MAX_OPS {
                    return false;
                }
                offset = next;
            }
        }
    }
}

fn run_payload_source_op(op: PayloadSourceOp) -> Option<PayloadLaunchResult> {
    match op {
        PayloadSourceOp::Noop => Some(PayloadLaunchResult::Ready),
        PayloadSourceOp::ExitStatus(result) => Some(result),
        PayloadSourceOp::ExitCode(code) => Some(PayloadLaunchResult::ExitCode(code)),
        PayloadSourceOp::ServiceReady(_)
        | PayloadSourceOp::ServiceHold
        | PayloadSourceOp::ExecPayload(_)
        | PayloadSourceOp::SpawnPayload(_)
        | PayloadSourceOp::SpawnWaitPayload(_)
        | PayloadSourceOp::SpawnKillPayload(_)
        | PayloadSourceOp::ExecBin(_)
        | PayloadSourceOp::ExecBinStdinHex { .. }
        | PayloadSourceOp::PipeBin { .. }
        | PayloadSourceOp::SpawnBin(_)
        | PayloadSourceOp::SpawnWaitBin(_)
        | PayloadSourceOp::SpawnKillBin(_)
        | PayloadSourceOp::SleepBin { .. }
        | PayloadSourceOp::SleepWaitBin { .. }
        | PayloadSourceOp::SchedulerTick
        | PayloadSourceOp::YieldNow
        | PayloadSourceOp::ReadTtyLine
        | PayloadSourceOp::WriteProcessSelf
        | PayloadSourceOp::WriteProcessTable
        | PayloadSourceOp::WriteServiceTable
        | PayloadSourceOp::WriteBootProfile
        | PayloadSourceOp::WriteDeviceTable
        | PayloadSourceOp::WriteExecTable
        | PayloadSourceOp::WritePendingExecTable
        | PayloadSourceOp::WriteSourceTable
        | PayloadSourceOp::WriteSchedulerState
        | PayloadSourceOp::WriteTaskTable
        | PayloadSourceOp::WriteWaitTable
        | PayloadSourceOp::WriteSyscallTable
        | PayloadSourceOp::WriteStdoutHex(_)
        | PayloadSourceOp::WriteStderrHex(_)
        | PayloadSourceOp::WriteVfsFile(_) => None,
    }
}

fn run_payload_source_image(bytes: &'static [u8]) -> PayloadLaunchResult {
    let Some(mut offset) = payload_source_op_start(bytes) else {
        return PayloadLaunchResult::Failed;
    };

    let mut ops = 0usize;
    loop {
        match next_payload_source_op(bytes, offset) {
            PayloadSourceStep::End => return PayloadLaunchResult::Ready,
            PayloadSourceStep::Invalid => return PayloadLaunchResult::Failed,
            PayloadSourceStep::Op { op, next } => {
                ops += 1;
                if ops > PAYLOAD_SOURCE_MAX_OPS {
                    return PayloadLaunchResult::Failed;
                }
                let Some(result) = run_payload_source_op(op) else {
                    return PayloadLaunchResult::Failed;
                };
                if result != PayloadLaunchResult::Ready {
                    return result;
                }
                offset = next;
            }
        }
    }
}

pub(crate) const fn payload_source_max_ops() -> usize {
    PAYLOAD_SOURCE_MAX_OPS
}

/// A fixed, copyable kernel-side profile summary used by the root shell.
#[derive(Clone, Copy)]
pub struct ProfileSummary {
    /// Current root profile label.
    pub name: &'static str,
    /// Whether launching a payload from CLI is permitted in this profile.
    pub launch_enabled: bool,
}

impl ProfileSummary {
    /// Build a profile summary from plain static fields.
    pub const fn new(name: &'static str, launch_enabled: bool) -> Self {
        Self {
            name,
            launch_enabled,
        }
    }
}

/// Root daemon state and callback seam for the shell fixture.
pub struct RootDaemon<'a> {
    profile: ProfileSummary,
    boot_info: BootInfo,
    devices: &'a [DeviceEntry],
    payloads: &'a [PayloadDescriptor],
    payload_sources: &'static [PayloadSourceArtifact],
    programs: &'static [ProgramDescriptor],
    program_sources: &'static [crate::program::ProgramSourceArtifact],
    vfs_file_writer: VfsFileWriter,
    program_help_writer: ProgramHelpWriter,
    dmesg: Option<DmesgSnapshot>,
    halt: Option<HaltKernel>,
    probe_hardware: Option<HardwareProbe>,
    console_input_status: Option<ConsoleInputStatus>,
    read_line: ReadLine,
    prompt: &'static str,
    boot_image: BootImageSummary,
    console_input: ConsoleInputSummary,
    write: WriteFn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadyWorkStatus {
    Program(ProgramStatus),
    Payload,
    Continued(reovim_uapi_syscall::SyscallRet),
}

impl<'a> RootDaemon<'a> {
    /// Build a fully-seeded root daemon for shell-only OS-mode.
    pub const fn new(
        profile: ProfileSummary,
        boot_info: BootInfo,
        devices: &'a [DeviceEntry],
        payloads: &'a [PayloadDescriptor],
        payload_sources: &'static [PayloadSourceArtifact],
        programs: &'static [ProgramDescriptor],
        program_sources: &'static [crate::program::ProgramSourceArtifact],
        vfs_file_writer: VfsFileWriter,
        program_help_writer: ProgramHelpWriter,
        dmesg: Option<DmesgSnapshot>,
        halt: Option<HaltKernel>,
        probe_hardware: Option<HardwareProbe>,
        console_input_status: Option<ConsoleInputStatus>,
        read_line: ReadLine,
        prompt: &'static str,
        boot_image: BootImageSummary,
        console_input: ConsoleInputSummary,
        write: WriteFn,
    ) -> Self {
        Self {
            profile,
            boot_info,
            devices,
            payloads,
            payload_sources,
            programs,
            program_sources,
            vfs_file_writer,
            program_help_writer,
            dmesg,
            halt,
            probe_hardware,
            console_input_status,
            read_line,
            prompt,
            boot_image,
            console_input,
            write,
        }
    }

    /// Returns the profile name selected for this boot.
    #[must_use]
    pub const fn profile_name(&self) -> &'static str {
        self.profile.name
    }

    /// Returns whether this profile allows payload launches.
    #[must_use]
    pub const fn launch_enabled(&self) -> bool {
        self.profile.launch_enabled
    }

    /// Returns this profile's prompt.
    #[must_use]
    pub const fn prompt(&self) -> &'static str {
        self.prompt
    }

    /// Compile-time image identity selected by the composition root.
    #[must_use]
    pub const fn boot_image(&self) -> BootImageSummary {
        self.boot_image
    }

    /// Console input source selected for this boot.
    #[must_use]
    pub fn console_input(&self) -> ConsoleInputSummary {
        if let Some(status) = self.console_input_status {
            return status(self.console_input);
        }
        self.console_input
    }

    /// Reads one line from the active TTY input callback.
    pub fn read_tty_line(&self, out: &mut [u8]) -> usize {
        (self.read_line)(out)
    }

    /// Runs one shell input line through parser, exec admission, and scheduler dispatch.
    ///
    /// Returned `false` means the daemon should remain alive and accept
    /// additional input. `true` means execution reaches the halt path.
    pub fn run_shell_line(&self, session: &mut RootShellSession, line: &[u8]) -> bool {
        append_shell_line_log(session, line);
        let status = match split_pipeline(line) {
            Ok(Some((left, right))) => self.run_pipeline(session, left, right),
            Ok(None) => self.run_single_shell_line(session, line),
            Err(error) => {
                self.write_line(error.message());
                ProgramStatus::Error
            }
        };
        append_shell_result_log(status);
        if matches!(status, ProgramStatus::Halt) {
            self.sync_dump_for_halt();
        }
        matches!(status, ProgramStatus::Halt)
    }

    fn run_single_shell_line(&self, session: &mut RootShellSession, line: &[u8]) -> ProgramStatus {
        match parse_program_invocation(line) {
            Ok(None) => ProgramStatus::Empty,
            Err(error) => {
                self.write_line(error.message());
                ProgramStatus::Error
            }
            Ok(Some(invocation)) => {
                match syscall::exec_bin_from_shell_argv_env_with_stdin(
                    self.programs,
                    self.source_store(),
                    *invocation.argv(),
                    *invocation.env(),
                    &[],
                ) {
                    Ok(target) => self.run_pending_programs_until(session, target, None),
                    Err(error) => {
                        self.write_exec_load_error(error);
                        ProgramStatus::Error
                    }
                }
            }
        }
    }

    fn run_pipeline(
        &self,
        session: &mut RootShellSession,
        producer_line: &[u8],
        consumer_line: &[u8],
    ) -> ProgramStatus {
        let consumer_invocation = match self.parse_pipeline_stage_invocation(consumer_line) {
            Ok(invocation) => invocation,
            Err(status) => return status,
        };
        let (read_fd, write_fd) = match syscall::create_process_pipe_fds(proc::SHELL_PID) {
            Ok(fds) => fds,
            Err(_) => {
                self.write_line("error: pipeline fd setup failed");
                return ProgramStatus::Error;
            }
        };

        let producer = match self.exec_pipeline_stage(producer_line) {
            Ok(ctx) => ctx,
            Err(status) => {
                let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, read_fd);
                let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, write_fd);
                return status;
            }
        };
        if syscall::duplicate_process_pipe_fd_to_process(proc::SHELL_PID, write_fd, producer.pid, 1)
            .is_err()
        {
            let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, read_fd);
            let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, write_fd);
            self.write_line("error: pipeline fd setup failed");
            return ProgramStatus::Error;
        }
        let producer_status = self.run_pending_programs_until(session, producer, None);
        let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, write_fd);
        if producer_status == ProgramStatus::Blocked {
            if store_shell_pipeline_continuation(
                producer.pid,
                read_fd,
                *consumer_invocation.argv(),
                *consumer_invocation.env(),
            ) {
                return ProgramStatus::Blocked;
            }
            let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, read_fd);
            self.write_line("error: pipeline continuation table full");
            return ProgramStatus::Error;
        }
        if !producer_status.is_success() {
            let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, read_fd);
            return producer_status;
        }

        let consumer = match self.exec_pipeline_invocation(consumer_invocation) {
            Ok(ctx) => ctx,
            Err(status) => {
                let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, read_fd);
                return status;
            }
        };
        if syscall::duplicate_process_pipe_fd_to_process(proc::SHELL_PID, read_fd, consumer.pid, 0)
            .is_err()
        {
            let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, read_fd);
            self.write_line("error: pipeline fd setup failed");
            return ProgramStatus::Error;
        }
        let consumer_status = self.run_pending_programs_until(session, consumer, None);
        let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, read_fd);
        consumer_status
    }

    fn parse_pipeline_stage_invocation(
        &self,
        line: &[u8],
    ) -> Result<ProgramInvocation, ProgramStatus> {
        match parse_program_invocation(line) {
            Ok(None) => {
                self.write_line(ShellLineError::InvalidPipe.message());
                Err(ProgramStatus::Error)
            }
            Err(error) => {
                self.write_line(error.message());
                Err(ProgramStatus::Error)
            }
            Ok(Some(invocation)) => Ok(invocation),
        }
    }

    fn exec_pipeline_invocation(
        &self,
        invocation: ProgramInvocation,
    ) -> Result<syscall::SyscallContext, ProgramStatus> {
        match syscall::exec_bin_from_shell_argv_env_with_stdin(
            self.programs,
            self.source_store(),
            *invocation.argv(),
            *invocation.env(),
            &[],
        ) {
            Ok(target) => Ok(target),
            Err(error) => {
                self.write_exec_load_error(error);
                Err(ProgramStatus::Error)
            }
        }
    }

    fn exec_pipeline_stage(&self, line: &[u8]) -> Result<syscall::SyscallContext, ProgramStatus> {
        let invocation = self.parse_pipeline_stage_invocation(line)?;
        self.exec_pipeline_invocation(invocation)
    }

    fn write_exec_load_error(&self, error: crate::exec::ExecLoadError) {
        match error {
            crate::exec::ExecLoadError::InvalidImage => {
                self.write_line("error: invalid /bin program image");
            }
            crate::exec::ExecLoadError::SourceNotFound => {
                self.write_line("error: missing /bin program source");
            }
            crate::exec::ExecLoadError::ProcessAdmissionFailed => {
                self.write_line("error: process table full");
            }
            crate::exec::ExecLoadError::EmptyArgv0 | crate::exec::ExecLoadError::NotFound => {
                self.write_line("error: unknown /bin program, try `help`");
            }
        }
    }

    pub(crate) fn run_pending_programs_until(
        &self,
        session: &mut RootShellSession,
        target: syscall::SyscallContext,
        target_stdout_capture: Option<&crate::syscall::ProgramStdoutCapture>,
    ) -> ProgramStatus {
        let mut steps = 0usize;
        while steps < crate::sched::MAX_KERNEL_TASKS {
            let Some(ctx) = syscall::dispatch_next_ready_program() else {
                self.write_line("error: scheduler did not dispatch program");
                return ProgramStatus::Error;
            };
            let status =
                self.run_selected_ready_work(session, ctx, target.pid, target_stdout_capture);
            match status {
                ReadyWorkStatus::Program(status) => {
                    if !matches!(status, ProgramStatus::Replaced | ProgramStatus::Blocked) {
                        syscall::exit_current(ctx, status);
                        append_exec_log(ctx, status);
                        self.resume_shell_pipeline_after_producer(ctx, status);
                    }
                    if status == ProgramStatus::Replaced && ctx.pid == target.pid {
                        steps += 1;
                        continue;
                    }
                    if matches!(status, ProgramStatus::Halt) || ctx.pid == target.pid {
                        return status;
                    }
                }
                ReadyWorkStatus::Continued(ret) => match ret.decode() {
                    Ok(_) => {
                        if ctx.pid == target.pid {
                            if matches!(
                                proc::process(ctx.pid),
                                Some(record)
                                    if record.state == proc::ProcessState::Blocked
                                        && record.block_reason == sched::BlockReason::UserResume
                            ) {
                                return ProgramStatus::Blocked;
                            }
                            return ProgramStatus::Ok;
                        }
                    }
                    Err(reovim_uapi_syscall::SyscallError::BUSY) => {
                        if ctx.pid == target.pid {
                            return ProgramStatus::Blocked;
                        }
                    }
                    Err(_) => {
                        syscall::exit_current(ctx, ProgramStatus::Error);
                        append_exec_log(ctx, ProgramStatus::Error);
                        self.resume_shell_pipeline_after_producer(ctx, ProgramStatus::Error);
                        if ctx.pid == target.pid {
                            return ProgramStatus::Error;
                        }
                    }
                },
                ReadyWorkStatus::Payload => {}
            }
            steps += 1;
        }

        self.write_line("error: scheduler dispatch budget exhausted");
        ProgramStatus::Error
    }

    /// Runs scheduler-ready `/bin` work while rootd is otherwise idle.
    ///
    /// This is the real root-daemon loop path for background ready work: it does
    /// not admit a new shell command and it does not require a later input line
    /// to make FIFO progress. Direct `run_shell_line` tests still inspect
    /// retained ready state before this idle drain is called.
    pub fn run_idle_ready_programs(&self, session: &mut RootShellSession) -> ProgramStatus {
        let mut steps = 0usize;
        while steps < crate::sched::MAX_KERNEL_TASKS {
            let Some(ctx) = syscall::dispatch_next_ready_program() else {
                return ProgramStatus::Ok;
            };
            let status = self.run_selected_ready_work(session, ctx, 0, None);
            match status {
                ReadyWorkStatus::Program(status) => {
                    if !matches!(status, ProgramStatus::Replaced | ProgramStatus::Blocked) {
                        syscall::exit_current(ctx, status);
                        append_exec_log(ctx, status);
                        self.resume_shell_pipeline_after_producer(ctx, status);
                    }
                    if matches!(status, ProgramStatus::Halt) {
                        self.sync_dump_for_halt();
                        return status;
                    }
                }
                ReadyWorkStatus::Continued(ret) => {
                    if let Err(error) = ret.decode() {
                        if error != reovim_uapi_syscall::SyscallError::BUSY {
                            syscall::exit_current(ctx, ProgramStatus::Error);
                            append_exec_log(ctx, ProgramStatus::Error);
                            self.resume_shell_pipeline_after_producer(ctx, ProgramStatus::Error);
                            return ProgramStatus::Error;
                        }
                    }
                }
                ReadyWorkStatus::Payload => {}
            }
            steps += 1;
        }

        self.write_line("error: idle scheduler dispatch budget exhausted");
        ProgramStatus::Error
    }

    fn resume_shell_pipeline_after_producer(
        &self,
        producer: syscall::SyscallContext,
        producer_status: ProgramStatus,
    ) {
        let Some(continuation) = take_shell_pipeline_continuation(producer.pid) else {
            return;
        };
        if !producer_status.is_success() {
            let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, continuation.read_fd);
            return;
        }

        let invocation =
            ProgramInvocation::new(continuation.consumer_argv, continuation.consumer_env);
        let consumer = match self.exec_pipeline_invocation(invocation) {
            Ok(ctx) => ctx,
            Err(_) => {
                let _ =
                    syscall::close_process_fd_for_pipeline(proc::SHELL_PID, continuation.read_fd);
                return;
            }
        };
        if syscall::duplicate_process_pipe_fd_to_process(
            proc::SHELL_PID,
            continuation.read_fd,
            consumer.pid,
            0,
        )
        .is_err()
        {
            let _ = syscall::take_pending_exec(consumer.pid);
            syscall::exit_current(consumer, ProgramStatus::Error);
            append_exec_log(consumer, ProgramStatus::Error);
            let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, continuation.read_fd);
            self.write_line("error: pipeline fd setup failed");
            return;
        }
        let _ = syscall::close_process_fd_for_pipeline(proc::SHELL_PID, continuation.read_fd);
    }

    fn run_selected_ready_work(
        &self,
        session: &mut RootShellSession,
        ctx: syscall::SyscallContext,
        target_pid: usize,
        target_stdout_capture: Option<&crate::syscall::ProgramStdoutCapture>,
    ) -> ReadyWorkStatus {
        if let Some(pending) = syscall::take_pending_exec(ctx.pid) {
            let pending_ctx = syscall::SyscallContext::from_process(pending.handle());
            let stdout_capture = if ctx.pid == target_pid {
                target_stdout_capture
            } else {
                None
            };
            return ReadyWorkStatus::Program(
                execute_loaded_program_argv(
                    self,
                    session,
                    pending.program(),
                    pending.argv(),
                    pending.env(),
                    pending.stdin(),
                    stdout_capture,
                    Some(pending_ctx),
                )
                .status(),
            );
        }

        if syscall::run_selected_pending_payload(self, session, ctx) {
            return ReadyWorkStatus::Payload;
        }

        if syscall::resume_payload_source_continuation(self, session, ctx) {
            return ReadyWorkStatus::Payload;
        }

        if let Some(ret) = syscall::resume_syscall_continuation(self, session, ctx) {
            if ret.decode().is_ok() {
                if let Some(status) = program::resume_bin_uapi_frame(self, session, ctx, ret) {
                    return ReadyWorkStatus::Program(status);
                }
            }
            return ReadyWorkStatus::Continued(ret);
        }

        self.write_line("error: scheduler selected process without program image");
        ReadyWorkStatus::Program(ProgramStatus::Error)
    }

    fn run_boot_init(&self, session: &mut RootShellSession) -> ProgramStatus {
        klog::append_line("rootd: start /bin/init");
        let target =
            match syscall::exec_bin_from_rootd_argv0(self.programs, self.source_store(), "init") {
                Ok(target) => target,
                Err(error) => {
                    self.write_exec_load_error(error);
                    klog::append_line("init.status=error");
                    return ProgramStatus::Error;
                }
            };
        let status = self.run_pending_programs_until(session, target, None);
        match status {
            ProgramStatus::Ok | ProgramStatus::Replaced | ProgramStatus::ExitCode(0) => {
                klog::append_line("init.status=ok")
            }
            ProgramStatus::Halt => klog::append_line("init.status=halt"),
            _ => klog::append_line("init.status=error"),
        }
        status
    }

    fn run_boot_shell_target(
        &self,
        session: &mut RootShellSession,
        program: &'static str,
    ) -> ProgramStatus {
        klog::append_bytes(b"rootd: start shell target ");
        klog::append_bytes(program.as_bytes());
        klog::append_bytes(b"\n");
        let target =
            match syscall::exec_bin_from_rootd_argv0(self.programs, self.source_store(), program) {
                Ok(target) => target,
                Err(error) => {
                    self.write_exec_load_error(error);
                    let _ = service::mark_failed("shell", service::ServiceReason::ExecLoadError);
                    klog::append_line("shell_target.status=error");
                    return ProgramStatus::Error;
                }
            };
        let status = self.run_pending_programs_until(session, target, None);
        match status {
            ProgramStatus::Ok | ProgramStatus::Replaced | ProgramStatus::ExitCode(0) => {
                let target_record = proc::process(target.pid).unwrap_or(proc::EMPTY_PROCESS_RECORD);
                proc::install_shell_session_with_body_metadata(
                    target.program_path,
                    target.loader,
                    target.entry_name,
                    target_record.artifact_body_format,
                    target_record.artifact_body_inner_format,
                );
                session.install_shell_owner(target.program_path, target.loader, target.entry_name);
                let service_process = proc::process(proc::SHELL_PID);
                let service_pid = service_process.map_or(proc::SHELL_PID, |record| record.pid);
                let service_task_id =
                    service_process.map_or(proc::SHELL_PID, |record| record.task_id);
                let _ = service::mark_started("shell", service_pid, service_task_id);
                klog::append_bytes(b"shell_session.owner=");
                klog::append_bytes(target.program_path.as_bytes());
                klog::append_bytes(b" loader=");
                klog::append_bytes(target.loader.as_bytes());
                klog::append_bytes(b" entry_fn=");
                klog::append_bytes(target.entry_name.as_bytes());
                klog::append_bytes(b"\n");
                klog::append_line("shell_target.status=ok")
            }
            ProgramStatus::Halt => {
                let _ = service::mark_failed("shell", service::ServiceReason::Halt);
                klog::append_line("shell_target.status=halt")
            }
            _ => {
                let _ = service::mark_failed("shell", service::ServiceReason::StartError);
                klog::append_line("shell_target.status=error")
            }
        }
        status
    }

    /// Emits the boot-level shell prompt.
    pub fn write_prompt(&self) {
        (self.write)(self.prompt.as_bytes());
    }

    /// Writes an output fragment via the active sink.
    pub fn write_bytes(&self, bytes: &[u8]) {
        (self.write)(bytes);
    }

    /// Writes one line with trailing `\n`.
    pub fn write_line(&self, line: &str) {
        (self.write)(line.as_bytes());
        (self.write)(b"\n");
    }

    /// Snapshot of boot-time device inventory for `device`.
    #[must_use]
    pub const fn devices(&self) -> &[DeviceEntry] {
        self.devices
    }

    /// `/bin` program catalog supplied by the OS image.
    #[must_use]
    pub const fn programs(&self) -> &'static [ProgramDescriptor] {
        self.programs
    }

    /// `/bin` source artifacts supplied by the OS image.
    #[must_use]
    pub const fn program_sources(&self) -> &'static [crate::program::ProgramSourceArtifact] {
        self.program_sources
    }

    /// Executable source-store view supplied by the OS image.
    #[must_use]
    pub const fn source_store(&self) -> ExecutableSourceStore {
        ExecutableSourceStore::new(self.program_sources, self.payload_sources)
    }

    /// VFS pseudo-file renderer supplied by the OS image.
    #[must_use]
    pub const fn vfs_file_writer(&self) -> VfsFileWriter {
        self.vfs_file_writer
    }

    /// `/bin/help` renderer supplied by the OS image.
    #[must_use]
    pub const fn program_help_writer(&self) -> ProgramHelpWriter {
        self.program_help_writer
    }

    /// Snapshot of boot diagnostics for pseudo files and `/bin` programs.
    #[must_use]
    pub const fn boot_info(&self) -> BootInfo {
        self.boot_info
    }

    /// Returns the configured extra diagnostics snapshot.
    #[must_use]
    pub const fn dmesg_fn(&self) -> Option<DmesgSnapshot> {
        self.dmesg
    }

    /// Run a lower-provider hardware probe by target name, if one is installed.
    pub fn run_hardware_probe(&self, target: &str) -> Option<HardwareProbeResult> {
        let probe = self.probe_hardware?;
        Some(probe(target, self.devices, self.write))
    }

    /// Returns the payload descriptor index and descriptor by name.
    pub fn payload_by_name(&self, name: &str) -> Option<(usize, &'a PayloadDescriptor)> {
        find_payload_by_name(self.payloads, name)
    }

    /// Loads a payload program by catalog index.
    pub fn load_payload_by_index(&self, index: usize) -> Option<LoadedPayloadProgram> {
        let payload = self.payloads.get(index)?;
        LoadedPayloadProgram::from_descriptor(index, payload, self.source_store()).ok()
    }

    /// Loads a payload program by name.
    pub fn load_payload_by_name(&self, name: &str) -> Option<LoadedPayloadProgram> {
        let (index, payload) = self.payload_by_name(name)?;
        LoadedPayloadProgram::from_descriptor(index, payload, self.source_store()).ok()
    }

    /// Executes a loaded payload program.
    pub fn run_loaded_payload(&self, payload: LoadedPayloadProgram) -> PayloadLaunchResult {
        if !self.profile.launch_enabled {
            return PayloadLaunchResult::NotConfigured;
        }

        match payload.image_kind {
            PayloadImageKind::SourceImage => run_payload_source_image(payload.source_bytes()),
            PayloadImageKind::ReovimExecBody => {
                let Ok(body) = exec_body::parse_exec_body(payload.source_bytes()) else {
                    return PayloadLaunchResult::Failed;
                };
                if body.inner != ExecBodyInnerFormat::PayloadSourceImage {
                    return PayloadLaunchResult::Failed;
                }
                run_payload_source_image(body.bytes)
            }
        }
    }

    /// Executes a configured payload by index.
    pub fn launch_payload_by_index(&self, index: usize) -> PayloadLaunchResult {
        let Some(payload) = self.load_payload_by_index(index) else {
            return PayloadLaunchResult::NotConfigured;
        };
        self.run_loaded_payload(payload)
    }

    /// Executes a configured payload by payload name.
    pub fn launch_payload_by_name(&self, name: &str) -> PayloadLaunchResult {
        let Some(payload) = self.load_payload_by_name(name) else {
            return PayloadLaunchResult::NotConfigured;
        };
        self.run_loaded_payload(payload)
    }

    /// Payload catalog for `/bin/launch` output.
    #[must_use]
    pub const fn payloads(&self) -> &'a [PayloadDescriptor] {
        self.payloads
    }

    /// Payload source artifacts supplied by the OS image.
    #[must_use]
    pub const fn payload_sources(&self) -> &'static [PayloadSourceArtifact] {
        self.payload_sources
    }

    /// Triggers the kernel halt callback.
    pub fn halt_kernel(&self) {
        if let Some(halt) = self.halt {
            halt();
        }
    }

    fn dump_image_identity(&self) -> dump::DumpImageIdentity {
        dump::DumpImageIdentity::new(
            self.boot_image.package,
            self.boot_image.version,
            self.boot_image.target,
            self.boot_image.selected_profile,
            self.boot_image.profile_request,
            self.boot_image.bootline,
            self.boot_image.launch_profile_feature,
        )
    }

    fn sync_dump_for_halt(&self) {
        klog::append_line("rootd: halt dump sync");
        klog::append_event_with_source_context(
            "rootd",
            "dump",
            "info",
            "halt-sync-start",
            proc::ROOTD_PID,
            1,
        );
        let status =
            syscall::sync_dump_from_rootd(self.dump_image_identity(), self.boot_info, self.devices);
        append_halt_dump_sync_log(status);
        klog::append_event_with_source_context(
            "rootd",
            "dump",
            if status.written { "info" } else { "warn" },
            if status.written {
                "halt-sync-ok"
            } else {
                "halt-sync-not-written"
            },
            proc::ROOTD_PID,
            1,
        );
    }
}

fn append_halt_dump_sync_log(status: dump::DumpSyncStatus) {
    klog::append_bytes(b"rootd.dump_sync persistent=");
    klog::append_bytes(if status.persistent_available {
        b"available"
    } else {
        b"unavailable"
    });
    klog::append_bytes(b" storage=");
    klog::append_bytes(status.storage.as_bytes());
    klog::append_bytes(b" storage_capacity_bytes=");
    klog::append_usize_dec(status.storage_capacity_bytes);
    klog::append_bytes(b" status=");
    klog::append_bytes(if status.written {
        b"written"
    } else {
        b"not-written"
    });
    klog::append_bytes(b" bytes=");
    klog::append_usize_dec(status.bytes_written);
    klog::append_bytes(b" checksum=");
    klog::append_usize_dec(status.checksum as usize);
    klog::append_bytes(b" verified=");
    klog::append_bytes(if status.verified { b"true" } else { b"false" });
    klog::append_bytes(b" reason=");
    klog::append_bytes(status.reason.as_bytes());
    klog::append_bytes(b"\n");
}

fn trim_line_end(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    while end > 0 {
        match bytes[end - 1] {
            b'\n' | b'\r' => end -= 1,
            _ => break,
        }
    }
    &bytes[..end]
}

fn append_shell_line_log(session: &RootShellSession, line: &[u8]) {
    let entered_line = trim_line_end(line);
    klog::append_bytes(b"shell: ");
    if entered_line.is_empty() {
        klog::append_bytes(b"<empty>");
    } else {
        klog::append_bytes(entered_line);
    }
    klog::append_bytes(b"\n");
    klog::append_bytes(b"shell.session path=");
    klog::append_bytes(session.owner_path().as_bytes());
    klog::append_bytes(b" loader=");
    klog::append_bytes(session.owner_loader().as_bytes());
    klog::append_bytes(b" entry_fn=");
    klog::append_bytes(session.owner_entry_name().as_bytes());
    klog::append_bytes(b" line=");
    if entered_line.is_empty() {
        klog::append_bytes(b"<empty>");
    } else {
        klog::append_bytes(entered_line);
    }
    klog::append_bytes(b"\n");
}

fn append_shell_result_log(status: ProgramStatus) {
    klog::append_bytes(b"shell.status=");
    klog::append_bytes(status.as_bytes());
    klog::append_bytes(b"\n");
}

fn append_exec_log(ctx: syscall::SyscallContext, status: ProgramStatus) {
    klog::append_bytes(b"exec.path=");
    klog::append_bytes(ctx.program_path.as_bytes());
    klog::append_bytes(b" pid=");
    klog::append_usize_dec(ctx.pid);
    klog::append_bytes(b" task=");
    klog::append_usize_dec(ctx.task_id);
    klog::append_bytes(b" status=");
    klog::append_bytes(status.as_bytes());
    klog::append_bytes(b" loader=");
    klog::append_bytes(ctx.loader.as_bytes());
    klog::append_bytes(b" entry_fn=");
    klog::append_bytes(ctx.entry_name.as_bytes());
    klog::append_bytes(b"\n");
    append_exec_parent_log(ctx);
    klog::append_event_with_context("proc", "info", "program-exit", ctx.pid, ctx.task_id);
}

fn append_exec_parent_log(ctx: syscall::SyscallContext) {
    let parent_pid = proc::process(ctx.pid).map_or(0, |record| record.parent_pid);
    let parent_task = sched::task(ctx.task_id).map_or(0, |record| record.parent_task_id);
    klog::append_bytes(b"exec.parent path=");
    klog::append_bytes(ctx.program_path.as_bytes());
    klog::append_bytes(b" pid=");
    klog::append_usize_dec(ctx.pid);
    klog::append_bytes(b" ppid=");
    klog::append_usize_dec(parent_pid);
    klog::append_bytes(b" task=");
    klog::append_usize_dec(ctx.task_id);
    klog::append_bytes(b" parent_task=");
    klog::append_usize_dec(parent_task);
    klog::append_bytes(b"\n");
}

fn write_usize_dec(write: WriteFn, value: usize) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut v = value;
    loop {
        i -= 1;
        #[allow(clippy::cast_possible_truncation)]
        let digit = (v % 10) as u8;
        buf[i] = b'0' + digit;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    write(&buf[i..]);
}

const fn status_label(state: BootCheckState) -> &'static [u8] {
    match state {
        BootCheckState::Ok => OK,
        BootCheckState::Warn => WARN,
    }
}

const fn status_log_label(state: BootCheckState) -> &'static [u8] {
    match state {
        BootCheckState::Ok => b"[  OK  ] ",
        BootCheckState::Warn => b"[ WARN ] ",
    }
}

fn write_boot_status_line(write: WriteFn, state: BootCheckState, message: &[u8]) {
    let status = status_label(state);
    write(status);
    write(message);
    write(b"\n");
    klog::append_bytes(status_log_label(state));
    klog::append_bytes(message);
    klog::append_bytes(b"\n");
}

fn write_boot_status_with_program(
    write: WriteFn,
    state: BootCheckState,
    prefix: &[u8],
    program: &str,
    suffix: &[u8],
) {
    let status = status_label(state);
    write(status);
    write(prefix);
    write(program.as_bytes());
    write(suffix);
    write(b"\n");
    klog::append_bytes(status_log_label(state));
    klog::append_bytes(prefix);
    klog::append_bytes(program.as_bytes());
    klog::append_bytes(suffix);
    klog::append_bytes(b"\n");
}

fn write_boot_status_with_count(
    write: WriteFn,
    state: BootCheckState,
    message: &[u8],
    count: usize,
    suffix: &[u8],
) {
    write(status_label(state));
    write(message);
    write_usize_dec(write, count);
    write(suffix);
    write(b"\n");
    klog::append_bytes(status_log_label(state));
    klog::append_bytes(message);
    klog::append_usize_dec(count);
    klog::append_bytes(suffix);
    klog::append_bytes(b"\n");
}

const fn input_state_word(state: BootCheckState) -> &'static [u8] {
    match state {
        BootCheckState::Ok => b"ready",
        BootCheckState::Warn => b"unavailable",
    }
}

const fn usb_keyboard_message(state: BootCheckState) -> &'static [u8] {
    match state {
        BootCheckState::Ok => b"USB keyboard input provider ready.",
        BootCheckState::Warn => b"USB keyboard input provider unavailable.",
    }
}

fn write_console_input_detail(write: WriteFn, input: ConsoleInputSummary) {
    write(b"         input=");
    write(input.source.as_bytes());
    write(b" mode=");
    write(input.mode.as_bytes());
    write(b" usb_keyboard=");
    write(input_state_word(input.usb_keyboard));
    write(b" last_poll=");
    write(input.usb_keyboard_last_poll.as_bytes());
    write(b"\n");

    klog::append_bytes(b"input=");
    klog::append_bytes(input.source.as_bytes());
    klog::append_bytes(b" mode=");
    klog::append_bytes(input.mode.as_bytes());
    klog::append_bytes(b" usb_keyboard=");
    klog::append_bytes(input_state_word(input.usb_keyboard));
    klog::append_bytes(b" last_poll=");
    klog::append_bytes(input.usb_keyboard_last_poll.as_bytes());
    klog::append_bytes(b"\n");
}

const fn memory_check(info: BootInfo) -> BootCheckState {
    if info.memory.range_count() > 0 && info.memory.usable_bytes() > 0 {
        BootCheckState::Ok
    } else {
        BootCheckState::Warn
    }
}

const fn cpu_check(info: BootInfo) -> BootCheckState {
    if info.cpu_count > 0 {
        BootCheckState::Ok
    } else {
        BootCheckState::Warn
    }
}

const fn device_check(devices: &[DeviceEntry]) -> BootCheckState {
    if devices.is_empty() {
        BootCheckState::Warn
    } else {
        BootCheckState::Ok
    }
}

fn write_boot_log(cfg: &RootBootConfig<'_>) {
    klog::append_line("rootd: boot report");
    (cfg.write)(b"\n");
    (cfg.write)(TITLE);
    (cfg.write)(b"\n\n");
    match cfg.splash_geometry {
        Some((width, height)) => {
            write_boot_status_line(
                cfg.write,
                BootCheckState::Ok,
                b"Initialized framebuffer console.",
            );
            (cfg.write)(b"         geometry=");
            write_usize_dec(cfg.write, width as usize);
            (cfg.write)(b"x");
            write_usize_dec(cfg.write, height as usize);
            (cfg.write)(b"x32\n");
            klog::append_bytes(b"geometry=");
            klog::append_usize_dec(width as usize);
            klog::append_bytes(b"x");
            klog::append_usize_dec(height as usize);
            klog::append_bytes(b"x32\n");
        }
        None => write_boot_status_line(
            cfg.write,
            BootCheckState::Warn,
            b"Framebuffer unavailable; using UART console.",
        ),
    }
    write_boot_status_line(
        cfg.write,
        cfg.runtime_checks.alloc_backend,
        b"Installed allocator backend.",
    );
    write_boot_status_line(
        cfg.write,
        cfg.runtime_checks.sync_backend,
        b"Installed scheduler/sync backend.",
    );
    write_boot_status_with_count(
        cfg.write,
        memory_check(cfg.boot_info),
        b"Discovered ",
        cfg.boot_info.memory.range_count(),
        b" memory ranges.",
    );
    write_boot_status_with_count(
        cfg.write,
        cpu_check(cfg.boot_info),
        b"Detected ",
        cfg.boot_info.cpu_count as usize,
        b" CPUs.",
    );
    write_boot_status_with_count(
        cfg.write,
        device_check(cfg.devices),
        b"Enumerated ",
        cfg.devices.len(),
        b" devices.",
    );
    write_boot_status_line(
        cfg.write,
        cfg.console_input.source_state,
        b"Selected console input source.",
    );
    write_console_input_detail(cfg.write, cfg.console_input);
    write_boot_status_line(
        cfg.write,
        cfg.console_input.usb_keyboard,
        usb_keyboard_message(cfg.console_input.usb_keyboard),
    );
    write_boot_status_line(cfg.write, BootCheckState::Ok, b"Selected boot profile.");
    (cfg.write)(b"         profile=");
    (cfg.write)(cfg.profile.name.as_bytes());
    (cfg.write)(b" launch=");
    klog::append_bytes(b"profile=");
    klog::append_bytes(cfg.profile.name.as_bytes());
    klog::append_bytes(b" launch=");
    if cfg.profile.launch_enabled {
        (cfg.write)(b"enabled\n");
        klog::append_bytes(b"enabled\n");
        write_boot_status_with_count(
            cfg.write,
            if cfg.payloads.is_empty() {
                BootCheckState::Warn
            } else {
                BootCheckState::Ok
            },
            b"Registered ",
            cfg.payloads.len(),
            b" payloads.",
        );
    } else {
        (cfg.write)(b"disabled\n");
        klog::append_bytes(b"disabled\n");
    }
    write_boot_status_line(cfg.write, BootCheckState::Ok, b"Reached target /bin/init.");
}

fn halt_or_park_after_boot_stop(halt: Option<HaltKernel>) -> ! {
    if let Some(halt) = halt {
        klog::append_line("rootd: halt callback");
        klog::append_event_with_source_context("rootd", "boot", "info", "halt-callback", 0, 0);
        halt();
    }

    loop {
        core::hint::spin_loop();
    }
}

/// Boots the root daemon shell and never returns.
///
/// The kernel owns this execution point for RTOS mode: splash, shell
/// bootstrap, line loop, and `/bin` program dispatch.
///
/// The `read_line` callback is caller-owned and architecture-specific. The
/// callback must return `0` on EOF or fatal read errors so this loop can exit
/// deterministically in test or host simulation builds.
pub fn run_root_daemon(cfg: RootBootConfig<'_>) -> ! {
    klog::reset();
    klog::install_identity(klog::DiagnosticIdentity {
        boot_id: 1,
        session_id: 1,
        identity_source: "rootd-volatile",
    });
    proc::reset();
    syscall::reset();
    klog::append_line("rootd: boot start");
    klog::append_event_with_source_context("rootd", "boot", "info", "boot-start", 0, 0);
    // The shell policy prints a branded splash before interactive control.
    splash::render(cfg.splash_geometry, cfg.write);
    if let Some(prepare_shell) = cfg.prepare_shell {
        prepare_shell();
    }
    write_boot_log(&cfg);

    let daemon = RootDaemon::new(
        cfg.profile,
        cfg.boot_info,
        cfg.devices,
        cfg.payloads,
        cfg.payload_sources,
        cfg.programs,
        cfg.program_sources,
        cfg.vfs_file_writer,
        cfg.program_help_writer,
        cfg.dmesg,
        cfg.halt,
        cfg.probe_hardware,
        cfg.console_input_status,
        cfg.read_line,
        cfg.prompt,
        cfg.boot_image,
        cfg.console_input,
        cfg.write,
    );
    let mut session = RootShellSession::new();
    let init_status = daemon.run_boot_init(&mut session);
    let shell_target = match init_status {
        ProgramStatus::Ok | ProgramStatus::Replaced | ProgramStatus::ExitCode(0)
            if !session.shell_start_requested() =>
        {
            write_boot_status_line(cfg.write, BootCheckState::Ok, b"Started /bin/init.");
            match service::service_target("shell") {
                Some(program) => program,
                None => {
                    write_boot_status_line(
                        cfg.write,
                        BootCheckState::Warn,
                        b"/bin/init did not request shell service target.",
                    );
                    klog::append_line("init.service.shell=missing");
                    halt_or_park_after_boot_stop(cfg.halt);
                }
            }
        }
        ProgramStatus::Halt => {
            write_boot_status_line(cfg.write, BootCheckState::Warn, b"/bin/init requested halt.");
            halt_or_park_after_boot_stop(cfg.halt);
        }
        _ => {
            write_boot_status_line(cfg.write, BootCheckState::Warn, b"/bin/init failed.");
            halt_or_park_after_boot_stop(cfg.halt);
        }
    };

    write_boot_status_with_program(
        cfg.write,
        BootCheckState::Ok,
        b"Reached target ",
        shell_target,
        b".",
    );
    let shell_status = daemon.run_boot_shell_target(&mut session, shell_target);
    match shell_status {
        ProgramStatus::Ok | ProgramStatus::Replaced | ProgramStatus::ExitCode(0)
            if session.shell_start_requested() && session.line_discipline_requested() =>
        {
            write_boot_status_with_program(
                cfg.write,
                BootCheckState::Ok,
                b"Started ",
                shell_target,
                b".",
            );
            write_boot_status_line(cfg.write, BootCheckState::Ok, b"Reached target root shell.");
        }
        ProgramStatus::Ok | ProgramStatus::Replaced | ProgramStatus::ExitCode(0)
            if !session.shell_start_requested() =>
        {
            write_boot_status_with_program(
                cfg.write,
                BootCheckState::Warn,
                b"",
                shell_target,
                b" did not request root shell.",
            );
            klog::append_line("shell_target.shell_start=missing");
            halt_or_park_after_boot_stop(cfg.halt);
        }
        ProgramStatus::Ok | ProgramStatus::Replaced | ProgramStatus::ExitCode(0) => {
            write_boot_status_with_program(
                cfg.write,
                BootCheckState::Warn,
                b"",
                shell_target,
                b" did not request shell line discipline.",
            );
            klog::append_line("shell_target.line_discipline=missing");
            halt_or_park_after_boot_stop(cfg.halt);
        }
        ProgramStatus::Halt => {
            write_boot_status_with_program(
                cfg.write,
                BootCheckState::Warn,
                b"",
                shell_target,
                b" requested halt.",
            );
            halt_or_park_after_boot_stop(cfg.halt);
        }
        _ => {
            write_boot_status_with_program(
                cfg.write,
                BootCheckState::Warn,
                b"",
                shell_target,
                b" failed.",
            );
            halt_or_park_after_boot_stop(cfg.halt);
        }
    }

    klog::append_line("rootd: shell ready");
    klog::append_event_with_source_context("rootd", "boot", "info", "shell-ready", 0, 0);
    daemon.write_prompt();

    let mut line = [0u8; ROOT_LINE_BYTES];
    loop {
        let len = (cfg.read_line)(&mut line);
        if len == 0 {
            klog::append_line("rootd: input eof");
            klog::append_event_with_source_context("rootd", "input", "warn", "eof", 0, 0);
            break;
        }

        if daemon.run_shell_line(&mut session, &line[..len]) {
            klog::append_line("rootd: halt requested");
            klog::append_event_with_source_context("rootd", "boot", "info", "halt-requested", 0, 0);
            break;
        }

        if matches!(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Halt) {
            klog::append_line("rootd: halt requested by idle program");
            klog::append_event_with_source_context("rootd", "boot", "info", "halt-requested", 0, 0);
            break;
        }

        daemon.write_prompt();
    }

    if let Some(halt) = cfg.halt {
        klog::append_line("rootd: halt callback");
        klog::append_event_with_source_context("rootd", "boot", "info", "halt-callback", 0, 0);
        halt();
    }

    // If the root profile has no explicit halt callback, keep the CPU parked in
    // a deterministic never-returning state so callers can still model boot as
    // one-way in architecture-independent form.
    loop {
        core::hint::spin_loop();
    }
}

/// Human-readable class names used by `device` output.
#[must_use]
pub const fn device_class_name(class: DeviceClass) -> &'static str {
    vfs::device_class_name(class)
}
