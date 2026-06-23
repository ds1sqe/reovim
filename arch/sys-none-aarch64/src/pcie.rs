//! BCM2711 PCIe root-complex raw facts and discovery helpers.
//!
//! This module stays below the system-kernel bridge. It touches only the
//! BCM2711 root-complex MMIO block and PCI configuration space; USB
//! descriptor walking and keyboard input routing are later cuts.

use core::ptr::{read_volatile, write_volatile};

/// BCM2711 PCIe root-complex bus address from the firmware DTB.
pub const BCM2711_PCIE_BUS_BASE: u64 = 0x7d50_0000;
/// BCM2711 PCIe root-complex ARM-physical MMIO base.
pub const BCM2711_PCIE_MMIO_BASE: usize = 0xfd50_0000;
/// BCM2711 PCIe root-complex MMIO window length.
pub const BCM2711_PCIE_MMIO_LEN: usize = 0x9310;

/// PCIe memory-space address used by the Pi 4 DTB for endpoint BARs.
pub const BCM2711_PCIE_MEM_BUS_BASE: u64 = 0xc000_0000;
/// ARM-physical address that maps [`BCM2711_PCIE_MEM_BUS_BASE`].
pub const BCM2711_PCIE_MEM_CPU_BASE: u64 = 0x6_0000_0000;
/// Length of the Pi 4 PCIe outbound memory window.
pub const BCM2711_PCIE_MEM_LEN: u64 = 0x4000_0000;

const PCIE_EXT_CFG_DATA: usize = 0x8000;
const PCIE_EXT_CFG_INDEX: usize = 0x9000;
const PCIE_MISC_PCIE_STATUS: usize = 0x4068;
const PCIE_MISC_REVISION: usize = 0x406c;

const PCIE_STATUS_PHY_LINK_UP: u32 = 0x10;
const PCIE_STATUS_DATA_LINK_ACTIVE: u32 = 0x20;
const PCIE_STATUS_LINK_IN_L23: u32 = 0x40;
const PCIE_STATUS_ROOT_COMPLEX_MODE: u32 = 0x80;

const PCI_VENDOR_ID_ABSENT: u16 = 0xffff;
const PCI_CLASS_SERIAL_BUS: u8 = 0x0c;
const PCI_SUBCLASS_USB: u8 = 0x03;
const PCI_PROG_IF_XHCI: u8 = 0x30;

/// Snapshot of the BCM2711 root-complex link status registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PcieLinkStatus {
    /// Raw `PCIE_MISC_PCIE_STATUS` register value.
    pub raw_status: u32,
    /// Raw `PCIE_MISC_REVISION` register value.
    pub revision: u32,
    /// The controller currently reports root-complex mode.
    pub root_complex_mode: bool,
    /// The physical PCIe link is up.
    pub phy_link_up: bool,
    /// The data link layer is active.
    pub data_link_active: bool,
    /// The link is in L2/L3 low-power state.
    pub link_in_l23: bool,
}

impl PcieLinkStatus {
    /// Returns true when the endpoint config space can be probed without the
    /// known link-down abort path.
    #[must_use]
    pub const fn link_up(self) -> bool {
        self.phy_link_up && self.data_link_active
    }
}

/// PCI bus/device/function tuple for configuration-space reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciLocation {
    /// PCI bus number.
    pub bus: u8,
    /// PCI device number on `bus` (`0..32`).
    pub device: u8,
    /// PCI function number on `device` (`0..8`).
    pub function: u8,
}

impl PciLocation {
    /// Builds a PCI location, rejecting impossible device/function numbers.
    #[must_use]
    pub const fn new(bus: u8, device: u8, function: u8) -> Option<Self> {
        if device < 32 && function < 8 {
            Some(Self {
                bus,
                device,
                function,
            })
        } else {
            None
        }
    }

    const fn devfn(self) -> u8 {
        (self.device << 3) | self.function
    }
}

/// Minimal PCI configuration header fields needed to find xHCI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciConfigHeader {
    /// Bus/device/function tuple used for this header read.
    pub location: PciLocation,
    /// PCI vendor ID.
    pub vendor_id: u16,
    /// PCI device ID.
    pub device_id: u16,
    /// PCI command register.
    pub command: u16,
    /// PCI status register.
    pub status: u16,
    /// PCI revision ID.
    pub revision_id: u8,
    /// PCI programming-interface byte.
    pub prog_if: u8,
    /// PCI subclass byte.
    pub subclass: u8,
    /// PCI base-class byte.
    pub class_code: u8,
    /// Raw BAR0 value.
    pub bar0: u32,
    /// Raw BAR1 value, used when BAR0 is 64-bit memory.
    pub bar1: u32,
}

impl PciConfigHeader {
    /// Whether this header represents an xHCI USB host controller.
    #[must_use]
    pub const fn is_xhci(self) -> bool {
        self.class_code == PCI_CLASS_SERIAL_BUS
            && self.subclass == PCI_SUBCLASS_USB
            && self.prog_if == PCI_PROG_IF_XHCI
    }

    /// Endpoint BAR0 memory address in PCIe bus address space.
    #[must_use]
    pub const fn bar0_bus_memory_base(self) -> Option<u64> {
        pci_memory_bar_base(self.bar0, self.bar1)
    }

    /// Endpoint BAR0 memory address translated to ARM-physical space.
    #[must_use]
    pub const fn bar0_cpu_memory_base(self) -> Option<usize> {
        let Some(bus_base) = self.bar0_bus_memory_base() else {
            return None;
        };
        pcie_bus_memory_to_cpu(bus_base)
    }
}

