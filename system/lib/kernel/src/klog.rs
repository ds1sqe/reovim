//! Bounded in-kernel diagnostic log for root-daemon `dmesg`.
//!
//! This is the kernel's own boot/runtime message source. It is deliberately
//! static, no-alloc, and byte-oriented so freestanding OS images can record
//! boot checks and shell actions before any filesystem exists.

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

/// Bytes retained in the kernel log ring.
pub const CAPACITY: usize = 32768;

struct KernelLog {
    buf: [u8; CAPACITY],
    start: usize,
    len: usize,
    dropped: usize,
}

impl KernelLog {
    const fn new() -> Self {
        Self {
            buf: [0u8; CAPACITY],
            start: 0,
            len: 0,
            dropped: 0,
        }
    }

    fn reset(&mut self) {
        self.start = 0;
        self.len = 0;
        self.dropped = 0;
        self.buf.fill(0);
    }

    fn append_byte(&mut self, byte: u8) {
        if self.len < self.buf.len() {
            let index = (self.start + self.len) % self.buf.len();
            self.buf[index] = byte;
            self.len += 1;
            return;
        }
        self.buf[self.start] = byte;
        self.start = (self.start + 1) % self.buf.len();
        self.dropped = self.dropped.saturating_add(1);
    }

    fn append_bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.append_byte(byte);
        }
    }
}

struct LogCell(UnsafeCell<KernelLog>);

// SAFETY: access is serialized by `LOCK`.
unsafe impl Sync for LogCell {}

static LOG: LogCell = LogCell(UnsafeCell::new(KernelLog::new()));
static LOCK: AtomicBool = AtomicBool::new(false);

struct Guard;

impl Guard {
    fn acquire() -> Self {
        while LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        LOCK.store(false, Ordering::Release);
    }
}

fn with_log<R>(f: impl FnOnce(&mut KernelLog) -> R) -> R {
    let _guard = Guard::acquire();
    // SAFETY: `LOCK` serializes all mutable access to the log cell.
    let log = unsafe { &mut *LOG.0.get() };
    f(log)
}

/// Clears the retained kernel log.
pub fn reset() {
    with_log(KernelLog::reset);
}

/// Appends raw bytes to the kernel log ring.
pub fn append_bytes(bytes: &[u8]) {
    with_log(|log| log.append_bytes(bytes));
}

/// Appends a UTF-8 line plus trailing newline.
pub fn append_line(line: &str) {
    append_bytes(line.as_bytes());
    append_bytes(b"\n");
}

/// Appends a decimal `usize` value.
pub fn append_usize_dec(value: usize) {
    let mut buf = [0u8; 20];
    let len = usize_to_dec(value, &mut buf);
    append_bytes(&buf[..len]);
}

/// Returns retained byte count.
#[must_use]
pub fn len() -> usize {
    with_log(|log| log.len)
}

/// Returns the number of bytes dropped due to ring overwrite.
#[must_use]
pub fn dropped_bytes() -> usize {
    with_log(|log| log.dropped)
}

/// Writes the retained log in chronological order.
///
/// Returns whether any log bytes were written.
pub fn write_to(mut write: impl FnMut(&[u8])) -> bool {
    with_log(|log| {
        if log.len == 0 {
            return false;
        }
        if log.dropped > 0 {
            write(b"[klog] dropped_bytes=");
            let mut buf = [0u8; 20];
            let len = usize_to_dec(log.dropped, &mut buf);
            write(&buf[..len]);
            write(b"\n");
        }
        let first = core::cmp::min(log.len, log.buf.len() - log.start);
        write(&log.buf[log.start..log.start + first]);
        if first < log.len {
            write(&log.buf[..log.len - first]);
        }
        true
    })
}

fn usize_to_dec(mut value: usize, out: &mut [u8]) -> usize {
    if value == 0 {
        out[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 20];
    let mut len = 0usize;
    while value > 0 {
        tmp[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
    }
    let mut i = 0usize;
    while len > 0 {
        len -= 1;
        out[i] = tmp[len];
        i += 1;
    }
    i
}

#[cfg(feature = "selftest")]
#[path = "klog_tests.rs"]
mod tests;
