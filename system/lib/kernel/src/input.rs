//! Common input translation used by OS-mode console paths.
//!
//! Hardware polling and USB host-controller mechanics stay below this bridge.
//! This module owns only device-neutral policy: turning a USB HID boot keyboard
//! report into bytes the root console line discipline already understands.

/// Length of a USB HID boot keyboard input report.
pub const BOOT_KEYBOARD_REPORT_BYTES: usize = 8;

/// One USB HID boot keyboard report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootKeyboardReport {
    bytes: [u8; BOOT_KEYBOARD_REPORT_BYTES],
}

impl BootKeyboardReport {
    /// Builds a report from the raw 8-byte interrupt payload.
    #[must_use]
    pub const fn new(bytes: [u8; BOOT_KEYBOARD_REPORT_BYTES]) -> Self {
        Self { bytes }
    }

    /// Modifier bitmap from report byte 0.
    #[must_use]
    pub const fn modifiers(self) -> u8 {
        self.bytes[0]
    }

    /// Six keycode slots from report bytes 2..8.
    #[must_use]
    pub const fn keycodes(self) -> [u8; 6] {
        [
            self.bytes[2],
            self.bytes[3],
            self.bytes[4],
            self.bytes[5],
            self.bytes[6],
            self.bytes[7],
        ]
    }
}

/// Stateful USB HID boot-keyboard to console-byte translator.
pub struct BootKeyboardDecoder {
    previous: [u8; 6],
    caps_lock: bool,
}

impl BootKeyboardDecoder {
    /// Starts with no keys held and Caps Lock off.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            previous: [0u8; 6],
            caps_lock: false,
        }
    }

    /// Decodes newly pressed keys from `report` into `out`.
    ///
    /// Held keys are not repeated here. The root shell can add repeat policy
    /// later at the console/input layer if it needs it.
    pub fn decode_report(&mut self, report: BootKeyboardReport, out: &mut [u8]) -> usize {
        let keycodes = report.keycodes();
        if has_error_rollover(keycodes) {
            return 0;
        }

        let mut written = 0usize;
        let shift = has_shift(report.modifiers());
        let ctrl = has_ctrl(report.modifiers());
        for keycode in keycodes {
            if keycode == 0 || contains(self.previous, keycode) {
                continue;
            }
            if keycode == KEY_CAPS_LOCK {
                self.caps_lock = !self.caps_lock;
                continue;
            }
            let Some(byte) = usage_to_console_byte(keycode, shift, ctrl, self.caps_lock) else {
                continue;
            };
            if written >= out.len() {
                break;
            }
            out[written] = byte;
            written += 1;
        }

        self.previous = keycodes;
        written
    }
}

impl Default for BootKeyboardDecoder {
    fn default() -> Self {
        Self::new()
    }
}

const MOD_LCTRL: u8 = 1 << 0;
const MOD_LSHIFT: u8 = 1 << 1;
const MOD_RCTRL: u8 = 1 << 4;
const MOD_RSHIFT: u8 = 1 << 5;

const KEY_A: u8 = 0x04;
const KEY_Z: u8 = 0x1d;
const KEY_1: u8 = 0x1e;
const KEY_0: u8 = 0x27;
const KEY_ENTER: u8 = 0x28;
const KEY_ESCAPE: u8 = 0x29;
const KEY_BACKSPACE: u8 = 0x2a;
const KEY_TAB: u8 = 0x2b;
const KEY_SPACE: u8 = 0x2c;
const KEY_CAPS_LOCK: u8 = 0x39;

fn has_shift(modifiers: u8) -> bool {
    modifiers & (MOD_LSHIFT | MOD_RSHIFT) != 0
}

fn has_ctrl(modifiers: u8) -> bool {
    modifiers & (MOD_LCTRL | MOD_RCTRL) != 0
}

fn has_error_rollover(keycodes: [u8; 6]) -> bool {
    for keycode in keycodes {
        if matches!(keycode, 0x01..=0x03) {
            return true;
        }
    }
    false
}

fn contains(keycodes: [u8; 6], needle: u8) -> bool {
    let mut i = 0usize;
    while i < keycodes.len() {
        if keycodes[i] == needle {
            return true;
        }
        i += 1;
    }
    false
}

#[allow(clippy::match_same_arms)]
fn usage_to_console_byte(keycode: u8, shift: bool, ctrl: bool, caps_lock: bool) -> Option<u8> {
    if (KEY_A..=KEY_Z).contains(&keycode) {
        if ctrl {
            return Some(keycode - KEY_A + 1);
        }
        let upper = shift ^ caps_lock;
        let base = if upper { b'A' } else { b'a' };
        return Some(base + (keycode - KEY_A));
    }

    if (KEY_1..=KEY_0).contains(&keycode) {
        return Some(number_key(keycode, shift));
    }

    match keycode {
        KEY_ENTER => Some(b'\n'),
        KEY_ESCAPE => Some(0x1b),
        KEY_BACKSPACE => Some(0x08),
        KEY_TAB => Some(b'\t'),
        KEY_SPACE => Some(b' '),
        0x2d => Some(if shift { b'_' } else { b'-' }),
        0x2e => Some(if shift { b'+' } else { b'=' }),
        0x2f => Some(if shift { b'{' } else { b'[' }),
        0x30 => Some(if shift { b'}' } else { b']' }),
        0x31 => Some(if shift { b'|' } else { b'\\' }),
        0x33 => Some(if shift { b':' } else { b';' }),
        0x34 => Some(if shift { b'"' } else { b'\'' }),
        0x35 => Some(if shift { b'~' } else { b'`' }),
        0x36 => Some(if shift { b'<' } else { b',' }),
        0x37 => Some(if shift { b'>' } else { b'.' }),
        0x38 => Some(if shift { b'?' } else { b'/' }),
        _ => None,
    }
}

fn number_key(keycode: u8, shift: bool) -> u8 {
    const NORMAL: [u8; 10] = *b"1234567890";
    const SHIFTED: [u8; 10] = *b"!@#$%^&*()";
    let index = (keycode - KEY_1) as usize;
    if shift { SHIFTED[index] } else { NORMAL[index] }
}

#[cfg(feature = "selftest")]
#[path = "input_tests.rs"]
mod tests;
