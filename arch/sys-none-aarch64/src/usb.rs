//! BCM2711 USB host-controller raw facts and early probe helpers.
//!
//! This module stays below the system-kernel bridge. It reads only native MMIO
//! controller registers and returns target-local facts; USB enumeration, HID
//! descriptor walking, and root-shell byte routing are later cuts.

use core::ptr::read_volatile;

use crate::pcie::{self, PciConfigHeader, PciLocation};

/// BCM2711 DWC2/OTG bus address from the firmware DTB.
pub const BCM2711_DWC2_BUS_BASE: u64 = 0x7e98_0000;
/// BCM2711 DWC2/OTG ARM-physical MMIO base.
pub const BCM2711_DWC2_MMIO_BASE: usize = 0xfe98_0000;
/// BCM2711 DWC2/OTG MMIO window length.
pub const BCM2711_DWC2_MMIO_LEN: usize = 0x1_0000;

/// BCM2711 built-in xHCI bus address from the firmware DTB.
pub const BCM2711_XHCI_BUS_BASE: u64 = 0x7e9c_0000;
/// BCM2711 built-in xHCI ARM-physical MMIO base.
pub const BCM2711_XHCI_MMIO_BASE: usize = 0xfe9c_0000;
/// BCM2711 built-in xHCI MMIO window length.
pub const BCM2711_XHCI_MMIO_LEN: usize = 0x10_0000;

const XHCI_CAP_HCIVERSION: usize = 0x00;
const XHCI_HCSPARAMS1: usize = 0x04;
const XHCI_HCCPARAMS1: usize = 0x10;
const XHCI_DBOFF: usize = 0x14;
const XHCI_RTSOFF: usize = 0x18;

const XHCI_OP_USBCMD: usize = 0x00;
const XHCI_OP_USBSTS: usize = 0x04;
const XHCI_OP_PAGESIZE: usize = 0x08;
const XHCI_OP_DNCTRL: usize = 0x14;
const XHCI_OP_CRCR: usize = 0x18;
const XHCI_OP_DCBAAP: usize = 0x30;
const XHCI_OP_CONFIG: usize = 0x38;
const XHCI_OP_PORTS_BASE: usize = 0x400;
const XHCI_PORT_REGISTER_STRIDE: usize = 0x10;

/// Read-only xHCI capability-register snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciCapabilities {
    /// Byte offset from MMIO base to xHCI operational registers.
    pub cap_length: u8,
    /// Raw xHCI interface version field, for example `0x0100`.
    pub hci_version: u16,
    /// Number of device slots supported by the controller.
    pub max_device_slots: u8,
    /// Number of interrupters supported by the controller.
    pub max_interrupters: u16,
    /// Number of root-hub ports.
    pub max_ports: u8,
    /// Raw HCCPARAMS1 capability flags.
    pub hcc_params1: u32,
    /// Doorbell-array offset from MMIO base.
    pub doorbell_offset: u32,
    /// Runtime-register-space offset from MMIO base.
    pub runtime_register_space_offset: u32,
}

/// Read-only xHCI operational-register snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciOperationalSnapshot {
    /// Raw USB Command register.
    pub usb_command: u32,
    /// Raw USB Status register.
    pub usb_status: u32,
    /// Raw Page Size register.
    pub page_size: u32,
    /// Raw Device Notification Control register.
    pub device_notification_control: u32,
    /// Raw Command Ring Control register.
    pub command_ring_control: u64,
    /// Raw Device Context Base Address Array Pointer register.
    pub device_context_base_address_array_pointer: u64,
    /// Raw Configure register.
    pub configure: u32,
    /// Number of enabled device slots requested in the Configure register.
    pub enabled_device_slots: u8,
}

/// Read-only xHCI port status snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciPortSnapshot {
    /// One-based xHCI root-hub port number.
    pub port: u8,
    /// Raw Port Status and Control register.
    pub port_status_control: u32,
    /// Current Connect Status.
    pub connected: bool,
    /// Port Enabled/Disabled.
    pub enabled: bool,
    /// Port Power.
    pub powered: bool,
    /// Port Link State field.
    pub link_state: u8,
    /// Port Speed field.
    pub speed: u8,
    /// Port Reset bit.
    pub reset_active: bool,
}

/// xHCI controller discovered behind the BCM2711 PCIe root complex.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PcieXhciController {
    /// PCI bus/device/function location.
    pub location: PciLocation,
    /// PCI vendor ID.
    pub vendor_id: u16,
    /// PCI device ID.
    pub device_id: u16,
    /// PCI revision ID.
    pub revision_id: u8,
    /// ARM-physical MMIO base translated from BAR0 when configured.
    pub mmio_base: Option<usize>,
}

impl PcieXhciController {
    /// Builds a PCIe xHCI summary from a PCI config header.
    #[must_use]
    pub const fn from_config_header(header: PciConfigHeader) -> Option<Self> {
        if !header.is_xhci() {
            return None;
        }
        Some(Self {
            location: header.location,
            vendor_id: header.vendor_id,
            device_id: header.device_id,
            revision_id: header.revision_id,
            mmio_base: header.bar0_cpu_memory_base(),
        })
    }
}

/// Reads the BCM2711 built-in xHCI capability registers.
///
/// Returns `None` when the register block does not look like an xHCI
/// controller. This is an early probe only; it does not start the controller or
/// imply a keyboard is attached.
#[must_use]
pub fn read_builtin_xhci_capabilities() -> Option<XhciCapabilities> {
    // SAFETY: the address is the BCM2711 built-in xHCI MMIO window translated
    // from the firmware DTB bus address into the ARM-physical peripheral map.
    unsafe { read_xhci_capabilities_at(BCM2711_XHCI_MMIO_BASE) }
}

