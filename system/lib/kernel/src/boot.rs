//! OS-mode boot orchestration for system-kernel-only shell entry.
//!
//! The module owns the high-level boot surface for RTOS-like entry:
//! installing runtime services, collecting neutral boot facts and inventory,
//! rendering splash, and launching the kernel root daemon. The composition root
//! supplies only architecture/provider-specific callbacks.

use {
    crate::{
        mm,
        program::{ProgramDescriptor, ProgramSourceArtifact},
        rootd::{
            BootCheckState, BootImageSummary, ConsoleInputStatus, ConsoleInputSummary,
            DmesgSnapshot, HaltKernel, HardwareProbe, PayloadDescriptor, PayloadSourceArtifact,
            PrepareShell, ProfileSummary, ProgramHelpWriter, ReadLine, RootBootConfig,
            RuntimeChecks, VfsFileWriter, WriteFn,
        },
        splash,
    },
    reovim_uapi_system::{BootInfo, DeviceInventory},
};

/// Callback that installs provider/floor runtime pieces and returns optional
/// splash geometry.
pub type InstallRuntimeServices = fn() -> Option<(u32, u32)>;

/// Callback that returns boot facts shaped for upper-system consumption.
pub type BootInfoProvider = fn() -> BootInfo;

/// Callback that returns inventory shaped for upper-system consumption.
pub type DeviceInventoryProvider = fn() -> DeviceInventory;

/// Callback that runs after splash rendering and before checked boot output.
pub type PrepareShellCallback = PrepareShell;

/// Callback that runs lower-provider shell hardware probes.
pub type HardwareProbeCallback = HardwareProbe;

/// Callback that returns a live console-input summary.
pub type ConsoleInputStatusCallback = ConsoleInputStatus;

/// Profile for one shell-capable boot entry.
#[derive(Clone, Copy)]
pub struct BootProfile<'a> {
    /// Shell label surfaced in `boot` output.
    pub name: &'static str,
    /// Whether `launch` is permitted for this boot profile.
    pub launch_enabled: bool,
    /// Payload registry available to the boot profile.
    pub payloads: &'a [PayloadDescriptor],
    /// Payload source artifacts available to the executable loader.
    pub payload_sources: &'static [PayloadSourceArtifact],
    /// `/bin` program registry supplied by the OS image.
    pub programs: &'static [ProgramDescriptor],
    /// `/bin` source artifacts available to the executable loader.
    pub program_sources: &'static [ProgramSourceArtifact],
    /// VFS pseudo-file renderer supplied by the OS image.
    pub vfs_file_writer: VfsFileWriter,
    /// `/bin/help` renderer supplied by the OS image.
    pub program_help_writer: ProgramHelpWriter,
    /// Optional extra diagnostics snapshot provider.
    pub dmesg: Option<DmesgSnapshot>,
    /// Optional halt callback after shell exit.
    pub halt: Option<HaltKernel>,
    /// Prompt text for this profile.
    pub prompt: &'static str,
}

impl<'a> BootProfile<'a> {
    pub const fn new(
        name: &'static str,
        launch_enabled: bool,
        payloads: &'a [PayloadDescriptor],
        payload_sources: &'static [PayloadSourceArtifact],
        programs: &'static [ProgramDescriptor],
        program_sources: &'static [ProgramSourceArtifact],
        vfs_file_writer: VfsFileWriter,
        program_help_writer: ProgramHelpWriter,
        dmesg: Option<DmesgSnapshot>,
        halt: Option<HaltKernel>,
        prompt: &'static str,
    ) -> Self {
        Self {
            name,
            launch_enabled,
            payloads,
            payload_sources,
            programs,
            program_sources,
            vfs_file_writer,
            program_help_writer,
            dmesg,
            halt,
            prompt,
        }
    }
}

