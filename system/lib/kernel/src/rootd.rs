//! Root daemon public surface for OS-mode shell execution.
//!
//! This module owns the kernel-only supervisor and payload launch registry.
//! It has no dependency on any provider, product crate, editor, server, or
//! client implementation.

use {
    crate::{
        klog,
        root_shell::{RootShellSession, execute_root_command},
        splash, vfs,
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

/// Callback that attempts to launch a registered payload.
pub type LaunchPayload = fn() -> PayloadLaunchResult;

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
        Self {
            source,
            mode,
            source_state,
            usb_keyboard,
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
    /// One-line payload description shown by `launch`.
    pub summary: &'static str,
    /// Optional launch callback; shells keep this as a registration seam only.
    pub launch: Option<LaunchPayload>,
}

/// Result from a launch callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayloadLaunchResult {
    /// Launch path completed and payload is ready.
    Ready,
    /// Callback not configured.
    NotConfigured,
    /// Callback returned an internal failure.
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
    dmesg: Option<DmesgSnapshot>,
    halt: Option<HaltKernel>,
    probe_hardware: Option<HardwareProbe>,
    console_input_status: Option<ConsoleInputStatus>,
    prompt: &'static str,
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
        dmesg: Option<DmesgSnapshot>,
        halt: Option<HaltKernel>,
        probe_hardware: Option<HardwareProbe>,
        console_input_status: Option<ConsoleInputStatus>,
        prompt: &'static str,
        console_input: ConsoleInputSummary,
        write: WriteFn,
    ) -> Self {
        Self {
            profile,
            boot_info,
            devices,
            payloads,
            dmesg,
            halt,
            probe_hardware,
            console_input_status,
            prompt,
            console_input,
            write,
        }
    }

    /// Returns the profile name selected for this boot.
    #[must_use]
    pub const fn profile_name(&self) -> &'static str {
        self.profile.name
    }

    /// Returns whether this profile allows `launch` callbacks.
    #[must_use]
    pub const fn launch_enabled(&self) -> bool {
        self.profile.launch_enabled
    }

    /// Returns this profile's prompt.
    #[must_use]
    pub const fn prompt(&self) -> &'static str {
        self.prompt
    }

    /// Console input source selected for this boot.
    #[must_use]
    pub fn console_input(&self) -> ConsoleInputSummary {
        if let Some(status) = self.console_input_status {
            return status(self.console_input);
        }
        self.console_input
    }

    /// Runs one shell input line through the root parser and command set.
    ///
    /// Returned `false` means the daemon should remain alive and accept
    /// additional input. `true` means execution reaches the halt path.
    pub fn run_command_line(&self, session: &mut RootShellSession, line: &[u8]) -> bool {
        execute_root_command(self, session, line)
    }

    /// Emits the boot-level command prompt.
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

    /// Snapshot of boot diagnostics for pseudo files and commands.
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

    /// Executes configured payload launch callback by index.
    pub fn launch_payload_by_index(&self, index: usize) -> PayloadLaunchResult {
        if !self.profile.launch_enabled {
            return PayloadLaunchResult::NotConfigured;
        }

        let Some(payload) = self.payloads.get(index) else {
            return PayloadLaunchResult::NotConfigured;
        };
        let Some(launch) = payload.launch else {
            return PayloadLaunchResult::NotConfigured;
        };
        launch()
    }

    /// Executes configured payload launch callback by payload name.
    pub fn launch_payload_by_name(&self, name: &str) -> PayloadLaunchResult {
        if !self.profile.launch_enabled {
            return PayloadLaunchResult::NotConfigured;
        }

        for (index, payload) in self.payloads.iter().enumerate() {
            if payload.name == name {
                return self.launch_payload_by_index(index);
            }
        }
        PayloadLaunchResult::NotConfigured
    }

    /// Payload catalog for command output.
    #[must_use]
    pub const fn payloads(&self) -> &'a [PayloadDescriptor] {
        self.payloads
    }

    /// Triggers the kernel halt callback.
    pub fn halt_kernel(&self) {
        if let Some(halt) = self.halt {
            halt();
        }
    }
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
    write(b"\n");

    klog::append_bytes(b"input=");
    klog::append_bytes(input.source.as_bytes());
    klog::append_bytes(b" mode=");
    klog::append_bytes(input.mode.as_bytes());
    klog::append_bytes(b" usb_keyboard=");
    klog::append_bytes(input_state_word(input.usb_keyboard));
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
    write_boot_status_line(cfg.write, BootCheckState::Ok, b"Reached target root shell.");
}

/// Boots the root daemon shell and never returns.
///
/// The kernel owns this execution point for RTOS mode: splash, shell
/// bootstrap, line loop, and command dispatch, with all policy in one place.
///
/// The `read_line` callback is caller-owned and architecture-specific. The
/// callback must return `0` on EOF or fatal read errors so this loop can exit
/// deterministically in test or host simulation builds.
pub fn run_root_daemon(cfg: RootBootConfig<'_>) -> ! {
    klog::reset();
    klog::append_line("rootd: boot start");
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
        cfg.dmesg,
        cfg.halt,
        cfg.probe_hardware,
        cfg.console_input_status,
        cfg.prompt,
        cfg.console_input,
        cfg.write,
    );

    (cfg.write)(b"reovim system kernel shell ready\n");
    klog::append_line("rootd: shell ready");
    daemon.write_prompt();

    let mut line = [0u8; ROOT_LINE_BYTES];
    let mut session = RootShellSession::new();
    loop {
        let len = (cfg.read_line)(&mut line);
        if len == 0 {
            klog::append_line("rootd: input eof");
            break;
        }

        klog::append_bytes(b"shell: ");
        klog::append_bytes(&line[..len]);
        klog::append_bytes(b"\n");
        if daemon.run_command_line(&mut session, &line[..len]) {
            klog::append_line("rootd: halt requested");
            break;
        }

        daemon.write_prompt();
    }

    if let Some(halt) = cfg.halt {
        klog::append_line("rootd: halt callback");
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
