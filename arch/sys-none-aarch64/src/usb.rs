//! BCM2711 USB host-controller raw facts and early probe helpers.
//!
//! This module stays below the system-kernel bridge. It reads only native MMIO
//! controller registers and returns target-local facts; USB enumeration, HID
//! descriptor walking, and root-shell byte routing are later cuts.

#[cfg(target_arch = "aarch64")]
use core::arch::asm;
use core::{
    cell::UnsafeCell,
    ptr::{read_volatile, write_volatile},
    sync::atomic::{Ordering, compiler_fence},
};

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
const XHCI_HCSPARAMS2: usize = 0x08;
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
const XHCI_RUNTIME_INTERRUPTER0: usize = 0x20;
const XHCI_INTR_IMAN: usize = 0x00;
const XHCI_INTR_IMOD: usize = 0x04;
const XHCI_INTR_ERSTSZ: usize = 0x08;
const XHCI_INTR_ERSTBA: usize = 0x10;
const XHCI_INTR_ERDP: usize = 0x18;

const XHCI_TRB_BYTES: usize = 16;
const XHCI_TRB_CYCLE: u32 = 1 << 0;
const XHCI_TRB_TYPE_SHIFT: u32 = 10;
const XHCI_TRB_TYPE_MASK: u32 = 0x3f;
const XHCI_TRB_COMPLETION_CODE_SHIFT: u32 = 24;
const XHCI_ENABLE_SLOT_SLOT_TYPE_SHIFT: u32 = 16;
const XHCI_ENABLE_SLOT_SLOT_TYPE_MASK: u8 = 0x1f;
const XHCI_TRB_TYPE_ENABLE_SLOT: u8 = 9;
const XHCI_TRB_TYPE_COMMAND_COMPLETION_EVENT: u8 = 33;
const XHCI_TRB_COMPLETION_SUCCESS: u8 = 1;
const XHCI_DOORBELL_COMMAND: u32 = 0;
const XHCI_ERDP_EVENT_HANDLER_BUSY: u64 = 1 << 3;

const XHCI_USBCMD_RUN_STOP: u32 = 1 << 0;
const XHCI_USBCMD_HOST_CONTROLLER_RESET: u32 = 1 << 1;
const XHCI_USBCMD_INTERRUPTER_ENABLE: u32 = 1 << 2;

const XHCI_USBSTS_HALTED: u32 = 1 << 0;
const XHCI_USBSTS_HOST_SYSTEM_ERROR: u32 = 1 << 2;
const XHCI_USBSTS_EVENT_INTERRUPT: u32 = 1 << 3;
const XHCI_USBSTS_PORT_CHANGE_DETECT: u32 = 1 << 4;
const XHCI_USBSTS_CONTROLLER_NOT_READY: u32 = 1 << 11;

/// Length of a USB HID boot-keyboard input report.
pub const BOOT_KEYBOARD_REPORT_BYTES: usize = 8;

/// Number of TRBs in the first command-ring segment.
pub const XHCI_COMMAND_RING_TRBS: usize = 64;
/// Number of TRBs in the first event-ring segment.
pub const XHCI_EVENT_RING_TRBS: usize = 64;
/// Number of Event Ring Segment Table entries prepared by the early provider.
pub const XHCI_EVENT_RING_SEGMENT_TABLE_ENTRIES: usize = 1;
/// Maximum scratchpad buffers backed by static early-driver storage.
pub const XHCI_STATIC_SCRATCHPAD_BUFFERS: usize = 8;
/// xHCI scratchpad buffer/page size used by the freestanding arena.
pub const XHCI_PAGE_BYTES: usize = 4096;

const XHCI_MAX_DEVICE_CONTEXT_POINTERS: usize = 256;
const XHCI_HCCPARAMS1_CONTEXT_SIZE: u32 = 1 << 2;
const XHCI_HCCPARAMS1_XECP_SHIFT: u32 = 16;
const XHCI_START_WAIT_SPINS: usize = 1_000_000;
const XHCI_COMMAND_WAIT_SPINS: usize = 2_000_000;
const XHCI_DMA_CACHE_LINE_BYTES: usize = 64;
const XHCI_REQUIRED_PCI_COMMAND_BITS: u16 =
    pcie::PCI_COMMAND_MEMORY_SPACE | pcie::PCI_COMMAND_BUS_MASTER;
const XHCI_EXT_CAP_ID_SUPPORTED_PROTOCOL: u8 = 2;
const XHCI_EXT_CAP_MAX_STEPS: usize = 32;

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
    /// Raw HCSPARAMS2 structural flags.
    pub hcs_params2: u32,
    /// Doorbell-array offset from MMIO base.
    pub doorbell_offset: u32,
    /// Runtime-register-space offset from MMIO base.
    pub runtime_register_space_offset: u32,
    /// Maximum number of Event Ring Segment Table entries the xHC reports.
    pub event_ring_segment_table_max: u16,
    /// Number of scratchpad buffers requested by the xHC.
    pub max_scratchpad_buffers: u16,
    /// Whether scratchpad buffers must survive power events for restore.
    pub scratchpad_restore: bool,
    /// Size in bytes of each xHCI context structure.
    pub context_size_bytes: u8,
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
    /// USBCMD Run/Stop bit.
    pub run_stop: bool,
    /// USBCMD Host Controller Reset bit.
    pub reset_active: bool,
    /// USBCMD Interrupter Enable bit.
    pub interrupter_enable: bool,
    /// USBSTS Host Controller Halted bit.
    pub halted: bool,
    /// USBSTS Host System Error bit.
    pub host_system_error: bool,
    /// USBSTS Event Interrupt bit.
    pub event_interrupt: bool,
    /// USBSTS Port Change Detect bit.
    pub port_change_detect: bool,
    /// USBSTS Controller Not Ready bit.
    pub controller_not_ready: bool,
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

