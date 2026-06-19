//! Tests for the pure `classify` function in `enumerate.rs`.
//!
//! L12 layout: declared in `enumerate.rs` via
//! `#[cfg(feature = "selftest")] #[path = "enumerate_tests.rs"] mod tests;`,
//! so `super::` reaches the private `classify` function and the imported
//! `DeviceClass`.
//!
//! These tests cover the classifier only — no arena or live DTB is needed.
//! The full `enumerate` path is exercised by the on-target bootcore smoke
//! test the main session runs with a real DTB under QEMU.

use {
    super::classify,
    crate::{arch_test, testrt},
    reovim_kabi_platform::DeviceClass,
};

arch_test!(classify_pl011_is_uart, {
    testrt::check(classify("arm,pl011") == DeviceClass::Uart, "arm,pl011 classifies as Uart");
});

arch_test!(classify_gic400_is_interrupt, {
    testrt::check(
        classify("arm,gic-400") == DeviceClass::Interrupt,
        "arm,gic-400 classifies as Interrupt",
    );
});

arch_test!(classify_bcm2835_mbox_is_mailbox, {
    testrt::check(
        classify("brcm,bcm2835-mbox") == DeviceClass::Mailbox,
        "brcm,bcm2835-mbox classifies as Mailbox",
    );
});

arch_test!(classify_emmc2_is_block, {
    testrt::check(
        classify("brcm,bcm2711-emmc2") == DeviceClass::Block,
        "brcm,bcm2711-emmc2 classifies as Block",
    );
});

arch_test!(classify_dwc2_usb_is_usb, {
    testrt::check(
        classify("brcm,bcm2708-usb") == DeviceClass::Usb,
        "brcm,bcm2708-usb classifies as Usb",
    );
});

arch_test!(classify_unknown_compatible_is_unknown, {
    testrt::check(
        classify("totally-unknown") == DeviceClass::Unknown,
        "unrecognised compatible classifies as Unknown",
    );
});
