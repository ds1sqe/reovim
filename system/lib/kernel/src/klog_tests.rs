//! Selftests for the bounded kernel log ring.

use {
    super::{
        CAPACITY, EMPTY_EVENT_RECORD, MAX_EVENTS, append_bytes, append_event,
        append_event_with_context, append_line, dropped_bytes, len, reset, snapshot_events, stats,
        write_to,
    },
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

arch_test!(klog_structured_events_have_sequence_numbers, {
    reset();
    sink_clear();

    append_event("proc", "info", "program-exit");
    append_event_with_context("dump", "warn", "persistent-unavailable", 7, 9);

    testrt::check_eq(stats().next_event_seq, 3usize);
    testrt::check_eq(stats().retained_events, 2usize);
    testrt::check_eq(stats().identity.boot_id, 1usize);
    testrt::check_eq(stats().identity.session_id, 1usize);
    testrt::check_eq(stats().identity.identity_source, "volatile-memory");
    testrt::check(write_to(sink_write), "event log writes content");
    assert_contains(
        sink_bytes(),
        b"event seq=1 component=proc severity=info kind=program-exit boot=1 session=1 source=kernel\n",
    );
    assert_contains(
        sink_bytes(),
        b"event seq=2 component=dump severity=warn kind=persistent-unavailable boot=1 session=1 source=process pid=7 task=9\n",
    );

    let mut events = [EMPTY_EVENT_RECORD; MAX_EVENTS];
    let count = snapshot_events(&mut events);
    testrt::check_eq(count, 2usize);
    testrt::check_eq(events[0].seq, 1usize);
    testrt::check_eq(events[0].boot_id, 1usize);
    testrt::check_eq(events[0].session_id, 1usize);
    testrt::check_eq(events[0].source, "kernel");
    testrt::check_eq(events[0].component, "proc");
    testrt::check_eq(events[0].kind, "program-exit");
    testrt::check_eq(events[1].seq, 2usize);
    testrt::check_eq(events[1].source, "process");
    testrt::check_eq(events[1].process_id, 7usize);
    testrt::check_eq(events[1].task_id, 9usize);
});

arch_test!(klog_structured_event_snapshot_wraps_in_sequence_order, {
    reset();

    let mut index = 0usize;
    while index < MAX_EVENTS + 3 {
        append_event("proc", "info", "program-exit");
        index += 1;
    }

    let mut events = [EMPTY_EVENT_RECORD; MAX_EVENTS];
    let count = snapshot_events(&mut events);
    testrt::check_eq(count, MAX_EVENTS);
    testrt::check_eq(events[0].seq, 4usize);
    testrt::check_eq(events[count - 1].seq, MAX_EVENTS + 3);
    testrt::check_eq(events[0].component, "proc");
    testrt::check_eq(events[count - 1].kind, "program-exit");
});