/// Read-only xHCI Supported Protocol extended-capability snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciSupportedProtocol {
    /// MMIO offset of this xHCI extended capability from controller base.
    pub offset: u32,
    /// Four-byte protocol name string.
    pub name: [u8; 4],
    /// BCD major revision from the capability header.
    pub major_revision: u8,
    /// BCD minor revision from the capability header.
    pub minor_revision: u8,
    /// First one-based root-hub port covered by this protocol.
    pub compatible_port_offset: u8,
    /// Number of consecutive root-hub ports covered by this protocol.
    pub compatible_port_count: u8,
    /// Protocol-defined field from offset 08h.
    pub protocol_defined: u16,
    /// Number of Protocol Speed ID dwords that follow the base structure.
    pub protocol_speed_id_count: u8,
    /// Slot Type value to put in an Enable Slot command for this protocol.
    pub protocol_slot_type: u8,
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

/// Early xHCI driver-memory addresses for the controller start path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciDriverMemoryPlan {
    /// Base address for the Device Context Base Address Array.
    pub dcbaa: u64,
    /// Base address for the command-ring segment.
    pub command_ring: u64,
    /// Initial value for CRCR, including the ring-cycle-state bit.
    pub command_ring_control: u64,
    /// Base address for the event-ring segment.
    pub event_ring: u64,
    /// Base address for the Event Ring Segment Table.
    pub event_ring_segment_table: u64,
    /// Number of valid ERST entries.
    pub event_ring_segment_table_entries: u16,
    /// Number of TRBs in the prepared event-ring segment.
    pub event_ring_trbs: u16,
    /// Number of TRBs in the prepared command-ring segment.
    pub command_ring_trbs: u16,
    /// Event-ring dequeue pointer value for ERDP.
    pub event_ring_dequeue_pointer: u64,
    /// Scratchpad buffer array address, or zero when no scratchpads are needed.
    pub scratchpad_array: u64,
    /// Number of scratchpad buffers prepared.
    pub scratchpad_buffers: u16,
    /// MaxSlotsEn value to write while the controller is stopped.
    pub max_slots_enabled: u8,
    /// Size in bytes of each xHCI context structure.
    pub context_size_bytes: u8,
}

/// Result of building an early xHCI driver-memory plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciDriverMemoryStatus {
    /// Static driver memory is sufficient and addresses are ready.
    Ready(XhciDriverMemoryPlan),
    /// The controller requested more scratchpads than this early provider owns.
    TooManyScratchpads {
        /// Number of scratchpads reported by HCSPARAMS2.
        requested: u16,
        /// Number of static scratchpad buffers available.
        supported: u16,
    },
}

/// Register values written by the explicit xHCI controller-start transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciControllerStartRegisters {
    /// Value written to DCBAAP.
    pub device_context_base_address_array_pointer: u64,
    /// Value written to CRCR.
    pub command_ring_control: u64,
    /// Value written to CONFIG.
    pub configure: u32,
    /// Value written to runtime interrupter 0 IMAN.
    pub interrupter_management: u32,
    /// Value written to runtime interrupter 0 IMOD.
    pub interrupter_moderation: u32,
    /// Value written to runtime interrupter 0 ERSTSZ.
    pub event_ring_segment_table_size: u32,
    /// Value written to runtime interrupter 0 ERSTBA.
    pub event_ring_segment_table_base_address: u64,
    /// Value written to runtime interrupter 0 ERDP.
    pub event_ring_dequeue_pointer: u64,
    /// RW1C status bits cleared before starting.
    pub usb_status_clear: u32,
    /// Value written to USBCMD to start the controller.
    pub usb_command: u32,
}

/// Status from the explicit xHCI controller-start transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciControllerStartStatus {
    /// No PCIe-attached xHCI function was discovered.
    NoPcieXhciController,
    /// The xHCI PCI function exists, but BAR0 is not configured.
    ControllerBarUnconfigured,
    /// The xHCI PCI command bits required for MMIO/DMA could not be enabled.
    PciCommandEnableFailed,
    /// The discovered BAR does not expose valid xHCI capability registers.
    InvalidXhciCapabilities,
    /// Static early-driver memory cannot satisfy the controller's scratchpad request.
    DriverMemoryUnavailable {
        /// Number of scratchpads reported by HCSPARAMS2.
        requested: u16,
        /// Number of static scratchpad buffers available.
        supported: u16,
    },
    /// Controller Not Ready stayed asserted before MMIO programming.
    ControllerNotReadyTimedOut,
    /// Run/Stop could not be cleared to reach HCHalted.
    StopTimedOut,
    /// Host Controller Reset did not clear.
    ResetTimedOut,
    /// Controller Not Ready stayed asserted after reset.
    PostResetControllerNotReadyTimedOut,
    /// The controller reported Host System Error after start.
    HostSystemErrorAfterStart,
    /// Run/Stop did not reach the running state after programming registers.
    StartTimedOut,
    /// Registers were programmed and the controller reported running.
    Started,
}

/// Evidence returned by the explicit xHCI controller-start transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciControllerStartReport {
    /// Final transition status.
    pub status: XhciControllerStartStatus,
    /// PCI command register before enabling Memory Space/Bus Master, if known.
    pub pcie_command_before: Option<u16>,
    /// PCI command register after enabling Memory Space/Bus Master, if known.
    pub pcie_command_after: Option<u16>,
    /// ARM-physical MMIO base used by the transition, if discovered.
    pub mmio_base: Option<usize>,
    /// Capability snapshot used by the transition, if decoded.
    pub capabilities: Option<XhciCapabilities>,
    /// Operational-register snapshot before changing xHCI state, if available.
    pub before: Option<XhciOperationalSnapshot>,
    /// Operational-register snapshot after the last transition step, if available.
    pub after: Option<XhciOperationalSnapshot>,
    /// Static memory plan used for the transition, if available.
    pub memory: Option<XhciDriverMemoryPlan>,
    /// Register values written during start, if MMIO programming was reached.
    pub registers: Option<XhciControllerStartRegisters>,
}

impl XhciControllerStartReport {
    const fn new(status: XhciControllerStartStatus) -> Self {
        Self {
            status,
            pcie_command_before: None,
            pcie_command_after: None,
            mmio_base: None,
            capabilities: None,
            before: None,
            after: None,
            memory: None,
            registers: None,
        }
    }
}

/// Decoded xHCI Command Completion Event TRB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciCommandCompletionEvent {
    /// Raw TRB dwords as DMA-written by the xHC.
    pub raw: [u32; 4],
    /// Command TRB pointer reported by the completion event.
    pub command_trb_pointer: u64,
    /// xHCI completion code.
    pub completion_code: u8,
    /// Event TRB type field.
    pub trb_type: u8,
    /// Event ring cycle bit.
    pub cycle: bool,
    /// Slot ID assigned by the completed command, if any.
    pub slot_id: u8,
}

