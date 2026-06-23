//! BCM2711 USB host-controller raw facts and early probe helpers.
//!
//! This module stays below the system-kernel bridge. It reads only native MMIO
//! controller registers and returns target-local facts; USB enumeration, HID
//! descriptor walking, and root-shell byte routing are later cuts.

use core::ptr::read_volatile;

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

fn read_mmio_u32(addr: usize) -> u32 {
    // SAFETY: callers pass native MMIO register addresses; volatile access is
    // the required mechanism for device registers.
    unsafe { read_volatile(addr as *const u32) }
}

#[cfg(feature = "selftest")]
#[path = "usb_tests.rs"]
mod tests;
