//! Selftests for the bounded kernel VFS namespace.

use {
    super::{Directory, File, Node, VfsError, device_name_parts, lookup, mounts, normalize},
    reovim_testrt::{self as testrt, arch_test},
    reovim_uapi_system::{DeviceClass, DeviceEntry},
};

const DEVICES: [DeviceEntry; 4] = [
    DeviceEntry {
        class: DeviceClass::Uart,
        mmio_base: 0x1000,
        mmio_len: 0x100,
        irq: 1,
        capacity_bytes: 0,
        compatible: "arm,pl011",
    },
    DeviceEntry {
        class: DeviceClass::Uart,
        mmio_base: 0x2000,
        mmio_len: 0x100,
        irq: 2,
        capacity_bytes: 0,
        compatible: "ns16550a",
    },
    DeviceEntry {
        class: DeviceClass::Block,
        mmio_base: 0x3000,
        mmio_len: 0x1000,
        irq: 3,
        capacity_bytes: 4096,
        compatible: "virtio,mmio",
    },
    DeviceEntry {
        class: DeviceClass::Bus,
        mmio_base: 0x4000,
        mmio_len: 0x1000,
        irq: 4,
        capacity_bytes: 0,
        compatible: "brcm,bcm2711-pcie",
    },
];

arch_test!(vfs_normalizes_absolute_and_relative_paths, {
    testrt::check_eq(normalize("/", "/").expect("root").as_str(), "/");
    testrt::check_eq(
        normalize("/", "/dev/../boot/./profile")
            .expect("abs")
            .as_str(),
        "/boot/profile",
    );
    testrt::check_eq(normalize("/dev", "../boot/memory").expect("rel").as_str(), "/boot/memory");
    testrt::check_eq(normalize("/dev", "./uart0").expect("same").as_str(), "/dev/uart0");
});

arch_test!(vfs_looks_up_static_and_device_nodes, {
    testrt::check_eq(lookup("/", &DEVICES), Ok(Node::Directory(Directory::Root)));
    testrt::check_eq(lookup("/boot/profile", &DEVICES), Ok(Node::File(File::BootProfile)));
    testrt::check_eq(lookup("/boot/image", &DEVICES), Ok(Node::File(File::BootImage)));
    testrt::check_eq(lookup("/boot/input", &DEVICES), Ok(Node::File(File::BootInput)));
    testrt::check_eq(lookup("/boot/mounts", &DEVICES), Ok(Node::File(File::BootMounts)));
    testrt::check_eq(lookup("/dev/uart0", &DEVICES), Ok(Node::File(File::DevDevice(0))));
    testrt::check_eq(lookup("/dev/uart1", &DEVICES), Ok(Node::File(File::DevDevice(1))));
    testrt::check_eq(lookup("/dev/block0", &DEVICES), Ok(Node::File(File::DevDevice(2))));
    testrt::check_eq(lookup("/dev/bus0", &DEVICES), Ok(Node::File(File::DevDevice(3))));
    testrt::check_eq(lookup("/dev/uart0/more", &DEVICES), Err(VfsError::NotDirectory));
    testrt::check_eq(lookup("/missing", &DEVICES), Err(VfsError::NotFound));
});

arch_test!(vfs_device_names_are_class_ordinals, {
    testrt::check_eq(device_name_parts(&DEVICES, 0), ("uart", 0));
    testrt::check_eq(device_name_parts(&DEVICES, 1), ("uart", 1));
    testrt::check_eq(device_name_parts(&DEVICES, 2), ("block", 0));
    testrt::check_eq(device_name_parts(&DEVICES, 3), ("bus", 0));
});

arch_test!(vfs_mount_table_names_kernel_pseudo_namespaces, {
    let table = mounts();

    testrt::check_eq(table.len(), 4usize);
    testrt::check_eq(table[0].target, "/");
    testrt::check_eq(table[0].fs_type, "rootfs");
    testrt::check_eq(table[1].target, "/boot");
    testrt::check_eq(table[2].target, "/dev");
    testrt::check_eq(table[3].target, "/log");
});