/// Shell boot orchestration config.
pub struct ShellBootConfig<'a> {
    /// Runtime install for provider-facing setup and splash geometry.
    pub install_runtime_services: InstallRuntimeServices,
    /// Neutral boot-fact provider.
    pub collect_boot_info: BootInfoProvider,
    /// Neutral device-inventory provider.
    pub collect_device_inventory: DeviceInventoryProvider,
    /// Console read callback.
    pub read_line: ReadLine,
    /// Console write callback.
    pub write: WriteFn,
    /// Optional shell preparation hook after splash rendering.
    pub prepare_shell: Option<PrepareShellCallback>,
    /// Optional lower-provider hardware probe callback for shell diagnostics.
    pub probe_hardware: Option<HardwareProbeCallback>,
    /// Optional live console-input status callback for `/boot/profile`.
    pub console_input_status: Option<ConsoleInputStatusCallback>,
    /// Selected console input source and USB-keyboard readiness.
    pub console_input: ConsoleInputSummary,
    /// Compile-time image identity for root-shell diagnostics.
    pub boot_image: BootImageSummary,
    /// Profile selected for this boot.
    pub profile: BootProfile<'a>,
}

/// Splash-only profile entry config.
#[derive(Clone, Copy)]
pub struct SplashBootConfig {
    /// Runtime install for provider-facing setup and splash geometry.
    pub install_runtime_services: InstallRuntimeServices,
    /// Console write callback.
    pub write: WriteFn,
    /// Optional halt callback after splash.
    pub halt: Option<HaltKernel>,
}

/// Boots services, gathers facts/inventory, and enters the root daemon shell.
pub fn run_shell_profile(cfg: ShellBootConfig<'_>) -> ! {
    let runtime = install_runtime_common(cfg.install_runtime_services);

    crate::rootd::run_root_daemon(RootBootConfig {
        boot_info: (cfg.collect_boot_info)(),
        devices: (cfg.collect_device_inventory)().devices,
        payloads: cfg.profile.payloads,
        payload_sources: cfg.profile.payload_sources,
        programs: cfg.profile.programs,
        program_sources: cfg.profile.program_sources,
        vfs_file_writer: cfg.profile.vfs_file_writer,
        program_help_writer: cfg.profile.program_help_writer,
        dmesg: cfg.profile.dmesg,
        halt: cfg.profile.halt,
        prepare_shell: cfg.prepare_shell,
        probe_hardware: cfg.probe_hardware,
        console_input_status: cfg.console_input_status,
        read_line: cfg.read_line,
        prompt: cfg.profile.prompt,
        write: cfg.write,
        splash_geometry: runtime.splash_geometry,
        boot_image: cfg.boot_image,
        profile: ProfileSummary::new(cfg.profile.name, cfg.profile.launch_enabled),
        runtime_checks: runtime.checks,
        console_input: cfg.console_input,
    })
}

/// Runs splash-only mode with shared runtime install, splash render, and halt.
pub fn run_splash_profile(cfg: SplashBootConfig) -> ! {
    let runtime = install_runtime_common(cfg.install_runtime_services);
    splash::render(runtime.splash_geometry, cfg.write);

    if let Some(halt) = cfg.halt {
        halt();
    }

    loop {
        core::hint::spin_loop();
    }
}

struct RuntimeInstall {
    splash_geometry: Option<(u32, u32)>,
    checks: RuntimeChecks,
}

fn install_runtime_common(install_runtime_services: InstallRuntimeServices) -> RuntimeInstall {
    let alloc_backend = match mm::install_lib_ds_alloc_backend() {
        Ok(()) => BootCheckState::Ok,
        Err(_) => BootCheckState::Warn,
    };
    let sync_backend = match crate::sched::install_lib_ds_sync_backend() {
        Ok(()) => BootCheckState::Ok,
        Err(_) => BootCheckState::Warn,
    };
    RuntimeInstall {
        splash_geometry: install_runtime_services(),
        checks: RuntimeChecks::new(alloc_backend, sync_backend),
    }
}
