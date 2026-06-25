//! BCM2711 USB host-controller raw facts and early probe helpers.
//!
//! This module stays below the system-kernel bridge. It reads only native MMIO
//! controller registers and returns target-local facts. USB/xHCI enumeration
//! and HID report polling live here; root-shell byte routing stays above this
//! raw provider in `apps/os` and `reovim-system-kernel`.

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

/// BCM2711 integrated xHCI bus address from the firmware DTB.
pub const BCM2711_XHCI_BUS_BASE: u64 = 0x7e9c_0000;
/// BCM2711 integrated xHCI ARM-physical MMIO base.
pub const BCM2711_XHCI_MMIO_BASE: usize = 0xfe9c_0000;
/// BCM2711 integrated xHCI MMIO window length.
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
const XHCI_TRB_TYPE_LINK: u8 = 6;
const XHCI_TRB_TYPE_NORMAL: u8 = 1;
const XHCI_TRB_TYPE_SETUP_STAGE: u8 = 2;
const XHCI_TRB_TYPE_DATA_STAGE: u8 = 3;
const XHCI_TRB_TYPE_STATUS_STAGE: u8 = 4;
const XHCI_TRB_TYPE_ENABLE_SLOT: u8 = 9;
const XHCI_TRB_TYPE_ADDRESS_DEVICE: u8 = 11;
const XHCI_TRB_TYPE_CONFIGURE_ENDPOINT: u8 = 12;
const XHCI_TRB_TYPE_TRANSFER_EVENT: u8 = 32;
const XHCI_TRB_TYPE_COMMAND_COMPLETION_EVENT: u8 = 33;
const XHCI_TRB_COMPLETION_SUCCESS: u8 = 1;
const XHCI_DOORBELL_COMMAND: u32 = 0;
const XHCI_DOORBELL_CONTROL_EP0: u32 = 1;
const XHCI_LINK_TRB_TOGGLE_CYCLE: u32 = 1 << 1;
const XHCI_ADDRESS_DEVICE_BLOCK_SET_ADDRESS_REQUEST_BIT: u32 = 1 << 9;
const XHCI_ERDP_EVENT_HANDLER_BUSY: u64 = 1 << 3;
const XHCI_TRB_IOC: u32 = 1 << 5;
const XHCI_TRB_IDT: u32 = 1 << 6;
const XHCI_TRB_DIR_IN: u32 = 1 << 16;
const XHCI_TRB_TRANSFER_LENGTH_MASK: u32 = 0x1_ffff;
const XHCI_TRANSFER_EVENT_LENGTH_MASK: u32 = 0x00ff_ffff;
const XHCI_TRANSFER_EVENT_ENDPOINT_ID_SHIFT: u32 = 16;
const XHCI_SETUP_TRT_SHIFT: u32 = 16;
const XHCI_SETUP_TRT_NO_DATA_STAGE: u8 = 0;
const XHCI_SETUP_TRT_IN_DATA_STAGE: u8 = 3;

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
/// Number of TRBs in the default control endpoint transfer-ring segment.
pub const XHCI_CONTROL_ENDPOINT_RING_TRBS: usize = 64;
/// Number of TRBs in the first HID interrupt-IN endpoint transfer-ring segment.
pub const XHCI_INTERRUPT_IN_ENDPOINT_RING_TRBS: usize = 64;
/// Number of data TRB slots before the interrupt-IN ring's Link TRB.
const XHCI_INTERRUPT_IN_ENDPOINT_DATA_TRBS: usize = XHCI_INTERRUPT_IN_ENDPOINT_RING_TRBS - 1;
/// Number of Event Ring Segment Table entries prepared by the early provider.
pub const XHCI_EVENT_RING_SEGMENT_TABLE_ENTRIES: usize = 1;
/// Maximum scratchpad buffers backed by static early-driver storage.
pub const XHCI_STATIC_SCRATCHPAD_BUFFERS: usize = 8;
/// xHCI scratchpad buffer/page size used by the freestanding arena.
pub const XHCI_PAGE_BYTES: usize = 4096;

const XHCI_MAX_DEVICE_CONTEXT_POINTERS: usize = 256;
const XHCI_CONTEXT_PAGE_BYTES: usize = 4096;
const XHCI_ADDRESS_DEVICE_COMMAND_INDEX: usize = 1;
const XHCI_ADDRESS_DEVICE_EVENT_INDEX: usize = 1;
const XHCI_ADDRESS_DEVICE_BLOCK_SET_ADDRESS_REQUEST: bool = true;
const XHCI_SET_ADDRESS_COMMAND_INDEX: usize = 2;
const XHCI_SET_ADDRESS_EVENT_INDEX: usize = 3;
const XHCI_CONFIGURE_ENDPOINT_COMMAND_INDEX: usize = 3;
const XHCI_CONFIGURE_ENDPOINT_EVENT_INDEX: usize = 8;
const XHCI_SET_HID_PROTOCOL_EVENT_INDEX: usize = 9;
const XHCI_BOOT_KEYBOARD_REPORT_EVENT_INDEX: usize = 10;
const XHCI_INTERRUPT_IN_REPORT_TRB_INDEX: usize = 0;
const XHCI_GET_DESCRIPTOR_EVENT_INDEX: usize = 2;
const XHCI_READ_DEVICE_DESCRIPTOR_EVENT_INDEX: usize = 4;
const XHCI_READ_CONFIGURATION_DESCRIPTOR_HEADER_EVENT_INDEX: usize = 5;
const XHCI_READ_CONFIGURATION_DESCRIPTOR_EVENT_INDEX: usize = 6;
const XHCI_SET_CONFIGURATION_EVENT_INDEX: usize = 7;
const XHCI_EP0_DESCRIPTOR_PREFIX_TRB_INDEX: usize = 0;
const XHCI_EP0_DEVICE_DESCRIPTOR_TRB_INDEX: usize = 3;
const XHCI_EP0_CONFIGURATION_DESCRIPTOR_HEADER_TRB_INDEX: usize = 6;
const XHCI_EP0_CONFIGURATION_DESCRIPTOR_TRB_INDEX: usize = 9;
const XHCI_EP0_SET_CONFIGURATION_TRB_INDEX: usize = 12;
const XHCI_EP0_SET_HID_PROTOCOL_TRB_INDEX: usize = 14;
const USB_BOOT_KEYBOARD_RETRY_POLLS: usize = 200;
const XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES: usize = 8;
const USB_DEVICE_DESCRIPTOR_BYTES: usize = 18;
const USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES: usize = 9;
const USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES: usize = 256;
const USB_REQUEST_TYPE_HOST_TO_DEVICE_STANDARD_DEVICE: u8 = 0x00;
const USB_REQUEST_TYPE_HOST_TO_DEVICE_CLASS_INTERFACE: u8 = 0x21;
const USB_REQUEST_TYPE_DEVICE_TO_HOST_STANDARD_DEVICE: u8 = 0x80;
const USB_REQUEST_GET_DESCRIPTOR: u8 = 6;
const USB_REQUEST_SET_CONFIGURATION: u8 = 9;
const USB_DESCRIPTOR_TYPE_DEVICE: u8 = 1;
const USB_DESCRIPTOR_TYPE_CONFIGURATION: u8 = 2;
const USB_DESCRIPTOR_TYPE_INTERFACE: u8 = 4;
const USB_DESCRIPTOR_TYPE_ENDPOINT: u8 = 5;
const USB_CLASS_HID: u8 = 0x03;
const USB_HID_SUBCLASS_BOOT: u8 = 0x01;
const USB_HID_PROTOCOL_KEYBOARD: u8 = 0x01;
const USB_HID_REQUEST_SET_PROTOCOL: u8 = 11;
const USB_HID_BOOT_PROTOCOL: u8 = 0;
const USB_ENDPOINT_DIRECTION_IN: u8 = 0x80;
const USB_ENDPOINT_TRANSFER_TYPE_INTERRUPT: u8 = 0x03;
const XHCI_HCCPARAMS1_CONTEXT_SIZE: u32 = 1 << 2;
const XHCI_HCCPARAMS1_XECP_SHIFT: u32 = 16;
const XHCI_START_WAIT_SPINS: usize = 1_000_000;
const XHCI_COMMAND_WAIT_SPINS: usize = 2_000_000;
const XHCI_DMA_CACHE_LINE_BYTES: usize = 64;
const XHCI_REQUIRED_PCI_COMMAND_BITS: u16 =
    pcie::PCI_COMMAND_MEMORY_SPACE | pcie::PCI_COMMAND_BUS_MASTER;
const XHCI_EXT_CAP_ID_SUPPORTED_PROTOCOL: u8 = 2;
const XHCI_EXT_CAP_MAX_STEPS: usize = 32;
const XHCI_INPUT_CONTEXT_DROP_FLAGS_DWORD: usize = 0;
const XHCI_INPUT_CONTEXT_ADD_FLAGS_DWORD: usize = 1;
const XHCI_INPUT_CONTEXT_SLOT_INDEX: usize = 1;
const XHCI_INPUT_CONTEXT_EP0_INDEX: usize = 2;
const XHCI_INPUT_ADD_SLOT_CONTEXT: u32 = 1 << 0;
const XHCI_INPUT_ADD_EP0_CONTEXT: u32 = 1 << 1;
const XHCI_SLOT_CONTEXT_SPEED_SHIFT: u32 = 20;
const XHCI_SLOT_CONTEXT_CONTEXT_ENTRIES_SHIFT: u32 = 27;
const XHCI_SLOT_CONTEXT_ROOT_HUB_PORT_SHIFT: u32 = 16;
const XHCI_ENDPOINT_DCI_MAX: u8 = 31;
const XHCI_EP_CONTEXT_INTERVAL_SHIFT: u32 = 16;
const XHCI_EP_CONTEXT_CERR_SHIFT: u32 = 1;
const XHCI_EP_CONTEXT_TYPE_SHIFT: u32 = 3;
const XHCI_EP_CONTEXT_MAX_PACKET_SIZE_SHIFT: u32 = 16;
const XHCI_EP_CONTEXT_MAX_ESIT_PAYLOAD_SHIFT: u32 = 16;
const XHCI_EP_CONTEXT_TYPE_CONTROL: u8 = 4;
const XHCI_EP_CONTEXT_TYPE_INTERRUPT_IN: u8 = 7;
const XHCI_EP_CONTEXT_CERR_DEFAULT: u8 = 3;
const XHCI_EP_CONTEXT_DCS: u64 = 1;
const XHCI_CONTROL_AVERAGE_TRB_LENGTH: u16 = 8;
const XHCI_INTERRUPT_AVERAGE_TRB_LENGTH: u16 = 8;

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
    /// Input Context base address for the first manual Address Device command.
    pub input_context: u64,
    /// Output Device Context base address for the first enabled slot.
    pub output_device_context: u64,
    /// Transfer ring for default control endpoint 0.
    pub control_endpoint_ring: u64,
    /// Transfer ring for the first HID interrupt-IN endpoint.
    pub interrupt_in_endpoint_ring: u64,
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

/// Decoded xHCI Transfer Event TRB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciTransferEvent {
    /// Raw TRB dwords as DMA-written by the xHC.
    pub raw: [u32; 4],
    /// Transfer TRB pointer reported by the event.
    pub trb_pointer: u64,
    /// Residual bytes not transferred for the generating TRB.
    pub transfer_length: u32,
    /// xHCI completion code.
    pub completion_code: u8,
    /// Event TRB type field.
    pub trb_type: u8,
    /// Event ring cycle bit.
    pub cycle: bool,
    /// Whether the event uses Event Data instead of a TRB pointer.
    pub event_data: bool,
    /// xHCI endpoint ID that produced the event.
    pub endpoint_id: u8,
    /// xHCI slot ID that produced the event.
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

/// Static context values prepared for the first manual Address Device command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciAddressDeviceContexts {
    /// Input Context base address used by the command.
    pub input_context: u64,
    /// Output Device Context address stored in the DCBAA slot entry.
    pub output_device_context: u64,
    /// Default control endpoint transfer-ring base address.
    pub control_endpoint_ring: u64,
    /// Drop Context flags dword.
    pub drop_context_flags: u32,
    /// Add Context flags dword.
    pub add_context_flags: u32,
    /// DCI 0 Slot Context dwords prepared in the Input Context.
    pub slot_context: [u32; 4],
    /// DCI 1 Endpoint 0 Context dwords prepared in the Input Context.
    pub endpoint0_context: [u32; 5],
    /// Default control endpoint max packet size selected from port speed.
    pub endpoint0_max_packet_size: u16,
}

/// Status from issuing the first manual xHCI Address Device command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciAddressDeviceStatus {
    /// Enable Slot did not complete successfully.
    EnableSlotFailed(XhciEnableSlotStatus),
    /// Enable Slot succeeded, but the Slot ID was unusable for the DCBAA.
    SlotIdOutOfRange {
        /// Slot ID returned by Enable Slot.
        slot_id: u8,
        /// Maximum slot ID this provider can back.
        max_supported: u8,
    },
    /// No connected root port was available to build the input context.
    NoConnectedRootPort,
    /// Required controller-start evidence was missing.
    StartEvidenceUnavailable,
    /// No Command Completion Event reached event-ring entry one.
    CommandTimedOut,
    /// A cycle-valid event arrived, but it was not a command-completion event.
    UnexpectedEventType {
        /// Event TRB type field.
        trb_type: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The completion event did not point back at the Address Device command TRB.
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
    /// The completion event pointed at this command but returned another Slot ID.
    SlotIdMismatch {
        /// Expected target slot ID.
        expected: u8,
        /// Actual Slot ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The command completed with a non-success code.
    CommandFailed {
        /// xHCI completion code.
        completion_code: u8,
        /// Slot ID carried by the event.
        slot_id: u8,
    },
    /// Address Device with BSR=1 completed and endpoint 0 is ready.
    DefaultControlEndpointReady {
        /// Assigned xHCI slot ID.
        slot_id: u8,
    },
}

/// Evidence returned by the first manual Address Device command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciAddressDeviceReport {
    /// Enable Slot report produced before issuing Address Device.
    pub enable: XhciEnableSlotReport,
    /// Final Address Device command status.
    pub status: XhciAddressDeviceStatus,
    /// Address Device command TRB pointer.
    pub command_trb_pointer: u64,
    /// Raw Address Device command TRB written by software.
    pub command_trb: [u32; 4],
    /// Doorbell value written to doorbell 0.
    pub doorbell: u32,
    /// Whether the command was issued with BSR=1.
    pub block_set_address_request: bool,
    /// Contexts and transfer-ring pointers prepared for the command.
    pub contexts: Option<XhciAddressDeviceContexts>,
    /// Command-completion event observed for Address Device, if one arrived.
    pub event: Option<XhciCommandCompletionEvent>,
}

impl XhciAddressDeviceReport {
    const fn new(enable: XhciEnableSlotReport, status: XhciAddressDeviceStatus) -> Self {
        Self {
            enable,
            status,
            command_trb_pointer: 0,
            command_trb: [0; 4],
            doorbell: XHCI_DOORBELL_COMMAND,
            block_set_address_request: XHCI_ADDRESS_DEVICE_BLOCK_SET_ADDRESS_REQUEST,
            contexts: None,
            event: None,
        }
    }
}

