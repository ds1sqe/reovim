//! Selftests for common input translation.

use {
    super::{BootKeyboardDecoder, BootKeyboardReport},
    reovim_testrt::{self as testrt, arch_test},
};

fn report(modifiers: u8, keys: [u8; 6]) -> BootKeyboardReport {
    BootKeyboardReport::new([
        modifiers, 0, keys[0], keys[1], keys[2], keys[3], keys[4], keys[5],
    ])
}

fn decode(decoder: &mut BootKeyboardDecoder, modifiers: u8, keys: [u8; 6]) -> [u8; 8] {
    let mut out = [0u8; 8];
    let len = decoder.decode_report(report(modifiers, keys), &mut out);
    out[7] = len as u8;
    out
}

arch_test!(hid_boot_keyboard_letters_and_held_keys, {
    let mut decoder = BootKeyboardDecoder::new();

    let first = decode(&mut decoder, 0, [0x04, 0, 0, 0, 0, 0]);
    testrt::check_eq(first[0], b'a');
    testrt::check_eq(first[7], 1);

    let held = decode(&mut decoder, 0, [0x04, 0, 0, 0, 0, 0]);
    testrt::check_eq(held[7], 0);

    let released = decode(&mut decoder, 0, [0, 0, 0, 0, 0, 0]);
    testrt::check_eq(released[7], 0);

    let pressed_again = decode(&mut decoder, 0, [0x04, 0, 0, 0, 0, 0]);
    testrt::check_eq(pressed_again[0], b'a');
    testrt::check_eq(pressed_again[7], 1);
});

arch_test!(hid_boot_keyboard_shift_caps_and_ctrl, {
    let mut decoder = BootKeyboardDecoder::new();

    let shifted = decode(&mut decoder, 1 << 1, [0x05, 0, 0, 0, 0, 0]);
    testrt::check_eq(shifted[0], b'B');
    testrt::check_eq(shifted[7], 1);
    let _ = decode(&mut decoder, 0, [0, 0, 0, 0, 0, 0]);

    let caps_toggle = decode(&mut decoder, 0, [0x39, 0, 0, 0, 0, 0]);
    testrt::check_eq(caps_toggle[7], 0);
    let _ = decode(&mut decoder, 0, [0, 0, 0, 0, 0, 0]);

    let caps = decode(&mut decoder, 0, [0x06, 0, 0, 0, 0, 0]);
    testrt::check_eq(caps[0], b'C');
    testrt::check_eq(caps[7], 1);
    let _ = decode(&mut decoder, 0, [0, 0, 0, 0, 0, 0]);

    let caps_shift = decode(&mut decoder, 1 << 1, [0x07, 0, 0, 0, 0, 0]);
    testrt::check_eq(caps_shift[0], b'd');
    testrt::check_eq(caps_shift[7], 1);
    let _ = decode(&mut decoder, 0, [0, 0, 0, 0, 0, 0]);

    let ctrl_a = decode(&mut decoder, 1, [0x04, 0, 0, 0, 0, 0]);
    testrt::check_eq(ctrl_a[0], 1);
    testrt::check_eq(ctrl_a[7], 1);
});

arch_test!(hid_boot_keyboard_control_and_punctuation_bytes, {
    let mut decoder = BootKeyboardDecoder::new();

    let enter_backspace_tab = decode(&mut decoder, 0, [0x28, 0x2a, 0x2b, 0, 0, 0]);
    testrt::check_eq(&enter_backspace_tab[..3], &[b'\n', 0x08, b'\t']);
    testrt::check_eq(enter_backspace_tab[7], 3);
    let _ = decode(&mut decoder, 0, [0, 0, 0, 0, 0, 0]);

    let shifted_numbers = decode(&mut decoder, 1 << 1, [0x1e, 0x1f, 0x20, 0, 0, 0]);
    testrt::check_eq(&shifted_numbers[..3], &[b'!', b'@', b'#']);
    testrt::check_eq(shifted_numbers[7], 3);
    let _ = decode(&mut decoder, 0, [0, 0, 0, 0, 0, 0]);

    let punctuation = decode(&mut decoder, 0, [0x2d, 0x2e, 0x38, 0, 0, 0]);
    testrt::check_eq(&punctuation[..3], &[b'-', b'=', b'/']);
    testrt::check_eq(punctuation[7], 3);
    let _ = decode(&mut decoder, 0, [0, 0, 0, 0, 0, 0]);

    let shifted_punctuation = decode(&mut decoder, 1 << 1, [0x2d, 0x2e, 0x38, 0, 0, 0]);
    testrt::check_eq(&shifted_punctuation[..3], &[b'_', b'+', b'?']);
    testrt::check_eq(shifted_punctuation[7], 3);
});

arch_test!(hid_boot_keyboard_ignores_error_rollover_report, {
    let mut decoder = BootKeyboardDecoder::new();

    let bad = decode(&mut decoder, 0, [0x01, 0x04, 0, 0, 0, 0]);
    testrt::check_eq(bad[7], 0);

    let good = decode(&mut decoder, 0, [0x04, 0, 0, 0, 0, 0]);
    testrt::check_eq(good[0], b'a');
    testrt::check_eq(good[7], 1);
});
