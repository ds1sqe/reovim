//! Selftests for the bounded kernel log ring.

use {
    super::{CAPACITY, append_bytes, append_line, dropped_bytes, len, reset, write_to},
    core::cell::UnsafeCell,
    reovim_testrt::{self as testrt, arch_test},
};

const SINK_CAPACITY: usize = CAPACITY + 128;

struct Sink {
    buf: UnsafeCell<[u8; SINK_CAPACITY]>,
    len: UnsafeCell<usize>,
}

// SAFETY: the selftest runner is single-threaded.
unsafe impl Sync for Sink {}

static SINK: Sink = Sink {
    buf: UnsafeCell::new([0u8; SINK_CAPACITY]),
    len: UnsafeCell::new(0),
};

fn sink_clear() {
    // SAFETY: single-threaded selftest runner.
    unsafe {
        *SINK.len.get() = 0;
        (*SINK.buf.get()).fill(0);
    }
}

fn sink_write(bytes: &[u8]) {
    // SAFETY: single-threaded selftest runner.
    unsafe {
        let buf = &mut *SINK.buf.get();
        let len = &mut *SINK.len.get();
        let mut i = 0usize;
        while i < bytes.len() && *len < buf.len() {
            buf[*len] = bytes[i];
            *len += 1;
            i += 1;
        }
    }
}

fn sink_bytes() -> &'static [u8] {
    // SAFETY: single-threaded selftest runner.
    unsafe {
        let len = *SINK.len.get();
        let ptr = SINK.buf.get() as *const [u8; SINK_CAPACITY];
        let buf = &*ptr;
        &buf[..len]
    }
}

fn assert_contains(haystack: &[u8], needle: &[u8]) {
    let mut i = 0usize;
    while i + needle.len() <= haystack.len() {
        let mut matched = 0usize;
        while matched < needle.len() && haystack[i + matched] == needle[matched] {
            matched += 1;
        }
        if matched == needle.len() {
            return;
        }
        i += 1;
    }
    testrt::check(false, "expected log chunk");
}

arch_test!(klog_appends_and_writes_in_order, {
    reset();
    sink_clear();

    append_line("boot: start");
    append_bytes(b"shell: pwd\n");

    testrt::check_eq(len(), 23);
    testrt::check(write_to(sink_write), "log writes non-empty content");
    testrt::check_eq(sink_bytes(), b"boot: start\nshell: pwd\n");
});

arch_test!(klog_wraps_and_reports_dropped_bytes, {
    reset();
    sink_clear();

    let block = [b'a'; 256];
    let mut i = 0usize;
    while i < (CAPACITY / block.len()) + 2 {
        append_bytes(&block);
        i += 1;
    }
    append_line("tail");

    testrt::check(dropped_bytes() > 0, "wrap drops old bytes");
    testrt::check_eq(len(), CAPACITY);
    testrt::check(write_to(sink_write), "wrapped log writes content");
    assert_contains(sink_bytes(), b"[klog] dropped_bytes=");
    assert_contains(sink_bytes(), b"tail\n");
});
