//! Selftests for root console line input.

use {
    super::read_line,
    core::{
        cell::UnsafeCell,
        sync::atomic::{AtomicUsize, Ordering},
    },
    reovim_testrt::{self as testrt, arch_test},
};

struct StaticBytes {
    input: UnsafeCell<&'static [u8]>,
    input_at: AtomicUsize,
    output: UnsafeCell<[u8; 128]>,
    output_len: AtomicUsize,
}

// SAFETY: selftests run single-threaded; atomics only make the shared fixture
// easy to reset/read through callback function pointers.
unsafe impl Sync for StaticBytes {}

static IO: StaticBytes = StaticBytes {
    input: UnsafeCell::new(b""),
    input_at: AtomicUsize::new(0),
    output: UnsafeCell::new([0u8; 128]),
    output_len: AtomicUsize::new(0),
};

fn reset(input: &'static [u8]) {
    // SAFETY: single-threaded selftest fixture.
    unsafe {
        *IO.input.get() = input;
        (*IO.output.get()).fill(0);
    }
    IO.input_at.store(0, Ordering::Release);
    IO.output_len.store(0, Ordering::Release);
}

fn read_byte() -> Option<u8> {
    let at = IO.input_at.fetch_add(1, Ordering::AcqRel);
    // SAFETY: input is a static byte slice set by `reset`.
    unsafe { (&*IO.input.get()).get(at).copied() }
}

fn write_bytes(bytes: &[u8]) {
    let mut len = IO.output_len.load(Ordering::Acquire);
    // SAFETY: single-threaded selftest fixture.
    unsafe {
        let output = &mut *IO.output.get();
        let mut i = 0usize;
        while i < bytes.len() && len < output.len() {
            output[len] = bytes[i];
            len += 1;
            i += 1;
        }
    }
    IO.output_len.store(len, Ordering::Release);
}

fn output() -> &'static [u8] {
    let len = IO.output_len.load(Ordering::Acquire);
    // SAFETY: output is only written by the single-threaded fixture callback.
    unsafe { &(&*IO.output.get())[..len] }
}

arch_test!(console_io_reads_and_echoes_line, {
    reset(b"help\r");
    let mut line = [0u8; 16];
    let len = read_line(&mut line, read_byte, write_bytes);
    testrt::check_eq(&line[..len], b"help");
    testrt::check_eq(output(), b"help\n");
});

arch_test!(console_io_backspace_edits_buffer_and_echoes_erase, {
    reset(b"ab\x08c\n");
    let mut line = [0u8; 16];
    let len = read_line(&mut line, read_byte, write_bytes);
    testrt::check_eq(&line[..len], b"ac");
    testrt::check_eq(output(), b"ab\x08 \x08c\n");
});

arch_test!(console_io_tab_normalizes_to_space, {
    reset(b"cd\t/dev\n");
    let mut line = [0u8; 16];
    let len = read_line(&mut line, read_byte, write_bytes);
    testrt::check_eq(&line[..len], b"cd /dev");
    testrt::check_eq(output(), b"cd /dev\n");
});

arch_test!(console_io_empty_enter_is_not_eof, {
    reset(b"\n");
    let mut line = [0u8; 16];
    let len = read_line(&mut line, read_byte, write_bytes);
    testrt::check_eq(&line[..len], b"\n");
    testrt::check_eq(output(), b"\n");
});

arch_test!(console_io_returns_partial_line_on_eof, {
    reset(b"dm");
    let mut line = [0u8; 16];
    let len = read_line(&mut line, read_byte, write_bytes);
    testrt::check_eq(&line[..len], b"dm");
    testrt::check_eq(output(), b"dm");
});
