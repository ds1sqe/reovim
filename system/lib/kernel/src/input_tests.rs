//! Selftests for common input translation.

use {
    super::{
        BootKeyboardDecoder, BootKeyboardIngest, BootKeyboardInputQueue, BootKeyboardReport,
        ConsoleInputByte, read_boot_keyboard_or_fallback,
    },
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

arch_test!(hid_boot_keyboard_queue_preserves_pending_bytes, {
    let mut queue = BootKeyboardInputQueue::new();

    testrt::check_eq(
        queue.try_ingest_report(report(0, [0x04, 0x05, 0, 0, 0, 0])),
        BootKeyboardIngest::Decoded { bytes: 2 },
    );
    testrt::check_eq(queue.pop_pending(), Some(b'a'));
    testrt::check_eq(queue.pending_remaining(), 1usize);

    testrt::check_eq(
        queue.try_ingest_report(report(0, [0x06, 0, 0, 0, 0, 0])),
        BootKeyboardIngest::Backlogged { pending_bytes: 1 },
    );
    testrt::check_eq(queue.pop_pending(), Some(b'b'));
    testrt::check_eq(queue.pop_pending(), None);

    testrt::check_eq(
        queue.try_ingest_report(report(0, [0x06, 0, 0, 0, 0, 0])),
        BootKeyboardIngest::Decoded { bytes: 1 },
    );
    testrt::check_eq(queue.pop_pending(), Some(b'c'));
});

arch_test!(hid_boot_keyboard_merge_prefers_usb_before_fallback, {
    let mut queue = BootKeyboardInputQueue::new();

    let _ = queue.try_ingest_report(report(0, [0x04, 0, 0, 0, 0, 0]));
    let mut report_polls = 0usize;
    let mut fallback_polls = 0usize;
    let pending = read_boot_keyboard_or_fallback(
        &mut queue,
        || {
            report_polls += 1;
            None
        },
        || {
            fallback_polls += 1;
            Some(b'u')
        },
    );
    testrt::check_eq(pending, Some(ConsoleInputByte::UsbKeyboard(b'a')));
    testrt::check_eq(report_polls, 0usize);
    testrt::check_eq(fallback_polls, 0usize);

    let next_report = read_boot_keyboard_or_fallback(
        &mut queue,
        || Some(report(0, [0x05, 0x06, 0, 0, 0, 0])),
        || {
            fallback_polls += 1;
            Some(b'u')
        },
    );
    testrt::check_eq(next_report, Some(ConsoleInputByte::UsbKeyboard(b'b')));
    testrt::check_eq(fallback_polls, 0usize);

    let queued = read_boot_keyboard_or_fallback(
        &mut queue,
        || None,
        || {
            fallback_polls += 1;
            Some(b'u')
        },
    );
    testrt::check_eq(queued, Some(ConsoleInputByte::UsbKeyboard(b'c')));
    testrt::check_eq(fallback_polls, 0usize);

    let release_falls_back = read_boot_keyboard_or_fallback(
        &mut queue,
        || Some(report(0, [0, 0, 0, 0, 0, 0])),
        || {
            fallback_polls += 1;
            Some(b'u')
        },
    );
    testrt::check_eq(release_falls_back, Some(ConsoleInputByte::Fallback(b'u')));
    testrt::check_eq(fallback_polls, 1usize);

    let no_sources = read_boot_keyboard_or_fallback(&mut queue, || None, || None);
    testrt::check_eq(no_sources, None);
});
