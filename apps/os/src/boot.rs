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
    rootd::{BootCheckState, ConsoleInputSummary, HardwareProbeResult, WriteFn},
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
        input::{BootKeyboardDecoder, BootKeyboardReport},
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
    decoder: BootKeyboardDecoder,
    pending: [u8; 8],
    pending_len: usize,
    pending_cursor: usize,
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
impl UsbKeyboardConsole {
    const fn new() -> Self {
        Self {
            decoder: BootKeyboardDecoder::new(),
            pending: [0u8; 8],
            pending_len: 0,
            pending_cursor: 0,
        }
    }

    fn pop_pending(&mut self) -> Option<u8> {
        if self.pending_cursor < self.pending_len {
            let byte = self.pending[self.pending_cursor];
            self.pending_cursor += 1;
            return Some(byte);
        }
        None
    }

    fn poll_next_byte(&mut self) -> Option<u8> {
        let arch_sys::usb::UsbBootKeyboardPoll::Report(report) =
            arch_sys::usb::poll_boot_keyboard_report()
        else {
            return None;
        };
        self.pending_len = self
            .decoder
            .decode_report(BootKeyboardReport::new(report), &mut self.pending);
        self.pending_cursor = 0;
        self.pop_pending()
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
const USB_KEYBOARD_POLL_NANOS: usize = 5_000_000;

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
        read_line: tty_read_line,
        write: tty_write,
        console_input: console_input_summary(),
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
        "pcie" => {
            probe_pcie(devices, write);
            HardwareProbeResult::Handled
        }
        "keyboard" | "usb-keyboard" => {
            probe_usb_keyboard(devices, write);
            HardwareProbeResult::Handled
        }
        _ => HardwareProbeResult::UnknownTarget,
    }
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
        probe_emit(write, b"state=unavailable\n");
        probe_emit(write, b"reason=device-tree-disabled-or-missing\n");
        return;
    }

    match arch_sys::usb::poll_boot_keyboard_report() {
        arch_sys::usb::UsbBootKeyboardPoll::Report(_) => {
            probe_emit(write, b"state=report-ready\n");
            probe_emit(write, b"report_bytes=8\n");
        }
        arch_sys::usb::UsbBootKeyboardPoll::Pending(pending) => {
            probe_usb_keyboard_pending(write, pending);
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
    }
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

    console_io::read_line(line, tty_read_byte, tty_write)
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
        if let Some(byte) = usb_keyboard_read_byte() {
            return Some(byte);
        }
        if let Some(byte) = arch_sys::try_read_stdin_byte() {
            return Some(byte);
        }
        core::hint::spin_loop();
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
fn usb_keyboard_read_byte() -> Option<u8> {
    if USB_KEYBOARD_PROBE_ENABLED.load(Ordering::Acquire) == 0 {
        return None;
    }
    let console = unsafe { &mut *USB_KEYBOARD_CONSOLE.0.get() };
    if let Some(byte) = console.pop_pending() {
        return Some(byte);
    }
    if !usb_keyboard_poll_due() {
        return None;
    }
    console.poll_next_byte()
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn configure_usb_keyboard_probe(devices: &[DeviceEntry]) {
    let enabled = device_inventory_has(
        devices,
        reovim_uapi::system::DeviceClass::Bus,
        "brcm,bcm2711-pcie",
    );
    USB_KEYBOARD_PROBE_ENABLED.store(enabled as usize, Ordering::Release);
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

#[cfg(target_os = "none")]
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