/// Reads xHCI capability registers at an already discovered MMIO base.
///
/// Returns `None` when the register block does not look like xHCI. This is a
/// read-only snapshot helper; it does not start or reset the controller.
#[must_use]
pub fn read_xhci_capabilities_at_mmio(base: usize) -> Option<XhciCapabilities> {
    // SAFETY: callers pass a BAR/MMIO base discovered below this bridge.
    unsafe { read_xhci_capabilities_at(base) }
}

/// Reads xHCI operational registers at an already discovered MMIO base.
#[must_use]
pub fn read_xhci_operational_snapshot(
    base: usize,
    caps: XhciCapabilities,
) -> XhciOperationalSnapshot {
    let op_base = base + caps.cap_length as usize;
    decode_xhci_operational_snapshot(
        read_mmio_u32(op_base + XHCI_OP_USBCMD),
        read_mmio_u32(op_base + XHCI_OP_USBSTS),
        read_mmio_u32(op_base + XHCI_OP_PAGESIZE),
        read_mmio_u32(op_base + XHCI_OP_DNCTRL),
        read_mmio_u64(op_base + XHCI_OP_CRCR),
        read_mmio_u64(op_base + XHCI_OP_DCBAAP),
        read_mmio_u32(op_base + XHCI_OP_CONFIG),
    )
}

/// Reads one xHCI port snapshot by one-based port number.
#[must_use]
pub fn read_xhci_port_snapshot(
    base: usize,
    caps: XhciCapabilities,
    port: u8,
) -> Option<XhciPortSnapshot> {
    if port == 0 || port > caps.max_ports {
        return None;
    }
    let op_base = base + caps.cap_length as usize;
    let portsc = read_mmio_u32(
        op_base + XHCI_OP_PORTS_BASE + ((port as usize - 1) * XHCI_PORT_REGISTER_STRIDE),
    );
    Some(decode_xhci_port_snapshot(port, portsc))
}

/// Probes for the Pi 4 PCIe-attached xHCI controller.
///
/// This discovers an xHCI PCI function and any already configured BAR0 MMIO
/// base. It does not reset the controller, enumerate USB devices, or imply a
/// keyboard is attached.
#[must_use]
pub fn probe_pcie_xhci_controller() -> Option<PcieXhciController> {
    let header = pcie::read_first_xhci_config_header()?;
    PcieXhciController::from_config_header(header)
}

unsafe fn read_xhci_capabilities_at(base: usize) -> Option<XhciCapabilities> {
    let cap_hci_version = read_mmio_u32(base + XHCI_CAP_HCIVERSION);
    let hcs_params1 = read_mmio_u32(base + XHCI_HCSPARAMS1);
    let hcc_params1 = read_mmio_u32(base + XHCI_HCCPARAMS1);
    let dboff = read_mmio_u32(base + XHCI_DBOFF);
    let rtsoff = read_mmio_u32(base + XHCI_RTSOFF);
    decode_xhci_capabilities(cap_hci_version, hcs_params1, hcc_params1, dboff, rtsoff)
}

fn decode_xhci_capabilities(
    cap_hci_version: u32,
    hcs_params1: u32,
    hcc_params1: u32,
    dboff: u32,
    rtsoff: u32,
) -> Option<XhciCapabilities> {
    let cap_length = (cap_hci_version & 0xff) as u8;
    let hci_version = (cap_hci_version >> 16) as u16;

    if !(0x20..=0x100).contains(&(cap_length as usize)) {
        return None;
    }
    if hci_version == 0 || hci_version == 0xffff {
        return None;
    }

    Some(XhciCapabilities {
        cap_length,
        hci_version,
        max_device_slots: (hcs_params1 & 0xff) as u8,
        max_interrupters: ((hcs_params1 >> 8) & 0x7ff) as u16,
        max_ports: ((hcs_params1 >> 24) & 0xff) as u8,
        hcc_params1,
        doorbell_offset: dboff & !0x3,
        runtime_register_space_offset: rtsoff & !0x1f,
    })
}

fn decode_xhci_operational_snapshot(
    usb_command: u32,
    usb_status: u32,
    page_size: u32,
    device_notification_control: u32,
    command_ring_control: u64,
    device_context_base_address_array_pointer: u64,
    configure: u32,
) -> XhciOperationalSnapshot {
    XhciOperationalSnapshot {
        usb_command,
        usb_status,
        page_size,
        device_notification_control,
        command_ring_control,
        device_context_base_address_array_pointer,
        configure,
        enabled_device_slots: (configure & 0xff) as u8,
    }
}

fn decode_xhci_port_snapshot(port: u8, port_status_control: u32) -> XhciPortSnapshot {
    XhciPortSnapshot {
        port,
        port_status_control,
        connected: port_status_control & 0x1 != 0,
        enabled: port_status_control & 0x2 != 0,
        powered: port_status_control & (1 << 9) != 0,
        link_state: ((port_status_control >> 5) & 0xf) as u8,
        speed: ((port_status_control >> 10) & 0xf) as u8,
        reset_active: port_status_control & (1 << 4) != 0,
    }
}

fn read_mmio_u32(addr: usize) -> u32 {
    // SAFETY: callers pass native MMIO register addresses; volatile access is
    // the required mechanism for device registers.
    unsafe { read_volatile(addr as *const u32) }
}

fn read_mmio_u64(addr: usize) -> u64 {
    let lo = read_mmio_u32(addr) as u64;
    let hi = read_mmio_u32(addr + 4) as u64;
    lo | (hi << 32)
}

#[cfg(feature = "selftest")]
#[path = "usb_tests.rs"]
mod tests;
