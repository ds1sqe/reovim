//! Root daemon public surface for OS-mode shell execution.
//!
//! This module owns the kernel-only supervisor and payload launch registry.
//! It has no dependency on any provider, product crate, editor, server, or
//! client implementation.

use {
    crate::{
        klog, proc,
        program::{MAX_PROGRAM_STDIN_BYTES, ProgramDescriptor, ProgramStatus},
        root_shell::{
            RootShellSession, ShellLineError, execute_loaded_program_argv, parse_program_argv,
            split_pipeline,
        },
        source_store::ExecutableSourceStore,
        splash,
        syscall::{self, ProgramStdoutCapture},
        vfs,
    },
    reovim_uapi_system::{BootInfo, DeviceClass, DeviceEntry},
};

const OK: &[u8] = b"\x1b[32m[  OK  ]\x1b[0m ";
const WARN: &[u8] = b"\x1b[33m[ WARN ]\x1b[0m ";
const TITLE: &[u8] = b"\x1b[38;2;90;210;255mReovim OS\x1b[0m boot report";
/// Maximum number of payload descriptors a profile can install.
pub const MAX_PAYLOADS: usize = 8;

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

/// Source kind for a launchable payload image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayloadImageKind {
    /// Payload body is interpreted from a bounded Reovim source image.
    SourceImage,
}

impl PayloadImageKind {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceImage => "source-image",
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

    pub(crate) const fn source_bytes(&self) -> &'static [u8] {
        self.source_bytes
    }
}

/// Result from executing a payload source image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayloadLaunchResult {
    /// Launch path completed and payload is ready.
    Ready,
    /// Payload launch is not configured for the current profile or catalog.
    NotConfigured,
    /// Payload source execution failed.
    Failed,
}

impl core::fmt::Display for PayloadLaunchResult {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ready => out.write_str("payload.ready"),
            Self::NotConfigured => out.write_str("payload.not_configured"),
            Self::Failed => out.write_str("payload.failed"),
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
const PAYLOAD_SOURCE_MAX_OPS: usize = 16;
const PAYLOAD_SOURCE_READY_IMAGE: &[u8] = b"reovim-payload-source-v1\nexit-status ready\n";
const PAYLOAD_SOURCE_FAILED_IMAGE: &[u8] = b"reovim-payload-source-v1\nexit-status failed\n";

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

fn validate_payload_source_op(line: &'static [u8]) -> bool {
    if line.is_empty() {
        return true;
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_EXIT_STATUS) {
        return parse_payload_source_status(&line[PAYLOAD_SOURCE_OP_EXIT_STATUS.len()..]).is_some();
    }

    false
}

fn validate_payload_source_image(bytes: &'static [u8]) -> bool {
    let Some((header, mut offset)) = next_payload_source_line(bytes, 0) else {
        return false;
    };
    if header != PAYLOAD_SOURCE_MAGIC {
        return false;
    }

    let mut ops = 0usize;
    while let Some((line, next)) = next_payload_source_line(bytes, offset) {
        ops += 1;
        if ops > PAYLOAD_SOURCE_MAX_OPS || !validate_payload_source_op(line) {
            return false;
        }
        offset = next;
    }

    true
}

fn run_payload_source_op(line: &'static [u8]) -> Option<PayloadLaunchResult> {
    if line.is_empty() {
        return Some(PayloadLaunchResult::Ready);
    }

    if line.starts_with(PAYLOAD_SOURCE_OP_EXIT_STATUS) {
        return parse_payload_source_status(&line[PAYLOAD_SOURCE_OP_EXIT_STATUS.len()..]);
    }

    None
}

fn run_payload_source_image(bytes: &'static [u8]) -> PayloadLaunchResult {
    let Some((header, mut offset)) = next_payload_source_line(bytes, 0) else {
        return PayloadLaunchResult::Failed;
    };
    if header != PAYLOAD_SOURCE_MAGIC {
        return PayloadLaunchResult::Failed;
    }

    let mut ops = 0usize;
    while let Some((line, next)) = next_payload_source_line(bytes, offset) {
        ops += 1;
        if ops > PAYLOAD_SOURCE_MAX_OPS {
            return PayloadLaunchResult::Failed;
        }
        let Some(result) = run_payload_source_op(line) else {
            return PayloadLaunchResult::Failed;
        };
        if result != PayloadLaunchResult::Ready {
            return result;
        }
        offset = next;
    }

    PayloadLaunchResult::Ready
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
        append_shell_line_log(line);
        let status = match split_pipeline(line) {
            Ok(Some((left, right))) => self.run_pipeline(session, left, right),
            Ok(None) => self.run_single_shell_line(session, line),
            Err(error) => {
                self.write_line(error.message());
                ProgramStatus::Error
            }
        };
        append_shell_result_log(status);
        matches!(status, ProgramStatus::Halt)
    }

