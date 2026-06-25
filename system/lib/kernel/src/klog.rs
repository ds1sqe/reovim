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
pub const CAPACITY: usize = 65536;
/// Structured events retained beside the rendered text ring.
pub const MAX_EVENTS: usize = 192;

/// Boot/session identity attached to retained diagnostic records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticIdentity {
    /// Current boot identifier.
    pub boot_id: usize,
    /// Current root-shell/session identifier.
    pub session_id: usize,
    /// Source of this identity, for example `volatile-memory`.
    pub identity_source: &'static str,
}

impl DiagnosticIdentity {
    /// Default identity before a persistent boot/session allocator exists.
    #[must_use]
    pub const fn volatile_default() -> Self {
        Self {
            boot_id: 1,
            session_id: 1,
            identity_source: "volatile-memory",
        }
    }
}

/// One retained structured kernel event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventRecord {
    /// Monotonic event sequence number.
    pub seq: usize,
    /// Boot identifier active when the event was recorded.
    pub boot_id: usize,
    /// Session identifier active when the event was recorded.
    pub session_id: usize,
    /// Local source for this event record.
    pub source: &'static str,
    /// Event component, for example `proc` or `dump`.
    pub component: &'static str,
    /// Event severity, for example `info` or `warn`.
    pub severity: &'static str,
    /// Event kind inside the component.
    pub kind: &'static str,
    /// Optional process identifier, or `0` when not attached.
    pub process_id: usize,
    /// Optional task identifier, or `0` when not attached.
    pub task_id: usize,
}

impl EventRecord {
    /// Empty event-table slot.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            seq: 0,
            boot_id: 0,
            session_id: 0,
            source: "",
            component: "",
            severity: "",
            kind: "",
            process_id: 0,
            task_id: 0,
        }
    }
}

/// Empty event record used for bounded snapshots.
pub const EMPTY_EVENT_RECORD: EventRecord = EventRecord::empty();

struct KernelLog {
    buf: [u8; CAPACITY],
    start: usize,
    len: usize,
    dropped: usize,
    next_event_seq: usize,
    identity: DiagnosticIdentity,
    events: [EventRecord; MAX_EVENTS],
}

impl KernelLog {
    const fn new() -> Self {
        Self {
            buf: [0u8; CAPACITY],
            start: 0,
            len: 0,
            dropped: 0,
            next_event_seq: 1,
            identity: DiagnosticIdentity::volatile_default(),
            events: [EventRecord::empty(); MAX_EVENTS],
        }
    }

    fn reset(&mut self) {
        self.start = 0;
        self.len = 0;
        self.dropped = 0;
        self.next_event_seq = 1;
        self.identity = DiagnosticIdentity::volatile_default();
        self.buf.fill(0);
        self.events = [EventRecord::empty(); MAX_EVENTS];
    }

    fn set_identity(&mut self, identity: DiagnosticIdentity) {
        self.identity = identity;
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

    fn append_event_record(
        &mut self,
        source: &'static str,
        component: &'static str,
        severity: &'static str,
        kind: &'static str,
        process_id: usize,
        task_id: usize,
    ) {
        let seq = self.next_event_seq;
        self.next_event_seq = self.next_event_seq.saturating_add(1);
        let slot = (seq.saturating_sub(1)) % self.events.len();
        self.events[slot] = EventRecord {
            seq,
            boot_id: self.identity.boot_id,
            session_id: self.identity.session_id,
            source,
            component,
            severity,
            kind,
            process_id,
            task_id,
        };

        self.append_bytes(b"event seq=");
        append_usize_dec_to(self, seq);
        self.append_bytes(b" component=");
        self.append_bytes(component.as_bytes());
        self.append_bytes(b" severity=");
        self.append_bytes(severity.as_bytes());
        self.append_bytes(b" kind=");
        self.append_bytes(kind.as_bytes());
        self.append_bytes(b" boot=");
        append_usize_dec_to(self, self.identity.boot_id);
        self.append_bytes(b" session=");
        append_usize_dec_to(self, self.identity.session_id);
        self.append_bytes(b" source=");
        self.append_bytes(source.as_bytes());
        if process_id != 0 || task_id != 0 {
            self.append_bytes(b" pid=");
            append_usize_dec_to(self, process_id);
            self.append_bytes(b" task=");
            append_usize_dec_to(self, task_id);
        }
        self.append_bytes(b"\n");
    }
}

/// Snapshot of retained kernel-log metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    /// Boot/session identity for this diagnostic stream.
    pub identity: DiagnosticIdentity,
    /// Ring capacity in bytes.
    pub capacity_bytes: usize,
    /// Retained bytes.
    pub retained_bytes: usize,
    /// Bytes dropped due to overwrite.
    pub dropped_bytes: usize,
    /// Next structured event sequence number.
    pub next_event_seq: usize,
    /// Structured event records retained.
    pub retained_events: usize,
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

/// Installs the boot/session identity for subsequent diagnostics.
pub fn install_identity(identity: DiagnosticIdentity) {
    with_log(|log| log.set_identity(identity));
}

/// Returns the current diagnostic boot/session identity.
#[must_use]
pub fn identity() -> DiagnosticIdentity {
    with_log(|log| log.identity)
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

/// Appends one structured event row.
pub fn append_event(component: &'static str, severity: &'static str, kind: &'static str) {
    append_event_with_source_context("kernel", component, severity, kind, 0, 0);
}

/// Appends one structured event row with process/task context.
pub fn append_event_with_context(
    component: &'static str,
    severity: &'static str,
    kind: &'static str,
    process_id: usize,
    task_id: usize,
) {
    append_event_with_source_context("process", component, severity, kind, process_id, task_id);
}

/// Appends one structured event row with source and process/task context.
pub fn append_event_with_source_context(
    source: &'static str,
    component: &'static str,
    severity: &'static str,
    kind: &'static str,
    process_id: usize,
    task_id: usize,
) {
    with_log(|log| log.append_event_record(source, component, severity, kind, process_id, task_id));
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

/// Returns log metadata.
#[must_use]
pub fn stats() -> Stats {
    with_log(|log| Stats {
        identity: log.identity,
        capacity_bytes: log.buf.len(),
        retained_bytes: log.len,
        dropped_bytes: log.dropped,
        next_event_seq: log.next_event_seq,
        retained_events: retained_event_count(log),
    })
}

/// Copies retained structured event records into `out`, returning the count.
pub fn snapshot_events(out: &mut [EventRecord]) -> usize {
    with_log(|log| {
        let retained = retained_event_count(log);
        let total = log.next_event_seq.saturating_sub(1);
        let first_seq = total.saturating_sub(retained).saturating_add(1);
        let mut written = 0usize;
        let mut offset = 0usize;
        while offset < retained && written < out.len() {
            let seq = first_seq + offset;
            let slot = (seq.saturating_sub(1)) % log.events.len();
            let record = log.events[slot];
            if record.seq == seq {
                out[written] = record;
                written += 1;
            }
            offset += 1;
        }
        written
    })
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

fn append_usize_dec_to(log: &mut KernelLog, value: usize) {
    let mut buf = [0u8; 20];
    let len = usize_to_dec(value, &mut buf);
    log.append_bytes(&buf[..len]);
}

fn retained_event_count(log: &KernelLog) -> usize {
    core::cmp::min(log.next_event_seq.saturating_sub(1), log.events.len())
}

#[cfg(feature = "selftest")]
#[path = "klog_tests.rs"]
mod tests;
