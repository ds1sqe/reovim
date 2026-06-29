//! Behavioral tests for `uapi/system` data wrappers.

use {
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr, SyscallRet},
    reovim_uapi_system::{
        BootInfo, DeviceClass, DeviceEntry, DeviceInventory, MemorySummary, SyscallSystemControl,
        SystemError,
    },
};

#[test]
fn memory_summary_reports_count_and_usable_bytes() {
    let summary = MemorySummary::new(2, 4096);
    assert_eq!(summary.range_count(), 2);
    assert_eq!(summary.usable_bytes(), 4096);
    assert!(!summary.is_empty());
}

#[test]
fn boot_info_default_is_empty_diagnostic_view() {
    let info = BootInfo::default();
    assert!(info.memory.is_empty());
    assert_eq!(info.cpu_count, 0);
}

#[test]
fn device_inventory_carries_static_entries() {
    static DEVICES: [DeviceEntry; 1] = [DeviceEntry {
        class: DeviceClass::Uart,
        mmio_base: 0x1000,
        mmio_len: 0x100,
        irq: 1,
        capacity_bytes: 0,
        compatible: "arm,pl011",
    }];

    let inventory = DeviceInventory { devices: &DEVICES };
    assert_eq!(inventory.devices[0].class, DeviceClass::Uart);
    assert_eq!(inventory.devices[0].compatible, "arm,pl011");
}

#[test]
fn syscall_system_control_lowers_halt_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SYSTEM_HALT);
        assert_eq!(args, SyscallArgs::EMPTY);
        SyscallRet::success(0)
    }

    let system = SyscallSystemControl::new(RawSyscall::new(syscall));
    assert_eq!(system.halt(), Ok(()));
}

#[test]
fn syscall_system_control_lowers_probe_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::PROVIDER_PROBE);
        let target = "usb-keyboard";
        assert_eq!(args.a0, target.as_ptr().addr());
        assert_eq!(args.a1, target.len());
        assert_eq!(args.a2, 0);
        assert_eq!(args.a3, 0);
        assert_eq!(args.a4, 0);
        assert_eq!(args.a5, 0);
        SyscallRet::success(0)
    }

    let system = SyscallSystemControl::new(RawSyscall::new(syscall));
    assert_eq!(system.probe("usb-keyboard"), Ok(()));
}

#[test]
fn syscall_system_control_maps_transport_errors() {
    fn no_current(_: SyscallNr, _: SyscallArgs) -> SyscallRet {
        SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS)
    }
    fn unknown(_: SyscallNr, _: SyscallArgs) -> SyscallRet {
        SyscallRet::failure(SyscallError::NOT_FOUND)
    }

    assert_eq!(
        SyscallSystemControl::new(RawSyscall::new(no_current)).halt(),
        Err(SystemError::NoCurrentProcess),
    );
    assert_eq!(
        SyscallSystemControl::new(RawSyscall::new(unknown)).probe("missing"),
        Err(SystemError::UnknownTarget),
    );
}
