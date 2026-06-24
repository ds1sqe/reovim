//! Shared kernel-first boot path for RTOS images.
//!
//! This module is now composition-only: the architecture-dependent fact gather and
//! boot-surface registration live here, while orchestration/render/shell ownership
//! is delegated to `reovim-system-kernel`.
#![allow(unsafe_code)]

use reovim_arch::sys as arch_sys;
use reovim_system_kernel::{
    boot::{self, ShellBootConfig, SplashBootConfig},
    console_io,
    rootd::{BootCheckState, BootImageSummary, ConsoleInputSummary, HardwareProbeResult, WriteFn},
};
use reovim_uapi::system::{BootInfo, DeviceEntry, DeviceInventory};
#[cfg(feature = "launch-profile")]
use reovim_system_kernel::rootd::PayloadDescriptor;
#[cfg(feature = "launch-profile")]
use reovim_system_kernel::rootd::PayloadLaunchResult;
#[cfg(feature = "launch-profile")]
use reovim_system_kernel::{
    fs::path_control,
    log::log_sink_control,
    net::net_control,
    panic::panic_control,
    sched::{clock_control, thread_control, thread_spawner},
};

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use {
    core::cell::UnsafeCell,
    core::arch::asm,
    arch_sys::framebuffer,
    reovim_system_kernel::{
        color::Color,
        console::{self, RenderSurface},
        input::{
            BootKeyboardIngest, BootKeyboardInputQueue, BootKeyboardReport, ConsoleInputByte,
            read_boot_keyboard_or_fallback,
        },
        inventory,
    },
};

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use arch_sys::{EMPTY_MEMORY_RANGE, MEMORY_STORAGE_ENTRIES, MemoryRange};
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use core::cell::UnsafeCell;

#[cfg(target_os = "none")]
use core::sync::atomic::{AtomicUsize, Ordering};