/// Status from issuing the first manual xHCI Enable Slot command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciEnableSlotStatus {
    /// Controller start failed before a command could be issued.
    ControllerStartFailed(XhciControllerStartStatus),
    /// The start report was internally incomplete.
    StartEvidenceUnavailable,
    /// The controller did not remain in a running state after start.
    ControllerNotRunning,
    /// No Command Completion Event reached event-ring entry zero.
    CommandTimedOut,
    /// A cycle-valid event arrived, but it was not a command-completion event.
    UnexpectedEventType {
        /// Event TRB type field.
        trb_type: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The completion event did not point back at the Enable Slot command TRB.
    CommandPointerMismatch {
        /// Expected command TRB pointer.
        expected: u64,
        /// Actual pointer reported by the event.
        actual: u64,
        /// xHCI completion code carried by the event.
        completion_code: u8,
        /// Slot ID carried by the event.
        slot_id: u8,
    },
    /// The command completed with a non-success code or no assigned slot.
    CommandFailed {
        /// xHCI completion code.
        completion_code: u8,
        /// Slot ID carried by the event.
        slot_id: u8,
    },
    /// Enable Slot completed successfully and returned an assigned slot ID.
    SlotEnabled {
        /// Assigned xHCI slot ID.
        slot_id: u8,
    },
}

/// Evidence returned by the first manual xHCI Enable Slot command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciEnableSlotReport {
    /// Controller-start report produced before issuing the command.
    pub start: XhciControllerStartReport,
    /// Final command status.
    pub status: XhciEnableSlotStatus,
    /// Address of the command TRB written into the command ring.
    pub command_trb_pointer: u64,
    /// Connected root port used to choose the protocol Slot Type, if found.
    pub connected_port: Option<XhciPortSnapshot>,
    /// Supported Protocol capability selected for the connected port, if found.
    pub protocol: Option<XhciSupportedProtocol>,
    /// Slot Type value encoded in the command TRB.
    pub slot_type: u8,
    /// Raw Enable Slot command TRB written by software.
    pub command_trb: [u32; 4],
    /// Doorbell value written to doorbell 0.
    pub doorbell: u32,
    /// First command-completion event observed, if one arrived.
    pub event: Option<XhciCommandCompletionEvent>,
}

impl XhciEnableSlotReport {
    const fn new(start: XhciControllerStartReport, status: XhciEnableSlotStatus) -> Self {
        Self {
            start,
            status,
            command_trb_pointer: 0,
            connected_port: None,
            protocol: None,
            slot_type: 0,
            command_trb: [0; 4],
            doorbell: XHCI_DOORBELL_COMMAND,
            event: None,
        }
    }
}

/// Nonblocking boot-keyboard provider poll result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsbBootKeyboardPoll {
    /// One raw 8-byte HID boot-keyboard report was read.
    Report([u8; BOOT_KEYBOARD_REPORT_BYTES]),
    /// The provider is not yet able to return reports.
    Pending(UsbBootKeyboardPending),
}

/// Reason the boot-keyboard provider cannot currently return reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsbBootKeyboardPending {
    /// The xHC reports Controller Not Ready.
    ControllerNotReady,
    /// A Host Controller Reset is in progress.
    ControllerResetInProgress,
    /// The xHC reports a host-system error.
    HostSystemError,
    /// No PCIe-attached xHCI function was discovered.
    NoPcieXhciController,
    /// An xHCI PCI function exists, but BAR0 is not configured.
    ControllerBarUnconfigured,
    /// The discovered BAR does not expose valid xHCI capability registers.
    InvalidXhciCapabilities,
    /// No connected xHCI root-hub port is visible.
    NoConnectedRootPort,
    /// A connected root port exists, but the xHC still needs driver-owned
    /// command/event/context memory and MaxSlots configuration.
    NeedsControllerInitialization {
        /// Maximum slots the controller reports in HCSPARAMS1.
        max_slots: u8,
        /// One-based xHCI root-hub port number.
        port: u8,
        /// Raw xHCI port-speed field.
        speed: u8,
        /// Raw xHCI port-link-state field.
        link_state: u8,
    },
    /// A keyboard may exist, but descriptor enumeration and transfer-ring
    /// setup have not landed yet.
    NeedsEnumeration {
        /// One-based xHCI root-hub port number.
        port: u8,
        /// Raw xHCI port-speed field.
        speed: u8,
        /// Raw xHCI port-link-state field.
        link_state: u8,
    },
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
    let op_base = xhci_operational_base(base, caps);
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
    let op_base = xhci_operational_base(base, caps);
    let portsc = read_mmio_u32(
        op_base + XHCI_OP_PORTS_BASE + ((port as usize - 1) * XHCI_PORT_REGISTER_STRIDE),
    );
    Some(decode_xhci_port_snapshot(port, portsc))
}