    fn run_single_shell_line(&self, session: &mut RootShellSession, line: &[u8]) -> ProgramStatus {
        match parse_program_argv(line) {
            Ok(None) => ProgramStatus::Empty,
            Err(error) => {
                self.write_line(error.message());
                ProgramStatus::Error
            }
            Ok(Some(argv)) => {
                match syscall::exec_bin_from_shell_argv_with_stdin(
                    self.programs,
                    self.source_store(),
                    argv,
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
        let capture = ProgramStdoutCapture::new();
        let producer = match self.exec_pipeline_stage(producer_line, &[]) {
            Ok(ctx) => ctx,
            Err(status) => return status,
        };
        let producer_status = self.run_pending_programs_until(session, producer, Some(&capture));
        if !producer_status.is_success() {
            return producer_status;
        }

        let mut stdin = [0u8; MAX_PROGRAM_STDIN_BYTES];
        let stdin_len = capture.copy_into(&mut stdin);
        let consumer = match self.exec_pipeline_stage(consumer_line, &stdin[..stdin_len]) {
            Ok(ctx) => ctx,
            Err(status) => return status,
        };
        self.run_pending_programs_until(session, consumer, None)
    }

    fn exec_pipeline_stage(
        &self,
        line: &[u8],
        stdin: &[u8],
    ) -> Result<syscall::SyscallContext, ProgramStatus> {
        match parse_program_argv(line) {
            Ok(None) => {
                self.write_line(ShellLineError::InvalidPipe.message());
                Err(ProgramStatus::Error)
            }
            Err(error) => {
                self.write_line(error.message());
                Err(ProgramStatus::Error)
            }
            Ok(Some(argv)) => {
                match syscall::exec_bin_from_shell_argv_with_stdin(
                    self.programs,
                    self.source_store(),
                    argv,
                    stdin,
                ) {
                    Ok(target) => Ok(target),
                    Err(error) => {
                        self.write_exec_load_error(error);
                        Err(ProgramStatus::Error)
                    }
                }
            }
        }
    }

    fn write_exec_load_error(&self, error: crate::exec::ExecLoadError) {
        match error {
            crate::exec::ExecLoadError::InvalidImage => {
                self.write_line("error: invalid /bin program image");
            }
            crate::exec::ExecLoadError::SourceNotFound => {
                self.write_line("error: missing /bin program source");
            }
            crate::exec::ExecLoadError::EmptyArgv0 | crate::exec::ExecLoadError::NotFound => {
                self.write_line("error: unknown /bin program, try `help`");
            }
        }
    }

    fn run_pending_programs_until(
        &self,
        session: &mut RootShellSession,
        target: syscall::SyscallContext,
        target_stdout_capture: Option<&ProgramStdoutCapture>,
    ) -> ProgramStatus {
        let mut steps = 0usize;
        while steps < crate::sched::MAX_KERNEL_TASKS {
            let Some(ctx) = syscall::dispatch_next_ready_program() else {
                self.write_line("error: scheduler did not dispatch program");
                return ProgramStatus::Error;
            };
            let status = match syscall::take_pending_exec(ctx.pid) {
                Some(pending) => {
                    let pending_ctx = syscall::SyscallContext::from_process(pending.handle());
                    let stdout_capture = if ctx.pid == target.pid {
                        target_stdout_capture
                    } else {
                        None
                    };
                    execute_loaded_program_argv(
                        self,
                        session,
                        pending.program(),
                        pending.argv(),
                        pending.stdin(),
                        stdout_capture,
                        Some(pending_ctx),
                    )
                    .status()
                }
                None => {
                    self.write_line("error: scheduler selected process without program image");
                    ProgramStatus::Error
                }
            };
            syscall::exit_current(ctx, status);
            append_exec_log(ctx, status);
            if matches!(status, ProgramStatus::Halt) || ctx.pid == target.pid {
                return status;
            }
            steps += 1;
        }

        self.write_line("error: scheduler dispatch budget exhausted");
        ProgramStatus::Error
    }

    fn run_boot_init(&self, session: &mut RootShellSession) -> ProgramStatus {
        klog::append_line("rootd: start /bin/init");
        let target =
            match syscall::exec_bin_from_shell_argv0(self.programs, self.source_store(), "init") {
                Ok(target) => target,
                Err(error) => {
                    self.write_exec_load_error(error);
                    klog::append_line("init.status=error");
                    return ProgramStatus::Error;
                }
            };
        let status = self.run_pending_programs_until(session, target, None);
        match status {
            ProgramStatus::Ok | ProgramStatus::ExitCode(0) => klog::append_line("init.status=ok"),
            ProgramStatus::Halt => klog::append_line("init.status=halt"),
            _ => klog::append_line("init.status=error"),
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
        for (index, payload) in self.payloads.iter().enumerate() {
            if payload.name == name {
                return Some((index, payload));
            }
        }
        None
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

        run_payload_source_image(payload.source_bytes())
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

fn append_shell_line_log(line: &[u8]) {
    let entered_line = trim_line_end(line);
    klog::append_bytes(b"shell: ");
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
    klog::append_event_with_context("proc", "info", "program-exit", ctx.pid, ctx.task_id);
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
    match init_status {
        ProgramStatus::Ok | ProgramStatus::ExitCode(0) if session.shell_start_requested() => {
            write_boot_status_line(cfg.write, BootCheckState::Ok, b"Started /bin/init.");
            write_boot_status_line(cfg.write, BootCheckState::Ok, b"Reached target root shell.");
        }
        ProgramStatus::Ok | ProgramStatus::ExitCode(0) => {
            write_boot_status_line(
                cfg.write,
                BootCheckState::Warn,
                b"/bin/init did not request root shell.",
            );
            klog::append_line("init.shell_start=missing");
            halt_or_park_after_boot_stop(cfg.halt);
        }
        ProgramStatus::Halt => {
            write_boot_status_line(cfg.write, BootCheckState::Warn, b"/bin/init requested halt.");
            halt_or_park_after_boot_stop(cfg.halt);
        }
        _ => {
            write_boot_status_line(cfg.write, BootCheckState::Warn, b"/bin/init failed.");
            halt_or_park_after_boot_stop(cfg.halt);
        }
    }

    (cfg.write)(b"reovim system kernel shell ready\n");
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
