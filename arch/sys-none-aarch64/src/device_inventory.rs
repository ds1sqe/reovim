//! BCM2711 DTB compatible-string classification.
//!
//! The DTB reader and inventory shaper live in `reovim-system-kernel`, but the
//! table that knows this target's firmware-compatible strings belongs below the
//! bridge. The table returns only coarse KABI classes; it does not drive devices.

use reovim_kabi_platform::DeviceClass;

/// Maps one BCM2711 / ARM-common DTB `compatible` string to a coarse KABI
/// [`DeviceClass`].
///
/// Bus roots, storage controllers, and USB controllers are only declared
/// present here. Capacity reads, descriptor walks, and device-specific
/// initialization are later driver work.
///
/// ```rust,ignore
/// use reovim_kabi_platform::DeviceClass;
///
/// assert_eq!(
///     reovim_arch_sys_none_aarch64::classify_compatible("arm,pl011"),
///     DeviceClass::Uart,
/// );
/// assert_eq!(
///     reovim_arch_sys_none_aarch64::classify_compatible("vendor,unknown"),
///     DeviceClass::Unknown,
/// );
/// ```
#[must_use]
pub fn classify_compatible(compatible: &str) -> DeviceClass {
    match compatible {
        "arm,pl011" => DeviceClass::Uart,
        "arm,gic-400" => DeviceClass::Interrupt,
        "brcm,bcm2835-mbox" => DeviceClass::Mailbox,
        "brcm,bcm2711-emmc2" => DeviceClass::Block,
        "brcm,bcm2708-usb" => DeviceClass::Usb,
        "generic-xhci" => DeviceClass::Usb,
        "brcm,bcm2711-pcie" => DeviceClass::Bus,
        _ => DeviceClass::Unknown,
    }
}

#[cfg(feature = "selftest")]
#[path = "device_inventory_tests.rs"]
mod tests;