/// Reads the xHCI Supported Protocol capability that covers a root-hub port.
#[must_use]
pub fn read_xhci_supported_protocol_for_port(
    base: usize,
    caps: XhciCapabilities,
    port: u8,
) -> Option<XhciSupportedProtocol> {
    let mut offset = xhci_extended_capability_offset(caps)?;
    let mut steps = 0usize;
    while steps < XHCI_EXT_CAP_MAX_STEPS {
        let header = read_mmio_u32(base + offset);
        let capability_id = (header & 0xff) as u8;
        if capability_id == XHCI_EXT_CAP_ID_SUPPORTED_PROTOCOL {
            if let Some(protocol) = read_xhci_supported_protocol_at(base, offset as u32, header) {
                if protocol_covers_port(protocol, port) {
                    return Some(protocol);
                }
            }
        }

        let next = ((header >> 8) & 0xff) as usize;
        if next == 0 {
            return None;
        }
        offset += next * core::mem::size_of::<u32>();
        steps += 1;
    }
    None
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

/// Performs the first explicit PCIe xHCI controller-start transition.
///
/// This is intentionally not called by passive probing. It enables the PCI
/// function's Memory Space and Bus Master command bits, prepares static
/// driver-owned xHCI memory, halts/resets the xHC, programs DCBAAP/CRCR/CONFIG
/// and primary event-ring registers, then sets Run/Stop. It does not ring the
/// command doorbell, issue Enable Slot, enumerate descriptors, or report USB
/// keyboard readiness.
pub fn start_pcie_xhci_controller() -> XhciControllerStartReport {
    let Some(header) = pcie::read_first_xhci_config_header() else {
        return XhciControllerStartReport::new(XhciControllerStartStatus::NoPcieXhciController);
    };

    let mut report = XhciControllerStartReport::new(XhciControllerStartStatus::Started);
    report.pcie_command_before = Some(header.command);

    let Some(mmio) = header.bar0_cpu_memory_base() else {
        report.status = XhciControllerStartStatus::ControllerBarUnconfigured;
        return report;
    };
    report.mmio_base = Some(mmio);

    let Some(command_after) =
        pcie::enable_external_command_bits(header.location, XHCI_REQUIRED_PCI_COMMAND_BITS)
    else {
        report.status = XhciControllerStartStatus::PciCommandEnableFailed;
        return report;
    };
    report.pcie_command_after = Some(command_after);
    if command_after & XHCI_REQUIRED_PCI_COMMAND_BITS != XHCI_REQUIRED_PCI_COMMAND_BITS {
        report.status = XhciControllerStartStatus::PciCommandEnableFailed;
        return report;
    }

    let Some(caps) = read_xhci_capabilities_at_mmio(mmio) else {
        report.status = XhciControllerStartStatus::InvalidXhciCapabilities;
        return report;
    };
    report.capabilities = Some(caps);

    let plan = match prepare_xhci_driver_memory(caps) {
        XhciDriverMemoryStatus::Ready(plan) => plan,
        XhciDriverMemoryStatus::TooManyScratchpads {
            requested,
            supported,
        } => {
            report.status = XhciControllerStartStatus::DriverMemoryUnavailable {
                requested,
                supported,
            };
            return report;
        }
    };
    report.memory = Some(plan);

    let mut current = read_xhci_operational_snapshot(mmio, caps);
    report.before = Some(current);
    if current.controller_not_ready {
        let Some(ready) = wait_for_xhci(mmio, caps, |op| !op.controller_not_ready) else {
            report.status = XhciControllerStartStatus::ControllerNotReadyTimedOut;
            report.after = Some(read_xhci_operational_snapshot(mmio, caps));
            return report;
        };
        current = ready;
    }

    if current.run_stop || !current.halted {
        write_mmio_u32(xhci_operational_base(mmio, caps) + XHCI_OP_USBCMD, 0);
        let Some(_stopped) = wait_for_xhci(mmio, caps, |op| op.halted) else {
            report.status = XhciControllerStartStatus::StopTimedOut;
            report.after = Some(read_xhci_operational_snapshot(mmio, caps));
            return report;
        };
    }

    write_mmio_u32(
        xhci_operational_base(mmio, caps) + XHCI_OP_USBCMD,
        XHCI_USBCMD_HOST_CONTROLLER_RESET,
    );
    let Some(reset_done) = wait_for_xhci(mmio, caps, |op| !op.reset_active) else {
        report.status = XhciControllerStartStatus::ResetTimedOut;
        report.after = Some(read_xhci_operational_snapshot(mmio, caps));
        return report;
    };
    if reset_done.controller_not_ready {
        let Some(_ready) = wait_for_xhci(mmio, caps, |op| !op.controller_not_ready) else {
            report.status = XhciControllerStartStatus::PostResetControllerNotReadyTimedOut;
            report.after = Some(read_xhci_operational_snapshot(mmio, caps));
            return report;
        };
    }

    let registers = xhci_controller_start_registers(plan);
    report.registers = Some(registers);
    clean_xhci_driver_memory_for_device(plan);
    compiler_fence(Ordering::SeqCst);
    write_xhci_start_registers(mmio, caps, registers);

    let Some(after) =
        wait_for_xhci(mmio, caps, |op| op.host_system_error || (op.run_stop && !op.halted))
    else {
        report.status = XhciControllerStartStatus::StartTimedOut;
        report.after = Some(read_xhci_operational_snapshot(mmio, caps));
        return report;
    };

    report.after = Some(after);
    if after.host_system_error {
        report.status = XhciControllerStartStatus::HostSystemErrorAfterStart;
        return report;
    }
    if !after.run_stop || after.halted {
        report.status = XhciControllerStartStatus::StartTimedOut;
        return report;
    }

    report.status = XhciControllerStartStatus::Started;
    report
}

/// Starts the PCIe xHCI controller, issues Enable Slot, and reads completion.
///
/// This is still a manual diagnostics transition. It does not address a device,
/// enumerate descriptors, configure endpoints, poll interrupt-IN transfers, or
/// claim USB keyboard readiness.
pub fn enable_slot_on_pcie_xhci_controller() -> XhciEnableSlotReport {
    let start = start_pcie_xhci_controller();
    if start.status != XhciControllerStartStatus::Started {
        return XhciEnableSlotReport::new(
            start,
            XhciEnableSlotStatus::ControllerStartFailed(start.status),
        );
    }

    let (Some(mmio), Some(caps), Some(plan), Some(after)) =
        (start.mmio_base, start.capabilities, start.memory, start.after)
    else {
        return XhciEnableSlotReport::new(start, XhciEnableSlotStatus::StartEvidenceUnavailable);
    };

    if !after.run_stop || after.halted {
        return XhciEnableSlotReport::new(start, XhciEnableSlotStatus::ControllerNotRunning);
    }

    let connected_port = first_connected_xhci_port(mmio, caps);
    let protocol = connected_port
        .and_then(|port| read_xhci_supported_protocol_for_port(mmio, caps, port.port));
    let slot_type = protocol
        .map(|protocol| protocol.protocol_slot_type)
        .unwrap_or(0);

    let command_trb_pointer = XHCI_COMMAND_RING.trb_addr(0);
    let command_trb = xhci_enable_slot_command_trb(slot_type);
    XHCI_COMMAND_RING.set_trb(0, command_trb);
    dma_clean_range(command_trb_pointer, XHCI_TRB_BYTES);

    let mut report = XhciEnableSlotReport {
        start,
        status: XhciEnableSlotStatus::CommandTimedOut,
        command_trb_pointer,
        connected_port,
        protocol,
        slot_type,
        command_trb,
        doorbell: XHCI_DOORBELL_COMMAND,
        event: None,
    };

    compiler_fence(Ordering::SeqCst);
    write_mmio_u32(mmio + caps.doorbell_offset as usize, XHCI_DOORBELL_COMMAND);

    let mut spins = 0usize;
    while spins < XHCI_COMMAND_WAIT_SPINS {
        dma_invalidate_range(plan.event_ring, XHCI_TRB_BYTES);
        let event = decode_xhci_command_completion_event(XHCI_EVENT_RING.read_trb(0));
        if event.cycle {
            acknowledge_xhci_event(mmio, caps, plan.event_ring + XHCI_TRB_BYTES as u64);
            report.event = Some(event);
            report.status = classify_enable_slot_event(command_trb_pointer, event);
            return report;
        }

        core::hint::spin_loop();
        spins += 1;
    }

    report
}

/// Polls the lower USB boot-keyboard provider without blocking.
///
/// The current implementation reaches the real PCIe/xHCI discovery and port
/// status path, then reports the first blocker before HID enumeration. It does
/// not claim keyboard readiness until a later cut installs descriptor walking
/// and interrupt-IN transfer polling that can return [`UsbBootKeyboardPoll::Report`].
#[must_use]
pub fn poll_boot_keyboard_report() -> UsbBootKeyboardPoll {
    let Some(controller) = probe_pcie_xhci_controller() else {
        return UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::NoPcieXhciController);
    };
    let Some(mmio) = controller.mmio_base else {
        return UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::ControllerBarUnconfigured);
    };
    let Some(caps) = read_xhci_capabilities_at_mmio(mmio) else {
        return UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::InvalidXhciCapabilities);
    };
    let op = read_xhci_operational_snapshot(mmio, caps);
    if op.controller_not_ready {
        return UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::ControllerNotReady);
    }
    if op.reset_active {
        return UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::ControllerResetInProgress);
    }
    if op.host_system_error {
        return UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::HostSystemError);
    }
    let Some(port) = first_connected_xhci_port(mmio, caps) else {
        return UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::NoConnectedRootPort);
    };
    if xhci_needs_driver_memory(op) {
        return UsbBootKeyboardPoll::Pending(
            UsbBootKeyboardPending::NeedsControllerInitialization {
                max_slots: caps.max_device_slots,
                port: port.port,
                speed: port.speed,
                link_state: port.link_state,
            },
        );
    }
    UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::NeedsEnumeration {
        port: port.port,
        speed: port.speed,
        link_state: port.link_state,
    })
}