/// Reads the BCM2711 root-complex link status.
#[must_use]
pub fn read_builtin_pcie_link_status() -> PcieLinkStatus {
    read_pcie_link_status_at(BCM2711_PCIE_MMIO_BASE)
}

fn read_pcie_link_status_at(base: usize) -> PcieLinkStatus {
    decode_pcie_link_status(
        read_mmio_u32(base + PCIE_MISC_PCIE_STATUS),
        read_mmio_u32(base + PCIE_MISC_REVISION),
    )
}

fn decode_pcie_link_status(raw_status: u32, revision: u32) -> PcieLinkStatus {
    PcieLinkStatus {
        raw_status,
        revision,
        root_complex_mode: raw_status & PCIE_STATUS_ROOT_COMPLEX_MODE != 0,
        phy_link_up: raw_status & PCIE_STATUS_PHY_LINK_UP != 0,
        data_link_active: raw_status & PCIE_STATUS_DATA_LINK_ACTIVE != 0,
        link_in_l23: raw_status & PCIE_STATUS_LINK_IN_L23 != 0,
    }
}

/// Reads one external PCI configuration header through the BCM2711 root
/// complex.
///
/// Returns `None` when the link is down or when the config read returns the
/// all-ones absent-device vendor ID. The helper writes only the root-complex
/// config-space index register before reading the config data window.
#[must_use]
pub fn read_external_config_header(location: PciLocation) -> Option<PciConfigHeader> {
    if !read_builtin_pcie_link_status().link_up() {
        return None;
    }
    read_external_config_header_at(BCM2711_PCIE_MMIO_BASE, location)
}

fn read_external_config_header_at(base: usize, location: PciLocation) -> Option<PciConfigHeader> {
    write_mmio_u32(base + PCIE_EXT_CFG_INDEX, pcie_external_config_index(location));

    let id = read_mmio_u32(base + PCIE_EXT_CFG_DATA);
    let vendor_id = (id & 0xffff) as u16;
    if vendor_id == PCI_VENDOR_ID_ABSENT {
        return None;
    }

    let command_status = read_mmio_u32(base + PCIE_EXT_CFG_DATA + 0x04);
    let class_revision = read_mmio_u32(base + PCIE_EXT_CFG_DATA + 0x08);
    Some(PciConfigHeader {
        location,
        vendor_id,
        device_id: (id >> 16) as u16,
        command: (command_status & 0xffff) as u16,
        status: (command_status >> 16) as u16,
        revision_id: (class_revision & 0xff) as u8,
        prog_if: ((class_revision >> 8) & 0xff) as u8,
        subclass: ((class_revision >> 16) & 0xff) as u8,
        class_code: ((class_revision >> 24) & 0xff) as u8,
        bar0: read_mmio_u32(base + PCIE_EXT_CFG_DATA + 0x10),
        bar1: read_mmio_u32(base + PCIE_EXT_CFG_DATA + 0x14),
    })
}

/// Scans the first two downstream PCI bus numbers for an xHCI function.
///
/// Raspberry Pi 4 firmware/device-tree variants disagree in how much bus
/// numbering they predescribe, so this deliberately keeps the scan small but
/// covers the bus-0 and bus-1 shapes seen in Pi 4 descriptions.
#[must_use]
pub fn read_first_xhci_config_header() -> Option<PciConfigHeader> {
    let mut bus = 0u8;
    while bus <= 1 {
        let mut device = 0u8;
        while device < 32 {
            let mut function = 0u8;
            while function < 8 {
                if let Some(location) = PciLocation::new(bus, device, function) {
                    if let Some(header) = read_external_config_header(location) {
                        if header.is_xhci() {
                            return Some(header);
                        }
                    }
                }
                function += 1;
            }
            device += 1;
        }
        bus += 1;
    }
    None
}

const fn pcie_external_config_index(location: PciLocation) -> u32 {
    ((location.bus as u32) << 20) | ((location.devfn() as u32) << 12)
}

const fn pci_memory_bar_base(bar0: u32, bar1: u32) -> Option<u64> {
    if bar0 == 0 || bar0 == u32::MAX || (bar0 & 0x1) != 0 {
        return None;
    }

    let bar_type = (bar0 >> 1) & 0x3;
    if bar_type == 0 {
        Some((bar0 & 0xffff_fff0) as u64)
    } else if bar_type == 0x2 {
        Some(((bar1 as u64) << 32) | ((bar0 & 0xffff_fff0) as u64))
    } else {
        None
    }
}

const fn pcie_bus_memory_to_cpu(bus_base: u64) -> Option<usize> {
    let Some(offset) = bus_base.checked_sub(BCM2711_PCIE_MEM_BUS_BASE) else {
        return None;
    };
    if offset >= BCM2711_PCIE_MEM_LEN {
        return None;
    }
    let Some(cpu_base) = BCM2711_PCIE_MEM_CPU_BASE.checked_add(offset) else {
        return None;
    };
    if cpu_base > usize::MAX as u64 {
        return None;
    }
    Some(cpu_base as usize)
}

fn read_mmio_u32(addr: usize) -> u32 {
    // SAFETY: callers pass native MMIO register addresses; volatile access is
    // the required mechanism for device registers.
    unsafe { read_volatile(addr as *const u32) }
}

fn write_mmio_u32(addr: usize, value: u32) {
    // SAFETY: callers pass native MMIO register addresses; volatile access is
    // the required mechanism for device registers.
    unsafe {
        write_volatile(addr as *mut u32, value);
    }
}

#[cfg(feature = "selftest")]
#[path = "pcie_tests.rs"]
mod tests;