/// Status from the first manual EP0 GET_DESCRIPTOR(Device) transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciDeviceDescriptorProbeStatus {
    /// Address Device did not reach default-control-endpoint readiness.
    AddressDeviceFailed(XhciAddressDeviceStatus),
    /// Required controller-start evidence was missing.
    StartEvidenceUnavailable,
    /// No Transfer Event reached the expected event-ring entry.
    TransferTimedOut,
    /// A cycle-valid event arrived, but it was not a transfer event.
    UnexpectedEventType {
        /// Event TRB type field.
        trb_type: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event did not point at the Status Stage TRB.
    TransferPointerMismatch {
        /// Expected Status Stage TRB pointer.
        expected: u64,
        /// Actual pointer reported by the event.
        actual: u64,
        /// xHCI completion code carried by the event.
        completion_code: u8,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// The transfer event reported a different Slot ID.
    SlotIdMismatch {
        /// Expected target slot ID.
        expected: u8,
        /// Actual Slot ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event reported a different Endpoint ID.
    EndpointIdMismatch {
        /// Expected endpoint ID.
        expected: u8,
        /// Actual Endpoint ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer completed with a non-success code.
    TransferFailed {
        /// xHCI completion code.
        completion_code: u8,
        /// Residual bytes not transferred for the generating TRB.
        residual_length: u32,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// The first bytes of the USB Device Descriptor are in the report buffer.
    DescriptorPrefixReady {
        /// Assigned xHCI slot ID.
        slot_id: u8,
        /// Number of descriptor bytes requested.
        length: u8,
    },
}

/// Evidence returned by the first manual EP0 GET_DESCRIPTOR(Device) transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciDeviceDescriptorProbeReport {
    /// Address Device report produced before queuing the control transfer.
    pub address: XhciAddressDeviceReport,
    /// Final transfer status.
    pub status: XhciDeviceDescriptorProbeStatus,
    /// xHCI slot ID targeted by the transfer.
    pub slot_id: u8,
    /// xHCI endpoint ID targeted by the transfer.
    pub endpoint_id: u8,
    /// Doorbell value written to the device slot.
    pub doorbell: u32,
    /// Setup Stage TRB pointer.
    pub setup_trb_pointer: u64,
    /// Data Stage TRB pointer.
    pub data_trb_pointer: u64,
    /// Status Stage TRB pointer.
    pub status_trb_pointer: u64,
    /// Raw Setup Stage TRB written by software.
    pub setup_trb: [u32; 4],
    /// Raw Data Stage TRB written by software.
    pub data_trb: [u32; 4],
    /// Raw Status Stage TRB written by software.
    pub status_trb: [u32; 4],
    /// DMA buffer address used for descriptor bytes.
    pub descriptor_buffer: u64,
    /// Descriptor bytes captured after a successful transfer.
    pub descriptor: [u8; XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES],
    /// Transfer event observed for the Status Stage TRB, if one arrived.
    pub event: Option<XhciTransferEvent>,
}

impl XhciDeviceDescriptorProbeReport {
    const fn new(
        address: XhciAddressDeviceReport,
        status: XhciDeviceDescriptorProbeStatus,
    ) -> Self {
        Self {
            address,
            status,
            slot_id: 0,
            endpoint_id: XHCI_DOORBELL_CONTROL_EP0 as u8,
            doorbell: XHCI_DOORBELL_CONTROL_EP0,
            setup_trb_pointer: 0,
            data_trb_pointer: 0,
            status_trb_pointer: 0,
            setup_trb: [0; 4],
            data_trb: [0; 4],
            status_trb: [0; 4],
            descriptor_buffer: 0,
            descriptor: [0; XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES],
            event: None,
        }
    }
}

/// Status from the manual Address Device command with BSR=0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciSetAddressStatus {
    /// The descriptor-prefix probe did not complete successfully.
    DescriptorPrefixFailed(XhciDeviceDescriptorProbeStatus),
    /// Required controller-start evidence was missing.
    StartEvidenceUnavailable,
    /// Descriptor bytes were not a minimally valid USB Device Descriptor prefix.
    InvalidDescriptorPrefix {
        /// USB descriptor bLength byte.
        length: u8,
        /// USB descriptor bDescriptorType byte.
        descriptor_type: u8,
        /// Raw bMaxPacketSize0 byte.
        max_packet_size0: u8,
        /// Raw bcdUSB field.
        bcd_usb: u16,
    },
    /// No Command Completion Event reached the expected event-ring entry.
    CommandTimedOut,
    /// A cycle-valid event arrived, but it was not a command-completion event.
    UnexpectedEventType {
        /// Event TRB type field.
        trb_type: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The completion event did not point back at the Address Device command TRB.
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
    /// The completion event pointed at this command but returned another Slot ID.
    SlotIdMismatch {
        /// Expected target slot ID.
        expected: u8,
        /// Actual Slot ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The command completed with a non-success code.
    CommandFailed {
        /// xHCI completion code.
        completion_code: u8,
        /// Slot ID carried by the event.
        slot_id: u8,
    },
    /// Address Device with BSR=0 completed and the slot is addressed.
    Addressed {
        /// Assigned xHCI slot ID.
        slot_id: u8,
        /// Endpoint 0 max-packet size used in the command context.
        endpoint0_max_packet_size: u16,
    },
}

/// Evidence returned by the manual Address Device command with BSR=0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciSetAddressReport {
    /// Descriptor-prefix report produced before issuing BSR=0.
    pub descriptor: XhciDeviceDescriptorProbeReport,
    /// Final set-address command status.
    pub status: XhciSetAddressStatus,
    /// Address Device command TRB pointer.
    pub command_trb_pointer: u64,
    /// Raw Address Device command TRB written by software.
    pub command_trb: [u32; 4],
    /// Doorbell value written to doorbell 0.
    pub doorbell: u32,
    /// Contexts and transfer-ring pointers prepared for the command.
    pub contexts: Option<XhciAddressDeviceContexts>,
    /// Endpoint 0 max-packet size decoded from the descriptor prefix.
    pub endpoint0_max_packet_size: u16,
    /// Command-completion event observed for Address Device, if one arrived.
    pub event: Option<XhciCommandCompletionEvent>,
}

impl XhciSetAddressReport {
    const fn new(
        descriptor: XhciDeviceDescriptorProbeReport,
        status: XhciSetAddressStatus,
    ) -> Self {
        Self {
            descriptor,
            status,
            command_trb_pointer: 0,
            command_trb: [0; 4],
            doorbell: XHCI_DOORBELL_COMMAND,
            contexts: None,
            endpoint0_max_packet_size: 0,
            event: None,
        }
    }
}

/// Parsed USB Device Descriptor fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsbDeviceDescriptor {
    /// Descriptor length byte.
    pub length: u8,
    /// Descriptor type byte.
    pub descriptor_type: u8,
    /// USB specification release in BCD.
    pub bcd_usb: u16,
    /// Device class code.
    pub device_class: u8,
    /// Device subclass code.
    pub device_subclass: u8,
    /// Device protocol code.
    pub device_protocol: u8,
    /// Endpoint 0 max-packet-size byte as encoded by USB.
    pub max_packet_size0: u8,
    /// Vendor ID.
    pub vendor_id: u16,
    /// Product ID.
    pub product_id: u16,
    /// Device release in BCD.
    pub bcd_device: u16,
    /// String descriptor index for manufacturer.
    pub manufacturer_index: u8,
    /// String descriptor index for product.
    pub product_index: u8,
    /// String descriptor index for serial number.
    pub serial_number_index: u8,
    /// Number of available configurations.
    pub num_configurations: u8,
}

/// Status from the manual full USB Device Descriptor read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciReadDeviceDescriptorStatus {
    /// The BSR=0 Address Device checkpoint did not complete successfully.
    SetAddressFailed(XhciSetAddressStatus),
    /// Required controller-start evidence was missing.
    StartEvidenceUnavailable,
    /// No Transfer Event reached the expected event-ring entry.
    TransferTimedOut,
    /// A cycle-valid event arrived, but it was not a transfer event.
    UnexpectedEventType {
        /// Event TRB type field.
        trb_type: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event did not point at the Status Stage TRB.
    TransferPointerMismatch {
        /// Expected Status Stage TRB pointer.
        expected: u64,
        /// Actual pointer reported by the event.
        actual: u64,
        /// xHCI completion code carried by the event.
        completion_code: u8,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// The transfer event reported a different Slot ID.
    SlotIdMismatch {
        /// Expected target slot ID.
        expected: u8,
        /// Actual Slot ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event reported a different Endpoint ID.
    EndpointIdMismatch {
        /// Expected endpoint ID.
        expected: u8,
        /// Actual Endpoint ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer completed with a non-success code.
    TransferFailed {
        /// xHCI completion code.
        completion_code: u8,
        /// Residual bytes not transferred for the generating TRB.
        residual_length: u32,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// Descriptor bytes returned, but did not decode as a full Device Descriptor.
    InvalidDeviceDescriptor {
        /// Descriptor length byte.
        length: u8,
        /// Descriptor type byte.
        descriptor_type: u8,
    },
    /// The full USB Device Descriptor is available in the report.
    DeviceDescriptorReady {
        /// Assigned xHCI slot ID.
        slot_id: u8,
        /// Vendor ID from the descriptor.
        vendor_id: u16,
        /// Product ID from the descriptor.
        product_id: u16,
        /// Number of available configurations.
        num_configurations: u8,
    },
}

/// Evidence returned by the manual full USB Device Descriptor read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciReadDeviceDescriptorReport {
    /// Set-address report produced before queuing the control transfer.
    pub set_address: XhciSetAddressReport,
    /// Final transfer status.
    pub status: XhciReadDeviceDescriptorStatus,
    /// xHCI slot ID targeted by the transfer.
    pub slot_id: u8,
    /// xHCI endpoint ID targeted by the transfer.
    pub endpoint_id: u8,
    /// Doorbell value written to the device slot.
    pub doorbell: u32,
    /// Setup Stage TRB pointer.
    pub setup_trb_pointer: u64,
    /// Data Stage TRB pointer.
    pub data_trb_pointer: u64,
    /// Status Stage TRB pointer.
    pub status_trb_pointer: u64,
    /// Raw Setup Stage TRB written by software.
    pub setup_trb: [u32; 4],
    /// Raw Data Stage TRB written by software.
    pub data_trb: [u32; 4],
    /// Raw Status Stage TRB written by software.
    pub status_trb: [u32; 4],
    /// DMA buffer address used for descriptor bytes.
    pub descriptor_buffer: u64,
    /// Raw full USB Device Descriptor bytes.
    pub descriptor: [u8; USB_DEVICE_DESCRIPTOR_BYTES],
    /// Parsed descriptor fields, if valid.
    pub fields: Option<UsbDeviceDescriptor>,
    /// Transfer event observed for the Status Stage TRB, if one arrived.
    pub event: Option<XhciTransferEvent>,
}

impl XhciReadDeviceDescriptorReport {
    const fn new(
        set_address: XhciSetAddressReport,
        status: XhciReadDeviceDescriptorStatus,
    ) -> Self {
        Self {
            set_address,
            status,
            slot_id: 0,
            endpoint_id: XHCI_DOORBELL_CONTROL_EP0 as u8,
            doorbell: XHCI_DOORBELL_CONTROL_EP0,
            setup_trb_pointer: 0,
            data_trb_pointer: 0,
            status_trb_pointer: 0,
            setup_trb: [0; 4],
            data_trb: [0; 4],
            status_trb: [0; 4],
            descriptor_buffer: 0,
            descriptor: [0; USB_DEVICE_DESCRIPTOR_BYTES],
            fields: None,
            event: None,
        }
    }
}

/// Parsed USB Configuration Descriptor header fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsbConfigurationDescriptorHeader {
    /// Descriptor length byte.
    pub length: u8,
    /// Descriptor type byte.
    pub descriptor_type: u8,
    /// Total bytes in this configuration's descriptor tree.
    pub total_length: u16,
    /// Number of interfaces in this configuration.
    pub num_interfaces: u8,
    /// Configuration value used by SET_CONFIGURATION.
    pub configuration_value: u8,
    /// String descriptor index for this configuration.
    pub configuration_index: u8,
    /// Raw bmAttributes byte.
    pub attributes: u8,
    /// Max bus power draw in 2 mA units.
    pub max_power: u8,
}

/// Status from the manual Configuration Descriptor header read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciReadConfigurationDescriptorHeaderStatus {
    /// The full Device Descriptor checkpoint did not complete successfully.
    DeviceDescriptorFailed(XhciReadDeviceDescriptorStatus),
    /// Required controller-start evidence was missing.
    StartEvidenceUnavailable,
    /// No Transfer Event reached the expected event-ring entry.
    TransferTimedOut,
    /// A cycle-valid event arrived, but it was not a transfer event.
    UnexpectedEventType {
        /// Event TRB type field.
        trb_type: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event did not point at the Status Stage TRB.
    TransferPointerMismatch {
        /// Expected Status Stage TRB pointer.
        expected: u64,
        /// Actual pointer reported by the event.
        actual: u64,
        /// xHCI completion code carried by the event.
        completion_code: u8,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// The transfer event reported a different Slot ID.
    SlotIdMismatch {
        /// Expected target slot ID.
        expected: u8,
        /// Actual Slot ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event reported a different Endpoint ID.
    EndpointIdMismatch {
        /// Expected endpoint ID.
        expected: u8,
        /// Actual Endpoint ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer completed with a non-success code.
    TransferFailed {
        /// xHCI completion code.
        completion_code: u8,
        /// Residual bytes not transferred for the generating TRB.
        residual_length: u32,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// Descriptor bytes returned, but did not decode as a Configuration header.
    InvalidConfigurationDescriptorHeader {
        /// Descriptor length byte.
        length: u8,
        /// Descriptor type byte.
        descriptor_type: u8,
        /// Raw wTotalLength value.
        total_length: u16,
    },
    /// The Configuration Descriptor header is available in the report.
    ConfigurationDescriptorHeaderReady {
        /// Assigned xHCI slot ID.
        slot_id: u8,
        /// Total bytes in this configuration's descriptor tree.
        total_length: u16,
        /// Number of interfaces in this configuration.
        num_interfaces: u8,
        /// Configuration value used by SET_CONFIGURATION.
        configuration_value: u8,
    },
}

/// Evidence returned by the manual Configuration Descriptor header read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciReadConfigurationDescriptorHeaderReport {
    /// Full Device Descriptor report produced before queuing this transfer.
    pub device_descriptor: XhciReadDeviceDescriptorReport,
    /// Final transfer status.
    pub status: XhciReadConfigurationDescriptorHeaderStatus,
    /// xHCI slot ID targeted by the transfer.
    pub slot_id: u8,
    /// xHCI endpoint ID targeted by the transfer.
    pub endpoint_id: u8,
    /// Doorbell value written to the device slot.
    pub doorbell: u32,
    /// Setup Stage TRB pointer.
    pub setup_trb_pointer: u64,
    /// Data Stage TRB pointer.
    pub data_trb_pointer: u64,
    /// Status Stage TRB pointer.
    pub status_trb_pointer: u64,
    /// Raw Setup Stage TRB written by software.
    pub setup_trb: [u32; 4],
    /// Raw Data Stage TRB written by software.
    pub data_trb: [u32; 4],
    /// Raw Status Stage TRB written by software.
    pub status_trb: [u32; 4],
    /// DMA buffer address used for descriptor bytes.
    pub descriptor_buffer: u64,
    /// Raw 9-byte USB Configuration Descriptor header.
    pub descriptor: [u8; USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES],
    /// Parsed descriptor fields, if valid.
    pub fields: Option<UsbConfigurationDescriptorHeader>,
    /// Transfer event observed for the Status Stage TRB, if one arrived.
    pub event: Option<XhciTransferEvent>,
}

impl XhciReadConfigurationDescriptorHeaderReport {
    const fn new(
        device_descriptor: XhciReadDeviceDescriptorReport,
        status: XhciReadConfigurationDescriptorHeaderStatus,
    ) -> Self {
        Self {
            device_descriptor,
            status,
            slot_id: 0,
            endpoint_id: XHCI_DOORBELL_CONTROL_EP0 as u8,
            doorbell: XHCI_DOORBELL_CONTROL_EP0,
            setup_trb_pointer: 0,
            data_trb_pointer: 0,
            status_trb_pointer: 0,
            setup_trb: [0; 4],
            data_trb: [0; 4],
            status_trb: [0; 4],
            descriptor_buffer: 0,
            descriptor: [0; USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES],
            fields: None,
            event: None,
        }
    }
}

/// Parsed USB endpoint descriptor facts relevant to boot-keyboard input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsbEndpointDescriptor {
    /// Raw endpoint address byte.
    pub address: u8,
    /// Endpoint number.
    pub endpoint_number: u8,
    /// Whether the endpoint direction is IN.
    pub direction_in: bool,
    /// Raw bmAttributes byte.
    pub attributes: u8,
    /// Transfer type bits from bmAttributes.
    pub transfer_type: u8,
    /// Maximum packet size.
    pub max_packet_size: u16,
    /// Polling interval.
    pub interval: u8,
}

/// Parsed HID boot-keyboard interface candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsbHidBootKeyboardInterface {
    /// Interface number.
    pub interface_number: u8,
    /// Alternate setting.
    pub alternate_setting: u8,
    /// Number of endpoints advertised by the interface descriptor.
    pub endpoint_count: u8,
    /// Interface protocol byte, expected to be keyboard.
    pub protocol: u8,
    /// Interrupt-IN endpoint for boot-keyboard reports, if found.
    pub interrupt_in_endpoint: Option<UsbEndpointDescriptor>,
}

/// Parsed USB Configuration descriptor tree summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsbConfigurationDescriptorTree {
    /// Parsed Configuration Descriptor header.
    pub header: UsbConfigurationDescriptorHeader,
    /// Number of descriptor records walked in the tree.
    pub descriptor_count: u8,
    /// First HID boot-keyboard interface candidate, if present.
    pub boot_keyboard: Option<UsbHidBootKeyboardInterface>,
}

/// Status from the manual full Configuration descriptor tree read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciReadConfigurationDescriptorStatus {
    /// The Configuration Descriptor header checkpoint did not complete.
    HeaderFailed(XhciReadConfigurationDescriptorHeaderStatus),
    /// The reported `wTotalLength` exceeds this early static buffer.
    ConfigurationTooLarge {
        /// Reported total length.
        total_length: u16,
        /// Maximum static buffer size supported by this probe.
        max_supported: u16,
    },
    /// Required controller-start evidence was missing.
    StartEvidenceUnavailable,
    /// No Transfer Event reached the expected event-ring entry.
    TransferTimedOut,
    /// A cycle-valid event arrived, but it was not a transfer event.
    UnexpectedEventType {
        /// Event TRB type field.
        trb_type: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event did not point at the Status Stage TRB.
    TransferPointerMismatch {
        /// Expected Status Stage TRB pointer.
        expected: u64,
        /// Actual pointer reported by the event.
        actual: u64,
        /// xHCI completion code carried by the event.
        completion_code: u8,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// The transfer event reported a different Slot ID.
    SlotIdMismatch {
        /// Expected target slot ID.
        expected: u8,
        /// Actual Slot ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event reported a different Endpoint ID.
    EndpointIdMismatch {
        /// Expected endpoint ID.
        expected: u8,
        /// Actual Endpoint ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer completed with a non-success code.
    TransferFailed {
        /// xHCI completion code.
        completion_code: u8,
        /// Residual bytes not transferred for the generating TRB.
        residual_length: u32,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// The descriptor tree could not be walked safely.
    InvalidConfigurationDescriptor {
        /// Offset of the invalid record.
        offset: u16,
        /// Descriptor length byte at that offset.
        length: u8,
        /// Descriptor type byte at that offset.
        descriptor_type: u8,
    },
    /// The full Configuration descriptor tree is available in the report.
    ConfigurationDescriptorReady {
        /// Assigned xHCI slot ID.
        slot_id: u8,
        /// Total bytes in this configuration's descriptor tree.
        total_length: u16,
        /// Number of interfaces in this configuration.
        num_interfaces: u8,
        /// Whether a HID boot-keyboard interface with interrupt-IN endpoint was found.
        boot_keyboard_ready_to_configure: bool,
    },
}

/// Evidence returned by the manual full Configuration descriptor tree read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciReadConfigurationDescriptorReport {
    /// Header report produced before queuing this transfer.
    pub header: XhciReadConfigurationDescriptorHeaderReport,
    /// Final transfer status.
    pub status: XhciReadConfigurationDescriptorStatus,
    /// xHCI slot ID targeted by the transfer.
    pub slot_id: u8,
    /// xHCI endpoint ID targeted by the transfer.
    pub endpoint_id: u8,
    /// Doorbell value written to the device slot.
    pub doorbell: u32,
    /// Setup Stage TRB pointer.
    pub setup_trb_pointer: u64,
    /// Data Stage TRB pointer.
    pub data_trb_pointer: u64,
    /// Status Stage TRB pointer.
    pub status_trb_pointer: u64,
    /// Raw Setup Stage TRB written by software.
    pub setup_trb: [u32; 4],
    /// Raw Data Stage TRB written by software.
    pub data_trb: [u32; 4],
    /// Raw Status Stage TRB written by software.
    pub status_trb: [u32; 4],
    /// DMA buffer address used for descriptor bytes.
    pub descriptor_buffer: u64,
    /// Number of valid descriptor bytes in `descriptor`.
    pub descriptor_length: u16,
    /// Raw USB Configuration descriptor tree bytes.
    pub descriptor: [u8; USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES],
    /// Parsed descriptor tree summary, if valid.
    pub fields: Option<UsbConfigurationDescriptorTree>,
    /// Transfer event observed for the Status Stage TRB, if one arrived.
    pub event: Option<XhciTransferEvent>,
}

impl XhciReadConfigurationDescriptorReport {
    const fn new(
        header: XhciReadConfigurationDescriptorHeaderReport,
        status: XhciReadConfigurationDescriptorStatus,
    ) -> Self {
        Self {
            header,
            status,
            slot_id: 0,
            endpoint_id: XHCI_DOORBELL_CONTROL_EP0 as u8,
            doorbell: XHCI_DOORBELL_CONTROL_EP0,
            setup_trb_pointer: 0,
            data_trb_pointer: 0,
            status_trb_pointer: 0,
            setup_trb: [0; 4],
            data_trb: [0; 4],
            status_trb: [0; 4],
            descriptor_buffer: 0,
            descriptor_length: 0,
            descriptor: [0; USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES],
            fields: None,
            event: None,
        }
    }
}

/// Status from the manual USB SET_CONFIGURATION control transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciSetConfigurationStatus {
    /// The full Configuration descriptor checkpoint did not complete.
    ConfigurationDescriptorFailed(XhciReadConfigurationDescriptorStatus),
    /// The descriptor tree did not contain a HID boot-keyboard interrupt-IN endpoint.
    BootKeyboardNotReadyToConfigure,
    /// Required controller-start evidence was missing.
    StartEvidenceUnavailable,
    /// No Transfer Event reached the expected event-ring entry.
    TransferTimedOut,
    /// A cycle-valid event arrived, but it was not a transfer event.
    UnexpectedEventType {
        /// Event TRB type field.
        trb_type: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event did not point at the Status Stage TRB.
    TransferPointerMismatch {
        /// Expected Status Stage TRB pointer.
        expected: u64,
        /// Actual pointer reported by the event.
        actual: u64,
        /// xHCI completion code carried by the event.
        completion_code: u8,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// The transfer event reported a different Slot ID.
    SlotIdMismatch {
        /// Expected target slot ID.
        expected: u8,
        /// Actual Slot ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event reported a different Endpoint ID.
    EndpointIdMismatch {
        /// Expected endpoint ID.
        expected: u8,
        /// Actual Endpoint ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer completed with a non-success code.
    TransferFailed {
        /// xHCI completion code.
        completion_code: u8,
        /// Residual bytes not transferred for the generating TRB.
        residual_length: u32,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// SET_CONFIGURATION completed for the boot-keyboard configuration.
    ConfigurationSet {
        /// Assigned xHCI slot ID.
        slot_id: u8,
        /// USB configuration value sent in the setup packet.
        configuration_value: u8,
        /// HID boot-keyboard interface number selected from descriptors.
        interface_number: u8,
        /// Interrupt-IN endpoint address advertised by that interface.
        endpoint_address: u8,
    },
}

/// Evidence returned by the manual USB SET_CONFIGURATION control transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciSetConfigurationReport {
    /// Full Configuration descriptor report produced before this transfer.
    pub configuration: XhciReadConfigurationDescriptorReport,
    /// Final transfer status.
    pub status: XhciSetConfigurationStatus,
    /// xHCI slot ID targeted by the transfer.
    pub slot_id: u8,
    /// xHCI endpoint ID targeted by the transfer.
    pub endpoint_id: u8,
    /// Doorbell value written to the device slot.
    pub doorbell: u32,
    /// USB configuration value sent in the setup packet.
    pub configuration_value: u8,
    /// HID boot-keyboard interface number selected from descriptors.
    pub interface_number: u8,
    /// Interrupt-IN endpoint address advertised by that interface.
    pub endpoint_address: u8,
    /// Setup Stage TRB pointer.
    pub setup_trb_pointer: u64,
    /// Status Stage TRB pointer.
    pub status_trb_pointer: u64,
    /// Raw Setup Stage TRB written by software.
    pub setup_trb: [u32; 4],
    /// Raw Status Stage TRB written by software.
    pub status_trb: [u32; 4],
    /// Transfer event observed for the Status Stage TRB, if one arrived.
    pub event: Option<XhciTransferEvent>,
}

impl XhciSetConfigurationReport {
    const fn new(
        configuration: XhciReadConfigurationDescriptorReport,
        status: XhciSetConfigurationStatus,
    ) -> Self {
        Self {
            configuration,
            status,
            slot_id: 0,
            endpoint_id: XHCI_DOORBELL_CONTROL_EP0 as u8,
            doorbell: XHCI_DOORBELL_CONTROL_EP0,
            configuration_value: 0,
            interface_number: 0,
            endpoint_address: 0,
            setup_trb_pointer: 0,
            status_trb_pointer: 0,
            setup_trb: [0; 4],
            status_trb: [0; 4],
            event: None,
        }
    }
}

/// Static context values prepared for a manual Configure Endpoint command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciConfigureEndpointContexts {
    /// Input Context base address used by the command.
    pub input_context: u64,
    /// Output Device Context address stored in the DCBAA slot entry.
    pub output_device_context: u64,
    /// Interrupt-IN endpoint transfer-ring base address.
    pub interrupt_in_endpoint_ring: u64,
    /// Drop Context flags dword.
    pub drop_context_flags: u32,
    /// Add Context flags dword.
    pub add_context_flags: u32,
    /// xHCI endpoint ID / DCI for the interrupt-IN endpoint.
    pub endpoint_id: u8,
    /// Input Context index where the endpoint context is written.
    pub endpoint_context_index: u8,
    /// Raw USB endpoint address.
    pub endpoint_address: u8,
    /// Endpoint number.
    pub endpoint_number: u8,
    /// Raw USB `bInterval` value.
    pub interval: u8,
    /// xHCI Interval field encoded from the USB speed and `bInterval`.
    pub interval_encoded: u8,
    /// Normalized endpoint max-packet size used in the endpoint context.
    pub max_packet_size: u16,
    /// Max ESIT payload used for the interrupt endpoint.
    pub max_esit_payload: u16,
    /// DCI 0 Slot Context dwords prepared in the Input Context.
    pub slot_context: [u32; 4],
    /// Interrupt-IN Endpoint Context dwords prepared in the Input Context.
    pub endpoint_context: [u32; 5],
}

/// Status from the manual xHCI Configure Endpoint command for the keyboard endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciConfigureEndpointStatus {
    /// SET_CONFIGURATION did not complete successfully.
    SetConfigurationFailed(XhciSetConfigurationStatus),
    /// The HID boot-keyboard endpoint was unavailable in descriptor evidence.
    BootKeyboardEndpointUnavailable,
    /// Required controller-start evidence was missing.
    StartEvidenceUnavailable,
    /// The endpoint address cannot be represented as an xHCI endpoint ID.
    EndpointIdOutOfRange {
        /// Raw USB endpoint address.
        endpoint_address: u8,
        /// Computed xHCI endpoint ID / DCI.
        endpoint_id: u8,
    },
    /// No Command Completion Event reached the expected event-ring entry.
    CommandTimedOut,
    /// A cycle-valid event arrived, but it was not a command-completion event.
    UnexpectedEventType {
        /// Event TRB type field.
        trb_type: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The completion event did not point back at the Configure Endpoint command TRB.
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
    /// The completion event pointed at this command but returned another Slot ID.
    SlotIdMismatch {
        /// Expected target slot ID.
        expected: u8,
        /// Actual Slot ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The command completed with a non-success code.
    CommandFailed {
        /// xHCI completion code.
        completion_code: u8,
        /// Slot ID carried by the event.
        slot_id: u8,
    },
    /// Configure Endpoint completed and the interrupt-IN endpoint is known to xHC.
    EndpointConfigured {
        /// Assigned xHCI slot ID.
        slot_id: u8,
        /// xHCI endpoint ID / DCI.
        endpoint_id: u8,
        /// Raw USB endpoint address.
        endpoint_address: u8,
        /// Normalized endpoint max-packet size.
        max_packet_size: u16,
        /// xHCI Interval field.
        interval_encoded: u8,
    },
}

/// Evidence returned by the manual xHCI Configure Endpoint command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciConfigureEndpointReport {
    /// SET_CONFIGURATION report produced before issuing Configure Endpoint.
    pub set_configuration: XhciSetConfigurationReport,
    /// Final Configure Endpoint command status.
    pub status: XhciConfigureEndpointStatus,
    /// Configure Endpoint command TRB pointer.
    pub command_trb_pointer: u64,
    /// Raw Configure Endpoint command TRB written by software.
    pub command_trb: [u32; 4],
    /// Doorbell value written to doorbell 0.
    pub doorbell: u32,
    /// Contexts and transfer-ring pointers prepared for the command.
    pub contexts: Option<XhciConfigureEndpointContexts>,
    /// Command-completion event observed for Configure Endpoint, if one arrived.
    pub event: Option<XhciCommandCompletionEvent>,
}

impl XhciConfigureEndpointReport {
    const fn new(
        set_configuration: XhciSetConfigurationReport,
        status: XhciConfigureEndpointStatus,
    ) -> Self {
        Self {
            set_configuration,
            status,
            command_trb_pointer: 0,
            command_trb: [0; 4],
            doorbell: XHCI_DOORBELL_COMMAND,
            contexts: None,
            event: None,
        }
    }
}

/// Status from the manual HID SET_PROTOCOL(Boot) control transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciSetHidProtocolStatus {
    /// Configure Endpoint did not complete successfully.
    ConfigureEndpointFailed(XhciConfigureEndpointStatus),
    /// The HID boot-keyboard interface was unavailable in descriptor evidence.
    BootKeyboardInterfaceUnavailable,
    /// Required controller-start evidence was missing.
    StartEvidenceUnavailable,
    /// No Transfer Event reached the expected event-ring entry.
    TransferTimedOut,
    /// A cycle-valid event arrived, but it was not a transfer event.
    UnexpectedEventType {
        /// Event TRB type field.
        trb_type: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event did not point at the Status Stage TRB.
    TransferPointerMismatch {
        /// Expected Status Stage TRB pointer.
        expected: u64,
        /// Actual pointer reported by the event.
        actual: u64,
        /// xHCI completion code carried by the event.
        completion_code: u8,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// The transfer event reported a different Slot ID.
    SlotIdMismatch {
        /// Expected target slot ID.
        expected: u8,
        /// Actual Slot ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event reported a different Endpoint ID.
    EndpointIdMismatch {
        /// Expected endpoint ID.
        expected: u8,
        /// Actual Endpoint ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer completed with a non-success code.
    TransferFailed {
        /// xHCI completion code.
        completion_code: u8,
        /// Residual bytes not transferred for the generating TRB.
        residual_length: u32,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// SET_PROTOCOL(Boot) completed for the boot-keyboard interface.
    BootProtocolSet {
        /// Assigned xHCI slot ID.
        slot_id: u8,
        /// HID interface number targeted by the request.
        interface_number: u8,
        /// HID protocol value, zero for boot protocol.
        protocol: u8,
    },
}

/// Evidence returned by the manual HID SET_PROTOCOL(Boot) control transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciSetHidProtocolReport {
    /// Configure Endpoint report produced before issuing SET_PROTOCOL.
    pub configure_endpoint: XhciConfigureEndpointReport,
    /// Final transfer status.
    pub status: XhciSetHidProtocolStatus,
    /// xHCI slot ID targeted by the transfer.
    pub slot_id: u8,
    /// xHCI endpoint ID targeted by the transfer.
    pub endpoint_id: u8,
    /// Doorbell value written to the device slot.
    pub doorbell: u32,
    /// HID interface number targeted by SET_PROTOCOL.
    pub interface_number: u8,
    /// HID protocol value, zero for boot protocol.
    pub protocol: u8,
    /// Setup Stage TRB pointer.
    pub setup_trb_pointer: u64,
    /// Status Stage TRB pointer.
    pub status_trb_pointer: u64,
    /// Raw Setup Stage TRB written by software.
    pub setup_trb: [u32; 4],
    /// Raw Status Stage TRB written by software.
    pub status_trb: [u32; 4],
    /// Transfer event observed for the Status Stage TRB, if one arrived.
    pub event: Option<XhciTransferEvent>,
}

impl XhciSetHidProtocolReport {
    const fn new(
        configure_endpoint: XhciConfigureEndpointReport,
        status: XhciSetHidProtocolStatus,
    ) -> Self {
        Self {
            configure_endpoint,
            status,
            slot_id: 0,
            endpoint_id: XHCI_DOORBELL_CONTROL_EP0 as u8,
            doorbell: XHCI_DOORBELL_CONTROL_EP0,
            interface_number: 0,
            protocol: USB_HID_BOOT_PROTOCOL,
            setup_trb_pointer: 0,
            status_trb_pointer: 0,
            setup_trb: [0; 4],
            status_trb: [0; 4],
            event: None,
        }
    }
}

/// Status from the manual HID boot-keyboard interrupt-IN report read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhciReadBootKeyboardReportStatus {
    /// HID SET_PROTOCOL(Boot) did not complete successfully.
    SetHidProtocolFailed(XhciSetHidProtocolStatus),
    /// The configured interrupt-IN endpoint context was unavailable.
    BootKeyboardEndpointUnavailable,
    /// Required controller-start evidence was missing.
    StartEvidenceUnavailable,
    /// No Transfer Event reached the expected event-ring entry.
    TransferTimedOut,
    /// A cycle-valid event arrived, but it was not a transfer event.
    UnexpectedEventType {
        /// Event TRB type field.
        trb_type: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event did not point at the Normal TRB.
    TransferPointerMismatch {
        /// Expected Normal TRB pointer.
        expected: u64,
        /// Actual pointer reported by the event.
        actual: u64,
        /// xHCI completion code carried by the event.
        completion_code: u8,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// The transfer event reported a different Slot ID.
    SlotIdMismatch {
        /// Expected target slot ID.
        expected: u8,
        /// Actual Slot ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The transfer event reported a different Endpoint ID.
    EndpointIdMismatch {
        /// Expected endpoint ID.
        expected: u8,
        /// Actual Endpoint ID carried by the event.
        actual: u8,
        /// xHCI completion code carried by the event.
        completion_code: u8,
    },
    /// The interrupt-IN transfer completed with a non-success code.
    TransferFailed {
        /// xHCI completion code.
        completion_code: u8,
        /// Residual bytes not transferred for the generating TRB.
        residual_length: u32,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
    /// One raw 8-byte HID boot-keyboard report is available.
    ReportReady {
        /// Assigned xHCI slot ID.
        slot_id: u8,
        /// Interrupt-IN xHCI endpoint ID / DCI.
        endpoint_id: u8,
        /// Number of report bytes requested.
        length: u8,
    },
}

/// Evidence returned by the manual HID boot-keyboard interrupt-IN report read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XhciReadBootKeyboardReport {
    /// HID SET_PROTOCOL(Boot) report produced before queuing interrupt-IN.
    pub set_hid_protocol: XhciSetHidProtocolReport,
    /// Final transfer status.
    pub status: XhciReadBootKeyboardReportStatus,
    /// xHCI slot ID targeted by the transfer.
    pub slot_id: u8,
    /// Interrupt-IN xHCI endpoint ID / DCI.
    pub endpoint_id: u8,
    /// Doorbell value written to the device slot.
    pub doorbell: u32,
    /// Normal TRB pointer queued on the interrupt-IN transfer ring.
    pub normal_trb_pointer: u64,
    /// Raw Normal TRB written by software.
    pub normal_trb: [u32; 4],
    /// DMA buffer address used for the boot-keyboard report.
    pub report_buffer: u64,
    /// Raw HID boot-keyboard report bytes.
    pub report: [u8; BOOT_KEYBOARD_REPORT_BYTES],
    /// Transfer event observed for the Normal TRB, if one arrived.
    pub event: Option<XhciTransferEvent>,
}

impl XhciReadBootKeyboardReport {
    const fn new(
        set_hid_protocol: XhciSetHidProtocolReport,
        status: XhciReadBootKeyboardReportStatus,
    ) -> Self {
        Self {
            set_hid_protocol,
            status,
            slot_id: 0,
            endpoint_id: 0,
            doorbell: 0,
            normal_trb_pointer: 0,
            normal_trb: [0; 4],
            report_buffer: 0,
            report: [0; BOOT_KEYBOARD_REPORT_BYTES],
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
    /// The retained provider is waiting for the queued interrupt-IN transfer
    /// to complete.
    ReportPending {
        /// Assigned xHCI slot ID.
        slot_id: u8,
        /// Interrupt-IN xHCI endpoint ID / DCI.
        endpoint_id: u8,
        /// Interrupt-IN transfer-ring TRB index currently owned by the xHC.
        transfer_trb_index: u8,
        /// Event-ring index expected for the completion event.
        event_index: u8,
    },
    /// HID enumeration/setup failed before the provider reached report polling.
    EnumerationFailed,
    /// The interrupt-IN report event arrived but did not match the queued
    /// transfer.
    ReportEventMismatch,
    /// The interrupt-IN report transfer completed with an xHCI failure code.
    ReportTransferFailed {
        /// xHCI completion code.
        completion_code: u8,
        /// Residual bytes not transferred for the generating TRB.
        residual_length: u32,
        /// Slot ID carried by the event.
        slot_id: u8,
        /// Endpoint ID carried by the event.
        endpoint_id: u8,
    },
}

struct UsbBootKeyboardProviderStore(UnsafeCell<UsbBootKeyboardProvider>);

unsafe impl Sync for UsbBootKeyboardProviderStore {}

static USB_BOOT_KEYBOARD_PROVIDER: UsbBootKeyboardProviderStore =
    UsbBootKeyboardProviderStore(UnsafeCell::new(UsbBootKeyboardProvider::new()));

struct UsbBootKeyboardProvider {
    state: UsbBootKeyboardProviderState,
}

impl UsbBootKeyboardProvider {
    const fn new() -> Self {
        Self {
            state: UsbBootKeyboardProviderState::Uninitialized,
        }
    }

    fn poll(&mut self) -> UsbBootKeyboardPoll {
        match self.state {
            UsbBootKeyboardProviderState::Uninitialized => {
                self.state = initialize_boot_keyboard_provider();
            }
            UsbBootKeyboardProviderState::RetryLater {
                pending,
                polls_remaining,
            } => {
                if polls_remaining > 0 {
                    self.state = UsbBootKeyboardProviderState::RetryLater {
                        pending,
                        polls_remaining: polls_remaining - 1,
                    };
                    return UsbBootKeyboardPoll::Pending(pending);
                }
                self.state = initialize_boot_keyboard_provider();
            }
            UsbBootKeyboardProviderState::Ready(_) => {}
        }

        if let UsbBootKeyboardProviderState::RetryLater { pending, .. } = self.state {
            return UsbBootKeyboardPoll::Pending(pending);
        }

        let UsbBootKeyboardProviderState::Ready(mut state) = self.state else {
            return UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::EnumerationFailed);
        };
        let poll = state.poll();
        self.state = match poll {
            UsbBootKeyboardPoll::Pending(pending @ UsbBootKeyboardPending::ReportEventMismatch)
            | UsbBootKeyboardPoll::Pending(
                pending @ UsbBootKeyboardPending::ReportTransferFailed { .. },
            ) => UsbBootKeyboardProviderState::retry_later(pending),
            _ => UsbBootKeyboardProviderState::Ready(state),
        };
        poll
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UsbBootKeyboardProviderState {
    Uninitialized,
    RetryLater {
        pending: UsbBootKeyboardPending,
        polls_remaining: usize,
    },
    Ready(XhciBootKeyboardProvider),
}

impl UsbBootKeyboardProviderState {
    const fn retry_later(pending: UsbBootKeyboardPending) -> Self {
        Self::RetryLater {
            pending,
            polls_remaining: USB_BOOT_KEYBOARD_RETRY_POLLS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct XhciBootKeyboardProvider {
    mmio: usize,
    caps: XhciCapabilities,
    plan: XhciDriverMemoryPlan,
    slot_id: u8,
    endpoint_id: u8,
    event_index: usize,
    event_cycle: bool,
    next_transfer_trb_index: usize,
    transfer_cycle: bool,
    transfer_pending: bool,
    pending_transfer_trb_index: usize,
    normal_trb_pointer: u64,
}

impl XhciBootKeyboardProvider {
    fn poll(&mut self) -> UsbBootKeyboardPoll {
        if !self.transfer_pending {
            self.queue_interrupt_in_transfer();
        }

        let Some(event) = poll_xhci_transfer_event(self.plan, self.event_index, self.event_cycle)
        else {
            return UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::ReportPending {
                slot_id: self.slot_id,
                endpoint_id: self.endpoint_id,
                transfer_trb_index: self.pending_transfer_trb_index as u8,
                event_index: self.event_index as u8,
            });
        };

        acknowledge_xhci_event(
            self.mmio,
            self.caps,
            xhci_event_dequeue_pointer_after(self.plan, self.event_index),
        );
        self.advance_event_index();
        self.transfer_pending = false;

        let status = classify_boot_keyboard_report_transfer_event(
            self.normal_trb_pointer,
            self.slot_id,
            self.endpoint_id,
            event,
        );
        match status {
            XhciReadBootKeyboardReportStatus::ReportReady { .. } => {
                dma_invalidate_range(XHCI_BOOT_KEYBOARD_REPORT.addr(), BOOT_KEYBOARD_REPORT_BYTES);
                UsbBootKeyboardPoll::Report(XHCI_BOOT_KEYBOARD_REPORT.read())
            }
            XhciReadBootKeyboardReportStatus::TransferFailed {
                completion_code,
                residual_length,
                slot_id,
                endpoint_id,
            } => UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::ReportTransferFailed {
                completion_code,
                residual_length,
                slot_id,
                endpoint_id,
            }),
            _ => UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::ReportEventMismatch),
        }
    }

    fn queue_interrupt_in_transfer(&mut self) {
        if self.next_transfer_trb_index >= XHCI_INTERRUPT_IN_ENDPOINT_DATA_TRBS {
            self.next_transfer_trb_index = 0;
            self.transfer_cycle = !self.transfer_cycle;
        }

        let trb_index = self.next_transfer_trb_index;
        let report_buffer = XHCI_BOOT_KEYBOARD_REPORT.addr();
        XHCI_BOOT_KEYBOARD_REPORT.zero();
        dma_clean_range(report_buffer, BOOT_KEYBOARD_REPORT_BYTES);

        self.normal_trb_pointer = XHCI_INTERRUPT_IN_ENDPOINT_RING.trb_addr(trb_index);
        let normal_trb = xhci_normal_transfer_trb_with_cycle(
            report_buffer,
            BOOT_KEYBOARD_REPORT_BYTES as u32,
            self.transfer_cycle,
        );
        write_xhci_interrupt_in_transfer_with_cycle(
            self.plan,
            trb_index,
            normal_trb,
            self.transfer_cycle,
        );

        self.pending_transfer_trb_index = trb_index;
        self.transfer_pending = true;
        self.next_transfer_trb_index += 1;

        compiler_fence(Ordering::SeqCst);
        write_mmio_u32(
            self.mmio
                + self.caps.doorbell_offset as usize
                + (self.slot_id as usize * core::mem::size_of::<u32>()),
            self.endpoint_id as u32,
        );
    }

    fn advance_event_index(&mut self) {
        self.event_index += 1;
        if self.event_index >= self.plan.event_ring_trbs as usize {
            self.event_index = 0;
            self.event_cycle = !self.event_cycle;
        }
    }
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

/// Reads the BCM2711 integrated xHCI capability registers.
///
/// Returns `None` when the register block does not look like an xHCI
/// controller. This is an early probe only; it does not start the controller or
/// imply a keyboard is attached.
#[must_use]
pub fn read_integrated_xhci_capabilities() -> Option<XhciCapabilities> {
    // SAFETY: the address is the BCM2711 integrated xHCI MMIO window translated
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

/// Starts xHCI, enables one slot, issues Address Device with BSR=1, and reads completion.
///
/// BSR=1 deliberately blocks the USB SET_ADDRESS request. This only asks the
/// xHC to install the slot/default-control-endpoint contexts and move the slot
/// into the Default state, which is the next manual bring-up checkpoint before
/// descriptor requests and HID interface selection.
pub fn address_device_on_pcie_xhci_controller() -> XhciAddressDeviceReport {
    let enable = enable_slot_on_pcie_xhci_controller();
    let slot_id = match enable.status {
        XhciEnableSlotStatus::SlotEnabled { slot_id } => slot_id,
        status => {
            return XhciAddressDeviceReport::new(
                enable,
                XhciAddressDeviceStatus::EnableSlotFailed(status),
            );
        }
    };

    let (Some(mmio), Some(caps), Some(plan)) =
        (enable.start.mmio_base, enable.start.capabilities, enable.start.memory)
    else {
        return XhciAddressDeviceReport::new(
            enable,
            XhciAddressDeviceStatus::StartEvidenceUnavailable,
        );
    };

    if slot_id == 0
        || slot_id > caps.max_device_slots
        || slot_id as usize >= XHCI_MAX_DEVICE_CONTEXT_POINTERS
    {
        return XhciAddressDeviceReport::new(
            enable,
            XhciAddressDeviceStatus::SlotIdOutOfRange {
                slot_id,
                max_supported: caps
                    .max_device_slots
                    .min((XHCI_MAX_DEVICE_CONTEXT_POINTERS - 1) as u8),
            },
        );
    }

    let Some(port) = enable.connected_port else {
        return XhciAddressDeviceReport::new(enable, XhciAddressDeviceStatus::NoConnectedRootPort);
    };

    let contexts = xhci_address_device_contexts(plan, port);
    write_xhci_address_device_contexts(plan, slot_id, contexts, true);

    let command_trb_pointer = XHCI_COMMAND_RING.trb_addr(XHCI_ADDRESS_DEVICE_COMMAND_INDEX);
    let command_trb = xhci_address_device_command_trb(
        contexts.input_context,
        slot_id,
        XHCI_ADDRESS_DEVICE_BLOCK_SET_ADDRESS_REQUEST,
    );
    XHCI_COMMAND_RING.set_trb(XHCI_ADDRESS_DEVICE_COMMAND_INDEX, command_trb);
    dma_clean_range(command_trb_pointer, XHCI_TRB_BYTES);

    let mut report = XhciAddressDeviceReport {
        enable,
        status: XhciAddressDeviceStatus::CommandTimedOut,
        command_trb_pointer,
        command_trb,
        doorbell: XHCI_DOORBELL_COMMAND,
        block_set_address_request: XHCI_ADDRESS_DEVICE_BLOCK_SET_ADDRESS_REQUEST,
        contexts: Some(contexts),
        event: None,
    };

    compiler_fence(Ordering::SeqCst);
    write_mmio_u32(mmio + caps.doorbell_offset as usize, XHCI_DOORBELL_COMMAND);

    if let Some(event) =
        wait_for_xhci_command_completion_event(plan, XHCI_ADDRESS_DEVICE_EVENT_INDEX)
    {
        acknowledge_xhci_event(
            mmio,
            caps,
            plan.event_ring + ((XHCI_ADDRESS_DEVICE_EVENT_INDEX + 1) * XHCI_TRB_BYTES) as u64,
        );
        report.event = Some(event);
        report.status = classify_address_device_event(command_trb_pointer, slot_id, event);
    }

    report
}

/// Issues a manual EP0 GET_DESCRIPTOR(Device) transfer after Address Device.
///
/// This is a diagnostic step toward HID enumeration. It reads only the first
/// eight descriptor bytes needed to learn the real EP0 max-packet size; it does
/// not SET_ADDRESS, configure interfaces, poll interrupt endpoints, or claim
/// keyboard readiness.
pub fn get_device_descriptor_prefix_on_pcie_xhci_controller() -> XhciDeviceDescriptorProbeReport {
    let address = address_device_on_pcie_xhci_controller();
    let slot_id = match address.status {
        XhciAddressDeviceStatus::DefaultControlEndpointReady { slot_id } => slot_id,
        status => {
            return XhciDeviceDescriptorProbeReport::new(
                address,
                XhciDeviceDescriptorProbeStatus::AddressDeviceFailed(status),
            );
        }
    };

    let (Some(mmio), Some(caps), Some(plan)) = (
        address.enable.start.mmio_base,
        address.enable.start.capabilities,
        address.enable.start.memory,
    ) else {
        return XhciDeviceDescriptorProbeReport::new(
            address,
            XhciDeviceDescriptorProbeStatus::StartEvidenceUnavailable,
        );
    };

    let (setup_trb_pointer, data_trb_pointer, status_trb_pointer) =
        xhci_ep0_control_trb_pointers(XHCI_EP0_DESCRIPTOR_PREFIX_TRB_INDEX);
    let descriptor_buffer = XHCI_DEVICE_DESCRIPTOR_PREFIX.addr();

    XHCI_DEVICE_DESCRIPTOR_PREFIX.zero();
    let setup = XhciSetupPacket {
        bm_request_type: USB_REQUEST_TYPE_DEVICE_TO_HOST_STANDARD_DEVICE,
        b_request: USB_REQUEST_GET_DESCRIPTOR,
        w_value: (USB_DESCRIPTOR_TYPE_DEVICE as u16) << 8,
        w_index: 0,
        w_length: XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES as u16,
    };
    let setup_trb = xhci_setup_stage_trb(setup);
    let data_trb =
        xhci_data_stage_trb(descriptor_buffer, XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES as u32, true);
    let status_trb = xhci_status_stage_trb(false);

    write_xhci_ep0_control_transfer(
        plan,
        XHCI_EP0_DESCRIPTOR_PREFIX_TRB_INDEX,
        setup_trb,
        data_trb,
        status_trb,
    );
    dma_clean_range(descriptor_buffer, XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES);

    let mut report = XhciDeviceDescriptorProbeReport {
        address,
        status: XhciDeviceDescriptorProbeStatus::TransferTimedOut,
        slot_id,
        endpoint_id: XHCI_DOORBELL_CONTROL_EP0 as u8,
        doorbell: XHCI_DOORBELL_CONTROL_EP0,
        setup_trb_pointer,
        data_trb_pointer,
        status_trb_pointer,
        setup_trb,
        data_trb,
        status_trb,
        descriptor_buffer,
        descriptor: [0; XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES],
        event: None,
    };

    compiler_fence(Ordering::SeqCst);
    write_mmio_u32(
        mmio + caps.doorbell_offset as usize + (slot_id as usize * core::mem::size_of::<u32>()),
        XHCI_DOORBELL_CONTROL_EP0,
    );

    if let Some(event) = wait_for_xhci_transfer_event(plan, XHCI_GET_DESCRIPTOR_EVENT_INDEX) {
        acknowledge_xhci_event(
            mmio,
            caps,
            plan.event_ring + ((XHCI_GET_DESCRIPTOR_EVENT_INDEX + 1) * XHCI_TRB_BYTES) as u64,
        );
        dma_invalidate_range(descriptor_buffer, XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES);
        report.descriptor = XHCI_DEVICE_DESCRIPTOR_PREFIX.read();
        report.event = Some(event);
        report.status = classify_device_descriptor_transfer_event(
            status_trb_pointer,
            slot_id,
            XHCI_DOORBELL_CONTROL_EP0 as u8,
            event,
        );
    }

    report
}

/// Issues Address Device with BSR=0 after reading the descriptor prefix.
///
/// This is the checkpoint where xHC sends the USB SET_ADDRESS request and moves
/// the slot from Default to Addressed. It still does not read full
/// configuration descriptors, configure endpoints, or claim keyboard readiness.
pub fn set_address_on_pcie_xhci_controller() -> XhciSetAddressReport {
    let descriptor = get_device_descriptor_prefix_on_pcie_xhci_controller();
    let slot_id = match descriptor.status {
        XhciDeviceDescriptorProbeStatus::DescriptorPrefixReady { slot_id, .. } => slot_id,
        status => {
            return XhciSetAddressReport::new(
                descriptor,
                XhciSetAddressStatus::DescriptorPrefixFailed(status),
            );
        }
    };

    let (Some(mmio), Some(caps), Some(plan)) = (
        descriptor.address.enable.start.mmio_base,
        descriptor.address.enable.start.capabilities,
        descriptor.address.enable.start.memory,
    ) else {
        return XhciSetAddressReport::new(
            descriptor,
            XhciSetAddressStatus::StartEvidenceUnavailable,
        );
    };

    let Some(endpoint0_max_packet_size) =
        usb_descriptor_endpoint0_max_packet_size(descriptor.descriptor)
    else {
        return XhciSetAddressReport::new(
            descriptor,
            XhciSetAddressStatus::InvalidDescriptorPrefix {
                length: descriptor.descriptor[0],
                descriptor_type: descriptor.descriptor[1],
                max_packet_size0: descriptor.descriptor[7],
                bcd_usb: u16::from_le_bytes([descriptor.descriptor[2], descriptor.descriptor[3]]),
            },
        );
    };

    let Some(port) = descriptor.address.enable.connected_port else {
        return XhciSetAddressReport::new(
            descriptor,
            XhciSetAddressStatus::StartEvidenceUnavailable,
        );
    };

    let contexts =
        xhci_address_device_contexts_with_max_packet_size(plan, port, endpoint0_max_packet_size);
    write_xhci_address_device_contexts(plan, slot_id, contexts, false);

    let command_trb_pointer = XHCI_COMMAND_RING.trb_addr(XHCI_SET_ADDRESS_COMMAND_INDEX);
    let command_trb = xhci_address_device_command_trb(contexts.input_context, slot_id, false);
    XHCI_COMMAND_RING.set_trb(XHCI_SET_ADDRESS_COMMAND_INDEX, command_trb);
    dma_clean_range(command_trb_pointer, XHCI_TRB_BYTES);

    let mut report = XhciSetAddressReport {
        descriptor,
        status: XhciSetAddressStatus::CommandTimedOut,
        command_trb_pointer,
        command_trb,
        doorbell: XHCI_DOORBELL_COMMAND,
        contexts: Some(contexts),
        endpoint0_max_packet_size,
        event: None,
    };

    compiler_fence(Ordering::SeqCst);
    write_mmio_u32(mmio + caps.doorbell_offset as usize, XHCI_DOORBELL_COMMAND);

    if let Some(event) = wait_for_xhci_command_completion_event(plan, XHCI_SET_ADDRESS_EVENT_INDEX)
    {
        acknowledge_xhci_event(
            mmio,
            caps,
            plan.event_ring + ((XHCI_SET_ADDRESS_EVENT_INDEX + 1) * XHCI_TRB_BYTES) as u64,
        );
        report.event = Some(event);
        report.status = classify_set_address_event(
            command_trb_pointer,
            slot_id,
            endpoint0_max_packet_size,
            event,
        );
    }

    report
}

/// Reads the full USB Device Descriptor after SET_ADDRESS.
///
/// This is the next manual enumeration checkpoint. It reports the standard
/// Device Descriptor fields needed before reading Configuration descriptors,
/// selecting a HID boot-keyboard interface, and polling interrupt-IN reports.
pub fn read_device_descriptor_on_pcie_xhci_controller() -> XhciReadDeviceDescriptorReport {
    let set_address = set_address_on_pcie_xhci_controller();
    let slot_id = match set_address.status {
        XhciSetAddressStatus::Addressed { slot_id, .. } => slot_id,
        status => {
            return XhciReadDeviceDescriptorReport::new(
                set_address,
                XhciReadDeviceDescriptorStatus::SetAddressFailed(status),
            );
        }
    };

    let (Some(mmio), Some(caps), Some(plan)) = (
        set_address.descriptor.address.enable.start.mmio_base,
        set_address.descriptor.address.enable.start.capabilities,
        set_address.descriptor.address.enable.start.memory,
    ) else {
        return XhciReadDeviceDescriptorReport::new(
            set_address,
            XhciReadDeviceDescriptorStatus::StartEvidenceUnavailable,
        );
    };

    let (setup_trb_pointer, data_trb_pointer, status_trb_pointer) =
        xhci_ep0_control_trb_pointers(XHCI_EP0_DEVICE_DESCRIPTOR_TRB_INDEX);
    let descriptor_buffer = XHCI_DEVICE_DESCRIPTOR.addr();

    XHCI_DEVICE_DESCRIPTOR.zero();
    let setup = XhciSetupPacket {
        bm_request_type: USB_REQUEST_TYPE_DEVICE_TO_HOST_STANDARD_DEVICE,
        b_request: USB_REQUEST_GET_DESCRIPTOR,
        w_value: (USB_DESCRIPTOR_TYPE_DEVICE as u16) << 8,
        w_index: 0,
        w_length: USB_DEVICE_DESCRIPTOR_BYTES as u16,
    };
    let setup_trb = xhci_setup_stage_trb(setup);
    let data_trb = xhci_data_stage_trb(descriptor_buffer, USB_DEVICE_DESCRIPTOR_BYTES as u32, true);
    let status_trb = xhci_status_stage_trb(false);

    write_xhci_ep0_control_transfer(
        plan,
        XHCI_EP0_DEVICE_DESCRIPTOR_TRB_INDEX,
        setup_trb,
        data_trb,
        status_trb,
    );
    dma_clean_range(descriptor_buffer, USB_DEVICE_DESCRIPTOR_BYTES);

    let mut report = XhciReadDeviceDescriptorReport {
        set_address,
        status: XhciReadDeviceDescriptorStatus::TransferTimedOut,
        slot_id,
        endpoint_id: XHCI_DOORBELL_CONTROL_EP0 as u8,
        doorbell: XHCI_DOORBELL_CONTROL_EP0,
        setup_trb_pointer,
        data_trb_pointer,
        status_trb_pointer,
        setup_trb,
        data_trb,
        status_trb,
        descriptor_buffer,
        descriptor: [0; USB_DEVICE_DESCRIPTOR_BYTES],
        fields: None,
        event: None,
    };

    compiler_fence(Ordering::SeqCst);
    write_mmio_u32(
        mmio + caps.doorbell_offset as usize + (slot_id as usize * core::mem::size_of::<u32>()),
        XHCI_DOORBELL_CONTROL_EP0,
    );

    if let Some(event) = wait_for_xhci_transfer_event(plan, XHCI_READ_DEVICE_DESCRIPTOR_EVENT_INDEX)
    {
        acknowledge_xhci_event(
            mmio,
            caps,
            plan.event_ring
                + ((XHCI_READ_DEVICE_DESCRIPTOR_EVENT_INDEX + 1) * XHCI_TRB_BYTES) as u64,
        );
        dma_invalidate_range(descriptor_buffer, USB_DEVICE_DESCRIPTOR_BYTES);
        report.descriptor = XHCI_DEVICE_DESCRIPTOR.read();
        report.fields = parse_usb_device_descriptor(report.descriptor);
        report.event = Some(event);
        report.status = classify_read_device_descriptor_transfer_event(
            status_trb_pointer,
            slot_id,
            XHCI_DOORBELL_CONTROL_EP0 as u8,
            report.descriptor,
            event,
        );
    }

    report
}

/// Reads the 9-byte USB Configuration Descriptor header after the Device Descriptor.
///
/// This checkpoint discovers `wTotalLength`, which bounds the later full
/// Configuration descriptor tree read used to find HID boot-keyboard
/// interfaces and interrupt-IN endpoints.
pub fn read_configuration_descriptor_header_on_pcie_xhci_controller()
-> XhciReadConfigurationDescriptorHeaderReport {
    let device_descriptor = read_device_descriptor_on_pcie_xhci_controller();
    let slot_id = match device_descriptor.status {
        XhciReadDeviceDescriptorStatus::DeviceDescriptorReady { slot_id, .. } => slot_id,
        status => {
            return XhciReadConfigurationDescriptorHeaderReport::new(
                device_descriptor,
                XhciReadConfigurationDescriptorHeaderStatus::DeviceDescriptorFailed(status),
            );
        }
    };

    let (Some(mmio), Some(caps), Some(plan)) = (
        device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .mmio_base,
        device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .capabilities,
        device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .memory,
    ) else {
        return XhciReadConfigurationDescriptorHeaderReport::new(
            device_descriptor,
            XhciReadConfigurationDescriptorHeaderStatus::StartEvidenceUnavailable,
        );
    };

    let (setup_trb_pointer, data_trb_pointer, status_trb_pointer) =
        xhci_ep0_control_trb_pointers(XHCI_EP0_CONFIGURATION_DESCRIPTOR_HEADER_TRB_INDEX);
    let descriptor_buffer = XHCI_CONFIGURATION_DESCRIPTOR_HEADER.addr();

    XHCI_CONFIGURATION_DESCRIPTOR_HEADER.zero();
    let setup = XhciSetupPacket {
        bm_request_type: USB_REQUEST_TYPE_DEVICE_TO_HOST_STANDARD_DEVICE,
        b_request: USB_REQUEST_GET_DESCRIPTOR,
        w_value: (USB_DESCRIPTOR_TYPE_CONFIGURATION as u16) << 8,
        w_index: 0,
        w_length: USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES as u16,
    };
    let setup_trb = xhci_setup_stage_trb(setup);
    let data_trb = xhci_data_stage_trb(
        descriptor_buffer,
        USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES as u32,
        true,
    );
    let status_trb = xhci_status_stage_trb(false);

    write_xhci_ep0_control_transfer(
        plan,
        XHCI_EP0_CONFIGURATION_DESCRIPTOR_HEADER_TRB_INDEX,
        setup_trb,
        data_trb,
        status_trb,
    );
    dma_clean_range(descriptor_buffer, USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES);

    let mut report = XhciReadConfigurationDescriptorHeaderReport {
        device_descriptor,
        status: XhciReadConfigurationDescriptorHeaderStatus::TransferTimedOut,
        slot_id,
        endpoint_id: XHCI_DOORBELL_CONTROL_EP0 as u8,
        doorbell: XHCI_DOORBELL_CONTROL_EP0,
        setup_trb_pointer,
        data_trb_pointer,
        status_trb_pointer,
        setup_trb,
        data_trb,
        status_trb,
        descriptor_buffer,
        descriptor: [0; USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES],
        fields: None,
        event: None,
    };

    compiler_fence(Ordering::SeqCst);
    write_mmio_u32(
        mmio + caps.doorbell_offset as usize + (slot_id as usize * core::mem::size_of::<u32>()),
        XHCI_DOORBELL_CONTROL_EP0,
    );

    if let Some(event) =
        wait_for_xhci_transfer_event(plan, XHCI_READ_CONFIGURATION_DESCRIPTOR_HEADER_EVENT_INDEX)
    {
        acknowledge_xhci_event(
            mmio,
            caps,
            plan.event_ring
                + ((XHCI_READ_CONFIGURATION_DESCRIPTOR_HEADER_EVENT_INDEX + 1) * XHCI_TRB_BYTES)
                    as u64,
        );
        dma_invalidate_range(descriptor_buffer, USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES);
        report.descriptor = XHCI_CONFIGURATION_DESCRIPTOR_HEADER.read();
        report.fields = parse_usb_configuration_descriptor_header(report.descriptor);
        report.event = Some(event);
        report.status = classify_read_configuration_descriptor_header_transfer_event(
            status_trb_pointer,
            slot_id,
            XHCI_DOORBELL_CONTROL_EP0 as u8,
            report.descriptor,
            event,
        );
    }

    report
}

/// Reads and walks the full USB Configuration descriptor tree.
///
/// This checkpoint still does not set a configuration, issue HID class
/// requests, or poll interrupt-IN. It identifies whether the attached device
/// advertises a HID boot-keyboard interface and interrupt-IN endpoint that the
/// next bring-up cuts can configure.
pub fn read_configuration_descriptor_on_pcie_xhci_controller()
-> XhciReadConfigurationDescriptorReport {
    let header = read_configuration_descriptor_header_on_pcie_xhci_controller();
    let (slot_id, total_length) = match header.status {
        XhciReadConfigurationDescriptorHeaderStatus::ConfigurationDescriptorHeaderReady {
            slot_id,
            total_length,
            ..
        } => (slot_id, total_length),
        status => {
            return XhciReadConfigurationDescriptorReport::new(
                header,
                XhciReadConfigurationDescriptorStatus::HeaderFailed(status),
            );
        }
    };

    if total_length as usize > USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES {
        return XhciReadConfigurationDescriptorReport::new(
            header,
            XhciReadConfigurationDescriptorStatus::ConfigurationTooLarge {
                total_length,
                max_supported: USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES as u16,
            },
        );
    }

    let (Some(mmio), Some(caps), Some(plan)) = (
        header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .mmio_base,
        header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .capabilities,
        header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .memory,
    ) else {
        return XhciReadConfigurationDescriptorReport::new(
            header,
            XhciReadConfigurationDescriptorStatus::StartEvidenceUnavailable,
        );
    };

    let (setup_trb_pointer, data_trb_pointer, status_trb_pointer) =
        xhci_ep0_control_trb_pointers(XHCI_EP0_CONFIGURATION_DESCRIPTOR_TRB_INDEX);
    let descriptor_buffer = XHCI_CONFIGURATION_DESCRIPTOR.addr();

    XHCI_CONFIGURATION_DESCRIPTOR.zero();
    let setup = XhciSetupPacket {
        bm_request_type: USB_REQUEST_TYPE_DEVICE_TO_HOST_STANDARD_DEVICE,
        b_request: USB_REQUEST_GET_DESCRIPTOR,
        w_value: (USB_DESCRIPTOR_TYPE_CONFIGURATION as u16) << 8,
        w_index: 0,
        w_length: total_length,
    };
    let setup_trb = xhci_setup_stage_trb(setup);
    let data_trb = xhci_data_stage_trb(descriptor_buffer, total_length as u32, true);
    let status_trb = xhci_status_stage_trb(false);

    write_xhci_ep0_control_transfer(
        plan,
        XHCI_EP0_CONFIGURATION_DESCRIPTOR_TRB_INDEX,
        setup_trb,
        data_trb,
        status_trb,
    );
    dma_clean_range(descriptor_buffer, total_length as usize);

    let mut report = XhciReadConfigurationDescriptorReport {
        header,
        status: XhciReadConfigurationDescriptorStatus::TransferTimedOut,
        slot_id,
        endpoint_id: XHCI_DOORBELL_CONTROL_EP0 as u8,
        doorbell: XHCI_DOORBELL_CONTROL_EP0,
        setup_trb_pointer,
        data_trb_pointer,
        status_trb_pointer,
        setup_trb,
        data_trb,
        status_trb,
        descriptor_buffer,
        descriptor_length: total_length,
        descriptor: [0; USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES],
        fields: None,
        event: None,
    };

    compiler_fence(Ordering::SeqCst);
    write_mmio_u32(
        mmio + caps.doorbell_offset as usize + (slot_id as usize * core::mem::size_of::<u32>()),
        XHCI_DOORBELL_CONTROL_EP0,
    );

    if let Some(event) =
        wait_for_xhci_transfer_event(plan, XHCI_READ_CONFIGURATION_DESCRIPTOR_EVENT_INDEX)
    {
        acknowledge_xhci_event(
            mmio,
            caps,
            plan.event_ring
                + ((XHCI_READ_CONFIGURATION_DESCRIPTOR_EVENT_INDEX + 1) * XHCI_TRB_BYTES) as u64,
        );
        dma_invalidate_range(descriptor_buffer, total_length as usize);
        report.descriptor = XHCI_CONFIGURATION_DESCRIPTOR.read();
        report.fields = parse_usb_configuration_descriptor_tree(report.descriptor, total_length);
        report.event = Some(event);
        report.status = classify_read_configuration_descriptor_transfer_event(
            status_trb_pointer,
            slot_id,
            XHCI_DOORBELL_CONTROL_EP0 as u8,
            report.descriptor,
            total_length,
            event,
        );
    }

    report
}

/// Issues SET_CONFIGURATION for the discovered HID boot-keyboard configuration.
///
/// This checkpoint changes the USB device configuration, but still does not
/// configure the xHCI interrupt endpoint context, issue HID class requests, poll
/// interrupt-IN, or claim keyboard readiness.
pub fn set_configuration_on_pcie_xhci_controller() -> XhciSetConfigurationReport {
    let configuration = read_configuration_descriptor_on_pcie_xhci_controller();
    let slot_id = match configuration.status {
        XhciReadConfigurationDescriptorStatus::ConfigurationDescriptorReady {
            slot_id,
            boot_keyboard_ready_to_configure,
            ..
        } if boot_keyboard_ready_to_configure => slot_id,
        XhciReadConfigurationDescriptorStatus::ConfigurationDescriptorReady { .. } => {
            return XhciSetConfigurationReport::new(
                configuration,
                XhciSetConfigurationStatus::BootKeyboardNotReadyToConfigure,
            );
        }
        status => {
            return XhciSetConfigurationReport::new(
                configuration,
                XhciSetConfigurationStatus::ConfigurationDescriptorFailed(status),
            );
        }
    };

    let Some(fields) = configuration.fields else {
        return XhciSetConfigurationReport::new(
            configuration,
            XhciSetConfigurationStatus::BootKeyboardNotReadyToConfigure,
        );
    };
    let Some(keyboard) = fields.boot_keyboard else {
        return XhciSetConfigurationReport::new(
            configuration,
            XhciSetConfigurationStatus::BootKeyboardNotReadyToConfigure,
        );
    };
    let Some(endpoint) = keyboard.interrupt_in_endpoint else {
        return XhciSetConfigurationReport::new(
            configuration,
            XhciSetConfigurationStatus::BootKeyboardNotReadyToConfigure,
        );
    };

    let (Some(mmio), Some(caps), Some(plan)) = (
        configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .mmio_base,
        configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .capabilities,
        configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .memory,
    ) else {
        return XhciSetConfigurationReport::new(
            configuration,
            XhciSetConfigurationStatus::StartEvidenceUnavailable,
        );
    };

    let (setup_trb_pointer, status_trb_pointer) =
        xhci_ep0_no_data_control_trb_pointers(XHCI_EP0_SET_CONFIGURATION_TRB_INDEX);
    let setup = XhciSetupPacket {
        bm_request_type: USB_REQUEST_TYPE_HOST_TO_DEVICE_STANDARD_DEVICE,
        b_request: USB_REQUEST_SET_CONFIGURATION,
        w_value: fields.header.configuration_value as u16,
        w_index: 0,
        w_length: 0,
    };
    let setup_trb = xhci_setup_stage_no_data_trb(setup);
    let status_trb = xhci_status_stage_trb(true);

    write_xhci_ep0_no_data_control_transfer(
        plan,
        XHCI_EP0_SET_CONFIGURATION_TRB_INDEX,
        setup_trb,
        status_trb,
    );

    let mut report = XhciSetConfigurationReport {
        configuration,
        status: XhciSetConfigurationStatus::TransferTimedOut,
        slot_id,
        endpoint_id: XHCI_DOORBELL_CONTROL_EP0 as u8,
        doorbell: XHCI_DOORBELL_CONTROL_EP0,
        configuration_value: fields.header.configuration_value,
        interface_number: keyboard.interface_number,
        endpoint_address: endpoint.address,
        setup_trb_pointer,
        status_trb_pointer,
        setup_trb,
        status_trb,
        event: None,
    };

    compiler_fence(Ordering::SeqCst);
    write_mmio_u32(
        mmio + caps.doorbell_offset as usize + (slot_id as usize * core::mem::size_of::<u32>()),
        XHCI_DOORBELL_CONTROL_EP0,
    );

    if let Some(event) = wait_for_xhci_transfer_event(plan, XHCI_SET_CONFIGURATION_EVENT_INDEX) {
        acknowledge_xhci_event(
            mmio,
            caps,
            plan.event_ring + ((XHCI_SET_CONFIGURATION_EVENT_INDEX + 1) * XHCI_TRB_BYTES) as u64,
        );
        report.event = Some(event);
        report.status = classify_set_configuration_transfer_event(
            status_trb_pointer,
            slot_id,
            XHCI_DOORBELL_CONTROL_EP0 as u8,
            fields.header.configuration_value,
            keyboard.interface_number,
            endpoint.address,
            event,
        );
    }

    report
}

/// Issues Configure Endpoint for the discovered HID boot-keyboard interrupt-IN endpoint.
///
/// This checkpoint teaches xHC about the interrupt-IN endpoint transfer ring.
/// It still does not queue interrupt-IN Normal TRBs, issue HID class requests,
/// return keyboard reports, or claim keyboard readiness.
pub fn configure_keyboard_endpoint_on_pcie_xhci_controller() -> XhciConfigureEndpointReport {
    let set_configuration = set_configuration_on_pcie_xhci_controller();
    let slot_id = match set_configuration.status {
        XhciSetConfigurationStatus::ConfigurationSet { slot_id, .. } => slot_id,
        status => {
            return XhciConfigureEndpointReport::new(
                set_configuration,
                XhciConfigureEndpointStatus::SetConfigurationFailed(status),
            );
        }
    };

    let Some(fields) = set_configuration.configuration.fields else {
        return XhciConfigureEndpointReport::new(
            set_configuration,
            XhciConfigureEndpointStatus::BootKeyboardEndpointUnavailable,
        );
    };
    let Some(keyboard) = fields.boot_keyboard else {
        return XhciConfigureEndpointReport::new(
            set_configuration,
            XhciConfigureEndpointStatus::BootKeyboardEndpointUnavailable,
        );
    };
    let Some(endpoint) = keyboard.interrupt_in_endpoint else {
        return XhciConfigureEndpointReport::new(
            set_configuration,
            XhciConfigureEndpointStatus::BootKeyboardEndpointUnavailable,
        );
    };
    let Some(port) = set_configuration
        .configuration
        .header
        .device_descriptor
        .set_address
        .descriptor
        .address
        .enable
        .connected_port
    else {
        return XhciConfigureEndpointReport::new(
            set_configuration,
            XhciConfigureEndpointStatus::StartEvidenceUnavailable,
        );
    };

    let (Some(mmio), Some(caps), Some(plan)) = (
        set_configuration
            .configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .mmio_base,
        set_configuration
            .configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .capabilities,
        set_configuration
            .configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .memory,
    ) else {
        return XhciConfigureEndpointReport::new(
            set_configuration,
            XhciConfigureEndpointStatus::StartEvidenceUnavailable,
        );
    };

    let endpoint_id = xhci_endpoint_id(endpoint);
    if endpoint_id == 0 || endpoint_id > XHCI_ENDPOINT_DCI_MAX {
        return XhciConfigureEndpointReport::new(
            set_configuration,
            XhciConfigureEndpointStatus::EndpointIdOutOfRange {
                endpoint_address: endpoint.address,
                endpoint_id,
            },
        );
    }

    let contexts = xhci_configure_keyboard_endpoint_contexts(plan, port, endpoint, endpoint_id);
    write_xhci_configure_endpoint_contexts(plan, slot_id, contexts);

    let command_trb_pointer = XHCI_COMMAND_RING.trb_addr(XHCI_CONFIGURE_ENDPOINT_COMMAND_INDEX);
    let command_trb = xhci_configure_endpoint_command_trb(contexts.input_context, slot_id);
    XHCI_COMMAND_RING.set_trb(XHCI_CONFIGURE_ENDPOINT_COMMAND_INDEX, command_trb);
    dma_clean_range(command_trb_pointer, XHCI_TRB_BYTES);

    let mut report = XhciConfigureEndpointReport {
        set_configuration,
        status: XhciConfigureEndpointStatus::CommandTimedOut,
        command_trb_pointer,
        command_trb,
        doorbell: XHCI_DOORBELL_COMMAND,
        contexts: Some(contexts),
        event: None,
    };

    compiler_fence(Ordering::SeqCst);
    write_mmio_u32(mmio + caps.doorbell_offset as usize, XHCI_DOORBELL_COMMAND);

    if let Some(event) =
        wait_for_xhci_command_completion_event(plan, XHCI_CONFIGURE_ENDPOINT_EVENT_INDEX)
    {
        acknowledge_xhci_event(
            mmio,
            caps,
            plan.event_ring + ((XHCI_CONFIGURE_ENDPOINT_EVENT_INDEX + 1) * XHCI_TRB_BYTES) as u64,
        );
        report.event = Some(event);
        report.status = classify_configure_endpoint_event(
            command_trb_pointer,
            slot_id,
            contexts.endpoint_id,
            contexts.endpoint_address,
            contexts.max_packet_size,
            contexts.interval_encoded,
            event,
        );
    }

    report
}

/// Issues HID SET_PROTOCOL(Boot) for the configured boot-keyboard interface.
///
/// This checkpoint switches the HID interface to boot protocol mode. It still
/// does not queue interrupt-IN Normal TRBs, return keyboard reports, or claim
/// keyboard readiness.
pub fn set_hid_boot_protocol_on_pcie_xhci_controller() -> XhciSetHidProtocolReport {
    let configure_endpoint = configure_keyboard_endpoint_on_pcie_xhci_controller();
    let slot_id = match configure_endpoint.status {
        XhciConfigureEndpointStatus::EndpointConfigured { slot_id, .. } => slot_id,
        status => {
            return XhciSetHidProtocolReport::new(
                configure_endpoint,
                XhciSetHidProtocolStatus::ConfigureEndpointFailed(status),
            );
        }
    };

    let Some(fields) = configure_endpoint.set_configuration.configuration.fields else {
        return XhciSetHidProtocolReport::new(
            configure_endpoint,
            XhciSetHidProtocolStatus::BootKeyboardInterfaceUnavailable,
        );
    };
    let Some(keyboard) = fields.boot_keyboard else {
        return XhciSetHidProtocolReport::new(
            configure_endpoint,
            XhciSetHidProtocolStatus::BootKeyboardInterfaceUnavailable,
        );
    };

    let (Some(mmio), Some(caps), Some(plan)) = (
        configure_endpoint
            .set_configuration
            .configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .mmio_base,
        configure_endpoint
            .set_configuration
            .configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .capabilities,
        configure_endpoint
            .set_configuration
            .configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .memory,
    ) else {
        return XhciSetHidProtocolReport::new(
            configure_endpoint,
            XhciSetHidProtocolStatus::StartEvidenceUnavailable,
        );
    };

    let (setup_trb_pointer, status_trb_pointer) =
        xhci_ep0_no_data_control_trb_pointers(XHCI_EP0_SET_HID_PROTOCOL_TRB_INDEX);
    let setup = XhciSetupPacket {
        bm_request_type: USB_REQUEST_TYPE_HOST_TO_DEVICE_CLASS_INTERFACE,
        b_request: USB_HID_REQUEST_SET_PROTOCOL,
        w_value: USB_HID_BOOT_PROTOCOL as u16,
        w_index: keyboard.interface_number as u16,
        w_length: 0,
    };
    let setup_trb = xhci_setup_stage_no_data_trb(setup);
    let status_trb = xhci_status_stage_trb(true);

    write_xhci_ep0_no_data_control_transfer(
        plan,
        XHCI_EP0_SET_HID_PROTOCOL_TRB_INDEX,
        setup_trb,
        status_trb,
    );

    let mut report = XhciSetHidProtocolReport {
        configure_endpoint,
        status: XhciSetHidProtocolStatus::TransferTimedOut,
        slot_id,
        endpoint_id: XHCI_DOORBELL_CONTROL_EP0 as u8,
        doorbell: XHCI_DOORBELL_CONTROL_EP0,
        interface_number: keyboard.interface_number,
        protocol: USB_HID_BOOT_PROTOCOL,
        setup_trb_pointer,
        status_trb_pointer,
        setup_trb,
        status_trb,
        event: None,
    };

    compiler_fence(Ordering::SeqCst);
    write_mmio_u32(
        mmio + caps.doorbell_offset as usize + (slot_id as usize * core::mem::size_of::<u32>()),
        XHCI_DOORBELL_CONTROL_EP0,
    );

    if let Some(event) = wait_for_xhci_transfer_event(plan, XHCI_SET_HID_PROTOCOL_EVENT_INDEX) {
        acknowledge_xhci_event(
            mmio,
            caps,
            plan.event_ring + ((XHCI_SET_HID_PROTOCOL_EVENT_INDEX + 1) * XHCI_TRB_BYTES) as u64,
        );
        report.event = Some(event);
        report.status = classify_set_hid_protocol_transfer_event(
            status_trb_pointer,
            slot_id,
            XHCI_DOORBELL_CONTROL_EP0 as u8,
            keyboard.interface_number,
            USB_HID_BOOT_PROTOCOL,
            event,
        );
    }

    report
}

/// Queues one HID boot-keyboard interrupt-IN transfer and returns the raw report.
///
/// This is the first checkpoint that can produce the actual 8-byte keyboard
/// report. It remains an explicit manual transition; the root-shell polling path
/// still needs a stateful provider before boot readiness can change.
pub fn read_boot_keyboard_report_on_pcie_xhci_controller() -> XhciReadBootKeyboardReport {
    let set_hid_protocol = set_hid_boot_protocol_on_pcie_xhci_controller();
    let slot_id = match set_hid_protocol.status {
        XhciSetHidProtocolStatus::BootProtocolSet { slot_id, .. } => slot_id,
        status => {
            return XhciReadBootKeyboardReport::new(
                set_hid_protocol,
                XhciReadBootKeyboardReportStatus::SetHidProtocolFailed(status),
            );
        }
    };
    let Some(contexts) = set_hid_protocol.configure_endpoint.contexts else {
        return XhciReadBootKeyboardReport::new(
            set_hid_protocol,
            XhciReadBootKeyboardReportStatus::BootKeyboardEndpointUnavailable,
        );
    };

    let (Some(mmio), Some(caps), Some(plan)) = (
        set_hid_protocol
            .configure_endpoint
            .set_configuration
            .configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .mmio_base,
        set_hid_protocol
            .configure_endpoint
            .set_configuration
            .configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .capabilities,
        set_hid_protocol
            .configure_endpoint
            .set_configuration
            .configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .memory,
    ) else {
        return XhciReadBootKeyboardReport::new(
            set_hid_protocol,
            XhciReadBootKeyboardReportStatus::StartEvidenceUnavailable,
        );
    };

    let report_buffer = XHCI_BOOT_KEYBOARD_REPORT.addr();
    XHCI_BOOT_KEYBOARD_REPORT.zero();
    dma_clean_range(report_buffer, BOOT_KEYBOARD_REPORT_BYTES);

    let normal_trb_pointer =
        XHCI_INTERRUPT_IN_ENDPOINT_RING.trb_addr(XHCI_INTERRUPT_IN_REPORT_TRB_INDEX);
    let normal_trb = xhci_normal_transfer_trb(report_buffer, BOOT_KEYBOARD_REPORT_BYTES as u32);
    write_xhci_interrupt_in_transfer(plan, XHCI_INTERRUPT_IN_REPORT_TRB_INDEX, normal_trb);

    let mut report = XhciReadBootKeyboardReport {
        set_hid_protocol,
        status: XhciReadBootKeyboardReportStatus::TransferTimedOut,
        slot_id,
        endpoint_id: contexts.endpoint_id,
        doorbell: contexts.endpoint_id as u32,
        normal_trb_pointer,
        normal_trb,
        report_buffer,
        report: [0; BOOT_KEYBOARD_REPORT_BYTES],
        event: None,
    };

    compiler_fence(Ordering::SeqCst);
    write_mmio_u32(
        mmio + caps.doorbell_offset as usize + (slot_id as usize * core::mem::size_of::<u32>()),
        contexts.endpoint_id as u32,
    );

    if let Some(event) = wait_for_xhci_transfer_event(plan, XHCI_BOOT_KEYBOARD_REPORT_EVENT_INDEX) {
        acknowledge_xhci_event(
            mmio,
            caps,
            plan.event_ring + ((XHCI_BOOT_KEYBOARD_REPORT_EVENT_INDEX + 1) * XHCI_TRB_BYTES) as u64,
        );
        dma_invalidate_range(report_buffer, BOOT_KEYBOARD_REPORT_BYTES);
        report.report = XHCI_BOOT_KEYBOARD_REPORT.read();
        report.event = Some(event);
        report.status = classify_boot_keyboard_report_transfer_event(
            normal_trb_pointer,
            slot_id,
            contexts.endpoint_id,
            event,
        );
    }

    report
}

/// Polls the retained lower USB boot-keyboard provider.
///
/// The first call attempts the manual xHCI/HID setup sequence and stores the
/// resulting controller/endpoint state. Once setup succeeds, later calls only
/// queue or poll the HID interrupt-IN transfer ring and never rerun descriptor
/// enumeration. Setup failures are retried after a bounded poll backoff so a
/// slow controller or late keyboard attach does not require a reboot.
#[must_use]
pub fn poll_boot_keyboard_report() -> UsbBootKeyboardPoll {
    let provider = unsafe { &mut *USB_BOOT_KEYBOARD_PROVIDER.0.get() };
    provider.poll()
}

fn initialize_boot_keyboard_provider() -> UsbBootKeyboardProviderState {
    let Some(controller) = probe_pcie_xhci_controller() else {
        return UsbBootKeyboardProviderState::retry_later(
            UsbBootKeyboardPending::NoPcieXhciController,
        );
    };
    let Some(_mmio) = controller.mmio_base else {
        return UsbBootKeyboardProviderState::retry_later(
            UsbBootKeyboardPending::ControllerBarUnconfigured,
        );
    };

    let set_hid_protocol = set_hid_boot_protocol_on_pcie_xhci_controller();
    let slot_id = match set_hid_protocol.status {
        XhciSetHidProtocolStatus::BootProtocolSet { slot_id, .. } => slot_id,
        _ => {
            return UsbBootKeyboardProviderState::retry_later(
                usb_boot_keyboard_pending_from_set_hid_protocol_status(set_hid_protocol.status),
            );
        }
    };
    let Some(contexts) = set_hid_protocol.configure_endpoint.contexts else {
        return UsbBootKeyboardProviderState::retry_later(
            UsbBootKeyboardPending::EnumerationFailed,
        );
    };
    let (Some(mmio), Some(caps), Some(plan)) = (
        set_hid_protocol
            .configure_endpoint
            .set_configuration
            .configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .mmio_base,
        set_hid_protocol
            .configure_endpoint
            .set_configuration
            .configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .capabilities,
        set_hid_protocol
            .configure_endpoint
            .set_configuration
            .configuration
            .header
            .device_descriptor
            .set_address
            .descriptor
            .address
            .enable
            .start
            .memory,
    ) else {
        return UsbBootKeyboardProviderState::retry_later(
            UsbBootKeyboardPending::EnumerationFailed,
        );
    };

    UsbBootKeyboardProviderState::Ready(XhciBootKeyboardProvider {
        mmio,
        caps,
        plan,
        slot_id,
        endpoint_id: contexts.endpoint_id,
        event_index: XHCI_BOOT_KEYBOARD_REPORT_EVENT_INDEX,
        event_cycle: true,
        next_transfer_trb_index: XHCI_INTERRUPT_IN_REPORT_TRB_INDEX,
        transfer_cycle: true,
        transfer_pending: false,
        pending_transfer_trb_index: XHCI_INTERRUPT_IN_REPORT_TRB_INDEX,
        normal_trb_pointer: 0,
    })
}

fn usb_boot_keyboard_pending_from_set_hid_protocol_status(
    status: XhciSetHidProtocolStatus,
) -> UsbBootKeyboardPending {
    match status {
        XhciSetHidProtocolStatus::ConfigureEndpointFailed(status) => {
            usb_boot_keyboard_pending_from_configure_endpoint_status(status)
        }
        XhciSetHidProtocolStatus::BootKeyboardInterfaceUnavailable => {
            UsbBootKeyboardPending::EnumerationFailed
        }
        XhciSetHidProtocolStatus::StartEvidenceUnavailable
        | XhciSetHidProtocolStatus::TransferTimedOut
        | XhciSetHidProtocolStatus::UnexpectedEventType { .. }
        | XhciSetHidProtocolStatus::TransferPointerMismatch { .. }
        | XhciSetHidProtocolStatus::SlotIdMismatch { .. }
        | XhciSetHidProtocolStatus::EndpointIdMismatch { .. }
        | XhciSetHidProtocolStatus::TransferFailed { .. }
        | XhciSetHidProtocolStatus::BootProtocolSet { .. } => {
            UsbBootKeyboardPending::EnumerationFailed
        }
    }
}

fn usb_boot_keyboard_pending_from_configure_endpoint_status(
    status: XhciConfigureEndpointStatus,
) -> UsbBootKeyboardPending {
    match status {
        XhciConfigureEndpointStatus::SetConfigurationFailed(status) => {
            usb_boot_keyboard_pending_from_set_configuration_status(status)
        }
        XhciConfigureEndpointStatus::BootKeyboardEndpointUnavailable
        | XhciConfigureEndpointStatus::StartEvidenceUnavailable
        | XhciConfigureEndpointStatus::EndpointIdOutOfRange { .. }
        | XhciConfigureEndpointStatus::CommandTimedOut
        | XhciConfigureEndpointStatus::UnexpectedEventType { .. }
        | XhciConfigureEndpointStatus::CommandPointerMismatch { .. }
        | XhciConfigureEndpointStatus::SlotIdMismatch { .. }
        | XhciConfigureEndpointStatus::CommandFailed { .. }
        | XhciConfigureEndpointStatus::EndpointConfigured { .. } => {
            UsbBootKeyboardPending::EnumerationFailed
        }
    }
}

fn usb_boot_keyboard_pending_from_set_configuration_status(
    status: XhciSetConfigurationStatus,
) -> UsbBootKeyboardPending {
    match status {
        XhciSetConfigurationStatus::ConfigurationDescriptorFailed(status) => {
            usb_boot_keyboard_pending_from_configuration_descriptor_status(status)
        }
        XhciSetConfigurationStatus::BootKeyboardNotReadyToConfigure
        | XhciSetConfigurationStatus::StartEvidenceUnavailable
        | XhciSetConfigurationStatus::TransferTimedOut
        | XhciSetConfigurationStatus::UnexpectedEventType { .. }
        | XhciSetConfigurationStatus::TransferPointerMismatch { .. }
        | XhciSetConfigurationStatus::SlotIdMismatch { .. }
        | XhciSetConfigurationStatus::EndpointIdMismatch { .. }
        | XhciSetConfigurationStatus::TransferFailed { .. }
        | XhciSetConfigurationStatus::ConfigurationSet { .. } => {
            UsbBootKeyboardPending::EnumerationFailed
        }
    }
}

fn usb_boot_keyboard_pending_from_configuration_descriptor_status(
    status: XhciReadConfigurationDescriptorStatus,
) -> UsbBootKeyboardPending {
    match status {
        XhciReadConfigurationDescriptorStatus::HeaderFailed(status) => {
            usb_boot_keyboard_pending_from_configuration_header_status(status)
        }
        XhciReadConfigurationDescriptorStatus::ConfigurationTooLarge { .. }
        | XhciReadConfigurationDescriptorStatus::StartEvidenceUnavailable
        | XhciReadConfigurationDescriptorStatus::TransferTimedOut
        | XhciReadConfigurationDescriptorStatus::UnexpectedEventType { .. }
        | XhciReadConfigurationDescriptorStatus::TransferPointerMismatch { .. }
        | XhciReadConfigurationDescriptorStatus::SlotIdMismatch { .. }
        | XhciReadConfigurationDescriptorStatus::EndpointIdMismatch { .. }
        | XhciReadConfigurationDescriptorStatus::TransferFailed { .. }
        | XhciReadConfigurationDescriptorStatus::InvalidConfigurationDescriptor { .. }
        | XhciReadConfigurationDescriptorStatus::ConfigurationDescriptorReady { .. } => {
            UsbBootKeyboardPending::EnumerationFailed
        }
    }
}

fn usb_boot_keyboard_pending_from_configuration_header_status(
    status: XhciReadConfigurationDescriptorHeaderStatus,
) -> UsbBootKeyboardPending {
    match status {
        XhciReadConfigurationDescriptorHeaderStatus::DeviceDescriptorFailed(status) => {
            usb_boot_keyboard_pending_from_device_descriptor_status(status)
        }
        XhciReadConfigurationDescriptorHeaderStatus::StartEvidenceUnavailable
        | XhciReadConfigurationDescriptorHeaderStatus::TransferTimedOut
        | XhciReadConfigurationDescriptorHeaderStatus::UnexpectedEventType { .. }
        | XhciReadConfigurationDescriptorHeaderStatus::TransferPointerMismatch { .. }
        | XhciReadConfigurationDescriptorHeaderStatus::SlotIdMismatch { .. }
        | XhciReadConfigurationDescriptorHeaderStatus::EndpointIdMismatch { .. }
        | XhciReadConfigurationDescriptorHeaderStatus::TransferFailed { .. }
        | XhciReadConfigurationDescriptorHeaderStatus::InvalidConfigurationDescriptorHeader {
            ..
        }
        | XhciReadConfigurationDescriptorHeaderStatus::ConfigurationDescriptorHeaderReady {
            ..
        } => UsbBootKeyboardPending::EnumerationFailed,
    }
}

fn usb_boot_keyboard_pending_from_device_descriptor_status(
    status: XhciReadDeviceDescriptorStatus,
) -> UsbBootKeyboardPending {
    match status {
        XhciReadDeviceDescriptorStatus::SetAddressFailed(status) => {
            usb_boot_keyboard_pending_from_set_address_status(status)
        }
        XhciReadDeviceDescriptorStatus::StartEvidenceUnavailable
        | XhciReadDeviceDescriptorStatus::TransferTimedOut
        | XhciReadDeviceDescriptorStatus::UnexpectedEventType { .. }
        | XhciReadDeviceDescriptorStatus::TransferPointerMismatch { .. }
        | XhciReadDeviceDescriptorStatus::SlotIdMismatch { .. }
        | XhciReadDeviceDescriptorStatus::EndpointIdMismatch { .. }
        | XhciReadDeviceDescriptorStatus::TransferFailed { .. }
        | XhciReadDeviceDescriptorStatus::InvalidDeviceDescriptor { .. }
        | XhciReadDeviceDescriptorStatus::DeviceDescriptorReady { .. } => {
            UsbBootKeyboardPending::EnumerationFailed
        }
    }
}

fn usb_boot_keyboard_pending_from_set_address_status(
    status: XhciSetAddressStatus,
) -> UsbBootKeyboardPending {
    match status {
        XhciSetAddressStatus::DescriptorPrefixFailed(status) => {
            usb_boot_keyboard_pending_from_descriptor_prefix_status(status)
        }
        XhciSetAddressStatus::StartEvidenceUnavailable
        | XhciSetAddressStatus::InvalidDescriptorPrefix { .. }
        | XhciSetAddressStatus::CommandTimedOut
        | XhciSetAddressStatus::UnexpectedEventType { .. }
        | XhciSetAddressStatus::CommandPointerMismatch { .. }
        | XhciSetAddressStatus::SlotIdMismatch { .. }
        | XhciSetAddressStatus::CommandFailed { .. }
        | XhciSetAddressStatus::Addressed { .. } => UsbBootKeyboardPending::EnumerationFailed,
    }
}

fn usb_boot_keyboard_pending_from_descriptor_prefix_status(
    status: XhciDeviceDescriptorProbeStatus,
) -> UsbBootKeyboardPending {
    match status {
        XhciDeviceDescriptorProbeStatus::AddressDeviceFailed(status) => {
            usb_boot_keyboard_pending_from_address_device_status(status)
        }
        XhciDeviceDescriptorProbeStatus::StartEvidenceUnavailable
        | XhciDeviceDescriptorProbeStatus::TransferTimedOut
        | XhciDeviceDescriptorProbeStatus::UnexpectedEventType { .. }
        | XhciDeviceDescriptorProbeStatus::TransferPointerMismatch { .. }
        | XhciDeviceDescriptorProbeStatus::SlotIdMismatch { .. }
        | XhciDeviceDescriptorProbeStatus::EndpointIdMismatch { .. }
        | XhciDeviceDescriptorProbeStatus::TransferFailed { .. }
        | XhciDeviceDescriptorProbeStatus::DescriptorPrefixReady { .. } => {
            UsbBootKeyboardPending::EnumerationFailed
        }
    }
}

fn usb_boot_keyboard_pending_from_address_device_status(
    status: XhciAddressDeviceStatus,
) -> UsbBootKeyboardPending {
    match status {
        XhciAddressDeviceStatus::EnableSlotFailed(status) => {
            usb_boot_keyboard_pending_from_enable_slot_status(status)
        }
        XhciAddressDeviceStatus::NoConnectedRootPort => UsbBootKeyboardPending::NoConnectedRootPort,
        XhciAddressDeviceStatus::SlotIdOutOfRange { .. }
        | XhciAddressDeviceStatus::StartEvidenceUnavailable
        | XhciAddressDeviceStatus::CommandTimedOut
        | XhciAddressDeviceStatus::UnexpectedEventType { .. }
        | XhciAddressDeviceStatus::CommandPointerMismatch { .. }
        | XhciAddressDeviceStatus::SlotIdMismatch { .. }
        | XhciAddressDeviceStatus::CommandFailed { .. }
        | XhciAddressDeviceStatus::DefaultControlEndpointReady { .. } => {
            UsbBootKeyboardPending::EnumerationFailed
        }
    }
}

fn usb_boot_keyboard_pending_from_enable_slot_status(
    status: XhciEnableSlotStatus,
) -> UsbBootKeyboardPending {
    match status {
        XhciEnableSlotStatus::ControllerStartFailed(status) => {
            usb_boot_keyboard_pending_from_controller_start_status(status)
        }
        XhciEnableSlotStatus::StartEvidenceUnavailable
        | XhciEnableSlotStatus::ControllerNotRunning
        | XhciEnableSlotStatus::CommandTimedOut
        | XhciEnableSlotStatus::UnexpectedEventType { .. }
        | XhciEnableSlotStatus::CommandPointerMismatch { .. }
        | XhciEnableSlotStatus::CommandFailed { .. }
        | XhciEnableSlotStatus::SlotEnabled { .. } => UsbBootKeyboardPending::EnumerationFailed,
    }
}

fn usb_boot_keyboard_pending_from_controller_start_status(
    status: XhciControllerStartStatus,
) -> UsbBootKeyboardPending {
    match status {
        XhciControllerStartStatus::NoPcieXhciController => {
            UsbBootKeyboardPending::NoPcieXhciController
        }
        XhciControllerStartStatus::ControllerBarUnconfigured => {
            UsbBootKeyboardPending::ControllerBarUnconfigured
        }
        XhciControllerStartStatus::InvalidXhciCapabilities => {
            UsbBootKeyboardPending::InvalidXhciCapabilities
        }
        XhciControllerStartStatus::ControllerNotReadyTimedOut
        | XhciControllerStartStatus::PostResetControllerNotReadyTimedOut => {
            UsbBootKeyboardPending::ControllerNotReady
        }
        XhciControllerStartStatus::ResetTimedOut => {
            UsbBootKeyboardPending::ControllerResetInProgress
        }
        XhciControllerStartStatus::HostSystemErrorAfterStart => {
            UsbBootKeyboardPending::HostSystemError
        }
        XhciControllerStartStatus::PciCommandEnableFailed
        | XhciControllerStartStatus::DriverMemoryUnavailable { .. }
        | XhciControllerStartStatus::StopTimedOut
        | XhciControllerStartStatus::StartTimedOut
        | XhciControllerStartStatus::Started => UsbBootKeyboardPending::EnumerationFailed,
    }
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
    XHCI_INPUT_CONTEXT.zero();
    XHCI_OUTPUT_DEVICE_CONTEXT.zero();
    XHCI_CONTROL_ENDPOINT_RING.zero();
    XHCI_INTERRUPT_IN_ENDPOINT_RING.zero();
    XHCI_DEVICE_DESCRIPTOR_PREFIX.zero();
    XHCI_DEVICE_DESCRIPTOR.zero();
    XHCI_CONFIGURATION_DESCRIPTOR_HEADER.zero();
    XHCI_CONFIGURATION_DESCRIPTOR.zero();
    XHCI_BOOT_KEYBOARD_REPORT.zero();

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

fn wait_for_xhci_command_completion_event(
    plan: XhciDriverMemoryPlan,
    event_index: usize,
) -> Option<XhciCommandCompletionEvent> {
    let event_addr = plan.event_ring + (event_index * XHCI_TRB_BYTES) as u64;
    let mut spins = 0usize;
    while spins < XHCI_COMMAND_WAIT_SPINS {
        dma_invalidate_range(event_addr, XHCI_TRB_BYTES);
        let event = decode_xhci_command_completion_event(XHCI_EVENT_RING.read_trb(event_index));
        if event.cycle {
            return Some(event);
        }

        core::hint::spin_loop();
        spins += 1;
    }
    None
}

fn wait_for_xhci_transfer_event(
    plan: XhciDriverMemoryPlan,
    event_index: usize,
) -> Option<XhciTransferEvent> {
    let event_addr = plan.event_ring + (event_index * XHCI_TRB_BYTES) as u64;
    let mut spins = 0usize;
    while spins < XHCI_COMMAND_WAIT_SPINS {
        dma_invalidate_range(event_addr, XHCI_TRB_BYTES);
        let event = decode_xhci_transfer_event(XHCI_EVENT_RING.read_trb(event_index));
        if event.cycle {
            return Some(event);
        }

        core::hint::spin_loop();
        spins += 1;
    }
    None
}

fn poll_xhci_transfer_event(
    plan: XhciDriverMemoryPlan,
    event_index: usize,
    expected_cycle: bool,
) -> Option<XhciTransferEvent> {
    let event_addr = plan.event_ring + (event_index * XHCI_TRB_BYTES) as u64;
    dma_invalidate_range(event_addr, XHCI_TRB_BYTES);
    let event = decode_xhci_transfer_event(XHCI_EVENT_RING.read_trb(event_index));
    if event.cycle == expected_cycle && event.trb_type != 0 {
        return Some(event);
    }
    None
}

fn xhci_event_dequeue_pointer_after(plan: XhciDriverMemoryPlan, event_index: usize) -> u64 {
    let next_index = event_index + 1;
    if next_index >= plan.event_ring_trbs as usize {
        return plan.event_ring;
    }
    plan.event_ring + (next_index * XHCI_TRB_BYTES) as u64
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct XhciSetupPacket {
    bm_request_type: u8,
    b_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
}

fn xhci_setup_stage_trb(setup: XhciSetupPacket) -> [u32; 4] {
    xhci_setup_stage_trb_with_transfer_type(setup, XHCI_SETUP_TRT_IN_DATA_STAGE)
}

fn xhci_setup_stage_no_data_trb(setup: XhciSetupPacket) -> [u32; 4] {
    xhci_setup_stage_trb_with_transfer_type(setup, XHCI_SETUP_TRT_NO_DATA_STAGE)
}

fn xhci_setup_stage_trb_with_transfer_type(setup: XhciSetupPacket, transfer_type: u8) -> [u32; 4] {
    [
        setup.bm_request_type as u32
            | ((setup.b_request as u32) << 8)
            | ((setup.w_value as u32) << 16),
        setup.w_index as u32 | ((setup.w_length as u32) << 16),
        XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES as u32,
        ((transfer_type as u32) << XHCI_SETUP_TRT_SHIFT)
            | ((XHCI_TRB_TYPE_SETUP_STAGE as u32) << XHCI_TRB_TYPE_SHIFT)
            | XHCI_TRB_IDT
            | XHCI_TRB_CYCLE,
    ]
}

fn xhci_data_stage_trb(data_buffer: u64, length: u32, input: bool) -> [u32; 4] {
    let direction = if input { XHCI_TRB_DIR_IN } else { 0 };
    [
        data_buffer as u32,
        (data_buffer >> 32) as u32,
        length & XHCI_TRB_TRANSFER_LENGTH_MASK,
        direction | ((XHCI_TRB_TYPE_DATA_STAGE as u32) << XHCI_TRB_TYPE_SHIFT) | XHCI_TRB_CYCLE,
    ]
}

fn xhci_normal_transfer_trb(data_buffer: u64, length: u32) -> [u32; 4] {
    xhci_normal_transfer_trb_with_cycle(data_buffer, length, true)
}

fn xhci_normal_transfer_trb_with_cycle(data_buffer: u64, length: u32, cycle: bool) -> [u32; 4] {
    let cycle_bit = if cycle { XHCI_TRB_CYCLE } else { 0 };
    [
        data_buffer as u32,
        (data_buffer >> 32) as u32,
        length & XHCI_TRB_TRANSFER_LENGTH_MASK,
        ((XHCI_TRB_TYPE_NORMAL as u32) << XHCI_TRB_TYPE_SHIFT) | XHCI_TRB_IOC | cycle_bit,
    ]
}

fn xhci_status_stage_trb(input: bool) -> [u32; 4] {
    let direction = if input { XHCI_TRB_DIR_IN } else { 0 };
    [
        0,
        0,
        0,
        direction
            | ((XHCI_TRB_TYPE_STATUS_STAGE as u32) << XHCI_TRB_TYPE_SHIFT)
            | XHCI_TRB_IOC
            | XHCI_TRB_CYCLE,
    ]
}

fn xhci_ep0_control_trb_pointers(base_index: usize) -> (u64, u64, u64) {
    (
        XHCI_CONTROL_ENDPOINT_RING.trb_addr(base_index),
        XHCI_CONTROL_ENDPOINT_RING.trb_addr(base_index + 1),
        XHCI_CONTROL_ENDPOINT_RING.trb_addr(base_index + 2),
    )
}

fn xhci_ep0_no_data_control_trb_pointers(base_index: usize) -> (u64, u64) {
    (
        XHCI_CONTROL_ENDPOINT_RING.trb_addr(base_index),
        XHCI_CONTROL_ENDPOINT_RING.trb_addr(base_index + 1),
    )
}

fn write_xhci_ep0_control_transfer(
    plan: XhciDriverMemoryPlan,
    base_index: usize,
    setup_trb: [u32; 4],
    data_trb: [u32; 4],
    status_trb: [u32; 4],
) {
    XHCI_CONTROL_ENDPOINT_RING.set_trb(base_index, setup_trb);
    XHCI_CONTROL_ENDPOINT_RING.set_trb(base_index + 1, data_trb);
    XHCI_CONTROL_ENDPOINT_RING.set_trb(base_index + 2, status_trb);
    XHCI_CONTROL_ENDPOINT_RING
        .set_trb(XHCI_CONTROL_ENDPOINT_RING_TRBS - 1, xhci_link_trb(plan.control_endpoint_ring));
    dma_clean_range(plan.control_endpoint_ring, XHCI_CONTROL_ENDPOINT_RING_TRBS * XHCI_TRB_BYTES);
}

fn write_xhci_ep0_no_data_control_transfer(
    plan: XhciDriverMemoryPlan,
    base_index: usize,
    setup_trb: [u32; 4],
    status_trb: [u32; 4],
) {
    XHCI_CONTROL_ENDPOINT_RING.set_trb(base_index, setup_trb);
    XHCI_CONTROL_ENDPOINT_RING.set_trb(base_index + 1, status_trb);
    XHCI_CONTROL_ENDPOINT_RING
        .set_trb(XHCI_CONTROL_ENDPOINT_RING_TRBS - 1, xhci_link_trb(plan.control_endpoint_ring));
    dma_clean_range(plan.control_endpoint_ring, XHCI_CONTROL_ENDPOINT_RING_TRBS * XHCI_TRB_BYTES);
}

fn write_xhci_interrupt_in_transfer(
    plan: XhciDriverMemoryPlan,
    trb_index: usize,
    normal_trb: [u32; 4],
) {
    write_xhci_interrupt_in_transfer_with_cycle(plan, trb_index, normal_trb, true);
}

fn write_xhci_interrupt_in_transfer_with_cycle(
    plan: XhciDriverMemoryPlan,
    trb_index: usize,
    normal_trb: [u32; 4],
    cycle: bool,
) {
    XHCI_INTERRUPT_IN_ENDPOINT_RING.set_trb(trb_index, normal_trb);
    XHCI_INTERRUPT_IN_ENDPOINT_RING.set_trb(
        XHCI_INTERRUPT_IN_ENDPOINT_RING_TRBS - 1,
        xhci_link_trb_with_cycle(plan.interrupt_in_endpoint_ring, cycle),
    );
    dma_clean_range(
        plan.interrupt_in_endpoint_ring,
        XHCI_INTERRUPT_IN_ENDPOINT_RING_TRBS * XHCI_TRB_BYTES,
    );
}

fn xhci_address_device_command_trb(
    input_context: u64,
    slot_id: u8,
    block_set_address_request: bool,
) -> [u32; 4] {
    let pointer = input_context & !0xf;
    let bsr = if block_set_address_request {
        XHCI_ADDRESS_DEVICE_BLOCK_SET_ADDRESS_REQUEST_BIT
    } else {
        0
    };
    [
        pointer as u32,
        (pointer >> 32) as u32,
        0,
        ((slot_id as u32) << 24)
            | ((XHCI_TRB_TYPE_ADDRESS_DEVICE as u32) << XHCI_TRB_TYPE_SHIFT)
            | bsr
            | XHCI_TRB_CYCLE,
    ]
}

fn xhci_configure_endpoint_command_trb(input_context: u64, slot_id: u8) -> [u32; 4] {
    let pointer = input_context & !0xf;
    [
        pointer as u32,
        (pointer >> 32) as u32,
        0,
        ((slot_id as u32) << 24)
            | ((XHCI_TRB_TYPE_CONFIGURE_ENDPOINT as u32) << XHCI_TRB_TYPE_SHIFT)
            | XHCI_TRB_CYCLE,
    ]
}

fn xhci_link_trb(ring_addr: u64) -> [u32; 4] {
    xhci_link_trb_with_cycle(ring_addr, true)
}

fn xhci_link_trb_with_cycle(ring_addr: u64, cycle: bool) -> [u32; 4] {
    let cycle_bit = if cycle { XHCI_TRB_CYCLE } else { 0 };
    [
        ring_addr as u32,
        (ring_addr >> 32) as u32,
        0,
        ((XHCI_TRB_TYPE_LINK as u32) << XHCI_TRB_TYPE_SHIFT)
            | XHCI_LINK_TRB_TOGGLE_CYCLE
            | cycle_bit,
    ]
}

fn xhci_address_device_contexts(
    plan: XhciDriverMemoryPlan,
    port: XhciPortSnapshot,
) -> XhciAddressDeviceContexts {
    let endpoint0_max_packet_size = xhci_default_control_max_packet_size(port.speed);
    xhci_address_device_contexts_with_max_packet_size(plan, port, endpoint0_max_packet_size)
}

fn xhci_address_device_contexts_with_max_packet_size(
    plan: XhciDriverMemoryPlan,
    port: XhciPortSnapshot,
    endpoint0_max_packet_size: u16,
) -> XhciAddressDeviceContexts {
    let endpoint0_dequeue_pointer = plan.control_endpoint_ring | XHCI_EP_CONTEXT_DCS;
    XhciAddressDeviceContexts {
        input_context: plan.input_context,
        output_device_context: plan.output_device_context,
        control_endpoint_ring: plan.control_endpoint_ring,
        drop_context_flags: 0,
        add_context_flags: XHCI_INPUT_ADD_SLOT_CONTEXT | XHCI_INPUT_ADD_EP0_CONTEXT,
        slot_context: [
            ((port.speed as u32) << XHCI_SLOT_CONTEXT_SPEED_SHIFT)
                | (1 << XHCI_SLOT_CONTEXT_CONTEXT_ENTRIES_SHIFT),
            (port.port as u32) << XHCI_SLOT_CONTEXT_ROOT_HUB_PORT_SHIFT,
            0,
            0,
        ],
        endpoint0_context: [
            0,
            ((endpoint0_max_packet_size as u32) << XHCI_EP_CONTEXT_MAX_PACKET_SIZE_SHIFT)
                | ((XHCI_EP_CONTEXT_TYPE_CONTROL as u32) << XHCI_EP_CONTEXT_TYPE_SHIFT)
                | ((XHCI_EP_CONTEXT_CERR_DEFAULT as u32) << XHCI_EP_CONTEXT_CERR_SHIFT),
            endpoint0_dequeue_pointer as u32,
            (endpoint0_dequeue_pointer >> 32) as u32,
            XHCI_CONTROL_AVERAGE_TRB_LENGTH as u32,
        ],
        endpoint0_max_packet_size,
    }
}

fn xhci_default_control_max_packet_size(port_speed: u8) -> u16 {
    match port_speed {
        3 => 64,
        4..=15 => 512,
        _ => 8,
    }
}

fn write_xhci_address_device_contexts(
    plan: XhciDriverMemoryPlan,
    slot_id: u8,
    contexts: XhciAddressDeviceContexts,
    clear_output_device_context: bool,
) {
    XHCI_INPUT_CONTEXT.zero();
    if clear_output_device_context {
        XHCI_OUTPUT_DEVICE_CONTEXT.zero();
    }
    XHCI_CONTROL_ENDPOINT_RING.zero();
    XHCI_CONTROL_ENDPOINT_RING
        .set_trb(XHCI_CONTROL_ENDPOINT_RING_TRBS - 1, xhci_link_trb(plan.control_endpoint_ring));
    XHCI_DCBAA.set(slot_id as usize, plan.output_device_context);

    XHCI_INPUT_CONTEXT.set_u32(
        xhci_context_dword_offset(plan.context_size_bytes, 0, XHCI_INPUT_CONTEXT_DROP_FLAGS_DWORD),
        contexts.drop_context_flags,
    );
    XHCI_INPUT_CONTEXT.set_u32(
        xhci_context_dword_offset(plan.context_size_bytes, 0, XHCI_INPUT_CONTEXT_ADD_FLAGS_DWORD),
        contexts.add_context_flags,
    );

    let mut index = 0usize;
    while index < contexts.slot_context.len() {
        XHCI_INPUT_CONTEXT.set_u32(
            xhci_context_dword_offset(
                plan.context_size_bytes,
                XHCI_INPUT_CONTEXT_SLOT_INDEX,
                index,
            ),
            contexts.slot_context[index],
        );
        index += 1;
    }

    index = 0;
    while index < contexts.endpoint0_context.len() {
        XHCI_INPUT_CONTEXT.set_u32(
            xhci_context_dword_offset(plan.context_size_bytes, XHCI_INPUT_CONTEXT_EP0_INDEX, index),
            contexts.endpoint0_context[index],
        );
        index += 1;
    }

    dma_clean_range(plan.dcbaa, XHCI_MAX_DEVICE_CONTEXT_POINTERS * core::mem::size_of::<u64>());
    dma_clean_range(plan.input_context, XHCI_CONTEXT_PAGE_BYTES);
    dma_clean_range(plan.output_device_context, XHCI_CONTEXT_PAGE_BYTES);
    dma_clean_range(plan.control_endpoint_ring, XHCI_CONTROL_ENDPOINT_RING_TRBS * XHCI_TRB_BYTES);
}

fn xhci_configure_keyboard_endpoint_contexts(
    plan: XhciDriverMemoryPlan,
    port: XhciPortSnapshot,
    endpoint: UsbEndpointDescriptor,
    endpoint_id: u8,
) -> XhciConfigureEndpointContexts {
    let endpoint_dequeue_pointer = plan.interrupt_in_endpoint_ring | XHCI_EP_CONTEXT_DCS;
    let interval_encoded = xhci_interrupt_endpoint_interval(port.speed, endpoint.interval);
    let max_packet_size = xhci_endpoint_max_packet_size(endpoint);
    let max_esit_payload = max_packet_size;
    XhciConfigureEndpointContexts {
        input_context: plan.input_context,
        output_device_context: plan.output_device_context,
        interrupt_in_endpoint_ring: plan.interrupt_in_endpoint_ring,
        drop_context_flags: 0,
        add_context_flags: XHCI_INPUT_ADD_SLOT_CONTEXT | (1u32 << endpoint_id),
        endpoint_id,
        endpoint_context_index: xhci_input_context_index_from_dci(endpoint_id),
        endpoint_address: endpoint.address,
        endpoint_number: endpoint.endpoint_number,
        interval: endpoint.interval,
        interval_encoded,
        max_packet_size,
        max_esit_payload,
        slot_context: [
            ((port.speed as u32) << XHCI_SLOT_CONTEXT_SPEED_SHIFT)
                | ((endpoint_id as u32) << XHCI_SLOT_CONTEXT_CONTEXT_ENTRIES_SHIFT),
            (port.port as u32) << XHCI_SLOT_CONTEXT_ROOT_HUB_PORT_SHIFT,
            0,
            0,
        ],
        endpoint_context: [
            (interval_encoded as u32) << XHCI_EP_CONTEXT_INTERVAL_SHIFT,
            ((max_packet_size as u32) << XHCI_EP_CONTEXT_MAX_PACKET_SIZE_SHIFT)
                | ((XHCI_EP_CONTEXT_TYPE_INTERRUPT_IN as u32) << XHCI_EP_CONTEXT_TYPE_SHIFT)
                | ((XHCI_EP_CONTEXT_CERR_DEFAULT as u32) << XHCI_EP_CONTEXT_CERR_SHIFT),
            endpoint_dequeue_pointer as u32,
            (endpoint_dequeue_pointer >> 32) as u32,
            (XHCI_INTERRUPT_AVERAGE_TRB_LENGTH as u32)
                | ((max_esit_payload as u32) << XHCI_EP_CONTEXT_MAX_ESIT_PAYLOAD_SHIFT),
        ],
    }
}

fn write_xhci_configure_endpoint_contexts(
    plan: XhciDriverMemoryPlan,
    slot_id: u8,
    contexts: XhciConfigureEndpointContexts,
) {
    XHCI_INPUT_CONTEXT.zero();
    XHCI_INTERRUPT_IN_ENDPOINT_RING.zero();
    XHCI_INTERRUPT_IN_ENDPOINT_RING.set_trb(
        XHCI_INTERRUPT_IN_ENDPOINT_RING_TRBS - 1,
        xhci_link_trb(plan.interrupt_in_endpoint_ring),
    );
    XHCI_DCBAA.set(slot_id as usize, plan.output_device_context);

    XHCI_INPUT_CONTEXT.set_u32(
        xhci_context_dword_offset(plan.context_size_bytes, 0, XHCI_INPUT_CONTEXT_DROP_FLAGS_DWORD),
        contexts.drop_context_flags,
    );
    XHCI_INPUT_CONTEXT.set_u32(
        xhci_context_dword_offset(plan.context_size_bytes, 0, XHCI_INPUT_CONTEXT_ADD_FLAGS_DWORD),
        contexts.add_context_flags,
    );

    let mut index = 0usize;
    while index < contexts.slot_context.len() {
        XHCI_INPUT_CONTEXT.set_u32(
            xhci_context_dword_offset(
                plan.context_size_bytes,
                XHCI_INPUT_CONTEXT_SLOT_INDEX,
                index,
            ),
            contexts.slot_context[index],
        );
        index += 1;
    }

    index = 0;
    while index < contexts.endpoint_context.len() {
        XHCI_INPUT_CONTEXT.set_u32(
            xhci_context_dword_offset(
                plan.context_size_bytes,
                contexts.endpoint_context_index as usize,
                index,
            ),
            contexts.endpoint_context[index],
        );
        index += 1;
    }

    dma_clean_range(plan.dcbaa, XHCI_MAX_DEVICE_CONTEXT_POINTERS * core::mem::size_of::<u64>());
    dma_clean_range(plan.input_context, XHCI_CONTEXT_PAGE_BYTES);
    dma_clean_range(
        plan.interrupt_in_endpoint_ring,
        XHCI_INTERRUPT_IN_ENDPOINT_RING_TRBS * XHCI_TRB_BYTES,
    );
}

fn xhci_endpoint_id(endpoint: UsbEndpointDescriptor) -> u8 {
    if endpoint.endpoint_number == 0 {
        return 0;
    }
    let direction = if endpoint.direction_in { 1 } else { 0 };
    endpoint
        .endpoint_number
        .saturating_mul(2)
        .saturating_add(direction)
}

const fn xhci_input_context_index_from_dci(dci: u8) -> u8 {
    dci + 1
}

fn xhci_endpoint_max_packet_size(endpoint: UsbEndpointDescriptor) -> u16 {
    endpoint.max_packet_size & 0x07ff
}

fn xhci_interrupt_endpoint_interval(port_speed: u8, interval: u8) -> u8 {
    if interval == 0 {
        return 0;
    }
    match port_speed {
        1 | 2 => ceil_log2_u16((interval as u16).saturating_mul(8)).min(15),
        3..=15 => interval.saturating_sub(1).min(15),
        _ => interval.saturating_sub(1).min(15),
    }
}

fn ceil_log2_u16(value: u16) -> u8 {
    if value <= 1 {
        return 0;
    }
    let mut power = 1u16;
    let mut log = 0u8;
    while power < value {
        power = power.saturating_mul(2);
        log += 1;
    }
    log
}

const fn xhci_context_dword_offset(
    context_size_bytes: u8,
    context_index: usize,
    dword: usize,
) -> usize {
    (context_index * context_size_bytes as usize) + (dword * core::mem::size_of::<u32>())
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

fn decode_xhci_transfer_event(raw: [u32; 4]) -> XhciTransferEvent {
    XhciTransferEvent {
        raw,
        trb_pointer: (((raw[1] as u64) << 32) | raw[0] as u64) & !0xf,
        transfer_length: raw[2] & XHCI_TRANSFER_EVENT_LENGTH_MASK,
        completion_code: (raw[2] >> XHCI_TRB_COMPLETION_CODE_SHIFT) as u8,
        trb_type: ((raw[3] >> XHCI_TRB_TYPE_SHIFT) & XHCI_TRB_TYPE_MASK) as u8,
        cycle: raw[3] & XHCI_TRB_CYCLE != 0,
        event_data: raw[3] & (1 << 2) != 0,
        endpoint_id: ((raw[3] >> XHCI_TRANSFER_EVENT_ENDPOINT_ID_SHIFT) & 0x1f) as u8,
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

fn classify_address_device_event(
    expected_command_trb_pointer: u64,
    expected_slot_id: u8,
    event: XhciCommandCompletionEvent,
) -> XhciAddressDeviceStatus {
    if event.trb_type != XHCI_TRB_TYPE_COMMAND_COMPLETION_EVENT {
        return XhciAddressDeviceStatus::UnexpectedEventType {
            trb_type: event.trb_type,
            completion_code: event.completion_code,
        };
    }
    if event.command_trb_pointer != expected_command_trb_pointer {
        return XhciAddressDeviceStatus::CommandPointerMismatch {
            expected: expected_command_trb_pointer,
            actual: event.command_trb_pointer,
            completion_code: event.completion_code,
            slot_id: event.slot_id,
        };
    }
    if event.slot_id != expected_slot_id {
        return XhciAddressDeviceStatus::SlotIdMismatch {
            expected: expected_slot_id,
            actual: event.slot_id,
            completion_code: event.completion_code,
        };
    }
    if event.completion_code != XHCI_TRB_COMPLETION_SUCCESS {
        return XhciAddressDeviceStatus::CommandFailed {
            completion_code: event.completion_code,
            slot_id: event.slot_id,
        };
    }
    XhciAddressDeviceStatus::DefaultControlEndpointReady {
        slot_id: event.slot_id,
    }
}

fn classify_set_address_event(
    expected_command_trb_pointer: u64,
    expected_slot_id: u8,
    endpoint0_max_packet_size: u16,
    event: XhciCommandCompletionEvent,
) -> XhciSetAddressStatus {
    if event.trb_type != XHCI_TRB_TYPE_COMMAND_COMPLETION_EVENT {
        return XhciSetAddressStatus::UnexpectedEventType {
            trb_type: event.trb_type,
            completion_code: event.completion_code,
        };
    }
    if event.command_trb_pointer != expected_command_trb_pointer {
        return XhciSetAddressStatus::CommandPointerMismatch {
            expected: expected_command_trb_pointer,
            actual: event.command_trb_pointer,
            completion_code: event.completion_code,
            slot_id: event.slot_id,
        };
    }
    if event.slot_id != expected_slot_id {
        return XhciSetAddressStatus::SlotIdMismatch {
            expected: expected_slot_id,
            actual: event.slot_id,
            completion_code: event.completion_code,
        };
    }
    if event.completion_code != XHCI_TRB_COMPLETION_SUCCESS {
        return XhciSetAddressStatus::CommandFailed {
            completion_code: event.completion_code,
            slot_id: event.slot_id,
        };
    }
    XhciSetAddressStatus::Addressed {
        slot_id: event.slot_id,
        endpoint0_max_packet_size,
    }
}

fn classify_device_descriptor_transfer_event(
    expected_status_trb_pointer: u64,
    expected_slot_id: u8,
    expected_endpoint_id: u8,
    event: XhciTransferEvent,
) -> XhciDeviceDescriptorProbeStatus {
    if event.trb_type != XHCI_TRB_TYPE_TRANSFER_EVENT {
        return XhciDeviceDescriptorProbeStatus::UnexpectedEventType {
            trb_type: event.trb_type,
            completion_code: event.completion_code,
        };
    }
    if event.trb_pointer != expected_status_trb_pointer {
        return XhciDeviceDescriptorProbeStatus::TransferPointerMismatch {
            expected: expected_status_trb_pointer,
            actual: event.trb_pointer,
            completion_code: event.completion_code,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    if event.slot_id != expected_slot_id {
        return XhciDeviceDescriptorProbeStatus::SlotIdMismatch {
            expected: expected_slot_id,
            actual: event.slot_id,
            completion_code: event.completion_code,
        };
    }
    if event.endpoint_id != expected_endpoint_id {
        return XhciDeviceDescriptorProbeStatus::EndpointIdMismatch {
            expected: expected_endpoint_id,
            actual: event.endpoint_id,
            completion_code: event.completion_code,
        };
    }
    if event.completion_code != XHCI_TRB_COMPLETION_SUCCESS {
        return XhciDeviceDescriptorProbeStatus::TransferFailed {
            completion_code: event.completion_code,
            residual_length: event.transfer_length,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    XhciDeviceDescriptorProbeStatus::DescriptorPrefixReady {
        slot_id: event.slot_id,
        length: XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES as u8,
    }
}

fn classify_read_device_descriptor_transfer_event(
    expected_status_trb_pointer: u64,
    expected_slot_id: u8,
    expected_endpoint_id: u8,
    descriptor: [u8; USB_DEVICE_DESCRIPTOR_BYTES],
    event: XhciTransferEvent,
) -> XhciReadDeviceDescriptorStatus {
    if event.trb_type != XHCI_TRB_TYPE_TRANSFER_EVENT {
        return XhciReadDeviceDescriptorStatus::UnexpectedEventType {
            trb_type: event.trb_type,
            completion_code: event.completion_code,
        };
    }
    if event.trb_pointer != expected_status_trb_pointer {
        return XhciReadDeviceDescriptorStatus::TransferPointerMismatch {
            expected: expected_status_trb_pointer,
            actual: event.trb_pointer,
            completion_code: event.completion_code,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    if event.slot_id != expected_slot_id {
        return XhciReadDeviceDescriptorStatus::SlotIdMismatch {
            expected: expected_slot_id,
            actual: event.slot_id,
            completion_code: event.completion_code,
        };
    }
    if event.endpoint_id != expected_endpoint_id {
        return XhciReadDeviceDescriptorStatus::EndpointIdMismatch {
            expected: expected_endpoint_id,
            actual: event.endpoint_id,
            completion_code: event.completion_code,
        };
    }
    if event.completion_code != XHCI_TRB_COMPLETION_SUCCESS {
        return XhciReadDeviceDescriptorStatus::TransferFailed {
            completion_code: event.completion_code,
            residual_length: event.transfer_length,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    let Some(fields) = parse_usb_device_descriptor(descriptor) else {
        return XhciReadDeviceDescriptorStatus::InvalidDeviceDescriptor {
            length: descriptor[0],
            descriptor_type: descriptor[1],
        };
    };
    XhciReadDeviceDescriptorStatus::DeviceDescriptorReady {
        slot_id: event.slot_id,
        vendor_id: fields.vendor_id,
        product_id: fields.product_id,
        num_configurations: fields.num_configurations,
    }
}

fn parse_usb_device_descriptor(
    descriptor: [u8; USB_DEVICE_DESCRIPTOR_BYTES],
) -> Option<UsbDeviceDescriptor> {
    if descriptor[0] != USB_DEVICE_DESCRIPTOR_BYTES as u8
        || descriptor[1] != USB_DESCRIPTOR_TYPE_DEVICE
    {
        return None;
    }
    Some(UsbDeviceDescriptor {
        length: descriptor[0],
        descriptor_type: descriptor[1],
        bcd_usb: u16::from_le_bytes([descriptor[2], descriptor[3]]),
        device_class: descriptor[4],
        device_subclass: descriptor[5],
        device_protocol: descriptor[6],
        max_packet_size0: descriptor[7],
        vendor_id: u16::from_le_bytes([descriptor[8], descriptor[9]]),
        product_id: u16::from_le_bytes([descriptor[10], descriptor[11]]),
        bcd_device: u16::from_le_bytes([descriptor[12], descriptor[13]]),
        manufacturer_index: descriptor[14],
        product_index: descriptor[15],
        serial_number_index: descriptor[16],
        num_configurations: descriptor[17],
    })
}

fn classify_read_configuration_descriptor_header_transfer_event(
    expected_status_trb_pointer: u64,
    expected_slot_id: u8,
    expected_endpoint_id: u8,
    descriptor: [u8; USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES],
    event: XhciTransferEvent,
) -> XhciReadConfigurationDescriptorHeaderStatus {
    if event.trb_type != XHCI_TRB_TYPE_TRANSFER_EVENT {
        return XhciReadConfigurationDescriptorHeaderStatus::UnexpectedEventType {
            trb_type: event.trb_type,
            completion_code: event.completion_code,
        };
    }
    if event.trb_pointer != expected_status_trb_pointer {
        return XhciReadConfigurationDescriptorHeaderStatus::TransferPointerMismatch {
            expected: expected_status_trb_pointer,
            actual: event.trb_pointer,
            completion_code: event.completion_code,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    if event.slot_id != expected_slot_id {
        return XhciReadConfigurationDescriptorHeaderStatus::SlotIdMismatch {
            expected: expected_slot_id,
            actual: event.slot_id,
            completion_code: event.completion_code,
        };
    }
    if event.endpoint_id != expected_endpoint_id {
        return XhciReadConfigurationDescriptorHeaderStatus::EndpointIdMismatch {
            expected: expected_endpoint_id,
            actual: event.endpoint_id,
            completion_code: event.completion_code,
        };
    }
    if event.completion_code != XHCI_TRB_COMPLETION_SUCCESS {
        return XhciReadConfigurationDescriptorHeaderStatus::TransferFailed {
            completion_code: event.completion_code,
            residual_length: event.transfer_length,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    let Some(fields) = parse_usb_configuration_descriptor_header(descriptor) else {
        return XhciReadConfigurationDescriptorHeaderStatus::InvalidConfigurationDescriptorHeader {
            length: descriptor[0],
            descriptor_type: descriptor[1],
            total_length: u16::from_le_bytes([descriptor[2], descriptor[3]]),
        };
    };
    XhciReadConfigurationDescriptorHeaderStatus::ConfigurationDescriptorHeaderReady {
        slot_id: event.slot_id,
        total_length: fields.total_length,
        num_interfaces: fields.num_interfaces,
        configuration_value: fields.configuration_value,
    }
}

fn parse_usb_configuration_descriptor_header(
    descriptor: [u8; USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES],
) -> Option<UsbConfigurationDescriptorHeader> {
    let total_length = u16::from_le_bytes([descriptor[2], descriptor[3]]);
    if descriptor[0] != USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES as u8
        || descriptor[1] != USB_DESCRIPTOR_TYPE_CONFIGURATION
        || total_length < USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES as u16
    {
        return None;
    }
    Some(UsbConfigurationDescriptorHeader {
        length: descriptor[0],
        descriptor_type: descriptor[1],
        total_length,
        num_interfaces: descriptor[4],
        configuration_value: descriptor[5],
        configuration_index: descriptor[6],
        attributes: descriptor[7],
        max_power: descriptor[8],
    })
}

fn classify_read_configuration_descriptor_transfer_event(
    expected_status_trb_pointer: u64,
    expected_slot_id: u8,
    expected_endpoint_id: u8,
    descriptor: [u8; USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES],
    total_length: u16,
    event: XhciTransferEvent,
) -> XhciReadConfigurationDescriptorStatus {
    if event.trb_type != XHCI_TRB_TYPE_TRANSFER_EVENT {
        return XhciReadConfigurationDescriptorStatus::UnexpectedEventType {
            trb_type: event.trb_type,
            completion_code: event.completion_code,
        };
    }
    if event.trb_pointer != expected_status_trb_pointer {
        return XhciReadConfigurationDescriptorStatus::TransferPointerMismatch {
            expected: expected_status_trb_pointer,
            actual: event.trb_pointer,
            completion_code: event.completion_code,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    if event.slot_id != expected_slot_id {
        return XhciReadConfigurationDescriptorStatus::SlotIdMismatch {
            expected: expected_slot_id,
            actual: event.slot_id,
            completion_code: event.completion_code,
        };
    }
    if event.endpoint_id != expected_endpoint_id {
        return XhciReadConfigurationDescriptorStatus::EndpointIdMismatch {
            expected: expected_endpoint_id,
            actual: event.endpoint_id,
            completion_code: event.completion_code,
        };
    }
    if event.completion_code != XHCI_TRB_COMPLETION_SUCCESS {
        return XhciReadConfigurationDescriptorStatus::TransferFailed {
            completion_code: event.completion_code,
            residual_length: event.transfer_length,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    let Some(fields) = parse_usb_configuration_descriptor_tree(descriptor, total_length) else {
        return XhciReadConfigurationDescriptorStatus::InvalidConfigurationDescriptor {
            offset: usb_configuration_descriptor_invalid_offset(descriptor, total_length),
            length: usb_configuration_descriptor_invalid_length(descriptor, total_length),
            descriptor_type: usb_configuration_descriptor_invalid_type(descriptor, total_length),
        };
    };
    XhciReadConfigurationDescriptorStatus::ConfigurationDescriptorReady {
        slot_id: event.slot_id,
        total_length: fields.header.total_length,
        num_interfaces: fields.header.num_interfaces,
        boot_keyboard_ready_to_configure: fields
            .boot_keyboard
            .and_then(|keyboard| keyboard.interrupt_in_endpoint)
            .is_some(),
    }
}

fn classify_set_configuration_transfer_event(
    expected_status_trb_pointer: u64,
    expected_slot_id: u8,
    expected_endpoint_id: u8,
    configuration_value: u8,
    interface_number: u8,
    endpoint_address: u8,
    event: XhciTransferEvent,
) -> XhciSetConfigurationStatus {
    if event.trb_type != XHCI_TRB_TYPE_TRANSFER_EVENT {
        return XhciSetConfigurationStatus::UnexpectedEventType {
            trb_type: event.trb_type,
            completion_code: event.completion_code,
        };
    }
    if event.trb_pointer != expected_status_trb_pointer {
        return XhciSetConfigurationStatus::TransferPointerMismatch {
            expected: expected_status_trb_pointer,
            actual: event.trb_pointer,
            completion_code: event.completion_code,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    if event.slot_id != expected_slot_id {
        return XhciSetConfigurationStatus::SlotIdMismatch {
            expected: expected_slot_id,
            actual: event.slot_id,
            completion_code: event.completion_code,
        };
    }
    if event.endpoint_id != expected_endpoint_id {
        return XhciSetConfigurationStatus::EndpointIdMismatch {
            expected: expected_endpoint_id,
            actual: event.endpoint_id,
            completion_code: event.completion_code,
        };
    }
    if event.completion_code != XHCI_TRB_COMPLETION_SUCCESS {
        return XhciSetConfigurationStatus::TransferFailed {
            completion_code: event.completion_code,
            residual_length: event.transfer_length,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    XhciSetConfigurationStatus::ConfigurationSet {
        slot_id: event.slot_id,
        configuration_value,
        interface_number,
        endpoint_address,
    }
}

fn classify_configure_endpoint_event(
    expected_command_trb_pointer: u64,
    expected_slot_id: u8,
    endpoint_id: u8,
    endpoint_address: u8,
    max_packet_size: u16,
    interval_encoded: u8,
    event: XhciCommandCompletionEvent,
) -> XhciConfigureEndpointStatus {
    if event.trb_type != XHCI_TRB_TYPE_COMMAND_COMPLETION_EVENT {
        return XhciConfigureEndpointStatus::UnexpectedEventType {
            trb_type: event.trb_type,
            completion_code: event.completion_code,
        };
    }
    if event.command_trb_pointer != expected_command_trb_pointer {
        return XhciConfigureEndpointStatus::CommandPointerMismatch {
            expected: expected_command_trb_pointer,
            actual: event.command_trb_pointer,
            completion_code: event.completion_code,
            slot_id: event.slot_id,
        };
    }
    if event.slot_id != expected_slot_id {
        return XhciConfigureEndpointStatus::SlotIdMismatch {
            expected: expected_slot_id,
            actual: event.slot_id,
            completion_code: event.completion_code,
        };
    }
    if event.completion_code != XHCI_TRB_COMPLETION_SUCCESS {
        return XhciConfigureEndpointStatus::CommandFailed {
            completion_code: event.completion_code,
            slot_id: event.slot_id,
        };
    }
    XhciConfigureEndpointStatus::EndpointConfigured {
        slot_id: event.slot_id,
        endpoint_id,
        endpoint_address,
        max_packet_size,
        interval_encoded,
    }
}

fn classify_set_hid_protocol_transfer_event(
    expected_status_trb_pointer: u64,
    expected_slot_id: u8,
    expected_endpoint_id: u8,
    interface_number: u8,
    protocol: u8,
    event: XhciTransferEvent,
) -> XhciSetHidProtocolStatus {
    if event.trb_type != XHCI_TRB_TYPE_TRANSFER_EVENT {
        return XhciSetHidProtocolStatus::UnexpectedEventType {
            trb_type: event.trb_type,
            completion_code: event.completion_code,
        };
    }
    if event.trb_pointer != expected_status_trb_pointer {
        return XhciSetHidProtocolStatus::TransferPointerMismatch {
            expected: expected_status_trb_pointer,
            actual: event.trb_pointer,
            completion_code: event.completion_code,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    if event.slot_id != expected_slot_id {
        return XhciSetHidProtocolStatus::SlotIdMismatch {
            expected: expected_slot_id,
            actual: event.slot_id,
            completion_code: event.completion_code,
        };
    }
    if event.endpoint_id != expected_endpoint_id {
        return XhciSetHidProtocolStatus::EndpointIdMismatch {
            expected: expected_endpoint_id,
            actual: event.endpoint_id,
            completion_code: event.completion_code,
        };
    }
    if event.completion_code != XHCI_TRB_COMPLETION_SUCCESS {
        return XhciSetHidProtocolStatus::TransferFailed {
            completion_code: event.completion_code,
            residual_length: event.transfer_length,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    XhciSetHidProtocolStatus::BootProtocolSet {
        slot_id: event.slot_id,
        interface_number,
        protocol,
    }
}

fn classify_boot_keyboard_report_transfer_event(
    expected_normal_trb_pointer: u64,
    expected_slot_id: u8,
    expected_endpoint_id: u8,
    event: XhciTransferEvent,
) -> XhciReadBootKeyboardReportStatus {
    if event.trb_type != XHCI_TRB_TYPE_TRANSFER_EVENT {
        return XhciReadBootKeyboardReportStatus::UnexpectedEventType {
            trb_type: event.trb_type,
            completion_code: event.completion_code,
        };
    }
    if event.trb_pointer != expected_normal_trb_pointer {
        return XhciReadBootKeyboardReportStatus::TransferPointerMismatch {
            expected: expected_normal_trb_pointer,
            actual: event.trb_pointer,
            completion_code: event.completion_code,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    if event.slot_id != expected_slot_id {
        return XhciReadBootKeyboardReportStatus::SlotIdMismatch {
            expected: expected_slot_id,
            actual: event.slot_id,
            completion_code: event.completion_code,
        };
    }
    if event.endpoint_id != expected_endpoint_id {
        return XhciReadBootKeyboardReportStatus::EndpointIdMismatch {
            expected: expected_endpoint_id,
            actual: event.endpoint_id,
            completion_code: event.completion_code,
        };
    }
    if event.completion_code != XHCI_TRB_COMPLETION_SUCCESS {
        return XhciReadBootKeyboardReportStatus::TransferFailed {
            completion_code: event.completion_code,
            residual_length: event.transfer_length,
            slot_id: event.slot_id,
            endpoint_id: event.endpoint_id,
        };
    }
    XhciReadBootKeyboardReportStatus::ReportReady {
        slot_id: event.slot_id,
        endpoint_id: event.endpoint_id,
        length: BOOT_KEYBOARD_REPORT_BYTES as u8,
    }
}

fn parse_usb_configuration_descriptor_tree(
    descriptor: [u8; USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES],
    total_length: u16,
) -> Option<UsbConfigurationDescriptorTree> {
    let total = total_length as usize;
    if total < USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES
        || total > USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES
    {
        return None;
    }

    let header = parse_usb_configuration_descriptor_header([
        descriptor[0],
        descriptor[1],
        descriptor[2],
        descriptor[3],
        descriptor[4],
        descriptor[5],
        descriptor[6],
        descriptor[7],
        descriptor[8],
    ])?;
    if header.total_length != total_length {
        return None;
    }

    let mut offset = 0usize;
    let mut descriptor_count = 0u8;
    let mut boot_keyboard: Option<UsbHidBootKeyboardInterface> = None;
    let mut active_boot_interface = false;

    while offset < total {
        if offset + 2 > total {
            return None;
        }
        let length = descriptor[offset] as usize;
        let descriptor_type = descriptor[offset + 1];
        if length < 2 || offset + length > total {
            return None;
        }

        descriptor_count = descriptor_count.saturating_add(1);
        if descriptor_type == USB_DESCRIPTOR_TYPE_INTERFACE && length >= 9 {
            let candidate = descriptor[offset + 5] == USB_CLASS_HID
                && descriptor[offset + 6] == USB_HID_SUBCLASS_BOOT
                && descriptor[offset + 7] == USB_HID_PROTOCOL_KEYBOARD;
            active_boot_interface = candidate;
            if candidate && boot_keyboard.is_none() {
                boot_keyboard = Some(UsbHidBootKeyboardInterface {
                    interface_number: descriptor[offset + 2],
                    alternate_setting: descriptor[offset + 3],
                    endpoint_count: descriptor[offset + 4],
                    protocol: descriptor[offset + 7],
                    interrupt_in_endpoint: None,
                });
            }
        } else if descriptor_type == USB_DESCRIPTOR_TYPE_ENDPOINT && length >= 7 {
            if active_boot_interface {
                let endpoint = UsbEndpointDescriptor {
                    address: descriptor[offset + 2],
                    endpoint_number: descriptor[offset + 2] & 0x0f,
                    direction_in: descriptor[offset + 2] & USB_ENDPOINT_DIRECTION_IN != 0,
                    attributes: descriptor[offset + 3],
                    transfer_type: descriptor[offset + 3] & 0x03,
                    max_packet_size: u16::from_le_bytes([
                        descriptor[offset + 4],
                        descriptor[offset + 5],
                    ]),
                    interval: descriptor[offset + 6],
                };
                if endpoint.direction_in
                    && endpoint.transfer_type == USB_ENDPOINT_TRANSFER_TYPE_INTERRUPT
                {
                    if let Some(mut keyboard) = boot_keyboard {
                        if keyboard.interrupt_in_endpoint.is_none() {
                            keyboard.interrupt_in_endpoint = Some(endpoint);
                            boot_keyboard = Some(keyboard);
                        }
                    }
                }
            }
        }

        offset += length;
    }

    if offset != total {
        return None;
    }

    Some(UsbConfigurationDescriptorTree {
        header,
        descriptor_count,
        boot_keyboard,
    })
}

fn usb_configuration_descriptor_invalid_offset(
    descriptor: [u8; USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES],
    total_length: u16,
) -> u16 {
    let total = (total_length as usize).min(USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES);
    let mut offset = 0usize;
    while offset < total {
        if offset + 2 > total {
            return offset as u16;
        }
        let length = descriptor[offset] as usize;
        if length < 2 || offset + length > total {
            return offset as u16;
        }
        offset += length;
    }
    0
}

fn usb_configuration_descriptor_invalid_length(
    descriptor: [u8; USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES],
    total_length: u16,
) -> u8 {
    let offset = usb_configuration_descriptor_invalid_offset(descriptor, total_length) as usize;
    descriptor[offset]
}

fn usb_configuration_descriptor_invalid_type(
    descriptor: [u8; USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES],
    total_length: u16,
) -> u8 {
    let offset = usb_configuration_descriptor_invalid_offset(descriptor, total_length) as usize;
    if offset + 1 >= USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES {
        return 0;
    }
    descriptor[offset + 1]
}

fn usb_descriptor_endpoint0_max_packet_size(
    descriptor: [u8; XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES],
) -> Option<u16> {
    let length = descriptor[0];
    let descriptor_type = descriptor[1];
    let bcd_usb = u16::from_le_bytes([descriptor[2], descriptor[3]]);
    let raw = descriptor[7];
    if length < XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES as u8
        || descriptor_type != USB_DESCRIPTOR_TYPE_DEVICE
    {
        return None;
    }
    match raw {
        8 | 16 | 32 | 64 => Some(raw as u16),
        9 if bcd_usb >= 0x0300 => Some(512),
        _ => None,
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
    dma_clean_range(plan.input_context, XHCI_CONTEXT_PAGE_BYTES);
    dma_clean_range(plan.output_device_context, XHCI_CONTEXT_PAGE_BYTES);
    dma_clean_range(plan.control_endpoint_ring, XHCI_CONTROL_ENDPOINT_RING_TRBS * XHCI_TRB_BYTES);
    dma_clean_range(
        plan.interrupt_in_endpoint_ring,
        XHCI_INTERRUPT_IN_ENDPOINT_RING_TRBS * XHCI_TRB_BYTES,
    );
    if plan.scratchpad_buffers > 0 {
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
        input_context: XHCI_INPUT_CONTEXT.addr(),
        output_device_context: XHCI_OUTPUT_DEVICE_CONTEXT.addr(),
        control_endpoint_ring: XHCI_CONTROL_ENDPOINT_RING.addr(),
        interrupt_in_endpoint_ring: XHCI_INTERRUPT_IN_ENDPOINT_RING.addr(),
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
struct XhciContextPage(UnsafeCell<[u8; XHCI_CONTEXT_PAGE_BYTES]>);

unsafe impl Sync for XhciContextPage {}

impl XhciContextPage {
    const fn new() -> Self {
        Self(UnsafeCell::new([0; XHCI_CONTEXT_PAGE_BYTES]))
    }

    fn addr(&self) -> u64 {
        self.0.get() as *mut u8 as usize as u64
    }

    fn zero(&self) {
        let bytes = unsafe { &mut *self.0.get() };
        let mut index = 0usize;
        while index < bytes.len() {
            bytes[index] = 0;
            index += 1;
        }
    }

    fn set_u32(&self, byte_offset: usize, value: u32) {
        let bytes = unsafe { &mut *self.0.get() };
        let raw = value.to_le_bytes();
        bytes[byte_offset] = raw[0];
        bytes[byte_offset + 1] = raw[1];
        bytes[byte_offset + 2] = raw[2];
        bytes[byte_offset + 3] = raw[3];
    }
}

#[repr(C, align(64))]
struct XhciAlignedBytes<const N: usize>(UnsafeCell<[u8; N]>);

unsafe impl<const N: usize> Sync for XhciAlignedBytes<N> {}

impl<const N: usize> XhciAlignedBytes<N> {
    const fn new() -> Self {
        Self(UnsafeCell::new([0; N]))
    }

    fn addr(&self) -> u64 {
        self.0.get() as *mut u8 as usize as u64
    }

    fn zero(&self) {
        let bytes = unsafe { &mut *self.0.get() };
        let mut index = 0usize;
        while index < N {
            bytes[index] = 0;
            index += 1;
        }
    }

    fn read(&self) -> [u8; N] {
        let bytes = unsafe { &*self.0.get() };
        *bytes
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
static XHCI_CONTROL_ENDPOINT_RING: XhciTrbRing<XHCI_CONTROL_ENDPOINT_RING_TRBS> =
    XhciTrbRing::new();
static XHCI_INTERRUPT_IN_ENDPOINT_RING: XhciTrbRing<XHCI_INTERRUPT_IN_ENDPOINT_RING_TRBS> =
    XhciTrbRing::new();
static XHCI_ERST: XhciAlignedU64<{ XHCI_EVENT_RING_SEGMENT_TABLE_ENTRIES * 2 }> =
    XhciAlignedU64::new();
static XHCI_SCRATCHPAD_ARRAY: XhciAlignedU64<XHCI_STATIC_SCRATCHPAD_BUFFERS> =
    XhciAlignedU64::new();
static XHCI_SCRATCHPAD_PAGES: XhciScratchpadPages = XhciScratchpadPages::new();
static XHCI_INPUT_CONTEXT: XhciContextPage = XhciContextPage::new();
static XHCI_OUTPUT_DEVICE_CONTEXT: XhciContextPage = XhciContextPage::new();
static XHCI_DEVICE_DESCRIPTOR_PREFIX: XhciAlignedBytes<XHCI_DEVICE_DESCRIPTOR_PREFIX_BYTES> =
    XhciAlignedBytes::new();
static XHCI_DEVICE_DESCRIPTOR: XhciAlignedBytes<USB_DEVICE_DESCRIPTOR_BYTES> =
    XhciAlignedBytes::new();
static XHCI_CONFIGURATION_DESCRIPTOR_HEADER: XhciAlignedBytes<
    USB_CONFIGURATION_DESCRIPTOR_HEADER_BYTES,
> = XhciAlignedBytes::new();
static XHCI_CONFIGURATION_DESCRIPTOR: XhciAlignedBytes<USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES> =
    XhciAlignedBytes::new();
static XHCI_BOOT_KEYBOARD_REPORT: XhciAlignedBytes<BOOT_KEYBOARD_REPORT_BYTES> =
    XhciAlignedBytes::new();

#[cfg(feature = "selftest")]
#[path = "usb_tests.rs"]
mod tests;