unsafe fn read_xhci_capabilities_at(base: usize) -> Option<XhciCapabilities> {
    let cap_hci_version = read_mmio_u32(base + XHCI_CAP_HCIVERSION);
    let hcs_params1 = read_mmio_u32(base + XHCI_HCSPARAMS1);
    let hcs_params2 = read_mmio_u32(base + XHCI_HCSPARAMS2);
    let hcc_params1 = read_mmio_u32(base + XHCI_HCCPARAMS1);
    let dboff = read_mmio_u32(base + XHCI_DBOFF);
    let rtsoff = read_mmio_u32(base + XHCI_RTSOFF);
    decode_xhci_capabilities(cap_hci_version, hcs_params1, hcs_params2, hcc_params1, dboff, rtsoff)
}

fn decode_xhci_capabilities(
    cap_hci_version: u32,
    hcs_params1: u32,
    hcs_params2: u32,
    hcc_params1: u32,
    dboff: u32,
    rtsoff: u32,
) -> Option<XhciCapabilities> {
    let cap_length = (cap_hci_version & 0xff) as u8;
    let hci_version = (cap_hci_version >> 16) as u16;
    let max_device_slots = (hcs_params1 & 0xff) as u8;

    if !(0x20..=0x100).contains(&(cap_length as usize)) {
        return None;
    }
    if hci_version == 0 || hci_version == 0xffff {
        return None;
    }
    if max_device_slots == 0 {
        return None;
    }

    Some(XhciCapabilities {
        cap_length,
        hci_version,
        max_device_slots,
        max_interrupters: ((hcs_params1 >> 8) & 0x7ff) as u16,
        max_ports: ((hcs_params1 >> 24) & 0xff) as u8,
        hcc_params1,
        hcs_params2,
        doorbell_offset: dboff & !0x3,
        runtime_register_space_offset: rtsoff & !0x1f,
        event_ring_segment_table_max: 1u16 << ((hcs_params2 >> 4) & 0xf),
        max_scratchpad_buffers: decode_xhci_scratchpad_count(hcs_params2),
        scratchpad_restore: hcs_params2 & (1 << 26) != 0,
        context_size_bytes: if hcc_params1 & XHCI_HCCPARAMS1_CONTEXT_SIZE != 0 {
            64
        } else {
            32
        },
    })
}

fn xhci_extended_capability_offset(caps: XhciCapabilities) -> Option<usize> {
    let dword_offset = (caps.hcc_params1 >> XHCI_HCCPARAMS1_XECP_SHIFT) as usize;
    if dword_offset == 0 {
        return None;
    }
    Some(dword_offset * core::mem::size_of::<u32>())
}

fn read_xhci_supported_protocol_at(
    base: usize,
    offset: u32,
    header: u32,
) -> Option<XhciSupportedProtocol> {
    let protocol = decode_xhci_supported_protocol(
        offset,
        header,
        read_mmio_u32(base + offset as usize + 0x04),
        read_mmio_u32(base + offset as usize + 0x08),
        read_mmio_u32(base + offset as usize + 0x0c),
    );
    if protocol.compatible_port_offset == 0 || protocol.compatible_port_count == 0 {
        return None;
    }
    Some(protocol)
}

fn decode_xhci_supported_protocol(
    offset: u32,
    header: u32,
    name: u32,
    port_range: u32,
    slot_type: u32,
) -> XhciSupportedProtocol {
    XhciSupportedProtocol {
        offset,
        name: name.to_le_bytes(),
        major_revision: (header >> 24) as u8,
        minor_revision: ((header >> 16) & 0xff) as u8,
        compatible_port_offset: (port_range & 0xff) as u8,
        compatible_port_count: ((port_range >> 8) & 0xff) as u8,
        protocol_defined: ((port_range >> 16) & 0x0fff) as u16,
        protocol_speed_id_count: ((port_range >> 28) & 0x0f) as u8,
        protocol_slot_type: (slot_type & 0x1f) as u8,
    }
}

