//! Tests for the target-specific BCM2711 compatible-string table.

use {
    super::classify_compatible,
    reovim_kabi_platform::DeviceClass,
    reovim_testrt::{self as testrt, arch_test},
};

arch_test!(classifies_bcm2711_compatible_strings, {
    testrt::check_eq(classify_compatible("arm,pl011"), DeviceClass::Uart);
    testrt::check_eq(classify_compatible("arm,gic-400"), DeviceClass::Interrupt);
    testrt::check_eq(classify_compatible("brcm,bcm2835-mbox"), DeviceClass::Mailbox);
    testrt::check_eq(classify_compatible("brcm,bcm2711-emmc2"), DeviceClass::Block);
    testrt::check_eq(classify_compatible("brcm,bcm2708-usb"), DeviceClass::Usb);
    testrt::check_eq(classify_compatible("generic-xhci"), DeviceClass::Usb);
    testrt::check_eq(classify_compatible("brcm,bcm2711-pcie"), DeviceClass::Bus);
});

arch_test!(leaves_unknown_compatible_unclassified, {
    testrt::check_eq(classify_compatible("unknown,vendor-device"), DeviceClass::Unknown);
});