/// Public profile type used by app-level image builders.
pub type BootProfile<'a> = boot::BootProfile<'a>;

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
struct FbStore(UnsafeCell<Option<framebuffer::Framebuffer>>);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static FRAMEBUFFER: FbStore = FbStore(UnsafeCell::new(None));

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
unsafe impl Sync for FbStore {}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn fb_put_pixel(_ctx: usize, x: u32, y: u32, color: u32) {
    if let Some(fb) = unsafe { &*FRAMEBUFFER.0.get() } {
        fb.put_pixel(x, y, color);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn fb_clear(_ctx: usize, color: u32) {
    if let Some(fb) = unsafe { &*FRAMEBUFFER.0.get() } {
        fb.clear(color);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn install_framebuffer_console(
    fb: framebuffer::Framebuffer,
    grid: console::ScreenGrid<'static>,
) {
    let width = fb.width();
    let height = fb.height();
    // SAFETY: this store is one-shot and static for the boot image.
    unsafe {
        *FRAMEBUFFER.0.get() = Some(fb);
    }
    let surface = RenderSurface::new(width, height, 0, fb_put_pixel, fb_clear);
    reovim_system_kernel::console::install_console(
        surface,
        &reovim_system_kernel::fonts::JETBRAINS_MONO,
        Color::Rgb(rgb(0xC8, 0xE0, 0xFF)),
        Color::Rgb(rgb(0x0A, 0x14, 0x28)),
        grid,
    );
    arch_sys::install_write_sink(reovim_system_kernel::console::write_bytes);
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
struct DeviceStore(UnsafeCell<[DeviceEntry; inventory::DEVICE_STORAGE_ENTRIES]>);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static DEVICES: DeviceStore = DeviceStore(UnsafeCell::new([inventory::EMPTY_DEVICE_ENTRY; inventory::DEVICE_STORAGE_ENTRIES]));

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
unsafe impl Sync for DeviceStore {}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
struct UsbKeyboardStore(UnsafeCell<UsbKeyboardConsole>);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
struct UsbKeyboardConsole {
    queue: BootKeyboardInputQueue,
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
impl UsbKeyboardConsole {
    const fn new() -> Self {
        Self {
            queue: BootKeyboardInputQueue::new(),
        }
    }

    fn pending_remaining(&self) -> usize {
        self.queue.pending_remaining()
    }

    fn ingest_report(
        &mut self,
        report: [u8; arch_sys::usb::BOOT_KEYBOARD_REPORT_BYTES],
    ) -> BootKeyboardIngest {
        self.queue.try_ingest_report(BootKeyboardReport::new(report))
    }

    fn read_byte<PollReport, Fallback>(
        &mut self,
        mut poll_report: PollReport,
        fallback: Fallback,
    ) -> Option<ConsoleInputByte>
    where
        PollReport: FnMut() -> Option<[u8; arch_sys::usb::BOOT_KEYBOARD_REPORT_BYTES]>,
        Fallback: FnMut() -> Option<u8>,
    {
        read_boot_keyboard_or_fallback(
            &mut self.queue,
            || poll_report().map(BootKeyboardReport::new),
            fallback,
        )
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static USB_KEYBOARD_CONSOLE: UsbKeyboardStore =
    UsbKeyboardStore(UnsafeCell::new(UsbKeyboardConsole::new()));

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
unsafe impl Sync for UsbKeyboardStore {}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static USB_KEYBOARD_PROBE_ENABLED: AtomicUsize = AtomicUsize::new(0);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static USB_KEYBOARD_LAST_POLL_NANOS: AtomicUsize = AtomicUsize::new(0);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static USB_KEYBOARD_READY: AtomicUsize = AtomicUsize::new(0);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static USB_KEYBOARD_LAST_POLL_STATE: AtomicUsize = AtomicUsize::new(USB_KEYBOARD_POLL_NOT_POLLED);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static USB_KEYBOARD_LOGGED_POLL_STATE: AtomicUsize = AtomicUsize::new(USB_KEYBOARD_POLL_LOG_UNSET);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static CONSOLE_LINE_USB_BYTES: AtomicUsize = AtomicUsize::new(0);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static CONSOLE_LINE_FALLBACK_BYTES: AtomicUsize = AtomicUsize::new(0);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_NANOS: usize = 5_000_000;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_INTERVAL_MS: usize = USB_KEYBOARD_POLL_NANOS / 1_000_000;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_LOG_UNSET: usize = usize::MAX;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_NOT_POLLED: usize = 0;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_PROBE_DISABLED: usize = 1;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_DTB_MISSING: usize = 2;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_DECODED_PENDING: usize = 3;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_REPORT_READY: usize = 4;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_CONTROLLER_NOT_READY: usize = 5;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_RESET_IN_PROGRESS: usize = 6;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_HOST_SYSTEM_ERROR: usize = 7;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_NO_PCIE_XHCI: usize = 8;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_BAR_UNCONFIGURED: usize = 9;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_INVALID_CAPS: usize = 10;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_NO_CONNECTED_PORT: usize = 11;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_NEEDS_CONTROLLER_INIT: usize = 12;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_NEEDS_ENUMERATION: usize = 13;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_REPORT_PENDING: usize = 14;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_ENUMERATION_FAILED: usize = 15;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_REPORT_EVENT_MISMATCH: usize = 16;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const USB_KEYBOARD_POLL_REPORT_TRANSFER_FAILED: usize = 17;

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn device_storage() -> &'static mut [DeviceEntry] {
    unsafe { &mut *DEVICES.0.get() }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn dtb_bytes() -> &'static [u8] {
    let ptr = arch_sys::dtb_ptr();
    if ptr == 0 {
        return &[];
    }
    let p = ptr as *const u8;
    // SAFETY: the DTB header is readable at `ptr` and reports its own length.
    let total = unsafe {
        let hdr = core::slice::from_raw_parts(p, 8);
        u32::from_be_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize
    };
    if !(40..=inventory::MAX_DTB_BYTES).contains(&total) {
        return &[];
    }
    // SAFETY: header-provided length is checked above.
    unsafe { core::slice::from_raw_parts(p, total) }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
struct MemoryStore(UnsafeCell<[MemoryRange; MEMORY_STORAGE_ENTRIES]>);

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
static MEMORY: MemoryStore = MemoryStore(UnsafeCell::new([EMPTY_MEMORY_RANGE; MEMORY_STORAGE_ENTRIES]));

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe impl Sync for MemoryStore {}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn memory_storage() -> &'static mut [MemoryRange] {
    unsafe { &mut *MEMORY.0.get() }
}

/// First-stage shell-only profile: no payload launch and no editor import.
pub const fn shell_only_profile() -> BootProfile<'static> {
    BootProfile::new(
        "shell-only",
        false,
        &[],
        None,
        Some(halt_kernel),
        "reovim-os> ",
    )
}

/// Optional launch-capable phase-4 entry profile. Payload callbacks are provided
/// as static descriptors so the root daemon can launch only when enabled by a
/// composition feature.
#[cfg(feature = "launch-profile")]
pub const fn launch_profile() -> BootProfile<'static> {
    BootProfile::new(
        "launch",
        true,
        &[
            PayloadDescriptor {
                name: "reovim",
                summary: "reovim editor-core payload",
                launch: Some(editor_smoke_payload),
            },
            PayloadDescriptor {
                name: "editor-smoke",
                summary: "editor-core smoke boot",
                launch: Some(editor_smoke_payload),
            },
            PayloadDescriptor {
                name: "server-smoke",
                summary: "server-runtime smoke launch",
                launch: Some(server_smoke_payload),
            },
        ],
        None,
        Some(halt_kernel),
        "reovim-os> ",
    )
}

#[cfg(feature = "launch-profile")]
fn editor_launcher_args() -> reovim_editor_core::LauncherArgs {
    reovim_editor_core::LauncherArgs {
        panic: panic_control(),
        log: log_sink_control(),
        clock: clock_control(),
        thread: thread_control(),
        ..reovim_editor_core::LauncherArgs::default()
    }
}

#[cfg(feature = "launch-profile")]
fn editor_smoke_payload() -> PayloadLaunchResult {
    match reovim_editor_core::EditorInit::new(editor_launcher_args()).boot() {
        Ok(_) => PayloadLaunchResult::Ready,
        Err(_) => PayloadLaunchResult::Failed,
    }
}

#[cfg(feature = "launch-profile")]
fn server_smoke_payload() -> PayloadLaunchResult {
    let editor_core = match reovim_editor_core::EditorInit::new(editor_launcher_args()).boot() {
        Ok(core) => core,
        Err(_) => {
            return PayloadLaunchResult::Failed;
        }
    };

    let _ = path_control().unlink(SERVER_SMOKE_SOCKET_PATH);
    match reovim_server_rt::start_listener(&editor_core, SERVER_SMOKE_SOCKET_PATH, net_control(), thread_spawner())
    {
        Ok(()) => PayloadLaunchResult::Ready,
        Err(_) => PayloadLaunchResult::Failed,
    }
}

#[cfg(feature = "launch-profile")]
static SERVER_SMOKE_SOCKET_PATH: &[u8] = b"/tmp/reovim-server-os.sock\0";

/// Boot wrapper that binds this composition root's provider-specific callbacks.
pub fn run_shell_profile(profile: BootProfile<'_>) -> ! {
    run_root_daemon_boot(ShellBootConfig {
        install_runtime_services: install_runtime_services,
        collect_boot_info,
        collect_device_inventory,
        prepare_shell: Some(prepare_shell_boot),
        probe_hardware: Some(hardware_probe),
        console_input_status: Some(console_input_status),
        read_line: tty_read_line,
        write: tty_write,
        console_input: console_input_summary(),
        boot_image: boot_image_summary(profile),
        profile,
    })
}

/// Boots the runtime services and renders the branded kernel splash only.
pub fn run_splash_profile() -> ! {
    run_splash_boot(SplashBootConfig {
        install_runtime_services,
        write: tty_write,
        halt: Some(halt_kernel),
    })
}

fn run_root_daemon_boot(cfg: ShellBootConfig<'_>) -> ! {
    boot::run_shell_profile(cfg)
}

fn run_splash_boot(cfg: SplashBootConfig) -> ! {
    boot::run_splash_profile(cfg)
}

fn install_runtime_services() -> Option<(u32, u32)> {
    #[cfg(not(target_os = "linux"))]
    let _ = reovim_platform_stub_none::install_platform();
    #[cfg(target_os = "linux")]
    let _ = reovim_platform_linux_native::install_platform();

    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    {
        let framebuffer = framebuffer::init();
        let geometry = framebuffer.as_ref().map(|fb| (fb.width(), fb.height()));
        if let Some((fb, grid)) = framebuffer.zip(console::screen_grid()) {
            install_framebuffer_console(fb, grid);
        }
        geometry
    }

    #[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
    {
        None
    }
}

fn collect_boot_info() -> BootInfo {
    #[cfg(target_os = "linux")]
    {
        Default::default()
    }

    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    {
        let cache = arch_sys::cache_geometry();
        reovim_system_kernel::boot_info::collect_boot_info(
            reovim_system_kernel::boot_info::BootFacts {
                memory: arch_sys::discover_memory(),
                cpu_freq_hz: arch_sys::timer_frequency(),
                cpu_id: arch_sys::cpu_id(),
                cpu_count: 1,
                cache_line_bytes: cache.cache_line_bytes,
                l1d_bytes: cache.l1d_bytes,
                l1i_bytes: cache.l1i_bytes,
                l2_bytes: cache.l2_bytes,
                cpu_affinity: arch_sys::cpu_affinity(),
                mem_freq_hz: arch_sys::sdram_clock_hz().unwrap_or(0),
                heap_total_bytes: arch_sys::arena_capacity() as u64,
            },
        )
    }

    #[cfg(all(target_os = "none", target_arch = "x86_64"))]
    {
        // SAFETY: the floor stores the multiboot pointer in process lifetime.
        let memory = unsafe { arch_sys::discover_memory(memory_storage()) };
        reovim_system_kernel::boot_info::collect_boot_info(
            reovim_system_kernel::boot_info::BootFacts {
                memory,
                cpu_id: arch_sys::cpu_id(),
                cpu_count: 1,
                cpu_freq_hz: arch_sys::timer_frequency(),
                ..Default::default()
            },
        )
    }
}

fn collect_device_inventory() -> DeviceInventory {
    #[cfg(target_os = "linux")]
    {
        Default::default()
    }
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    {
        let inventory = inventory::collect_device_inventory(
            dtb_bytes(),
            device_storage(),
            arch_sys::classify_device_compatible,
        );
        configure_usb_keyboard_probe(inventory.devices);
        inventory
    }
    #[cfg(all(target_os = "none", target_arch = "x86_64"))]
    {
        Default::default()
    }
}

fn prepare_shell_boot() {
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    {
        if BOOTLINE_SCRIPT.is_some() {
            run_bootline_countdown_once();
            reovim_system_kernel::console::clear_screen();
        }
    }
}

fn hardware_probe(target: &str, devices: &[DeviceEntry], write: WriteFn) -> HardwareProbeResult {
    match target {
        "help" | "list" => {
            probe_help(write);
            HardwareProbeResult::Handled
        }
        "pcie" => {
            probe_pcie(devices, write);
            HardwareProbeResult::Handled
        }
        "keyboard" | "usb-keyboard" => {
            probe_usb_keyboard(devices, write);
            HardwareProbeResult::Handled
        }
        "xhci-start" | "usb-keyboard-start" => {
            probe_xhci_start(devices, write);
            HardwareProbeResult::Handled
        }
        "xhci-enable-slot" | "usb-keyboard-enable-slot" => {
            probe_xhci_enable_slot(devices, write);
            HardwareProbeResult::Handled
        }
        "xhci-address-device" | "usb-keyboard-address-device" => {
            probe_xhci_address_device(devices, write);
            HardwareProbeResult::Handled
        }
        "xhci-get-device-descriptor" | "usb-keyboard-get-device-descriptor" => {
            probe_xhci_get_device_descriptor(devices, write);
            HardwareProbeResult::Handled
        }
        "xhci-set-address" | "usb-keyboard-set-address" => {
            probe_xhci_set_address(devices, write);
            HardwareProbeResult::Handled
        }
        "xhci-read-device-descriptor" | "usb-keyboard-read-device-descriptor" => {
            probe_xhci_read_device_descriptor(devices, write);
            HardwareProbeResult::Handled
        }
        "xhci-read-config-descriptor-header"
        | "usb-keyboard-read-config-descriptor-header" => {
            probe_xhci_read_config_descriptor_header(devices, write);
            HardwareProbeResult::Handled
        }
        "xhci-read-config-descriptor" | "usb-keyboard-read-config-descriptor" => {
            probe_xhci_read_config_descriptor(devices, write);
            HardwareProbeResult::Handled
        }
        "xhci-set-configuration" | "usb-keyboard-set-configuration" => {
            probe_xhci_set_configuration(devices, write);
            HardwareProbeResult::Handled
        }
        "xhci-configure-endpoint" | "usb-keyboard-configure-endpoint" => {
            probe_xhci_configure_endpoint(devices, write);
            HardwareProbeResult::Handled
        }
        "xhci-set-hid-protocol" | "usb-keyboard-set-hid-protocol" => {
            probe_xhci_set_hid_protocol(devices, write);
            HardwareProbeResult::Handled
        }
        "xhci-read-keyboard-report" | "usb-keyboard-read-report" => {
            probe_xhci_read_keyboard_report(devices, write);
            HardwareProbeResult::Handled
        }
        _ => HardwareProbeResult::UnknownTarget,
    }
}

fn probe_help(write: WriteFn) {
    probe_emit(write, b"probe targets:\n");
    probe_emit(write, b"  pcie\n");
    probe_emit(write, b"  usb-keyboard (alias: keyboard)\n");
    probe_emit(write, b"  xhci-start (alias: usb-keyboard-start)\n");
    probe_emit(write, b"  xhci-enable-slot (alias: usb-keyboard-enable-slot)\n");
    probe_emit(write, b"  xhci-address-device (alias: usb-keyboard-address-device)\n");
    probe_emit(
        write,
        b"  xhci-get-device-descriptor (alias: usb-keyboard-get-device-descriptor)\n",
    );
    probe_emit(write, b"  xhci-set-address (alias: usb-keyboard-set-address)\n");
    probe_emit(
        write,
        b"  xhci-read-device-descriptor (alias: usb-keyboard-read-device-descriptor)\n",
    );
    probe_emit(
        write,
        b"  xhci-read-config-descriptor-header (alias: usb-keyboard-read-config-descriptor-header)\n",
    );
    probe_emit(
        write,
        b"  xhci-read-config-descriptor (alias: usb-keyboard-read-config-descriptor)\n",
    );
    probe_emit(
        write,
        b"  xhci-set-configuration (alias: usb-keyboard-set-configuration)\n",
    );
    probe_emit(
        write,
        b"  xhci-configure-endpoint (alias: usb-keyboard-configure-endpoint)\n",
    );
    probe_emit(
        write,
        b"  xhci-set-hid-protocol (alias: usb-keyboard-set-hid-protocol)\n",
    );
    probe_emit(
        write,
        b"  xhci-read-keyboard-report (alias: usb-keyboard-read-report)\n",
    );
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_pcie(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe pcie:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let status = arch_sys::pcie::read_builtin_pcie_link_status();
    probe_emit(write, b"state=present\n");
    probe_emit(write, b"raw_status=");
    probe_write_u32_hex(write, status.raw_status);
    probe_emit(write, b"\nrevision=");
    probe_write_u32_hex(write, status.revision);
    probe_emit(write, b"\nroot_complex=");
    probe_write_bool(write, status.root_complex_mode);
    probe_emit(write, b"\nphy_link_up=");
    probe_write_bool(write, status.phy_link_up);
    probe_emit(write, b"\ndata_link_active=");
    probe_write_bool(write, status.data_link_active);
    probe_emit(write, b"\nlink_up=");
    probe_write_bool(write, status.link_up());
    probe_emit(write, b"\n");

    if !status.link_up() {
        probe_emit(write, b"xhci=not-probed\n");
        probe_emit(write, b"reason=pcie-link-down\n");
        return;
    }

    match arch_sys::usb::probe_pcie_xhci_controller() {
        Some(controller) => {
            probe_emit(write, b"xhci=present\n");
            probe_emit(write, b"xhci.bus=");
            probe_write_u64_dec(write, controller.location.bus as u64);
            probe_emit(write, b"\nxhci.device=");
            probe_write_u64_dec(write, controller.location.device as u64);
            probe_emit(write, b"\nxhci.function=");
            probe_write_u64_dec(write, controller.location.function as u64);
            probe_emit(write, b"\nxhci.vendor=");
            probe_write_u32_hex(write, controller.vendor_id as u32);
            probe_emit(write, b"\nxhci.device_id=");
            probe_write_u32_hex(write, controller.device_id as u32);
            probe_emit(write, b"\nxhci.revision=");
            probe_write_u64_dec(write, controller.revision_id as u64);
            probe_emit(write, b"\nxhci.mmio=");
            if let Some(mmio) = controller.mmio_base {
                probe_write_u64_hex(write, mmio as u64);
                probe_emit(write, b"\n");
                probe_xhci_mmio(mmio, write);
            } else {
                probe_emit(write, b"unconfigured");
                probe_emit(write, b"\n");
            }
        }
        None => {
            probe_emit(write, b"xhci=not-found\n");
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_mmio(mmio: usize, write: WriteFn) {
    let Some(caps) = arch_sys::usb::read_xhci_capabilities_at_mmio(mmio) else {
        probe_emit(write, b"xhci.capabilities=invalid\n");
        return;
    };

    probe_emit(write, b"xhci.cap_length=");
    probe_write_u64_dec(write, caps.cap_length as u64);
    probe_emit(write, b"\nxhci.hci_version=");
    probe_write_u32_hex(write, caps.hci_version as u32);
    probe_emit(write, b"\nxhci.max_slots=");
    probe_write_u64_dec(write, caps.max_device_slots as u64);
    probe_emit(write, b"\nxhci.max_interrupters=");
    probe_write_u64_dec(write, caps.max_interrupters as u64);
    probe_emit(write, b"\nxhci.max_ports=");
    probe_write_u64_dec(write, caps.max_ports as u64);
    probe_emit(write, b"\nxhci.doorbell_offset=");
    probe_write_u32_hex(write, caps.doorbell_offset);
    probe_emit(write, b"\nxhci.runtime_offset=");
    probe_write_u32_hex(write, caps.runtime_register_space_offset);

    let op = arch_sys::usb::read_xhci_operational_snapshot(mmio, caps);
    probe_emit(write, b"\nxhci.usbcmd=");
    probe_write_u32_hex(write, op.usb_command);
    probe_emit(write, b"\nxhci.usbsts=");
    probe_write_u32_hex(write, op.usb_status);
    probe_emit(write, b"\nxhci.pagesize=");
    probe_write_u32_hex(write, op.page_size);
    probe_emit(write, b"\nxhci.config=");
    probe_write_u32_hex(write, op.configure);
    probe_emit(write, b"\nxhci.enabled_slots=");
    probe_write_u64_dec(write, op.enabled_device_slots as u64);
    probe_emit(write, b"\nxhci.running=");
    probe_write_bool(write, op.run_stop);
    probe_emit(write, b"\nxhci.halted=");
    probe_write_bool(write, op.halted);
    probe_emit(write, b"\nxhci.reset_active=");
    probe_write_bool(write, op.reset_active);
    probe_emit(write, b"\nxhci.controller_not_ready=");
    probe_write_bool(write, op.controller_not_ready);
    probe_emit(write, b"\nxhci.host_system_error=");
    probe_write_bool(write, op.host_system_error);
    probe_emit(write, b"\n");
    probe_xhci_memory_plan(caps, write);

    let max_probe_ports = core::cmp::min(caps.max_ports, 8);
    let mut port = 1u8;
    while port <= max_probe_ports {
        if let Some(snapshot) = arch_sys::usb::read_xhci_port_snapshot(mmio, caps, port) {
            probe_emit(write, b"xhci.port");
            probe_write_u64_dec(write, snapshot.port as u64);
            probe_emit(write, b".portsc=");
            probe_write_u32_hex(write, snapshot.port_status_control);
            probe_emit(write, b" connected=");
            probe_write_bool(write, snapshot.connected);
            probe_emit(write, b" enabled=");
            probe_write_bool(write, snapshot.enabled);
            probe_emit(write, b" powered=");
            probe_write_bool(write, snapshot.powered);
            probe_emit(write, b" speed=");
            probe_write_u64_dec(write, snapshot.speed as u64);
            probe_emit(write, b" link_state=");
            probe_write_u64_dec(write, snapshot.link_state as u64);
            probe_emit(write, b"\n");
        }
        port += 1;
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_memory_plan(caps: arch_sys::usb::XhciCapabilities, write: WriteFn) {
    match arch_sys::usb::xhci_driver_memory_plan(caps) {
        arch_sys::usb::XhciDriverMemoryStatus::Ready(plan) => {
            probe_emit(write, b"xhci.memory=planned\n");
            probe_emit(write, b"xhci.memory.dcbaa=");
            probe_write_u64_hex(write, plan.dcbaa);
            probe_emit(write, b"\nxhci.memory.command_ring=");
            probe_write_u64_hex(write, plan.command_ring);
            probe_emit(write, b"\nxhci.memory.crcr=");
            probe_write_u64_hex(write, plan.command_ring_control);
            probe_emit(write, b"\nxhci.memory.event_ring=");
            probe_write_u64_hex(write, plan.event_ring);
            probe_emit(write, b"\nxhci.memory.control_endpoint_ring=");
            probe_write_u64_hex(write, plan.control_endpoint_ring);
            probe_emit(write, b"\nxhci.memory.interrupt_in_endpoint_ring=");
            probe_write_u64_hex(write, plan.interrupt_in_endpoint_ring);
            probe_emit(write, b"\nxhci.memory.erst=");
            probe_write_u64_hex(write, plan.event_ring_segment_table);
            probe_emit(write, b"\nxhci.memory.erdp=");
            probe_write_u64_hex(write, plan.event_ring_dequeue_pointer);
            probe_emit(write, b"\nxhci.memory.max_slots=");
            probe_write_u64_dec(write, plan.max_slots_enabled as u64);
            probe_emit(write, b"\nxhci.memory.context_size=");
            probe_write_u64_dec(write, plan.context_size_bytes as u64);
            probe_emit(write, b"\nxhci.memory.scratchpads=");
            probe_write_u64_dec(write, plan.scratchpad_buffers as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciDriverMemoryStatus::TooManyScratchpads {
            requested,
            supported,
        } => {
            probe_emit(write, b"xhci.memory=insufficient\n");
            probe_emit(write, b"xhci.memory.scratchpads.requested=");
            probe_write_u64_dec(write, requested as u64);
            probe_emit(write, b"\nxhci.memory.scratchpads.supported=");
            probe_write_u64_dec(write, supported as u64);
            probe_emit(write, b"\n");
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_usb_keyboard(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe usb-keyboard:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        store_usb_keyboard_poll_state(USB_KEYBOARD_POLL_DTB_MISSING);
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        probe_usb_keyboard_manual_next(write, USB_KEYBOARD_POLL_DTB_MISSING);
        return;
    }

    let console = unsafe { &mut *USB_KEYBOARD_CONSOLE.0.get() };
    let pending = console.pending_remaining();
    if pending > 0 {
        store_usb_keyboard_poll_state(USB_KEYBOARD_POLL_DECODED_PENDING);
        probe_emit(write, b"state=decoded-pending\n");
        probe_emit(write, b"decoded_bytes=");
        probe_write_u64_dec(write, pending as u64);
        probe_emit(write, b"\n");
        probe_usb_keyboard_manual_next(write, USB_KEYBOARD_POLL_DECODED_PENDING);
        return;
    }

    match arch_sys::usb::poll_boot_keyboard_report() {
        arch_sys::usb::UsbBootKeyboardPoll::Report(report) => {
            match console.ingest_report(report) {
                BootKeyboardIngest::Decoded { bytes } => {
                    store_usb_keyboard_poll_state(USB_KEYBOARD_POLL_REPORT_READY);
                    if bytes > 0 {
                        mark_usb_keyboard_ready();
                    }
                    probe_emit(write, b"state=report-ready\n");
                    probe_emit(write, b"report_bytes=8\n");
                    probe_emit(write, b"decoded_bytes=");
                    probe_write_u64_dec(write, bytes as u64);
                    probe_emit(write, b"\n");
                    probe_usb_keyboard_manual_next(write, USB_KEYBOARD_POLL_REPORT_READY);
                }
                BootKeyboardIngest::Backlogged { pending_bytes } => {
                    store_usb_keyboard_poll_state(USB_KEYBOARD_POLL_DECODED_PENDING);
                    if pending_bytes > 0 {
                        mark_usb_keyboard_ready();
                    }
                    probe_emit(write, b"state=decoded-pending\n");
                    probe_emit(write, b"decoded_bytes=");
                    probe_write_u64_dec(write, pending_bytes as u64);
                    probe_emit(write, b"\n");
                    probe_usb_keyboard_manual_next(write, USB_KEYBOARD_POLL_DECODED_PENDING);
                }
            }
        }
        arch_sys::usb::UsbBootKeyboardPoll::Pending(pending) => {
            let state = usb_keyboard_poll_state_from_pending(pending);
            store_usb_keyboard_poll_state(state);
            probe_usb_keyboard_pending(write, pending);
            probe_usb_keyboard_manual_next(write, state);
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn usb_keyboard_poll_state_from_pending(pending: arch_sys::usb::UsbBootKeyboardPending) -> usize {
    match pending {
        arch_sys::usb::UsbBootKeyboardPending::ControllerNotReady => {
            USB_KEYBOARD_POLL_CONTROLLER_NOT_READY
        }
        arch_sys::usb::UsbBootKeyboardPending::ControllerResetInProgress => {
            USB_KEYBOARD_POLL_RESET_IN_PROGRESS
        }
        arch_sys::usb::UsbBootKeyboardPending::HostSystemError => USB_KEYBOARD_POLL_HOST_SYSTEM_ERROR,
        arch_sys::usb::UsbBootKeyboardPending::NoPcieXhciController => USB_KEYBOARD_POLL_NO_PCIE_XHCI,
        arch_sys::usb::UsbBootKeyboardPending::ControllerBarUnconfigured => {
            USB_KEYBOARD_POLL_BAR_UNCONFIGURED
        }
        arch_sys::usb::UsbBootKeyboardPending::InvalidXhciCapabilities => {
            USB_KEYBOARD_POLL_INVALID_CAPS
        }
        arch_sys::usb::UsbBootKeyboardPending::NoConnectedRootPort => {
            USB_KEYBOARD_POLL_NO_CONNECTED_PORT
        }
        arch_sys::usb::UsbBootKeyboardPending::NeedsControllerInitialization { .. } => {
            USB_KEYBOARD_POLL_NEEDS_CONTROLLER_INIT
        }
        arch_sys::usb::UsbBootKeyboardPending::NeedsEnumeration { .. } => {
            USB_KEYBOARD_POLL_NEEDS_ENUMERATION
        }
        arch_sys::usb::UsbBootKeyboardPending::ReportPending { .. } => {
            USB_KEYBOARD_POLL_REPORT_PENDING
        }
        arch_sys::usb::UsbBootKeyboardPending::EnumerationFailed => {
            USB_KEYBOARD_POLL_ENUMERATION_FAILED
        }
        arch_sys::usb::UsbBootKeyboardPending::ReportEventMismatch => {
            USB_KEYBOARD_POLL_REPORT_EVENT_MISMATCH
        }
        arch_sys::usb::UsbBootKeyboardPending::ReportTransferFailed { .. } => {
            USB_KEYBOARD_POLL_REPORT_TRANSFER_FAILED
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_usb_keyboard_pending(write: WriteFn, pending: arch_sys::usb::UsbBootKeyboardPending) {
    match pending {
        arch_sys::usb::UsbBootKeyboardPending::ControllerNotReady => {
            probe_emit(write, b"state=unavailable\n");
            probe_emit(write, b"reason=xhci-controller-not-ready\n");
        }
        arch_sys::usb::UsbBootKeyboardPending::ControllerResetInProgress => {
            probe_emit(write, b"state=unavailable\n");
            probe_emit(write, b"reason=xhci-reset-in-progress\n");
        }
        arch_sys::usb::UsbBootKeyboardPending::HostSystemError => {
            probe_emit(write, b"state=unavailable\n");
            probe_emit(write, b"reason=xhci-host-system-error\n");
        }
        arch_sys::usb::UsbBootKeyboardPending::NoPcieXhciController => {
            probe_emit(write, b"state=unavailable\n");
            probe_emit(write, b"reason=no-pcie-xhci-controller\n");
        }
        arch_sys::usb::UsbBootKeyboardPending::ControllerBarUnconfigured => {
            probe_emit(write, b"state=unavailable\n");
            probe_emit(write, b"reason=xhci-bar-unconfigured\n");
        }
        arch_sys::usb::UsbBootKeyboardPending::InvalidXhciCapabilities => {
            probe_emit(write, b"state=unavailable\n");
            probe_emit(write, b"reason=xhci-capabilities-invalid\n");
        }
        arch_sys::usb::UsbBootKeyboardPending::NoConnectedRootPort => {
            probe_emit(write, b"state=unavailable\n");
            probe_emit(write, b"reason=no-connected-root-port\n");
        }
        arch_sys::usb::UsbBootKeyboardPending::NeedsControllerInitialization {
            max_slots,
            port,
            speed,
            link_state,
        } => {
            probe_emit(write, b"state=needs-controller-init\n");
            probe_emit(write, b"max_slots=");
            probe_write_u64_dec(write, max_slots as u64);
            probe_emit(write, b"\nport=");
            probe_write_u64_dec(write, port as u64);
            probe_emit(write, b"\nspeed=");
            probe_write_u64_dec(write, speed as u64);
            probe_emit(write, b"\nlink_state=");
            probe_write_u64_dec(write, link_state as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::UsbBootKeyboardPending::NeedsEnumeration {
            port,
            speed,
            link_state,
        } => {
            probe_emit(write, b"state=needs-enumeration\n");
            probe_emit(write, b"port=");
            probe_write_u64_dec(write, port as u64);
            probe_emit(write, b"\nspeed=");
            probe_write_u64_dec(write, speed as u64);
            probe_emit(write, b"\nlink_state=");
            probe_write_u64_dec(write, link_state as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::UsbBootKeyboardPending::ReportPending {
            slot_id,
            endpoint_id,
            transfer_trb_index,
            event_index,
        } => {
            probe_emit(write, b"state=report-pending\n");
            probe_emit(write, b"slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nendpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\ntransfer_trb_index=");
            probe_write_u64_dec(write, transfer_trb_index as u64);
            probe_emit(write, b"\nevent_index=");
            probe_write_u64_dec(write, event_index as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::UsbBootKeyboardPending::EnumerationFailed => {
            probe_emit(write, b"state=unavailable\n");
            probe_emit(write, b"reason=hid-enumeration-failed\n");
        }
        arch_sys::usb::UsbBootKeyboardPending::ReportEventMismatch => {
            probe_emit(write, b"state=unavailable\n");
            probe_emit(write, b"reason=keyboard-report-event-mismatch\n");
        }
        arch_sys::usb::UsbBootKeyboardPending::ReportTransferFailed {
            completion_code,
            residual_length,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"state=unavailable\n");
            probe_emit(write, b"reason=keyboard-report-transfer-failed\n");
            probe_emit(write, b"completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nresidual_length=");
            probe_write_u64_dec(write, residual_length as u64);
            probe_emit(write, b"\nslot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nendpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_usb_keyboard_manual_next(write: WriteFn, poll_state: usize) {
    probe_emit(write, b"manual_next=");
    probe_emit(write, usb_keyboard_manual_next_from_poll_state(poll_state));
    probe_emit(write, b"\n");
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_start(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-start:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let report = arch_sys::usb::start_pcie_xhci_controller();
    probe_emit(write, b"state=");
    probe_emit(write, probe_xhci_start_status_name(report.status));
    probe_emit(write, b"\n");
    if let arch_sys::usb::XhciControllerStartStatus::DriverMemoryUnavailable {
        requested,
        supported,
    } = report.status
    {
        probe_emit(write, b"scratchpads.requested=");
        probe_write_u64_dec(write, requested as u64);
        probe_emit(write, b"\nscratchpads.supported=");
        probe_write_u64_dec(write, supported as u64);
        probe_emit(write, b"\n");
    }
    if let Some(command) = report.pcie_command_before {
        probe_emit(write, b"pcie.command.before=");
        probe_write_u32_hex(write, command as u32);
        probe_emit(write, b"\n");
    }
    if let Some(command) = report.pcie_command_after {
        probe_emit(write, b"pcie.command.after=");
        probe_write_u32_hex(write, command as u32);
        probe_emit(write, b"\n");
    }
    if let Some(before) = report.before {
        probe_xhci_snapshot(write, b"before", before);
    }
    if let Some(plan) = report.memory {
        probe_xhci_start_memory(write, plan);
    }
    if let Some(registers) = report.registers {
        probe_xhci_start_registers(write, registers);
    }
    if let Some(after) = report.after {
        probe_xhci_snapshot(write, b"after", after);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_start_status_name(
    status: arch_sys::usb::XhciControllerStartStatus,
) -> &'static [u8] {
    match status {
        arch_sys::usb::XhciControllerStartStatus::NoPcieXhciController => {
            b"no-pcie-xhci-controller"
        }
        arch_sys::usb::XhciControllerStartStatus::ControllerBarUnconfigured => {
            b"xhci-bar-unconfigured"
        }
        arch_sys::usb::XhciControllerStartStatus::PciCommandEnableFailed => {
            b"pci-command-enable-failed"
        }
        arch_sys::usb::XhciControllerStartStatus::InvalidXhciCapabilities => {
            b"xhci-capabilities-invalid"
        }
        arch_sys::usb::XhciControllerStartStatus::DriverMemoryUnavailable { .. } => {
            b"driver-memory-unavailable"
        }
        arch_sys::usb::XhciControllerStartStatus::ControllerNotReadyTimedOut => {
            b"controller-not-ready-timeout"
        }
        arch_sys::usb::XhciControllerStartStatus::StopTimedOut => b"stop-timeout",
        arch_sys::usb::XhciControllerStartStatus::ResetTimedOut => b"reset-timeout",
        arch_sys::usb::XhciControllerStartStatus::PostResetControllerNotReadyTimedOut => {
            b"post-reset-controller-not-ready-timeout"
        }
        arch_sys::usb::XhciControllerStartStatus::HostSystemErrorAfterStart => {
            b"host-system-error-after-start"
        }
        arch_sys::usb::XhciControllerStartStatus::StartTimedOut => b"start-timeout",
        arch_sys::usb::XhciControllerStartStatus::Started => b"started",
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_snapshot(
    write: WriteFn,
    label: &[u8],
    snapshot: arch_sys::usb::XhciOperationalSnapshot,
) {
    probe_emit(write, b"xhci.");
    probe_emit(write, label);
    probe_emit(write, b".usbcmd=");
    probe_write_u32_hex(write, snapshot.usb_command);
    probe_emit(write, b"\nxhci.");
    probe_emit(write, label);
    probe_emit(write, b".usbsts=");
    probe_write_u32_hex(write, snapshot.usb_status);
    probe_emit(write, b"\nxhci.");
    probe_emit(write, label);
    probe_emit(write, b".running=");
    probe_write_bool(write, snapshot.run_stop);
    probe_emit(write, b"\nxhci.");
    probe_emit(write, label);
    probe_emit(write, b".halted=");
    probe_write_bool(write, snapshot.halted);
    probe_emit(write, b"\nxhci.");
    probe_emit(write, label);
    probe_emit(write, b".reset_active=");
    probe_write_bool(write, snapshot.reset_active);
    probe_emit(write, b"\nxhci.");
    probe_emit(write, label);
    probe_emit(write, b".controller_not_ready=");
    probe_write_bool(write, snapshot.controller_not_ready);
    probe_emit(write, b"\nxhci.");
    probe_emit(write, label);
    probe_emit(write, b".host_system_error=");
    probe_write_bool(write, snapshot.host_system_error);
    probe_emit(write, b"\nxhci.");
    probe_emit(write, label);
    probe_emit(write, b".enabled_slots=");
    probe_write_u64_dec(write, snapshot.enabled_device_slots as u64);
    probe_emit(write, b"\n");
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_start_memory(write: WriteFn, plan: arch_sys::usb::XhciDriverMemoryPlan) {
    probe_emit(write, b"xhci.start.memory.dcbaa=");
    probe_write_u64_hex(write, plan.dcbaa);
    probe_emit(write, b"\nxhci.start.memory.crcr=");
    probe_write_u64_hex(write, plan.command_ring_control);
    probe_emit(write, b"\nxhci.start.memory.erst=");
    probe_write_u64_hex(write, plan.event_ring_segment_table);
    probe_emit(write, b"\nxhci.start.memory.erdp=");
    probe_write_u64_hex(write, plan.event_ring_dequeue_pointer);
    probe_emit(write, b"\nxhci.start.memory.max_slots=");
    probe_write_u64_dec(write, plan.max_slots_enabled as u64);
    probe_emit(write, b"\n");
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_start_registers(
    write: WriteFn,
    registers: arch_sys::usb::XhciControllerStartRegisters,
) {
    probe_emit(write, b"xhci.start.register.dcbaap=");
    probe_write_u64_hex(write, registers.device_context_base_address_array_pointer);
    probe_emit(write, b"\nxhci.start.register.crcr=");
    probe_write_u64_hex(write, registers.command_ring_control);
    probe_emit(write, b"\nxhci.start.register.config=");
    probe_write_u32_hex(write, registers.configure);
    probe_emit(write, b"\nxhci.start.register.iman=");
    probe_write_u32_hex(write, registers.interrupter_management);
    probe_emit(write, b"\nxhci.start.register.imod=");
    probe_write_u32_hex(write, registers.interrupter_moderation);
    probe_emit(write, b"\nxhci.start.register.erstsz=");
    probe_write_u32_hex(write, registers.event_ring_segment_table_size);
    probe_emit(write, b"\nxhci.start.register.erstba=");
    probe_write_u64_hex(write, registers.event_ring_segment_table_base_address);
    probe_emit(write, b"\nxhci.start.register.erdp=");
    probe_write_u64_hex(write, registers.event_ring_dequeue_pointer);
    probe_emit(write, b"\nxhci.start.register.usbsts_clear=");
    probe_write_u32_hex(write, registers.usb_status_clear);
    probe_emit(write, b"\nxhci.start.register.usbcmd=");
    probe_write_u32_hex(write, registers.usb_command);
    probe_emit(write, b"\n");
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_enable_slot(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-enable-slot:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let report = arch_sys::usb::enable_slot_on_pcie_xhci_controller();
    probe_emit(write, b"state=");
    probe_emit(write, probe_xhci_enable_slot_status_name(report.status));
    probe_emit(write, b"\nstart.state=");
    probe_emit(write, probe_xhci_start_status_name(report.start.status));
    probe_emit(write, b"\ncommand.trb_pointer=");
    probe_write_u64_hex(write, report.command_trb_pointer);
    if let Some(port) = report.connected_port {
        probe_emit(write, b"\nport=");
        probe_write_u64_dec(write, port.port as u64);
        probe_emit(write, b"\nport.speed=");
        probe_write_u64_dec(write, port.speed as u64);
        probe_emit(write, b"\nport.link_state=");
        probe_write_u64_dec(write, port.link_state as u64);
    }
    if let Some(protocol) = report.protocol {
        probe_emit(write, b"\nprotocol.offset=");
        probe_write_u32_hex(write, protocol.offset);
        probe_emit(write, b"\nprotocol.name=");
        probe_emit(write, &protocol.name);
        probe_emit(write, b"\nprotocol.revision_major=");
        probe_write_u64_dec(write, protocol.major_revision as u64);
        probe_emit(write, b"\nprotocol.revision_minor=");
        probe_write_u64_dec(write, protocol.minor_revision as u64);
        probe_emit(write, b"\nprotocol.port_offset=");
        probe_write_u64_dec(write, protocol.compatible_port_offset as u64);
        probe_emit(write, b"\nprotocol.port_count=");
        probe_write_u64_dec(write, protocol.compatible_port_count as u64);
        probe_emit(write, b"\nprotocol.psic=");
        probe_write_u64_dec(write, protocol.protocol_speed_id_count as u64);
        probe_emit(write, b"\nprotocol.slot_type=");
        probe_write_u64_dec(write, protocol.protocol_slot_type as u64);
    }
    probe_emit(write, b"\ncommand.slot_type=");
    probe_write_u64_dec(write, report.slot_type as u64);
    probe_emit(write, b"\ncommand.trb0=");
    probe_write_u32_hex(write, report.command_trb[0]);
    probe_emit(write, b"\ncommand.trb1=");
    probe_write_u32_hex(write, report.command_trb[1]);
    probe_emit(write, b"\ncommand.trb2=");
    probe_write_u32_hex(write, report.command_trb[2]);
    probe_emit(write, b"\ncommand.trb3=");
    probe_write_u32_hex(write, report.command_trb[3]);
    probe_emit(write, b"\ncommand.doorbell=");
    probe_write_u32_hex(write, report.doorbell);
    probe_emit(write, b"\n");

    match report.status {
        arch_sys::usb::XhciEnableSlotStatus::ControllerStartFailed(status) => {
            probe_emit(write, b"start.failure=");
            probe_emit(write, probe_xhci_start_status_name(status));
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciEnableSlotStatus::CommandPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
        } => {
            probe_emit(write, b"event.expected_command_trb_pointer=");
            probe_write_u64_hex(write, expected);
            probe_emit(write, b"\nevent.actual_command_trb_pointer=");
            probe_write_u64_hex(write, actual);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciEnableSlotStatus::UnexpectedEventType {
            trb_type,
            completion_code,
        } => {
            probe_emit(write, b"event.trb_type=");
            probe_write_u64_dec(write, trb_type as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciEnableSlotStatus::CommandFailed {
            completion_code,
            slot_id,
        } => {
            probe_emit(write, b"event.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciEnableSlotStatus::SlotEnabled { slot_id } => {
            probe_emit(write, b"slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciEnableSlotStatus::StartEvidenceUnavailable
        | arch_sys::usb::XhciEnableSlotStatus::ControllerNotRunning
        | arch_sys::usb::XhciEnableSlotStatus::CommandTimedOut => {}
    }

    if let Some(event) = report.event {
        probe_xhci_command_completion_event(write, event);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_enable_slot_status_name(
    status: arch_sys::usb::XhciEnableSlotStatus,
) -> &'static [u8] {
    match status {
        arch_sys::usb::XhciEnableSlotStatus::ControllerStartFailed(_) => {
            b"controller-start-failed"
        }
        arch_sys::usb::XhciEnableSlotStatus::StartEvidenceUnavailable => {
            b"start-evidence-unavailable"
        }
        arch_sys::usb::XhciEnableSlotStatus::ControllerNotRunning => b"controller-not-running",
        arch_sys::usb::XhciEnableSlotStatus::CommandTimedOut => b"command-timeout",
        arch_sys::usb::XhciEnableSlotStatus::UnexpectedEventType { .. } => {
            b"unexpected-event-type"
        }
        arch_sys::usb::XhciEnableSlotStatus::CommandPointerMismatch { .. } => {
            b"command-pointer-mismatch"
        }
        arch_sys::usb::XhciEnableSlotStatus::CommandFailed { .. } => b"command-failed",
        arch_sys::usb::XhciEnableSlotStatus::SlotEnabled { .. } => b"slot-enabled",
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_address_device(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-address-device:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let report = arch_sys::usb::address_device_on_pcie_xhci_controller();
    probe_emit(write, b"state=");
    probe_emit(write, probe_xhci_address_device_status_name(report.status));
    probe_emit(write, b"\nenable.state=");
    probe_emit(write, probe_xhci_enable_slot_status_name(report.enable.status));
    probe_emit(write, b"\nstart.state=");
    probe_emit(write, probe_xhci_start_status_name(report.enable.start.status));
    probe_emit(write, b"\ncommand.trb_pointer=");
    probe_write_u64_hex(write, report.command_trb_pointer);
    probe_emit(write, b"\ncommand.bsr=");
    probe_write_bool(write, report.block_set_address_request);
    probe_emit(write, b"\ncommand.trb0=");
    probe_write_u32_hex(write, report.command_trb[0]);
    probe_emit(write, b"\ncommand.trb1=");
    probe_write_u32_hex(write, report.command_trb[1]);
    probe_emit(write, b"\ncommand.trb2=");
    probe_write_u32_hex(write, report.command_trb[2]);
    probe_emit(write, b"\ncommand.trb3=");
    probe_write_u32_hex(write, report.command_trb[3]);
    probe_emit(write, b"\ncommand.doorbell=");
    probe_write_u32_hex(write, report.doorbell);

    if let Some(contexts) = report.contexts {
        probe_emit(write, b"\ncontext.input=");
        probe_write_u64_hex(write, contexts.input_context);
        probe_emit(write, b"\ncontext.output=");
        probe_write_u64_hex(write, contexts.output_device_context);
        probe_emit(write, b"\ncontext.ep0_ring=");
        probe_write_u64_hex(write, contexts.control_endpoint_ring);
        probe_emit(write, b"\ncontext.drop_flags=");
        probe_write_u32_hex(write, contexts.drop_context_flags);
        probe_emit(write, b"\ncontext.add_flags=");
        probe_write_u32_hex(write, contexts.add_context_flags);
        probe_emit(write, b"\ncontext.slot0=");
        probe_write_u32_hex(write, contexts.slot_context[0]);
        probe_emit(write, b"\ncontext.slot1=");
        probe_write_u32_hex(write, contexts.slot_context[1]);
        probe_emit(write, b"\ncontext.slot2=");
        probe_write_u32_hex(write, contexts.slot_context[2]);
        probe_emit(write, b"\ncontext.slot3=");
        probe_write_u32_hex(write, contexts.slot_context[3]);
        probe_emit(write, b"\ncontext.ep0_0=");
        probe_write_u32_hex(write, contexts.endpoint0_context[0]);
        probe_emit(write, b"\ncontext.ep0_1=");
        probe_write_u32_hex(write, contexts.endpoint0_context[1]);
        probe_emit(write, b"\ncontext.ep0_dequeue_lo=");
        probe_write_u32_hex(write, contexts.endpoint0_context[2]);
        probe_emit(write, b"\ncontext.ep0_dequeue_hi=");
        probe_write_u32_hex(write, contexts.endpoint0_context[3]);
        probe_emit(write, b"\ncontext.ep0_avg_trb_len=");
        probe_write_u64_dec(write, contexts.endpoint0_context[4] as u64);
        probe_emit(write, b"\ncontext.ep0_max_packet_size=");
        probe_write_u64_dec(write, contexts.endpoint0_max_packet_size as u64);
    }
    probe_emit(write, b"\n");

    match report.status {
        arch_sys::usb::XhciAddressDeviceStatus::EnableSlotFailed(status) => {
            probe_emit(write, b"enable.failure=");
            probe_emit(write, probe_xhci_enable_slot_status_name(status));
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciAddressDeviceStatus::SlotIdOutOfRange {
            slot_id,
            max_supported,
        } => {
            probe_emit(write, b"slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nslot_id.max_supported=");
            probe_write_u64_dec(write, max_supported as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciAddressDeviceStatus::UnexpectedEventType {
            trb_type,
            completion_code,
        } => {
            probe_emit(write, b"event.trb_type=");
            probe_write_u64_dec(write, trb_type as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciAddressDeviceStatus::CommandPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
        } => {
            probe_emit(write, b"event.expected_command_trb_pointer=");
            probe_write_u64_hex(write, expected);
            probe_emit(write, b"\nevent.actual_command_trb_pointer=");
            probe_write_u64_hex(write, actual);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciAddressDeviceStatus::SlotIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_slot_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_slot_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciAddressDeviceStatus::CommandFailed {
            completion_code,
            slot_id,
        } => {
            probe_emit(write, b"event.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciAddressDeviceStatus::DefaultControlEndpointReady { slot_id } => {
            probe_emit(write, b"slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciAddressDeviceStatus::NoConnectedRootPort
        | arch_sys::usb::XhciAddressDeviceStatus::StartEvidenceUnavailable
        | arch_sys::usb::XhciAddressDeviceStatus::CommandTimedOut => {}
    }

    if let Some(event) = report.event {
        probe_xhci_command_completion_event(write, event);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_address_device_status_name(
    status: arch_sys::usb::XhciAddressDeviceStatus,
) -> &'static [u8] {
    match status {
        arch_sys::usb::XhciAddressDeviceStatus::EnableSlotFailed(_) => b"enable-slot-failed",
        arch_sys::usb::XhciAddressDeviceStatus::SlotIdOutOfRange { .. } => b"slot-id-out-of-range",
        arch_sys::usb::XhciAddressDeviceStatus::NoConnectedRootPort => b"no-connected-root-port",
        arch_sys::usb::XhciAddressDeviceStatus::StartEvidenceUnavailable => {
            b"start-evidence-unavailable"
        }
        arch_sys::usb::XhciAddressDeviceStatus::CommandTimedOut => b"command-timeout",
        arch_sys::usb::XhciAddressDeviceStatus::UnexpectedEventType { .. } => {
            b"unexpected-event-type"
        }
        arch_sys::usb::XhciAddressDeviceStatus::CommandPointerMismatch { .. } => {
            b"command-pointer-mismatch"
        }
        arch_sys::usb::XhciAddressDeviceStatus::SlotIdMismatch { .. } => b"slot-id-mismatch",
        arch_sys::usb::XhciAddressDeviceStatus::CommandFailed { .. } => b"command-failed",
        arch_sys::usb::XhciAddressDeviceStatus::DefaultControlEndpointReady { .. } => {
            b"default-control-endpoint-ready"
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_get_device_descriptor(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-get-device-descriptor:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let report = arch_sys::usb::get_device_descriptor_prefix_on_pcie_xhci_controller();
    probe_emit(write, b"state=");
    probe_emit(write, probe_xhci_device_descriptor_status_name(report.status));
    probe_emit(write, b"\naddress.state=");
    probe_emit(write, probe_xhci_address_device_status_name(report.address.status));
    probe_emit(write, b"\nenable.state=");
    probe_emit(
        write,
        probe_xhci_enable_slot_status_name(report.address.enable.status),
    );
    probe_emit(write, b"\nstart.state=");
    probe_emit(
        write,
        probe_xhci_start_status_name(report.address.enable.start.status),
    );
    probe_emit(write, b"\nslot_id=");
    probe_write_u64_dec(write, report.slot_id as u64);
    probe_emit(write, b"\nendpoint_id=");
    probe_write_u64_dec(write, report.endpoint_id as u64);
    probe_emit(write, b"\ndoorbell=");
    probe_write_u32_hex(write, report.doorbell);
    probe_emit(write, b"\nsetup.trb_pointer=");
    probe_write_u64_hex(write, report.setup_trb_pointer);
    probe_emit(write, b"\ndata.trb_pointer=");
    probe_write_u64_hex(write, report.data_trb_pointer);
    probe_emit(write, b"\nstatus.trb_pointer=");
    probe_write_u64_hex(write, report.status_trb_pointer);
    probe_xhci_trb(write, b"setup", report.setup_trb);
    probe_xhci_trb(write, b"data", report.data_trb);
    probe_xhci_trb(write, b"status", report.status_trb);
    probe_emit(write, b"descriptor.buffer=");
    probe_write_u64_hex(write, report.descriptor_buffer);
    probe_emit(write, b"\n");

    match report.status {
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::AddressDeviceFailed(status) => {
            probe_emit(write, b"address.failure=");
            probe_emit(write, probe_xhci_address_device_status_name(status));
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::UnexpectedEventType {
            trb_type,
            completion_code,
        } => {
            probe_emit(write, b"event.trb_type=");
            probe_write_u64_dec(write, trb_type as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::TransferPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.expected_trb_pointer=");
            probe_write_u64_hex(write, expected);
            probe_emit(write, b"\nevent.actual_trb_pointer=");
            probe_write_u64_hex(write, actual);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::SlotIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_slot_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_slot_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::EndpointIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_endpoint_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_endpoint_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::TransferFailed {
            completion_code,
            residual_length,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.residual_length=");
            probe_write_u64_dec(write, residual_length as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::DescriptorPrefixReady {
            length,
            ..
        } => {
            probe_emit(write, b"descriptor.length=");
            probe_write_u64_dec(write, length as u64);
            probe_emit(write, b"\n");
            probe_descriptor_bytes(write, &report.descriptor);
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::StartEvidenceUnavailable
        | arch_sys::usb::XhciDeviceDescriptorProbeStatus::TransferTimedOut => {}
    }

    if let Some(event) = report.event {
        probe_xhci_transfer_event(write, event);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_device_descriptor_status_name(
    status: arch_sys::usb::XhciDeviceDescriptorProbeStatus,
) -> &'static [u8] {
    match status {
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::AddressDeviceFailed(_) => {
            b"address-device-failed"
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::StartEvidenceUnavailable => {
            b"start-evidence-unavailable"
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::TransferTimedOut => b"transfer-timeout",
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::UnexpectedEventType { .. } => {
            b"unexpected-event-type"
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::TransferPointerMismatch { .. } => {
            b"transfer-pointer-mismatch"
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::SlotIdMismatch { .. } => {
            b"slot-id-mismatch"
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::EndpointIdMismatch { .. } => {
            b"endpoint-id-mismatch"
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::TransferFailed { .. } => {
            b"transfer-failed"
        }
        arch_sys::usb::XhciDeviceDescriptorProbeStatus::DescriptorPrefixReady { .. } => {
            b"descriptor-prefix-ready"
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_set_address(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-set-address:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let report = arch_sys::usb::set_address_on_pcie_xhci_controller();
    probe_emit(write, b"state=");
    probe_emit(write, probe_xhci_set_address_status_name(report.status));
    probe_emit(write, b"\ndescriptor.state=");
    probe_emit(
        write,
        probe_xhci_device_descriptor_status_name(report.descriptor.status),
    );
    probe_emit(write, b"\naddress.state=");
    probe_emit(
        write,
        probe_xhci_address_device_status_name(report.descriptor.address.status),
    );
    probe_emit(write, b"\nenable.state=");
    probe_emit(
        write,
        probe_xhci_enable_slot_status_name(report.descriptor.address.enable.status),
    );
    probe_emit(write, b"\nstart.state=");
    probe_emit(
        write,
        probe_xhci_start_status_name(report.descriptor.address.enable.start.status),
    );
    probe_emit(write, b"\ncommand.trb_pointer=");
    probe_write_u64_hex(write, report.command_trb_pointer);
    probe_emit(write, b"\ncommand.bsr=false");
    probe_emit(write, b"\ncommand.trb0=");
    probe_write_u32_hex(write, report.command_trb[0]);
    probe_emit(write, b"\ncommand.trb1=");
    probe_write_u32_hex(write, report.command_trb[1]);
    probe_emit(write, b"\ncommand.trb2=");
    probe_write_u32_hex(write, report.command_trb[2]);
    probe_emit(write, b"\ncommand.trb3=");
    probe_write_u32_hex(write, report.command_trb[3]);
    probe_emit(write, b"\ncommand.doorbell=");
    probe_write_u32_hex(write, report.doorbell);
    probe_emit(write, b"\nep0.max_packet_size=");
    probe_write_u64_dec(write, report.endpoint0_max_packet_size as u64);

    if let Some(contexts) = report.contexts {
        probe_emit(write, b"\ncontext.input=");
        probe_write_u64_hex(write, contexts.input_context);
        probe_emit(write, b"\ncontext.output=");
        probe_write_u64_hex(write, contexts.output_device_context);
        probe_emit(write, b"\ncontext.ep0_ring=");
        probe_write_u64_hex(write, contexts.control_endpoint_ring);
        probe_emit(write, b"\ncontext.add_flags=");
        probe_write_u32_hex(write, contexts.add_context_flags);
        probe_emit(write, b"\ncontext.slot0=");
        probe_write_u32_hex(write, contexts.slot_context[0]);
        probe_emit(write, b"\ncontext.slot1=");
        probe_write_u32_hex(write, contexts.slot_context[1]);
        probe_emit(write, b"\ncontext.ep0_1=");
        probe_write_u32_hex(write, contexts.endpoint0_context[1]);
        probe_emit(write, b"\ncontext.ep0_dequeue_lo=");
        probe_write_u32_hex(write, contexts.endpoint0_context[2]);
        probe_emit(write, b"\ncontext.ep0_dequeue_hi=");
        probe_write_u32_hex(write, contexts.endpoint0_context[3]);
    }
    probe_emit(write, b"\n");

    match report.status {
        arch_sys::usb::XhciSetAddressStatus::DescriptorPrefixFailed(status) => {
            probe_emit(write, b"descriptor.failure=");
            probe_emit(write, probe_xhci_device_descriptor_status_name(status));
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetAddressStatus::InvalidDescriptorPrefix {
            length,
            descriptor_type,
            max_packet_size0,
            bcd_usb,
        } => {
            probe_emit(write, b"descriptor.length=");
            probe_write_u64_dec(write, length as u64);
            probe_emit(write, b"\ndescriptor.type=");
            probe_write_u64_dec(write, descriptor_type as u64);
            probe_emit(write, b"\ndescriptor.max_packet_size0=");
            probe_write_u64_dec(write, max_packet_size0 as u64);
            probe_emit(write, b"\ndescriptor.bcd_usb=");
            probe_write_u32_hex(write, bcd_usb as u32);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetAddressStatus::UnexpectedEventType {
            trb_type,
            completion_code,
        } => {
            probe_emit(write, b"event.trb_type=");
            probe_write_u64_dec(write, trb_type as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetAddressStatus::CommandPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
        } => {
            probe_emit(write, b"event.expected_command_trb_pointer=");
            probe_write_u64_hex(write, expected);
            probe_emit(write, b"\nevent.actual_command_trb_pointer=");
            probe_write_u64_hex(write, actual);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetAddressStatus::SlotIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_slot_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_slot_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetAddressStatus::CommandFailed {
            completion_code,
            slot_id,
        } => {
            probe_emit(write, b"event.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetAddressStatus::Addressed {
            slot_id,
            endpoint0_max_packet_size,
        } => {
            probe_emit(write, b"slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nep0.addressed_max_packet_size=");
            probe_write_u64_dec(write, endpoint0_max_packet_size as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetAddressStatus::StartEvidenceUnavailable
        | arch_sys::usb::XhciSetAddressStatus::CommandTimedOut => {}
    }

    if let Some(event) = report.event {
        probe_xhci_command_completion_event(write, event);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_read_device_descriptor(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-read-device-descriptor:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let report = arch_sys::usb::read_device_descriptor_on_pcie_xhci_controller();
    probe_emit(write, b"state=");
    probe_emit(
        write,
        probe_xhci_read_device_descriptor_status_name(report.status),
    );
    probe_emit(write, b"\nset_address.state=");
    probe_emit(
        write,
        probe_xhci_set_address_status_name(report.set_address.status),
    );
    probe_emit(write, b"\ndescriptor_prefix.state=");
    probe_emit(
        write,
        probe_xhci_device_descriptor_status_name(report.set_address.descriptor.status),
    );
    probe_emit(write, b"\naddress.state=");
    probe_emit(
        write,
        probe_xhci_address_device_status_name(report.set_address.descriptor.address.status),
    );
    probe_emit(write, b"\nenable.state=");
    probe_emit(
        write,
        probe_xhci_enable_slot_status_name(report.set_address.descriptor.address.enable.status),
    );
    probe_emit(write, b"\nstart.state=");
    probe_emit(
        write,
        probe_xhci_start_status_name(report.set_address.descriptor.address.enable.start.status),
    );
    probe_emit(write, b"\nslot_id=");
    probe_write_u64_dec(write, report.slot_id as u64);
    probe_emit(write, b"\nendpoint_id=");
    probe_write_u64_dec(write, report.endpoint_id as u64);
    probe_emit(write, b"\ndoorbell=");
    probe_write_u32_hex(write, report.doorbell);
    probe_emit(write, b"\nsetup.trb_pointer=");
    probe_write_u64_hex(write, report.setup_trb_pointer);
    probe_emit(write, b"\ndata.trb_pointer=");
    probe_write_u64_hex(write, report.data_trb_pointer);
    probe_emit(write, b"\nstatus.trb_pointer=");
    probe_write_u64_hex(write, report.status_trb_pointer);
    probe_xhci_trb(write, b"setup", report.setup_trb);
    probe_xhci_trb(write, b"data", report.data_trb);
    probe_xhci_trb(write, b"status", report.status_trb);
    probe_emit(write, b"descriptor.buffer=");
    probe_write_u64_hex(write, report.descriptor_buffer);
    probe_emit(write, b"\n");

    match report.status {
        arch_sys::usb::XhciReadDeviceDescriptorStatus::SetAddressFailed(status) => {
            probe_emit(write, b"set_address.failure=");
            probe_emit(write, probe_xhci_set_address_status_name(status));
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::UnexpectedEventType {
            trb_type,
            completion_code,
        } => {
            probe_emit(write, b"event.trb_type=");
            probe_write_u64_dec(write, trb_type as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::TransferPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.expected_trb_pointer=");
            probe_write_u64_hex(write, expected);
            probe_emit(write, b"\nevent.actual_trb_pointer=");
            probe_write_u64_hex(write, actual);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::SlotIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_slot_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_slot_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::EndpointIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_endpoint_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_endpoint_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::TransferFailed {
            completion_code,
            residual_length,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.residual_length=");
            probe_write_u64_dec(write, residual_length as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::InvalidDeviceDescriptor {
            length,
            descriptor_type,
        } => {
            probe_emit(write, b"descriptor.length=");
            probe_write_u64_dec(write, length as u64);
            probe_emit(write, b"\ndescriptor.type=");
            probe_write_u64_dec(write, descriptor_type as u64);
            probe_emit(write, b"\n");
            probe_descriptor_bytes(write, &report.descriptor);
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::DeviceDescriptorReady { .. } => {
            if let Some(fields) = report.fields {
                probe_emit(write, b"descriptor.length=");
                probe_write_u64_dec(write, fields.length as u64);
                probe_emit(write, b"\ndescriptor.type=");
                probe_write_u64_dec(write, fields.descriptor_type as u64);
                probe_emit(write, b"\ndescriptor.bcd_usb=");
                probe_write_u32_hex(write, fields.bcd_usb as u32);
                probe_emit(write, b"\ndescriptor.class=");
                probe_write_u64_dec(write, fields.device_class as u64);
                probe_emit(write, b"\ndescriptor.subclass=");
                probe_write_u64_dec(write, fields.device_subclass as u64);
                probe_emit(write, b"\ndescriptor.protocol=");
                probe_write_u64_dec(write, fields.device_protocol as u64);
                probe_emit(write, b"\ndescriptor.max_packet_size0=");
                probe_write_u64_dec(write, fields.max_packet_size0 as u64);
                probe_emit(write, b"\ndescriptor.vendor=");
                probe_write_u32_hex(write, fields.vendor_id as u32);
                probe_emit(write, b"\ndescriptor.product=");
                probe_write_u32_hex(write, fields.product_id as u32);
                probe_emit(write, b"\ndescriptor.bcd_device=");
                probe_write_u32_hex(write, fields.bcd_device as u32);
                probe_emit(write, b"\ndescriptor.manufacturer_index=");
                probe_write_u64_dec(write, fields.manufacturer_index as u64);
                probe_emit(write, b"\ndescriptor.product_index=");
                probe_write_u64_dec(write, fields.product_index as u64);
                probe_emit(write, b"\ndescriptor.serial_number_index=");
                probe_write_u64_dec(write, fields.serial_number_index as u64);
                probe_emit(write, b"\ndescriptor.num_configurations=");
                probe_write_u64_dec(write, fields.num_configurations as u64);
                probe_emit(write, b"\n");
            }
            probe_descriptor_bytes(write, &report.descriptor);
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::StartEvidenceUnavailable
        | arch_sys::usb::XhciReadDeviceDescriptorStatus::TransferTimedOut => {}
    }

    if let Some(event) = report.event {
        probe_xhci_transfer_event(write, event);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_read_config_descriptor_header(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-read-config-descriptor-header:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let report = arch_sys::usb::read_configuration_descriptor_header_on_pcie_xhci_controller();
    probe_emit(write, b"state=");
    probe_emit(
        write,
        probe_xhci_read_config_descriptor_header_status_name(report.status),
    );
    probe_emit(write, b"\ndevice_descriptor.state=");
    probe_emit(
        write,
        probe_xhci_read_device_descriptor_status_name(report.device_descriptor.status),
    );
    probe_emit(write, b"\nset_address.state=");
    probe_emit(
        write,
        probe_xhci_set_address_status_name(report.device_descriptor.set_address.status),
    );
    probe_emit(write, b"\ndescriptor_prefix.state=");
    probe_emit(
        write,
        probe_xhci_device_descriptor_status_name(
            report.device_descriptor.set_address.descriptor.status,
        ),
    );
    probe_emit(write, b"\naddress.state=");
    probe_emit(
        write,
        probe_xhci_address_device_status_name(
            report
                .device_descriptor
                .set_address
                .descriptor
                .address
                .status,
        ),
    );
    probe_emit(write, b"\nenable.state=");
    probe_emit(
        write,
        probe_xhci_enable_slot_status_name(
            report
                .device_descriptor
                .set_address
                .descriptor
                .address
                .enable
                .status,
        ),
    );
    probe_emit(write, b"\nstart.state=");
    probe_emit(
        write,
        probe_xhci_start_status_name(
            report
                .device_descriptor
                .set_address
                .descriptor
                .address
                .enable
                .start
                .status,
        ),
    );
    probe_emit(write, b"\nslot_id=");
    probe_write_u64_dec(write, report.slot_id as u64);
    probe_emit(write, b"\nendpoint_id=");
    probe_write_u64_dec(write, report.endpoint_id as u64);
    probe_emit(write, b"\ndoorbell=");
    probe_write_u32_hex(write, report.doorbell);
    probe_emit(write, b"\nsetup.trb_pointer=");
    probe_write_u64_hex(write, report.setup_trb_pointer);
    probe_emit(write, b"\ndata.trb_pointer=");
    probe_write_u64_hex(write, report.data_trb_pointer);
    probe_emit(write, b"\nstatus.trb_pointer=");
    probe_write_u64_hex(write, report.status_trb_pointer);
    probe_xhci_trb(write, b"setup", report.setup_trb);
    probe_xhci_trb(write, b"data", report.data_trb);
    probe_xhci_trb(write, b"status", report.status_trb);
    probe_emit(write, b"descriptor.buffer=");
    probe_write_u64_hex(write, report.descriptor_buffer);
    probe_emit(write, b"\n");

    match report.status {
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::DeviceDescriptorFailed(
            status,
        ) => {
            probe_emit(write, b"device_descriptor.failure=");
            probe_emit(write, probe_xhci_read_device_descriptor_status_name(status));
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::UnexpectedEventType {
            trb_type,
            completion_code,
        } => {
            probe_emit(write, b"event.trb_type=");
            probe_write_u64_dec(write, trb_type as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::TransferPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.expected_trb_pointer=");
            probe_write_u64_hex(write, expected);
            probe_emit(write, b"\nevent.actual_trb_pointer=");
            probe_write_u64_hex(write, actual);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::SlotIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_slot_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_slot_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::EndpointIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_endpoint_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_endpoint_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::TransferFailed {
            completion_code,
            residual_length,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.residual_length=");
            probe_write_u64_dec(write, residual_length as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::InvalidConfigurationDescriptorHeader {
            length,
            descriptor_type,
            total_length,
        } => {
            probe_emit(write, b"descriptor.length=");
            probe_write_u64_dec(write, length as u64);
            probe_emit(write, b"\ndescriptor.type=");
            probe_write_u64_dec(write, descriptor_type as u64);
            probe_emit(write, b"\ndescriptor.total_length=");
            probe_write_u64_dec(write, total_length as u64);
            probe_emit(write, b"\n");
            probe_descriptor_bytes(write, &report.descriptor);
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::ConfigurationDescriptorHeaderReady { .. } => {
            if let Some(fields) = report.fields {
                probe_emit(write, b"descriptor.length=");
                probe_write_u64_dec(write, fields.length as u64);
                probe_emit(write, b"\ndescriptor.type=");
                probe_write_u64_dec(write, fields.descriptor_type as u64);
                probe_emit(write, b"\ndescriptor.total_length=");
                probe_write_u64_dec(write, fields.total_length as u64);
                probe_emit(write, b"\ndescriptor.num_interfaces=");
                probe_write_u64_dec(write, fields.num_interfaces as u64);
                probe_emit(write, b"\ndescriptor.configuration_value=");
                probe_write_u64_dec(write, fields.configuration_value as u64);
                probe_emit(write, b"\ndescriptor.configuration_index=");
                probe_write_u64_dec(write, fields.configuration_index as u64);
                probe_emit(write, b"\ndescriptor.attributes=");
                probe_write_u32_hex(write, fields.attributes as u32);
                probe_emit(write, b"\ndescriptor.max_power_2ma=");
                probe_write_u64_dec(write, fields.max_power as u64);
                probe_emit(write, b"\n");
            }
            probe_descriptor_bytes(write, &report.descriptor);
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::StartEvidenceUnavailable
        | arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::TransferTimedOut => {}
    }

    if let Some(event) = report.event {
        probe_xhci_transfer_event(write, event);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_read_config_descriptor(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-read-config-descriptor:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let report = arch_sys::usb::read_configuration_descriptor_on_pcie_xhci_controller();
    probe_emit(write, b"state=");
    probe_emit(write, probe_xhci_read_config_descriptor_status_name(report.status));
    probe_emit(write, b"\nconfig_header.state=");
    probe_emit(
        write,
        probe_xhci_read_config_descriptor_header_status_name(report.header.status),
    );
    probe_emit(write, b"\ndevice_descriptor.state=");
    probe_emit(
        write,
        probe_xhci_read_device_descriptor_status_name(report.header.device_descriptor.status),
    );
    probe_emit(write, b"\nset_address.state=");
    probe_emit(
        write,
        probe_xhci_set_address_status_name(report.header.device_descriptor.set_address.status),
    );
    probe_emit(write, b"\nslot_id=");
    probe_write_u64_dec(write, report.slot_id as u64);
    probe_emit(write, b"\nendpoint_id=");
    probe_write_u64_dec(write, report.endpoint_id as u64);
    probe_emit(write, b"\ndoorbell=");
    probe_write_u32_hex(write, report.doorbell);
    probe_emit(write, b"\nsetup.trb_pointer=");
    probe_write_u64_hex(write, report.setup_trb_pointer);
    probe_emit(write, b"\ndata.trb_pointer=");
    probe_write_u64_hex(write, report.data_trb_pointer);
    probe_emit(write, b"\nstatus.trb_pointer=");
    probe_write_u64_hex(write, report.status_trb_pointer);
    probe_xhci_trb(write, b"setup", report.setup_trb);
    probe_xhci_trb(write, b"data", report.data_trb);
    probe_xhci_trb(write, b"status", report.status_trb);
    probe_emit(write, b"descriptor.buffer=");
    probe_write_u64_hex(write, report.descriptor_buffer);
    probe_emit(write, b"\ndescriptor.bytes=");
    probe_write_u64_dec(write, report.descriptor_length as u64);
    probe_emit(write, b"\n");

    match report.status {
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::HeaderFailed(status) => {
            probe_emit(write, b"config_header.failure=");
            probe_emit(write, probe_xhci_read_config_descriptor_header_status_name(status));
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::ConfigurationTooLarge {
            total_length,
            max_supported,
        } => {
            probe_emit(write, b"descriptor.total_length=");
            probe_write_u64_dec(write, total_length as u64);
            probe_emit(write, b"\ndescriptor.max_supported=");
            probe_write_u64_dec(write, max_supported as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::UnexpectedEventType {
            trb_type,
            completion_code,
        } => {
            probe_emit(write, b"event.trb_type=");
            probe_write_u64_dec(write, trb_type as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::TransferPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.expected_trb_pointer=");
            probe_write_u64_hex(write, expected);
            probe_emit(write, b"\nevent.actual_trb_pointer=");
            probe_write_u64_hex(write, actual);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::SlotIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_slot_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_slot_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::EndpointIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_endpoint_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_endpoint_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::TransferFailed {
            completion_code,
            residual_length,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.residual_length=");
            probe_write_u64_dec(write, residual_length as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::InvalidConfigurationDescriptor {
            offset,
            length,
            descriptor_type,
        } => {
            probe_emit(write, b"descriptor.invalid_offset=");
            probe_write_u64_dec(write, offset as u64);
            probe_emit(write, b"\ndescriptor.invalid_length=");
            probe_write_u64_dec(write, length as u64);
            probe_emit(write, b"\ndescriptor.invalid_type=");
            probe_write_u64_dec(write, descriptor_type as u64);
            probe_emit(write, b"\n");
            probe_descriptor_bytes(write, &report.descriptor[..report.descriptor_length as usize]);
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::ConfigurationDescriptorReady {
            boot_keyboard_ready_to_configure,
            ..
        } => {
            if let Some(fields) = report.fields {
                probe_emit(write, b"descriptor.total_length=");
                probe_write_u64_dec(write, fields.header.total_length as u64);
                probe_emit(write, b"\ndescriptor.num_interfaces=");
                probe_write_u64_dec(write, fields.header.num_interfaces as u64);
                probe_emit(write, b"\ndescriptor.configuration_value=");
                probe_write_u64_dec(write, fields.header.configuration_value as u64);
                probe_emit(write, b"\ndescriptor.count=");
                probe_write_u64_dec(write, fields.descriptor_count as u64);
                probe_emit(write, b"\nboot_keyboard.ready_to_configure=");
                probe_write_bool(write, boot_keyboard_ready_to_configure);
                if let Some(keyboard) = fields.boot_keyboard {
                    probe_emit(write, b"\nboot_keyboard.interface=");
                    probe_write_u64_dec(write, keyboard.interface_number as u64);
                    probe_emit(write, b"\nboot_keyboard.alternate=");
                    probe_write_u64_dec(write, keyboard.alternate_setting as u64);
                    probe_emit(write, b"\nboot_keyboard.endpoint_count=");
                    probe_write_u64_dec(write, keyboard.endpoint_count as u64);
                    probe_emit(write, b"\nboot_keyboard.protocol=");
                    probe_write_u64_dec(write, keyboard.protocol as u64);
                    if let Some(endpoint) = keyboard.interrupt_in_endpoint {
                        probe_emit(write, b"\nboot_keyboard.endpoint.address=");
                        probe_write_u32_hex(write, endpoint.address as u32);
                        probe_emit(write, b"\nboot_keyboard.endpoint.number=");
                        probe_write_u64_dec(write, endpoint.endpoint_number as u64);
                        probe_emit(write, b"\nboot_keyboard.endpoint.direction_in=");
                        probe_write_bool(write, endpoint.direction_in);
                        probe_emit(write, b"\nboot_keyboard.endpoint.attributes=");
                        probe_write_u32_hex(write, endpoint.attributes as u32);
                        probe_emit(write, b"\nboot_keyboard.endpoint.transfer_type=");
                        probe_write_u64_dec(write, endpoint.transfer_type as u64);
                        probe_emit(write, b"\nboot_keyboard.endpoint.max_packet_size=");
                        probe_write_u64_dec(write, endpoint.max_packet_size as u64);
                        probe_emit(write, b"\nboot_keyboard.endpoint.interval=");
                        probe_write_u64_dec(write, endpoint.interval as u64);
                    }
                }
                probe_emit(write, b"\n");
            }
            probe_descriptor_bytes(write, &report.descriptor[..report.descriptor_length as usize]);
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::StartEvidenceUnavailable
        | arch_sys::usb::XhciReadConfigurationDescriptorStatus::TransferTimedOut => {}
    }

    if let Some(event) = report.event {
        probe_xhci_transfer_event(write, event);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_set_configuration(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-set-configuration:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let report = arch_sys::usb::set_configuration_on_pcie_xhci_controller();
    probe_emit(write, b"state=");
    probe_emit(write, probe_xhci_set_configuration_status_name(report.status));
    probe_emit(write, b"\nconfig_descriptor.state=");
    probe_emit(
        write,
        probe_xhci_read_config_descriptor_status_name(report.configuration.status),
    );
    probe_emit(write, b"\nconfig_header.state=");
    probe_emit(
        write,
        probe_xhci_read_config_descriptor_header_status_name(report.configuration.header.status),
    );
    probe_emit(write, b"\ndevice_descriptor.state=");
    probe_emit(
        write,
        probe_xhci_read_device_descriptor_status_name(
            report.configuration.header.device_descriptor.status,
        ),
    );
    probe_emit(write, b"\nset_address.state=");
    probe_emit(
        write,
        probe_xhci_set_address_status_name(
            report
                .configuration
                .header
                .device_descriptor
                .set_address
                .status,
        ),
    );
    probe_emit(write, b"\nslot_id=");
    probe_write_u64_dec(write, report.slot_id as u64);
    probe_emit(write, b"\nendpoint_id=");
    probe_write_u64_dec(write, report.endpoint_id as u64);
    probe_emit(write, b"\ndoorbell=");
    probe_write_u32_hex(write, report.doorbell);
    probe_emit(write, b"\nconfiguration.value=");
    probe_write_u64_dec(write, report.configuration_value as u64);
    probe_emit(write, b"\nboot_keyboard.interface=");
    probe_write_u64_dec(write, report.interface_number as u64);
    probe_emit(write, b"\nboot_keyboard.endpoint.address=");
    probe_write_u32_hex(write, report.endpoint_address as u32);
    probe_emit(write, b"\nsetup.trb_pointer=");
    probe_write_u64_hex(write, report.setup_trb_pointer);
    probe_emit(write, b"\nstatus.trb_pointer=");
    probe_write_u64_hex(write, report.status_trb_pointer);
    probe_emit(write, b"\n");
    probe_xhci_trb(write, b"setup", report.setup_trb);
    probe_xhci_trb(write, b"status", report.status_trb);

    match report.status {
        arch_sys::usb::XhciSetConfigurationStatus::ConfigurationDescriptorFailed(status) => {
            probe_emit(write, b"config_descriptor.failure=");
            probe_emit(write, probe_xhci_read_config_descriptor_status_name(status));
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetConfigurationStatus::BootKeyboardNotReadyToConfigure => {
            probe_emit(write, b"boot_keyboard.ready_to_configure=false\n");
        }
        arch_sys::usb::XhciSetConfigurationStatus::UnexpectedEventType {
            trb_type,
            completion_code,
        } => {
            probe_emit(write, b"event.trb_type=");
            probe_write_u64_dec(write, trb_type as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetConfigurationStatus::TransferPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.expected_trb_pointer=");
            probe_write_u64_hex(write, expected);
            probe_emit(write, b"\nevent.actual_trb_pointer=");
            probe_write_u64_hex(write, actual);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetConfigurationStatus::SlotIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_slot_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_slot_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetConfigurationStatus::EndpointIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_endpoint_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_endpoint_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetConfigurationStatus::TransferFailed {
            completion_code,
            residual_length,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.residual_length=");
            probe_write_u64_dec(write, residual_length as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetConfigurationStatus::ConfigurationSet {
            slot_id,
            configuration_value,
            interface_number,
            endpoint_address,
        } => {
            probe_emit(write, b"configuration.set=true\nconfiguration.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nconfiguration.value=");
            probe_write_u64_dec(write, configuration_value as u64);
            probe_emit(write, b"\nboot_keyboard.interface=");
            probe_write_u64_dec(write, interface_number as u64);
            probe_emit(write, b"\nboot_keyboard.endpoint.address=");
            probe_write_u32_hex(write, endpoint_address as u32);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetConfigurationStatus::StartEvidenceUnavailable
        | arch_sys::usb::XhciSetConfigurationStatus::TransferTimedOut => {}
    }

    if let Some(event) = report.event {
        probe_xhci_transfer_event(write, event);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_configure_endpoint(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-configure-endpoint:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let report = arch_sys::usb::configure_keyboard_endpoint_on_pcie_xhci_controller();
    probe_emit(write, b"state=");
    probe_emit(write, probe_xhci_configure_endpoint_status_name(report.status));
    probe_emit(write, b"\nset_configuration.state=");
    probe_emit(
        write,
        probe_xhci_set_configuration_status_name(report.set_configuration.status),
    );
    probe_emit(write, b"\nconfig_descriptor.state=");
    probe_emit(
        write,
        probe_xhci_read_config_descriptor_status_name(
            report.set_configuration.configuration.status,
        ),
    );
    probe_emit(write, b"\ncommand.trb_pointer=");
    probe_write_u64_hex(write, report.command_trb_pointer);
    probe_emit(write, b"\ncommand.doorbell=");
    probe_write_u32_hex(write, report.doorbell);
    probe_emit(write, b"\n");
    probe_xhci_trb(write, b"command", report.command_trb);

    if let Some(contexts) = report.contexts {
        probe_emit(write, b"context.input=");
        probe_write_u64_hex(write, contexts.input_context);
        probe_emit(write, b"\ncontext.output=");
        probe_write_u64_hex(write, contexts.output_device_context);
        probe_emit(write, b"\ncontext.interrupt_ring=");
        probe_write_u64_hex(write, contexts.interrupt_in_endpoint_ring);
        probe_emit(write, b"\ncontext.drop_flags=");
        probe_write_u32_hex(write, contexts.drop_context_flags);
        probe_emit(write, b"\ncontext.add_flags=");
        probe_write_u32_hex(write, contexts.add_context_flags);
        probe_emit(write, b"\ncontext.endpoint_id=");
        probe_write_u64_dec(write, contexts.endpoint_id as u64);
        probe_emit(write, b"\ncontext.endpoint_index=");
        probe_write_u64_dec(write, contexts.endpoint_context_index as u64);
        probe_emit(write, b"\ncontext.endpoint_address=");
        probe_write_u32_hex(write, contexts.endpoint_address as u32);
        probe_emit(write, b"\ncontext.endpoint_number=");
        probe_write_u64_dec(write, contexts.endpoint_number as u64);
        probe_emit(write, b"\ncontext.interval=");
        probe_write_u64_dec(write, contexts.interval as u64);
        probe_emit(write, b"\ncontext.interval_encoded=");
        probe_write_u64_dec(write, contexts.interval_encoded as u64);
        probe_emit(write, b"\ncontext.max_packet_size=");
        probe_write_u64_dec(write, contexts.max_packet_size as u64);
        probe_emit(write, b"\ncontext.max_esit_payload=");
        probe_write_u64_dec(write, contexts.max_esit_payload as u64);
        probe_emit(write, b"\ncontext.slot0=");
        probe_write_u32_hex(write, contexts.slot_context[0]);
        probe_emit(write, b"\ncontext.slot1=");
        probe_write_u32_hex(write, contexts.slot_context[1]);
        probe_emit(write, b"\ncontext.endpoint0=");
        probe_write_u32_hex(write, contexts.endpoint_context[0]);
        probe_emit(write, b"\ncontext.endpoint1=");
        probe_write_u32_hex(write, contexts.endpoint_context[1]);
        probe_emit(write, b"\ncontext.endpoint_dequeue_lo=");
        probe_write_u32_hex(write, contexts.endpoint_context[2]);
        probe_emit(write, b"\ncontext.endpoint_dequeue_hi=");
        probe_write_u32_hex(write, contexts.endpoint_context[3]);
        probe_emit(write, b"\ncontext.endpoint_avg_trb_len=");
        probe_write_u32_hex(write, contexts.endpoint_context[4]);
        probe_emit(write, b"\n");
    }

    match report.status {
        arch_sys::usb::XhciConfigureEndpointStatus::SetConfigurationFailed(status) => {
            probe_emit(write, b"set_configuration.failure=");
            probe_emit(write, probe_xhci_set_configuration_status_name(status));
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciConfigureEndpointStatus::EndpointIdOutOfRange {
            endpoint_address,
            endpoint_id,
        } => {
            probe_emit(write, b"endpoint.address=");
            probe_write_u32_hex(write, endpoint_address as u32);
            probe_emit(write, b"\nendpoint.id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciConfigureEndpointStatus::UnexpectedEventType {
            trb_type,
            completion_code,
        } => {
            probe_emit(write, b"event.trb_type=");
            probe_write_u64_dec(write, trb_type as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciConfigureEndpointStatus::CommandPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
        } => {
            probe_emit(write, b"event.expected_command_trb_pointer=");
            probe_write_u64_hex(write, expected);
            probe_emit(write, b"\nevent.actual_command_trb_pointer=");
            probe_write_u64_hex(write, actual);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciConfigureEndpointStatus::SlotIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_slot_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_slot_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciConfigureEndpointStatus::CommandFailed {
            completion_code,
            slot_id,
        } => {
            probe_emit(write, b"event.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciConfigureEndpointStatus::EndpointConfigured {
            slot_id,
            endpoint_id,
            endpoint_address,
            max_packet_size,
            interval_encoded,
        } => {
            probe_emit(write, b"endpoint.configured=true\nendpoint.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nendpoint.id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\nendpoint.address=");
            probe_write_u32_hex(write, endpoint_address as u32);
            probe_emit(write, b"\nendpoint.max_packet_size=");
            probe_write_u64_dec(write, max_packet_size as u64);
            probe_emit(write, b"\nendpoint.interval_encoded=");
            probe_write_u64_dec(write, interval_encoded as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciConfigureEndpointStatus::BootKeyboardEndpointUnavailable
        | arch_sys::usb::XhciConfigureEndpointStatus::StartEvidenceUnavailable
        | arch_sys::usb::XhciConfigureEndpointStatus::CommandTimedOut => {}
    }

    if let Some(event) = report.event {
        probe_xhci_command_completion_event(write, event);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_set_hid_protocol(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-set-hid-protocol:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let report = arch_sys::usb::set_hid_boot_protocol_on_pcie_xhci_controller();
    probe_emit(write, b"state=");
    probe_emit(write, probe_xhci_set_hid_protocol_status_name(report.status));
    probe_emit(write, b"\nconfigure_endpoint.state=");
    probe_emit(
        write,
        probe_xhci_configure_endpoint_status_name(report.configure_endpoint.status),
    );
    probe_emit(write, b"\nset_configuration.state=");
    probe_emit(
        write,
        probe_xhci_set_configuration_status_name(
            report.configure_endpoint.set_configuration.status,
        ),
    );
    probe_emit(write, b"\nslot_id=");
    probe_write_u64_dec(write, report.slot_id as u64);
    probe_emit(write, b"\nendpoint_id=");
    probe_write_u64_dec(write, report.endpoint_id as u64);
    probe_emit(write, b"\ndoorbell=");
    probe_write_u32_hex(write, report.doorbell);
    probe_emit(write, b"\nhid.interface=");
    probe_write_u64_dec(write, report.interface_number as u64);
    probe_emit(write, b"\nhid.protocol=");
    probe_write_u64_dec(write, report.protocol as u64);
    probe_emit(write, b"\nsetup.trb_pointer=");
    probe_write_u64_hex(write, report.setup_trb_pointer);
    probe_emit(write, b"\nstatus.trb_pointer=");
    probe_write_u64_hex(write, report.status_trb_pointer);
    probe_emit(write, b"\n");
    probe_xhci_trb(write, b"setup", report.setup_trb);
    probe_xhci_trb(write, b"status", report.status_trb);

    match report.status {
        arch_sys::usb::XhciSetHidProtocolStatus::ConfigureEndpointFailed(status) => {
            probe_emit(write, b"configure_endpoint.failure=");
            probe_emit(write, probe_xhci_configure_endpoint_status_name(status));
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetHidProtocolStatus::UnexpectedEventType {
            trb_type,
            completion_code,
        } => {
            probe_emit(write, b"event.trb_type=");
            probe_write_u64_dec(write, trb_type as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetHidProtocolStatus::TransferPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.expected_trb_pointer=");
            probe_write_u64_hex(write, expected);
            probe_emit(write, b"\nevent.actual_trb_pointer=");
            probe_write_u64_hex(write, actual);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetHidProtocolStatus::SlotIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_slot_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_slot_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetHidProtocolStatus::EndpointIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_endpoint_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_endpoint_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetHidProtocolStatus::TransferFailed {
            completion_code,
            residual_length,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.residual_length=");
            probe_write_u64_dec(write, residual_length as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetHidProtocolStatus::BootProtocolSet {
            slot_id,
            interface_number,
            protocol,
        } => {
            probe_emit(write, b"hid.boot_protocol=true\nhid.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nhid.interface=");
            probe_write_u64_dec(write, interface_number as u64);
            probe_emit(write, b"\nhid.protocol=");
            probe_write_u64_dec(write, protocol as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciSetHidProtocolStatus::BootKeyboardInterfaceUnavailable
        | arch_sys::usb::XhciSetHidProtocolStatus::StartEvidenceUnavailable
        | arch_sys::usb::XhciSetHidProtocolStatus::TransferTimedOut => {}
    }

    if let Some(event) = report.event {
        probe_xhci_transfer_event(write, event);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_read_keyboard_report(devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-read-keyboard-report:\n");
    if !device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    ) {
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    let report = arch_sys::usb::read_boot_keyboard_report_on_pcie_xhci_controller();
    probe_emit(write, b"state=");
    probe_emit(write, probe_xhci_read_keyboard_report_status_name(report.status));
    probe_emit(write, b"\nset_hid_protocol.state=");
    probe_emit(
        write,
        probe_xhci_set_hid_protocol_status_name(report.set_hid_protocol.status),
    );
    probe_emit(write, b"\nconfigure_endpoint.state=");
    probe_emit(
        write,
        probe_xhci_configure_endpoint_status_name(
            report.set_hid_protocol.configure_endpoint.status,
        ),
    );
    probe_emit(write, b"\nslot_id=");
    probe_write_u64_dec(write, report.slot_id as u64);
    probe_emit(write, b"\nendpoint_id=");
    probe_write_u64_dec(write, report.endpoint_id as u64);
    probe_emit(write, b"\ndoorbell=");
    probe_write_u32_hex(write, report.doorbell);
    probe_emit(write, b"\nnormal.trb_pointer=");
    probe_write_u64_hex(write, report.normal_trb_pointer);
    probe_emit(write, b"\nreport.buffer=");
    probe_write_u64_hex(write, report.report_buffer);
    probe_emit(write, b"\n");
    probe_xhci_trb(write, b"normal", report.normal_trb);

    match report.status {
        arch_sys::usb::XhciReadBootKeyboardReportStatus::SetHidProtocolFailed(status) => {
            probe_emit(write, b"set_hid_protocol.failure=");
            probe_emit(write, probe_xhci_set_hid_protocol_status_name(status));
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::UnexpectedEventType {
            trb_type,
            completion_code,
        } => {
            probe_emit(write, b"event.trb_type=");
            probe_write_u64_dec(write, trb_type as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::TransferPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.expected_trb_pointer=");
            probe_write_u64_hex(write, expected);
            probe_emit(write, b"\nevent.actual_trb_pointer=");
            probe_write_u64_hex(write, actual);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::SlotIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_slot_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_slot_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::EndpointIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            probe_emit(write, b"event.expected_endpoint_id=");
            probe_write_u64_dec(write, expected as u64);
            probe_emit(write, b"\nevent.actual_endpoint_id=");
            probe_write_u64_dec(write, actual as u64);
            probe_emit(write, b"\nevent.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::TransferFailed {
            completion_code,
            residual_length,
            slot_id,
            endpoint_id,
        } => {
            probe_emit(write, b"event.completion_code=");
            probe_write_u64_dec(write, completion_code as u64);
            probe_emit(write, b"\nevent.residual_length=");
            probe_write_u64_dec(write, residual_length as u64);
            probe_emit(write, b"\nevent.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nevent.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\n");
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::ReportReady {
            slot_id,
            endpoint_id,
            length,
        } => {
            probe_emit(write, b"report.ready=true\nreport.slot_id=");
            probe_write_u64_dec(write, slot_id as u64);
            probe_emit(write, b"\nreport.endpoint_id=");
            probe_write_u64_dec(write, endpoint_id as u64);
            probe_emit(write, b"\nreport.length=");
            probe_write_u64_dec(write, length as u64);
            probe_emit(write, b"\n");
            probe_report_bytes(write, &report.report);
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::BootKeyboardEndpointUnavailable
        | arch_sys::usb::XhciReadBootKeyboardReportStatus::StartEvidenceUnavailable
        | arch_sys::usb::XhciReadBootKeyboardReportStatus::TransferTimedOut => {}
    }

    if let Some(event) = report.event {
        probe_xhci_transfer_event(write, event);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_set_address_status_name(
    status: arch_sys::usb::XhciSetAddressStatus,
) -> &'static [u8] {
    match status {
        arch_sys::usb::XhciSetAddressStatus::DescriptorPrefixFailed(_) => {
            b"descriptor-prefix-failed"
        }
        arch_sys::usb::XhciSetAddressStatus::StartEvidenceUnavailable => {
            b"start-evidence-unavailable"
        }
        arch_sys::usb::XhciSetAddressStatus::InvalidDescriptorPrefix { .. } => {
            b"invalid-descriptor-prefix"
        }
        arch_sys::usb::XhciSetAddressStatus::CommandTimedOut => b"command-timeout",
        arch_sys::usb::XhciSetAddressStatus::UnexpectedEventType { .. } => {
            b"unexpected-event-type"
        }
        arch_sys::usb::XhciSetAddressStatus::CommandPointerMismatch { .. } => {
            b"command-pointer-mismatch"
        }
        arch_sys::usb::XhciSetAddressStatus::SlotIdMismatch { .. } => b"slot-id-mismatch",
        arch_sys::usb::XhciSetAddressStatus::CommandFailed { .. } => b"command-failed",
        arch_sys::usb::XhciSetAddressStatus::Addressed { .. } => b"addressed",
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_read_device_descriptor_status_name(
    status: arch_sys::usb::XhciReadDeviceDescriptorStatus,
) -> &'static [u8] {
    match status {
        arch_sys::usb::XhciReadDeviceDescriptorStatus::SetAddressFailed(_) => {
            b"set-address-failed"
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::StartEvidenceUnavailable => {
            b"start-evidence-unavailable"
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::TransferTimedOut => b"transfer-timeout",
        arch_sys::usb::XhciReadDeviceDescriptorStatus::UnexpectedEventType { .. } => {
            b"unexpected-event-type"
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::TransferPointerMismatch { .. } => {
            b"transfer-pointer-mismatch"
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::SlotIdMismatch { .. } => {
            b"slot-id-mismatch"
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::EndpointIdMismatch { .. } => {
            b"endpoint-id-mismatch"
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::TransferFailed { .. } => {
            b"transfer-failed"
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::InvalidDeviceDescriptor { .. } => {
            b"invalid-device-descriptor"
        }
        arch_sys::usb::XhciReadDeviceDescriptorStatus::DeviceDescriptorReady { .. } => {
            b"device-descriptor-ready"
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_read_config_descriptor_header_status_name(
    status: arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus,
) -> &'static [u8] {
    match status {
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::DeviceDescriptorFailed(_) => {
            b"device-descriptor-failed"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::StartEvidenceUnavailable => {
            b"start-evidence-unavailable"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::TransferTimedOut => {
            b"transfer-timeout"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::UnexpectedEventType {
            ..
        } => b"unexpected-event-type",
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::TransferPointerMismatch {
            ..
        } => b"transfer-pointer-mismatch",
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::SlotIdMismatch { .. } => {
            b"slot-id-mismatch"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::EndpointIdMismatch {
            ..
        } => b"endpoint-id-mismatch",
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::TransferFailed { .. } => {
            b"transfer-failed"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::InvalidConfigurationDescriptorHeader { .. } => {
            b"invalid-config-descriptor-header"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorHeaderStatus::ConfigurationDescriptorHeaderReady { .. } => {
            b"config-descriptor-header-ready"
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_read_config_descriptor_status_name(
    status: arch_sys::usb::XhciReadConfigurationDescriptorStatus,
) -> &'static [u8] {
    match status {
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::HeaderFailed(_) => {
            b"config-header-failed"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::ConfigurationTooLarge { .. } => {
            b"configuration-too-large"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::StartEvidenceUnavailable => {
            b"start-evidence-unavailable"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::TransferTimedOut => {
            b"transfer-timeout"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::UnexpectedEventType { .. } => {
            b"unexpected-event-type"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::TransferPointerMismatch { .. } => {
            b"transfer-pointer-mismatch"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::SlotIdMismatch { .. } => {
            b"slot-id-mismatch"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::EndpointIdMismatch { .. } => {
            b"endpoint-id-mismatch"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::TransferFailed { .. } => {
            b"transfer-failed"
        }
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::InvalidConfigurationDescriptor {
            ..
        } => b"invalid-config-descriptor",
        arch_sys::usb::XhciReadConfigurationDescriptorStatus::ConfigurationDescriptorReady {
            ..
        } => b"config-descriptor-ready",
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_set_configuration_status_name(
    status: arch_sys::usb::XhciSetConfigurationStatus,
) -> &'static [u8] {
    match status {
        arch_sys::usb::XhciSetConfigurationStatus::ConfigurationDescriptorFailed(_) => {
            b"config-descriptor-failed"
        }
        arch_sys::usb::XhciSetConfigurationStatus::BootKeyboardNotReadyToConfigure => {
            b"boot-keyboard-not-ready-to-configure"
        }
        arch_sys::usb::XhciSetConfigurationStatus::StartEvidenceUnavailable => {
            b"start-evidence-unavailable"
        }
        arch_sys::usb::XhciSetConfigurationStatus::TransferTimedOut => b"transfer-timeout",
        arch_sys::usb::XhciSetConfigurationStatus::UnexpectedEventType { .. } => {
            b"unexpected-event-type"
        }
        arch_sys::usb::XhciSetConfigurationStatus::TransferPointerMismatch { .. } => {
            b"transfer-pointer-mismatch"
        }
        arch_sys::usb::XhciSetConfigurationStatus::SlotIdMismatch { .. } => {
            b"slot-id-mismatch"
        }
        arch_sys::usb::XhciSetConfigurationStatus::EndpointIdMismatch { .. } => {
            b"endpoint-id-mismatch"
        }
        arch_sys::usb::XhciSetConfigurationStatus::TransferFailed { .. } => b"transfer-failed",
        arch_sys::usb::XhciSetConfigurationStatus::ConfigurationSet { .. } => {
            b"configuration-set"
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_configure_endpoint_status_name(
    status: arch_sys::usb::XhciConfigureEndpointStatus,
) -> &'static [u8] {
    match status {
        arch_sys::usb::XhciConfigureEndpointStatus::SetConfigurationFailed(_) => {
            b"set-configuration-failed"
        }
        arch_sys::usb::XhciConfigureEndpointStatus::BootKeyboardEndpointUnavailable => {
            b"boot-keyboard-endpoint-unavailable"
        }
        arch_sys::usb::XhciConfigureEndpointStatus::StartEvidenceUnavailable => {
            b"start-evidence-unavailable"
        }
        arch_sys::usb::XhciConfigureEndpointStatus::EndpointIdOutOfRange { .. } => {
            b"endpoint-id-out-of-range"
        }
        arch_sys::usb::XhciConfigureEndpointStatus::CommandTimedOut => b"command-timeout",
        arch_sys::usb::XhciConfigureEndpointStatus::UnexpectedEventType { .. } => {
            b"unexpected-event-type"
        }
        arch_sys::usb::XhciConfigureEndpointStatus::CommandPointerMismatch { .. } => {
            b"command-pointer-mismatch"
        }
        arch_sys::usb::XhciConfigureEndpointStatus::SlotIdMismatch { .. } => {
            b"slot-id-mismatch"
        }
        arch_sys::usb::XhciConfigureEndpointStatus::CommandFailed { .. } => b"command-failed",
        arch_sys::usb::XhciConfigureEndpointStatus::EndpointConfigured { .. } => {
            b"endpoint-configured"
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_set_hid_protocol_status_name(
    status: arch_sys::usb::XhciSetHidProtocolStatus,
) -> &'static [u8] {
    match status {
        arch_sys::usb::XhciSetHidProtocolStatus::ConfigureEndpointFailed(_) => {
            b"configure-endpoint-failed"
        }
        arch_sys::usb::XhciSetHidProtocolStatus::BootKeyboardInterfaceUnavailable => {
            b"boot-keyboard-interface-unavailable"
        }
        arch_sys::usb::XhciSetHidProtocolStatus::StartEvidenceUnavailable => {
            b"start-evidence-unavailable"
        }
        arch_sys::usb::XhciSetHidProtocolStatus::TransferTimedOut => b"transfer-timeout",
        arch_sys::usb::XhciSetHidProtocolStatus::UnexpectedEventType { .. } => {
            b"unexpected-event-type"
        }
        arch_sys::usb::XhciSetHidProtocolStatus::TransferPointerMismatch { .. } => {
            b"transfer-pointer-mismatch"
        }
        arch_sys::usb::XhciSetHidProtocolStatus::SlotIdMismatch { .. } => b"slot-id-mismatch",
        arch_sys::usb::XhciSetHidProtocolStatus::EndpointIdMismatch { .. } => {
            b"endpoint-id-mismatch"
        }
        arch_sys::usb::XhciSetHidProtocolStatus::TransferFailed { .. } => b"transfer-failed",
        arch_sys::usb::XhciSetHidProtocolStatus::BootProtocolSet { .. } => {
            b"boot-protocol-set"
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_read_keyboard_report_status_name(
    status: arch_sys::usb::XhciReadBootKeyboardReportStatus,
) -> &'static [u8] {
    match status {
        arch_sys::usb::XhciReadBootKeyboardReportStatus::SetHidProtocolFailed(_) => {
            b"set-hid-protocol-failed"
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::BootKeyboardEndpointUnavailable => {
            b"boot-keyboard-endpoint-unavailable"
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::StartEvidenceUnavailable => {
            b"start-evidence-unavailable"
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::TransferTimedOut => {
            b"transfer-timeout"
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::UnexpectedEventType { .. } => {
            b"unexpected-event-type"
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::TransferPointerMismatch { .. } => {
            b"transfer-pointer-mismatch"
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::SlotIdMismatch { .. } => {
            b"slot-id-mismatch"
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::EndpointIdMismatch { .. } => {
            b"endpoint-id-mismatch"
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::TransferFailed { .. } => {
            b"transfer-failed"
        }
        arch_sys::usb::XhciReadBootKeyboardReportStatus::ReportReady { .. } => b"report-ready",
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_trb(write: WriteFn, label: &[u8], trb: [u32; 4]) {
    probe_emit(write, label);
    probe_emit(write, b".trb0=");
    probe_write_u32_hex(write, trb[0]);
    probe_emit(write, b"\n");
    probe_emit(write, label);
    probe_emit(write, b".trb1=");
    probe_write_u32_hex(write, trb[1]);
    probe_emit(write, b"\n");
    probe_emit(write, label);
    probe_emit(write, b".trb2=");
    probe_write_u32_hex(write, trb[2]);
    probe_emit(write, b"\n");
    probe_emit(write, label);
    probe_emit(write, b".trb3=");
    probe_write_u32_hex(write, trb[3]);
    probe_emit(write, b"\n");
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_descriptor_bytes(write: WriteFn, descriptor: &[u8]) {
    let mut index = 0usize;
    while index < descriptor.len() {
        probe_emit(write, b"descriptor.byte");
        probe_write_u64_dec(write, index as u64);
        probe_emit(write, b"=");
        probe_write_u32_hex(write, descriptor[index] as u32);
        probe_emit(write, b"\n");
        index += 1;
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_report_bytes(write: WriteFn, report: &[u8]) {
    let mut index = 0usize;
    while index < report.len() {
        probe_emit(write, b"report.byte");
        probe_write_u64_dec(write, index as u64);
        probe_emit(write, b"=");
        probe_write_u32_hex(write, report[index] as u32);
        probe_emit(write, b"\n");
        index += 1;
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_command_completion_event(
    write: WriteFn,
    event: arch_sys::usb::XhciCommandCompletionEvent,
) {
    probe_emit(write, b"event.raw0=");
    probe_write_u32_hex(write, event.raw[0]);
    probe_emit(write, b"\nevent.raw1=");
    probe_write_u32_hex(write, event.raw[1]);
    probe_emit(write, b"\nevent.raw2=");
    probe_write_u32_hex(write, event.raw[2]);
    probe_emit(write, b"\nevent.raw3=");
    probe_write_u32_hex(write, event.raw[3]);
    probe_emit(write, b"\nevent.command_trb_pointer=");
    probe_write_u64_hex(write, event.command_trb_pointer);
    probe_emit(write, b"\nevent.completion_code=");
    probe_write_u64_dec(write, event.completion_code as u64);
    probe_emit(write, b"\nevent.trb_type=");
    probe_write_u64_dec(write, event.trb_type as u64);
    probe_emit(write, b"\nevent.cycle=");
    probe_write_bool(write, event.cycle);
    probe_emit(write, b"\nevent.slot_id=");
    probe_write_u64_dec(write, event.slot_id as u64);
    probe_emit(write, b"\n");
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_xhci_transfer_event(write: WriteFn, event: arch_sys::usb::XhciTransferEvent) {
    probe_emit(write, b"event.raw0=");
    probe_write_u32_hex(write, event.raw[0]);
    probe_emit(write, b"\nevent.raw1=");
    probe_write_u32_hex(write, event.raw[1]);
    probe_emit(write, b"\nevent.raw2=");
    probe_write_u32_hex(write, event.raw[2]);
    probe_emit(write, b"\nevent.raw3=");
    probe_write_u32_hex(write, event.raw[3]);
    probe_emit(write, b"\nevent.trb_pointer=");
    probe_write_u64_hex(write, event.trb_pointer);
    probe_emit(write, b"\nevent.transfer_length=");
    probe_write_u64_dec(write, event.transfer_length as u64);
    probe_emit(write, b"\nevent.completion_code=");
    probe_write_u64_dec(write, event.completion_code as u64);
    probe_emit(write, b"\nevent.trb_type=");
    probe_write_u64_dec(write, event.trb_type as u64);
    probe_emit(write, b"\nevent.cycle=");
    probe_write_bool(write, event.cycle);
    probe_emit(write, b"\nevent.event_data=");
    probe_write_bool(write, event.event_data);
    probe_emit(write, b"\nevent.endpoint_id=");
    probe_write_u64_dec(write, event.endpoint_id as u64);
    probe_emit(write, b"\nevent.slot_id=");
    probe_write_u64_dec(write, event.slot_id as u64);
    probe_emit(write, b"\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_pcie(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe pcie:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_usb_keyboard(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe usb-keyboard:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_xhci_start(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-start:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_xhci_enable_slot(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-enable-slot:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_xhci_address_device(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-address-device:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_xhci_get_device_descriptor(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-get-device-descriptor:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_xhci_set_address(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-set-address:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_xhci_read_device_descriptor(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-read-device-descriptor:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_xhci_read_config_descriptor_header(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-read-config-descriptor-header:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_xhci_read_config_descriptor(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-read-config-descriptor:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_xhci_set_configuration(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-set-configuration:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_xhci_configure_endpoint(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-configure-endpoint:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_xhci_set_hid_protocol(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-set-hid-protocol:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
fn probe_xhci_read_keyboard_report(_devices: &[DeviceEntry], write: WriteFn) {
    probe_emit(write, b"probe xhci-read-keyboard-report:\n");
    probe_emit(write, b"state=unsupported-on-this-target\n");
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn device_inventory_has(
    devices: &[DeviceEntry],
    class: reovim_uapi::system::DeviceClass,
    compatible: &str,
) -> bool {
    devices
        .iter()
        .any(|device| device.class == class && device.compatible == compatible)
}

fn probe_emit(write: WriteFn, bytes: &[u8]) {
    write(bytes);
    reovim_system_kernel::klog::append_bytes(bytes);
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_write_bool(write: WriteFn, value: bool) {
    probe_emit(write, if value { b"true" } else { b"false" });
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_write_u64_dec(write: WriteFn, mut value: u64) {
    let mut buf = [0u8; 20];
    if value == 0 {
        probe_emit(write, b"0");
        return;
    }
    let mut len = 0usize;
    while value > 0 {
        buf[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
    }
    while len > 0 {
        len -= 1;
        probe_emit(write, &buf[len..len + 1]);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_write_u32_hex(write: WriteFn, value: u32) {
    probe_write_u64_hex(write, value as u64);
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn probe_write_u64_hex(write: WriteFn, mut value: u64) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    if value == 0 {
        buf[2] = b'0';
        probe_emit(write, &buf[..3]);
        return;
    }
    let mut len = 0usize;
    while value > 0 {
        buf[2 + len] = HEX[(value & 0xf) as usize];
        len += 1;
        value >>= 4;
    }
    let mut out = [0u8; 18];
    out[0] = b'0';
    out[1] = b'x';
    let mut i = 0usize;
    while i < len {
        out[2 + i] = buf[2 + len - 1 - i];
        i += 1;
    }
    probe_emit(write, &out[..2 + len]);
}

fn tty_write(bytes: &[u8]) {
    let _ = arch_sys::write(1, bytes);
}

fn tty_read_line(line: &mut [u8]) -> usize {
    #[cfg(target_os = "none")]
    {
        if BOOTLINE_SCRIPT.is_some() {
            return consume_bootline(line).unwrap_or(0);
        }
    }

    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    reset_aarch64_line_input_audit();

    let len = console_io::read_line(line, tty_read_byte, tty_write);

    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    log_aarch64_line_input_audit(len);

    len
}

fn tty_read_byte() -> Option<u8> {
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    {
        return read_aarch64_console_byte();
    }

    #[cfg(all(target_os = "none", target_arch = "x86_64"))]
    {
        return read_none_console_byte();
    }

    #[cfg(not(target_os = "none"))]
    {
        return read_fd_stdin_byte();
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn read_aarch64_console_byte() -> Option<u8> {
    loop {
        let console = unsafe { &mut *USB_KEYBOARD_CONSOLE.0.get() };
        match console.read_byte(
            usb_keyboard_poll_report_if_due,
            arch_sys::try_read_stdin_byte,
        ) {
            Some(ConsoleInputByte::UsbKeyboard(byte)) => {
                mark_usb_keyboard_ready();
                note_aarch64_usb_line_byte();
                return Some(byte);
            }
            Some(ConsoleInputByte::Fallback(byte)) => {
                note_aarch64_fallback_line_byte();
                return Some(byte);
            }
            None => {}
        }
        core::hint::spin_loop();
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn reset_aarch64_line_input_audit() {
    CONSOLE_LINE_USB_BYTES.store(0, Ordering::Relaxed);
    CONSOLE_LINE_FALLBACK_BYTES.store(0, Ordering::Relaxed);
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn note_aarch64_usb_line_byte() {
    CONSOLE_LINE_USB_BYTES.fetch_add(1, Ordering::Relaxed);
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn note_aarch64_fallback_line_byte() {
    CONSOLE_LINE_FALLBACK_BYTES.fetch_add(1, Ordering::Relaxed);
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn log_aarch64_line_input_audit(line_len: usize) {
    let usb_bytes = CONSOLE_LINE_USB_BYTES.load(Ordering::Relaxed);
    let fallback_bytes = CONSOLE_LINE_FALLBACK_BYTES.load(Ordering::Relaxed);
    reovim_system_kernel::klog::append_bytes(b"input.line_source=");
    reovim_system_kernel::klog::append_bytes(aarch64_line_source_word(usb_bytes, fallback_bytes));
    reovim_system_kernel::klog::append_bytes(b" usb_bytes=");
    reovim_system_kernel::klog::append_usize_dec(usb_bytes);
    reovim_system_kernel::klog::append_bytes(b" fallback_bytes=");
    reovim_system_kernel::klog::append_usize_dec(fallback_bytes);
    reovim_system_kernel::klog::append_bytes(b" line_bytes=");
    reovim_system_kernel::klog::append_usize_dec(line_len);
    reovim_system_kernel::klog::append_bytes(b"\n");
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const fn aarch64_line_source_word(usb_bytes: usize, fallback_bytes: usize) -> &'static [u8] {
    if usb_bytes > 0 && fallback_bytes == 0 {
        b"usb-keyboard"
    } else if fallback_bytes > 0 && usb_bytes == 0 {
        b"uart-fallback"
    } else if usb_bytes > 0 && fallback_bytes > 0 {
        b"mixed"
    } else {
        b"empty"
    }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn read_none_console_byte() -> Option<u8> {
    loop {
        if let Some(byte) = arch_sys::try_read_stdin_byte() {
            return Some(byte);
        }
        core::hint::spin_loop();
    }
}

#[cfg(not(target_os = "none"))]
fn read_fd_stdin_byte() -> Option<u8> {
    let mut byte = [0u8; 1];
    match arch_sys::read(0, &mut byte) {
        Ok(1) => Some(byte[0]),
        _ => None,
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn usb_keyboard_poll_report_if_due() -> Option<[u8; arch_sys::usb::BOOT_KEYBOARD_REPORT_BYTES]> {
    if USB_KEYBOARD_PROBE_ENABLED.load(Ordering::Acquire) == 0 {
        store_usb_keyboard_poll_state(USB_KEYBOARD_POLL_PROBE_DISABLED);
        return None;
    }
    if !usb_keyboard_poll_due() {
        return None;
    }
    match arch_sys::usb::poll_boot_keyboard_report() {
        arch_sys::usb::UsbBootKeyboardPoll::Report(report) => {
            store_usb_keyboard_poll_state(USB_KEYBOARD_POLL_REPORT_READY);
            Some(report)
        }
        arch_sys::usb::UsbBootKeyboardPoll::Pending(pending) => {
            store_usb_keyboard_poll_state(usb_keyboard_poll_state_from_pending(pending));
            None
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn configure_usb_keyboard_probe(devices: &[DeviceEntry]) {
    let enabled = device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    );
    USB_KEYBOARD_PROBE_ENABLED.store(enabled as usize, Ordering::Release);
    USB_KEYBOARD_READY.store(0, Ordering::Release);
    USB_KEYBOARD_LAST_POLL_NANOS.store(0, Ordering::Release);
    USB_KEYBOARD_LOGGED_POLL_STATE.store(USB_KEYBOARD_POLL_LOG_UNSET, Ordering::Release);
    USB_KEYBOARD_LAST_POLL_STATE.store(
        if enabled {
            USB_KEYBOARD_POLL_NOT_POLLED
        } else {
            USB_KEYBOARD_POLL_PROBE_DISABLED
        },
        Ordering::Release,
    );
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn usb_keyboard_poll_due() -> bool {
    let Some(now) = monotonic_nanos() else {
        return true;
    };
    let now = now as usize;
    let last = USB_KEYBOARD_LAST_POLL_NANOS.load(Ordering::Acquire);
    if last != 0 && now.wrapping_sub(last) < USB_KEYBOARD_POLL_NANOS {
        return false;
    }
    USB_KEYBOARD_LAST_POLL_NANOS.store(now, Ordering::Release);
    true
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn mark_usb_keyboard_ready() {
    if USB_KEYBOARD_READY.swap(1, Ordering::AcqRel) == 0 {
        reovim_system_kernel::klog::append_bytes(
            b"input.usb_keyboard=ready source=usb-keyboard+uart-fallback last_poll=",
        );
        reovim_system_kernel::klog::append_bytes(usb_keyboard_last_poll_word().as_bytes());
        reovim_system_kernel::klog::append_bytes(b"\n");
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn store_usb_keyboard_poll_state(state: usize) {
    USB_KEYBOARD_LAST_POLL_STATE.store(state, Ordering::Release);
    let previous = USB_KEYBOARD_LOGGED_POLL_STATE.swap(state, Ordering::AcqRel);
    if previous != state && state != USB_KEYBOARD_POLL_NOT_POLLED {
        reovim_system_kernel::klog::append_bytes(b"input.usb_keyboard_poll=");
        reovim_system_kernel::klog::append_bytes(usb_keyboard_poll_word(state).as_bytes());
        reovim_system_kernel::klog::append_bytes(b"\n");
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn usb_keyboard_last_poll_word() -> &'static str {
    usb_keyboard_poll_word(USB_KEYBOARD_LAST_POLL_STATE.load(Ordering::Acquire))
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const fn usb_keyboard_poll_word(state: usize) -> &'static str {
    match state {
        USB_KEYBOARD_POLL_NOT_POLLED => "not-polled",
        USB_KEYBOARD_POLL_PROBE_DISABLED => "probe-disabled",
        USB_KEYBOARD_POLL_DTB_MISSING => "device-tree-disabled-or-missing",
        USB_KEYBOARD_POLL_DECODED_PENDING => "decoded-pending",
        USB_KEYBOARD_POLL_REPORT_READY => "report-ready",
        USB_KEYBOARD_POLL_CONTROLLER_NOT_READY => "xhci-controller-not-ready",
        USB_KEYBOARD_POLL_RESET_IN_PROGRESS => "xhci-reset-in-progress",
        USB_KEYBOARD_POLL_HOST_SYSTEM_ERROR => "xhci-host-system-error",
        USB_KEYBOARD_POLL_NO_PCIE_XHCI => "no-pcie-xhci-controller",
        USB_KEYBOARD_POLL_BAR_UNCONFIGURED => "xhci-bar-unconfigured",
        USB_KEYBOARD_POLL_INVALID_CAPS => "xhci-capabilities-invalid",
        USB_KEYBOARD_POLL_NO_CONNECTED_PORT => "no-connected-root-port",
        USB_KEYBOARD_POLL_NEEDS_CONTROLLER_INIT => "needs-controller-init",
        USB_KEYBOARD_POLL_NEEDS_ENUMERATION => "needs-enumeration",
        USB_KEYBOARD_POLL_REPORT_PENDING => "report-pending",
        USB_KEYBOARD_POLL_ENUMERATION_FAILED => "hid-enumeration-failed",
        USB_KEYBOARD_POLL_REPORT_EVENT_MISMATCH => "keyboard-report-event-mismatch",
        USB_KEYBOARD_POLL_REPORT_TRANSFER_FAILED => "keyboard-report-transfer-failed",
        _ => "unknown",
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const fn usb_keyboard_manual_next_from_poll_state(state: usize) -> &'static [u8] {
    match state {
        USB_KEYBOARD_POLL_DTB_MISSING
        | USB_KEYBOARD_POLL_CONTROLLER_NOT_READY
        | USB_KEYBOARD_POLL_RESET_IN_PROGRESS
        | USB_KEYBOARD_POLL_HOST_SYSTEM_ERROR
        | USB_KEYBOARD_POLL_NO_PCIE_XHCI
        | USB_KEYBOARD_POLL_BAR_UNCONFIGURED
        | USB_KEYBOARD_POLL_INVALID_CAPS
        | USB_KEYBOARD_POLL_NO_CONNECTED_PORT => b"probe-pcie",
        USB_KEYBOARD_POLL_NEEDS_CONTROLLER_INIT => b"probe-xhci-start",
        USB_KEYBOARD_POLL_NEEDS_ENUMERATION
        | USB_KEYBOARD_POLL_ENUMERATION_FAILED
        | USB_KEYBOARD_POLL_REPORT_EVENT_MISMATCH
        | USB_KEYBOARD_POLL_REPORT_TRANSFER_FAILED => b"probe-xhci-read-keyboard-report",
        USB_KEYBOARD_POLL_REPORT_PENDING => b"press-usb-key",
        USB_KEYBOARD_POLL_DECODED_PENDING | USB_KEYBOARD_POLL_REPORT_READY => {
            b"type-shell-command"
        }
        USB_KEYBOARD_POLL_NOT_POLLED | USB_KEYBOARD_POLL_PROBE_DISABLED => b"probe-usb-keyboard",
        _ => b"probe-usb-keyboard",
    }
}

fn console_input_summary() -> ConsoleInputSummary {
    #[cfg(target_os = "linux")]
    {
        ConsoleInputSummary::new(
            "host-stdin",
            "live",
            BootCheckState::Ok,
            BootCheckState::Warn,
        )
    }

    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    {
        if BOOTLINE_SCRIPT.is_some() {
            ConsoleInputSummary::new(
                "bootline-script",
                "scripted-test-harness",
                BootCheckState::Warn,
                BootCheckState::Warn,
            )
        } else {
            ConsoleInputSummary::new(
                "pl011-uart",
                "live",
                BootCheckState::Ok,
                BootCheckState::Warn,
            )
        }
    }

    #[cfg(all(target_os = "none", target_arch = "x86_64"))]
    {
        if BOOTLINE_SCRIPT.is_some() {
            ConsoleInputSummary::new(
                "bootline-script",
                "scripted-test-harness",
                BootCheckState::Warn,
                BootCheckState::Warn,
            )
        } else {
            ConsoleInputSummary::new(
                "com1-uart",
                "live",
                BootCheckState::Ok,
                BootCheckState::Warn,
            )
        }
    }
}

fn console_input_status(base: ConsoleInputSummary) -> ConsoleInputSummary {
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    {
        let probe_enabled = USB_KEYBOARD_PROBE_ENABLED.load(Ordering::Acquire) != 0;
        let pending = unsafe { &*USB_KEYBOARD_CONSOLE.0.get() }.pending_remaining();
        if BOOTLINE_SCRIPT.is_none() && USB_KEYBOARD_READY.load(Ordering::Acquire) != 0 {
            return ConsoleInputSummary::with_usb_diagnostics(
                "usb-keyboard+uart-fallback",
                "live",
                BootCheckState::Ok,
                BootCheckState::Ok,
                pending,
                probe_enabled,
                USB_KEYBOARD_POLL_INTERVAL_MS,
                usb_keyboard_last_poll_word(),
            );
        }
        return ConsoleInputSummary::with_usb_diagnostics(
            base.source,
            base.mode,
            base.source_state,
            base.usb_keyboard,
            pending,
            probe_enabled,
            USB_KEYBOARD_POLL_INTERVAL_MS,
            usb_keyboard_last_poll_word(),
        );
    }

    #[cfg(not(all(target_os = "none", target_arch = "aarch64")))]
    {
        base
    }
}

fn boot_image_summary(profile: BootProfile<'_>) -> BootImageSummary {
    BootImageSummary::new(
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        target_triple(),
        profile.name,
        profile_request(),
        bootline_state(),
        launch_profile_feature_state(),
    )
}

fn target_triple() -> &'static str {
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    {
        "aarch64-unknown-none"
    }
    #[cfg(all(target_os = "none", target_arch = "x86_64"))]
    {
        "x86_64-unknown-none"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "aarch64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(not(any(
        all(target_os = "none", target_arch = "aarch64"),
        all(target_os = "none", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64")
    )))]
    {
        "unknown"
    }
}

fn profile_request() -> &'static str {
    #[cfg(feature = "launch-profile")]
    {
        option_env!("REOVIM_OS_PROFILE").unwrap_or("shell-only")
    }
    #[cfg(not(feature = "launch-profile"))]
    {
        "shell-only"
    }
}

fn bootline_state() -> &'static str {
    if BOOTLINE_SCRIPT.is_some() {
        "present"
    } else {
        "absent"
    }
}

const fn launch_profile_feature_state() -> &'static str {
    #[cfg(feature = "launch-profile")]
    {
        "enabled"
    }
    #[cfg(not(feature = "launch-profile"))]
    {
        "disabled"
    }
}

#[cfg(target_os = "none")]
const BOOTLINE_SCRIPT: Option<&str> = option_env!("REOVIM_OS_BOOTLINE");

#[cfg(not(target_os = "none"))]
const BOOTLINE_SCRIPT: Option<&str> = option_env!("REOVIM_OS_BOOTLINE");

#[cfg(target_os = "none")]
static BOOTLINE_CURSOR: AtomicUsize = AtomicUsize::new(0);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static BOOTLINE_COUNTDOWN_DONE: AtomicUsize = AtomicUsize::new(0);

#[cfg(target_os = "none")]
fn consume_bootline(line: &mut [u8]) -> Option<usize> {
    let Some(script) = BOOTLINE_SCRIPT else {
        return None;
    };

    let script = script.as_bytes();
    let mut start = BOOTLINE_CURSOR.load(Ordering::Acquire);

    while start < script.len() {
        match script[start] {
            b'\n' | b'\r' => {
                start += 1;
            }
            _ => break,
        }
    }

    if start >= script.len() {
        BOOTLINE_CURSOR.store(start, Ordering::Release);
        return None;
    }

    let mut end = start;
    while end < script.len() && script[end] != b'\n' && script[end] != b'\r' {
        end += 1;
    }

    let mut next = end;
    while next < script.len() {
        match script[next] {
            b'\n' | b'\r' => {
                next += 1;
            }
            _ => break,
        }
    }

    let line_len = end.saturating_sub(start);
    if line_len == 0 {
        BOOTLINE_CURSOR.store(next, Ordering::Release);
        return None;
    }

    if line_len > line.len() {
        BOOTLINE_CURSOR.store(next, Ordering::Release);
        return None;
    }

    line[..line_len].copy_from_slice(&script[start..end]);
    BOOTLINE_CURSOR.store(next, Ordering::Release);
    Some(line_len)
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn run_bootline_countdown_once() {
    if BOOTLINE_COUNTDOWN_DONE.swap(1, Ordering::AcqRel) != 0 {
        return;
    }

    countdown_digit(b'5');
    countdown_digit(b'4');
    countdown_digit(b'3');
    countdown_digit(b'2');
    countdown_digit(b'1');
    tty_write(b"\r\x1b[32m[  OK  ]\x1b[0m Starting scripted input.          \n");
    wait_nanos(200_000_000);
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn countdown_digit(digit: u8) {
    countdown_frame(digit, b"    ");
    wait_nanos(200_000_000);
    countdown_frame(digit, b".   ");
    wait_nanos(200_000_000);
    countdown_frame(digit, b"..  ");
    wait_nanos(200_000_000);
    countdown_frame(digit, b"... ");
    wait_nanos(200_000_000);
    countdown_frame(digit, b"....");
    wait_nanos(200_000_000);
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn countdown_frame(digit: u8, dots: &[u8]) {
    tty_write(b"\r\x1b[33m[ WAIT ]\x1b[0m scripted input starts in ");
    tty_write(&[digit]);
    tty_write(dots);
    tty_write(b" seconds ");
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn wait_nanos(duration: u64) {
    let Some(start) = monotonic_nanos() else {
        return;
    };

    loop {
        let Some(now) = monotonic_nanos() else {
            return;
        };
        if now.saturating_sub(start) >= duration {
            return;
        }
        core::hint::spin_loop();
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn monotonic_nanos() -> Option<u64> {
    let mut ts = arch_sys::Timespec::default();
    if arch_sys::clock_gettime(arch_sys::CLOCK_MONOTONIC, &mut ts).is_err() {
        return None;
    }
    let sec = u64::try_from(ts.tv_sec).ok()?;
    let nsec = u64::try_from(ts.tv_nsec).ok()?;
    sec.checked_mul(1_000_000_000)?.checked_add(nsec)
}

#[cfg(not(target_os = "none"))]
fn consume_bootline(_line: &mut [u8]) -> Option<usize> {
    None
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn halt_kernel() {
    loop {
        // SAFETY: WFE is an idle hint used only in the kernel boot shell path.
        unsafe {
            asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn halt_kernel() {
    arch_sys::exit_group(0)
}