fn protocol_covers_port(protocol: XhciSupportedProtocol, port: u8) -> bool {
    let first = protocol.compatible_port_offset;
    let Some(last) = first.checked_add(protocol.compatible_port_count.saturating_sub(1)) else {
        return false;
    };
    first <= port && port <= last
}

/// Returns the static xHCI memory layout that would be used for controller
/// initialization, without clearing or writing any memory.
#[must_use]
pub fn xhci_driver_memory_plan(caps: XhciCapabilities) -> XhciDriverMemoryStatus {
    if caps.max_scratchpad_buffers > XHCI_STATIC_SCRATCHPAD_BUFFERS as u16 {
        return XhciDriverMemoryStatus::TooManyScratchpads {
            requested: caps.max_scratchpad_buffers,
            supported: XHCI_STATIC_SCRATCHPAD_BUFFERS as u16,
        };
    }
    XhciDriverMemoryStatus::Ready(build_xhci_driver_memory_plan(caps))
}

/// Clears and initializes static early xHCI driver memory.
///
/// This prepares host memory only. It does not write xHCI MMIO registers,
/// ring doorbells, start schedules, or change USB keyboard readiness.
pub fn prepare_xhci_driver_memory(caps: XhciCapabilities) -> XhciDriverMemoryStatus {
    let status = xhci_driver_memory_plan(caps);
    let XhciDriverMemoryStatus::Ready(plan) = status else {
        return status;
    };

    XHCI_DCBAA.zero();
    XHCI_COMMAND_RING.zero();
    XHCI_EVENT_RING.zero();
    XHCI_ERST.zero();
    XHCI_SCRATCHPAD_ARRAY.zero();
    XHCI_SCRATCHPAD_PAGES.zero();

    if plan.scratchpad_buffers > 0 {
        let mut index = 0usize;
        while index < plan.scratchpad_buffers as usize {
            XHCI_SCRATCHPAD_ARRAY.set(index, XHCI_SCRATCHPAD_PAGES.page_addr(index));
            index += 1;
        }
        XHCI_DCBAA.set(0, plan.scratchpad_array);
    }

    XHCI_ERST.set(0, plan.event_ring);
    XHCI_ERST.set(1, plan.event_ring_trbs as u64);
    status
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
        run_stop: usb_command & XHCI_USBCMD_RUN_STOP != 0,
        reset_active: usb_command & XHCI_USBCMD_HOST_CONTROLLER_RESET != 0,
        interrupter_enable: usb_command & XHCI_USBCMD_INTERRUPTER_ENABLE != 0,
        halted: usb_status & XHCI_USBSTS_HALTED != 0,
        host_system_error: usb_status & XHCI_USBSTS_HOST_SYSTEM_ERROR != 0,
        event_interrupt: usb_status & XHCI_USBSTS_EVENT_INTERRUPT != 0,
        port_change_detect: usb_status & XHCI_USBSTS_PORT_CHANGE_DETECT != 0,
        controller_not_ready: usb_status & XHCI_USBSTS_CONTROLLER_NOT_READY != 0,
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

fn first_connected_xhci_port(base: usize, caps: XhciCapabilities) -> Option<XhciPortSnapshot> {
    let mut index = 1usize;
    while index <= caps.max_ports as usize {
        let port = index as u8;
        if let Some(snapshot) = read_xhci_port_snapshot(base, caps, port) {
            if snapshot.connected {
                return Some(snapshot);
            }
        }
        index += 1;
    }
    None
}

fn xhci_needs_driver_memory(op: XhciOperationalSnapshot) -> bool {
    op.enabled_device_slots == 0
        || op.command_ring_control == 0
        || op.device_context_base_address_array_pointer == 0
}

const fn xhci_operational_base(base: usize, caps: XhciCapabilities) -> usize {
    base + caps.cap_length as usize
}

const fn xhci_runtime_base(base: usize, caps: XhciCapabilities) -> usize {
    base + caps.runtime_register_space_offset as usize
}

fn wait_for_xhci<F>(
    base: usize,
    caps: XhciCapabilities,
    mut predicate: F,
) -> Option<XhciOperationalSnapshot>
where
    F: FnMut(XhciOperationalSnapshot) -> bool,
{
    let mut spins = 0usize;
    while spins < XHCI_START_WAIT_SPINS {
        let snapshot = read_xhci_operational_snapshot(base, caps);
        if predicate(snapshot) {
            return Some(snapshot);
        }
        core::hint::spin_loop();
        spins += 1;
    }
    None
}

fn xhci_controller_start_registers(plan: XhciDriverMemoryPlan) -> XhciControllerStartRegisters {
    XhciControllerStartRegisters {
        device_context_base_address_array_pointer: plan.dcbaa,
        command_ring_control: plan.command_ring_control,
        configure: plan.max_slots_enabled as u32,
        interrupter_management: 0,
        interrupter_moderation: 0,
        event_ring_segment_table_size: plan.event_ring_segment_table_entries as u32,
        event_ring_segment_table_base_address: plan.event_ring_segment_table,
        event_ring_dequeue_pointer: plan.event_ring_dequeue_pointer,
        usb_status_clear: XHCI_USBSTS_HOST_SYSTEM_ERROR
            | XHCI_USBSTS_EVENT_INTERRUPT
            | XHCI_USBSTS_PORT_CHANGE_DETECT,
        usb_command: XHCI_USBCMD_RUN_STOP,
    }
}

fn write_xhci_start_registers(
    base: usize,
    caps: XhciCapabilities,
    registers: XhciControllerStartRegisters,
) {
    let op_base = xhci_operational_base(base, caps);
    let intr0 = xhci_runtime_base(base, caps) + XHCI_RUNTIME_INTERRUPTER0;

    write_mmio_u32(op_base + XHCI_OP_USBSTS, registers.usb_status_clear);
    write_mmio_u64(op_base + XHCI_OP_DCBAAP, registers.device_context_base_address_array_pointer);
    write_mmio_u64(op_base + XHCI_OP_CRCR, registers.command_ring_control);
    write_mmio_u32(op_base + XHCI_OP_CONFIG, registers.configure);

    write_mmio_u32(intr0 + XHCI_INTR_IMAN, registers.interrupter_management);
    write_mmio_u32(intr0 + XHCI_INTR_IMOD, registers.interrupter_moderation);
    write_mmio_u32(intr0 + XHCI_INTR_ERSTSZ, registers.event_ring_segment_table_size);
    write_mmio_u64(intr0 + XHCI_INTR_ERSTBA, registers.event_ring_segment_table_base_address);
    write_mmio_u64(intr0 + XHCI_INTR_ERDP, registers.event_ring_dequeue_pointer);
    write_mmio_u32(op_base + XHCI_OP_USBCMD, registers.usb_command);
}

fn acknowledge_xhci_event(base: usize, caps: XhciCapabilities, event_dequeue_pointer: u64) {
    let op_base = xhci_operational_base(base, caps);
    let intr0 = xhci_runtime_base(base, caps) + XHCI_RUNTIME_INTERRUPTER0;
    write_mmio_u64(intr0 + XHCI_INTR_ERDP, event_dequeue_pointer | XHCI_ERDP_EVENT_HANDLER_BUSY);
    write_mmio_u32(op_base + XHCI_OP_USBSTS, XHCI_USBSTS_EVENT_INTERRUPT);
}

fn xhci_enable_slot_command_trb(slot_type: u8) -> [u32; 4] {
    [
        0,
        0,
        0,
        ((slot_type & XHCI_ENABLE_SLOT_SLOT_TYPE_MASK) as u32) << XHCI_ENABLE_SLOT_SLOT_TYPE_SHIFT
            | ((XHCI_TRB_TYPE_ENABLE_SLOT as u32) << XHCI_TRB_TYPE_SHIFT)
            | XHCI_TRB_CYCLE,
    ]
}

fn decode_xhci_command_completion_event(raw: [u32; 4]) -> XhciCommandCompletionEvent {
    XhciCommandCompletionEvent {
        raw,
        command_trb_pointer: (((raw[1] as u64) << 32) | raw[0] as u64) & !0xf,
        completion_code: (raw[2] >> XHCI_TRB_COMPLETION_CODE_SHIFT) as u8,
        trb_type: ((raw[3] >> XHCI_TRB_TYPE_SHIFT) & XHCI_TRB_TYPE_MASK) as u8,
        cycle: raw[3] & XHCI_TRB_CYCLE != 0,
        slot_id: (raw[3] >> 24) as u8,
    }
}

fn classify_enable_slot_event(
    expected_command_trb_pointer: u64,
    event: XhciCommandCompletionEvent,
) -> XhciEnableSlotStatus {
    if event.trb_type != XHCI_TRB_TYPE_COMMAND_COMPLETION_EVENT {
        return XhciEnableSlotStatus::UnexpectedEventType {
            trb_type: event.trb_type,
            completion_code: event.completion_code,
        };
    }
    if event.command_trb_pointer != expected_command_trb_pointer {
        return XhciEnableSlotStatus::CommandPointerMismatch {
            expected: expected_command_trb_pointer,
            actual: event.command_trb_pointer,
            completion_code: event.completion_code,
            slot_id: event.slot_id,
        };
    }
    if event.completion_code != XHCI_TRB_COMPLETION_SUCCESS || event.slot_id == 0 {
        return XhciEnableSlotStatus::CommandFailed {
            completion_code: event.completion_code,
            slot_id: event.slot_id,
        };
    }
    XhciEnableSlotStatus::SlotEnabled {
        slot_id: event.slot_id,
    }
}

fn clean_xhci_driver_memory_for_device(plan: XhciDriverMemoryPlan) {
    dma_clean_range(plan.dcbaa, XHCI_MAX_DEVICE_CONTEXT_POINTERS * core::mem::size_of::<u64>());
    dma_clean_range(plan.command_ring, XHCI_COMMAND_RING_TRBS * XHCI_TRB_BYTES);
    dma_clean_range(plan.event_ring, XHCI_EVENT_RING_TRBS * XHCI_TRB_BYTES);
    dma_clean_range(
        plan.event_ring_segment_table,
        XHCI_EVENT_RING_SEGMENT_TABLE_ENTRIES * 2 * core::mem::size_of::<u64>(),
    );
    if plan.scratchpad_buffers == 0 {
        return;
    }
    dma_clean_range(
        plan.scratchpad_array,
        plan.scratchpad_buffers as usize * core::mem::size_of::<u64>(),
    );
    let mut index = 0usize;
    while index < plan.scratchpad_buffers as usize {
        dma_clean_range(XHCI_SCRATCHPAD_PAGES.page_addr(index), XHCI_PAGE_BYTES);
        index += 1;
    }
}

fn dma_clean_range(addr: u64, len: usize) {
    dma_cache_range(addr, len, clean_cache_line);
    dsb_sy();
}

fn dma_invalidate_range(addr: u64, len: usize) {
    dma_cache_range(addr, len, invalidate_cache_line);
    dsb_sy();
}

fn dma_cache_range(addr: u64, len: usize, op: fn(usize)) {
    if len == 0 {
        return;
    }
    let start = (addr as usize) & !(XHCI_DMA_CACHE_LINE_BYTES - 1);
    let end =
        (addr as usize + len + XHCI_DMA_CACHE_LINE_BYTES - 1) & !(XHCI_DMA_CACHE_LINE_BYTES - 1);
    let mut line = start;
    while line < end {
        op(line);
        line += XHCI_DMA_CACHE_LINE_BYTES;
    }
}

#[cfg(target_arch = "aarch64")]
fn clean_cache_line(addr: usize) {
    // SAFETY: `dc cvac` is the architected aarch64 cache-clean-by-VA operation.
    unsafe {
        asm!("dc cvac, {addr}", addr = in(reg) addr, options(nostack, preserves_flags));
    }
}

#[cfg(not(target_arch = "aarch64"))]
fn clean_cache_line(_addr: usize) {
    compiler_fence(Ordering::SeqCst);
}

#[cfg(target_arch = "aarch64")]
fn invalidate_cache_line(addr: usize) {
    // SAFETY: `dc ivac` is the architected aarch64 cache-invalidate-by-VA operation.
    unsafe {
        asm!("dc ivac, {addr}", addr = in(reg) addr, options(nostack, preserves_flags));
    }
}

#[cfg(not(target_arch = "aarch64"))]
fn invalidate_cache_line(_addr: usize) {
    compiler_fence(Ordering::SeqCst);
}

#[cfg(target_arch = "aarch64")]
fn dsb_sy() {
    // SAFETY: `dsb sy` orders CPU cache maintenance and device-visible memory.
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(not(target_arch = "aarch64"))]
fn dsb_sy() {
    compiler_fence(Ordering::SeqCst);
}

const fn decode_xhci_scratchpad_count(hcs_params2: u32) -> u16 {
    let hi = ((hcs_params2 >> 21) & 0x1f) as u16;
    let lo = ((hcs_params2 >> 27) & 0x1f) as u16;
    (hi << 5) | lo
}

fn build_xhci_driver_memory_plan(caps: XhciCapabilities) -> XhciDriverMemoryPlan {
    let dcbaa = XHCI_DCBAA.addr();
    let command_ring = XHCI_COMMAND_RING.addr();
    let event_ring = XHCI_EVENT_RING.addr();
    let scratchpad_array = if caps.max_scratchpad_buffers == 0 {
        0
    } else {
        XHCI_SCRATCHPAD_ARRAY.addr()
    };
    XhciDriverMemoryPlan {
        dcbaa,
        command_ring,
        command_ring_control: command_ring | 1,
        event_ring,
        event_ring_segment_table: XHCI_ERST.addr(),
        event_ring_segment_table_entries: XHCI_EVENT_RING_SEGMENT_TABLE_ENTRIES as u16,
        event_ring_trbs: XHCI_EVENT_RING_TRBS as u16,
        command_ring_trbs: XHCI_COMMAND_RING_TRBS as u16,
        event_ring_dequeue_pointer: event_ring,
        scratchpad_array,
        scratchpad_buffers: caps.max_scratchpad_buffers,
        max_slots_enabled: caps.max_device_slots,
        context_size_bytes: caps.context_size_bytes,
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

fn write_mmio_u32(addr: usize, value: u32) {
    // SAFETY: callers pass native MMIO register addresses; volatile access is
    // the required mechanism for device registers.
    unsafe {
        write_volatile(addr as *mut u32, value);
    }
}

fn write_mmio_u64(addr: usize, value: u64) {
    write_mmio_u32(addr, value as u32);
    write_mmio_u32(addr + 4, (value >> 32) as u32);
}

#[repr(C, align(64))]
struct XhciAlignedU64<const N: usize>(UnsafeCell<[u64; N]>);

unsafe impl<const N: usize> Sync for XhciAlignedU64<N> {}

impl<const N: usize> XhciAlignedU64<N> {
    const fn new() -> Self {
        Self(UnsafeCell::new([0; N]))
    }

    fn addr(&self) -> u64 {
        self.0.get() as *mut u64 as usize as u64
    }

    fn zero(&self) {
        let values = unsafe { &mut *self.0.get() };
        let mut index = 0usize;
        while index < N {
            values[index] = 0;
            index += 1;
        }
    }

    fn set(&self, index: usize, value: u64) {
        let values = unsafe { &mut *self.0.get() };
        values[index] = value;
    }
}

#[repr(C, align(64))]
struct XhciTrbRing<const N: usize>(UnsafeCell<[[u32; 4]; N]>);

unsafe impl<const N: usize> Sync for XhciTrbRing<N> {}

impl<const N: usize> XhciTrbRing<N> {
    const fn new() -> Self {
        Self(UnsafeCell::new([[0; 4]; N]))
    }

    fn addr(&self) -> u64 {
        self.0.get() as *mut [u32; 4] as usize as u64
    }

    fn trb_addr(&self, index: usize) -> u64 {
        self.addr() + (index * XHCI_TRB_BYTES) as u64
    }

    fn zero(&self) {
        let trbs = unsafe { &mut *self.0.get() };
        let mut index = 0usize;
        while index < N {
            trbs[index] = [0; 4];
            index += 1;
        }
    }

    fn set_trb(&self, index: usize, value: [u32; 4]) {
        let trbs = unsafe { &mut *self.0.get() };
        trbs[index] = value;
    }

    fn read_trb(&self, index: usize) -> [u32; 4] {
        let trbs = unsafe { &*self.0.get() };
        trbs[index]
    }
}

#[repr(C, align(4096))]
struct XhciScratchpadPages(UnsafeCell<[u8; XHCI_PAGE_BYTES * XHCI_STATIC_SCRATCHPAD_BUFFERS]>);

unsafe impl Sync for XhciScratchpadPages {}

impl XhciScratchpadPages {
    const fn new() -> Self {
        Self(UnsafeCell::new([0; XHCI_PAGE_BYTES * XHCI_STATIC_SCRATCHPAD_BUFFERS]))
    }

    fn page_addr(&self, index: usize) -> u64 {
        self.0.get() as *mut u8 as usize as u64 + (index * XHCI_PAGE_BYTES) as u64
    }

    fn zero(&self) {
        let bytes = unsafe { &mut *self.0.get() };
        let mut index = 0usize;
        while index < bytes.len() {
            bytes[index] = 0;
            index += 1;
        }
    }
}

static XHCI_DCBAA: XhciAlignedU64<XHCI_MAX_DEVICE_CONTEXT_POINTERS> = XhciAlignedU64::new();
static XHCI_COMMAND_RING: XhciTrbRing<XHCI_COMMAND_RING_TRBS> = XhciTrbRing::new();
static XHCI_EVENT_RING: XhciTrbRing<XHCI_EVENT_RING_TRBS> = XhciTrbRing::new();
static XHCI_ERST: XhciAlignedU64<{ XHCI_EVENT_RING_SEGMENT_TABLE_ENTRIES * 2 }> =
    XhciAlignedU64::new();
static XHCI_SCRATCHPAD_ARRAY: XhciAlignedU64<XHCI_STATIC_SCRATCHPAD_BUFFERS> =
    XhciAlignedU64::new();
static XHCI_SCRATCHPAD_PAGES: XhciScratchpadPages = XhciScratchpadPages::new();

#[cfg(feature = "selftest")]
#[path = "usb_tests.rs"]
mod tests;
