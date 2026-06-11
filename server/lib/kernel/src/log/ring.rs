//! `LogRing` — the kernel log ring (LOG6, 9.5 §8).
//!
//! A bounded ring of rendered LOG2 entries over `arch::ds::Ring`. Each entry
//! carries its [`LogLevel`] alongside the pre-rendered [`Bytes`] so readers
//! can filter without parsing. Capacity comes from
//! [`LauncherArgs::log_ring_bytes`] (default 1 MiB; LOG6 spec default).
//!
//! ## Sizing
//!
//! The ring is sized by bytes, not entry count. Since each entry's size varies
//! with the length of its rendered line, we approximate a worst-case entry
//! size to compute the slot count:
//!
//! - Maximum LOG2 line: timestamp (14 bytes) + separators (~10 bytes) +
//!   max subsystem name (~20 bytes) + max message (4096 bytes LOG4 cap) +
//!   newline. We use a conservative `MAX_ENTRY_BYTES = 4200`.
//! - Minimum capacity: 2 entries (so the ring-wraps test can exercise eviction).
//!
//! ## LOG1: one rendering
//!
//! `LogRing` calls the LOG2 renderer on every push. The same rendered bytes go
//! into the ring entry AND are the bytes written by the file sink and early
//! stderr. There is no second rendering, no second buffer, no second pipeline.
//!
//! ## Boot-stage position
//!
//! The ring is allocated in boot stage 0 (before any subscriber/sink). It is
//! the DS12 bus's always-present built-in subscriber, stored as
//! `Shared<LogRing>` directly on the bus — no raw-pointer indirection. Every
//! event from stage 0 onward is captured (LOG6).
//!
//! ## LOG8: early stderr gate
//!
//! After appending each entry, `push_event` calls `sink::stderr_echo` with the
//! rendered bytes. That function writes to fd 2 when the file sink is not yet
//! open, or when the headless flag is set after open. This is the one-way
//! ring→sink gate: the sink's ring replay (sink→ring) is the other direction
//! and the two do not form a cycle at runtime.

use {
    reovim_arch::{
        alloc::AllocError,
        ds::{Bytes, Ring},
        sync::Mutex,
    },
    reovim_uapi_abi::error::LogLevel,
};

use crate::{
    event_bus::DS12Event,
    log::{
        flush,
        render::{EmitterAddress, InstanceAddress, RenderInput, render_line},
        sink,
    },
};

// ── Sizing constants ──────────────────────────────────────────────────────────

/// Conservative worst-case per-entry byte count for capacity calculation.
///
/// LOG4 caps `message` at 4096 bytes. Adding timestamp (14 bytes), separators
/// (~20 bytes), and max subsystem name (~20 bytes) gives a conservative
/// ceiling of 4200 bytes per entry.
///
/// ```rust
/// use reovim_kernel::log::ring::MAX_ENTRY_BYTES;
/// assert!(MAX_ENTRY_BYTES >= 128);
/// ```
pub const MAX_ENTRY_BYTES: usize = 4_200;

/// Minimum number of ring slots (log entries).
///
/// Ensures the ring can hold at least two entries so the oldest-first eviction
/// path (LOG6) is reachable by tests.
///
/// ```rust
/// use reovim_kernel::log::ring::MIN_RING_CAPACITY;
/// assert!(MIN_RING_CAPACITY >= 2);
/// ```
pub const MIN_RING_CAPACITY: usize = 2;

// ── LogEntry ─────────────────────────────────────────────────────────────────

/// One entry in the kernel log ring (LOG6).
///
/// Carries the log level alongside the pre-rendered LOG2 bytes so readers can
/// filter by level without parsing the rendered line.
///
/// ```rust
/// use reovim_arch::ds::Bytes;
/// use reovim_kernel::log::ring::LogEntry;
/// use reovim_uapi_abi::error::LogLevel;
///
/// let entry = LogEntry {
///     level: LogLevel::Info,
///     line: Bytes::try_from_slice(b"[    0.000001] kernel boot: test\n").unwrap(),
/// };
/// assert_eq!(entry.level, LogLevel::Info);
/// assert!(!entry.line.is_empty());
/// ```
pub struct LogEntry {
    /// Log severity (6.3 §2.4). Stored alongside rendered bytes so ring
    /// readers can filter without parsing.
    pub level: LogLevel,
    /// Pre-rendered LOG2 line including the trailing `\n`.
    pub line: Bytes,
}

// ── LogRing ───────────────────────────────────────────────────────────────────

/// The kernel log ring (LOG6, 9.5 §8).
///
/// Allocated once in boot stage 0, before any subscriber or sink. Every DS12
/// event from stage 0 onward is rendered (LOG2) and appended here as a
/// [`LogEntry`]. Capacity is derived from [`LauncherArgs::log_ring_bytes`].
///
/// Use [`LogRing::try_new`] to allocate. Wire it into the bus by passing a
/// `Shared<LogRing>` clone to [`DS12EventBus::set_builtin`].
///
/// [`DS12EventBus`]: crate::event_bus::DS12EventBus
/// [`DS12EventBus::set_builtin`]: crate::event_bus::DS12EventBus::set_builtin
/// [`LauncherArgs::log_ring_bytes`]: crate::init::LauncherArgs::log_ring_bytes
///
/// # Example
///
/// ```rust
/// use reovim_kernel::log::ring::LogRing;
///
/// let ring = LogRing::try_new(1024 * 1024).expect("1 MiB ring");
/// assert_eq!(ring.len(), 0);
/// ```
pub struct LogRing {
    /// The backing ring of entries, under a `Mutex` for interior-mutability
    /// access from `push_event` called through `&Self`.
    ///
    /// CC6: this lock is distinct from the bus's `RwLock<Seq<Subscriber>>`.
    /// The bus invokes the built-in path before acquiring its own read lock,
    /// so there is no lock-order dependency.
    inner: Mutex<Ring<LogEntry>>,
    /// Slot count fixed at construction.
    capacity: usize,
}

impl LogRing {
    /// Allocates a new empty `LogRing` with capacity derived from `ring_bytes`.
    ///
    /// The slot count is `max(ring_bytes / MAX_ENTRY_BYTES, MIN_RING_CAPACITY)`.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] if the backing `Ring` allocation fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::log::ring::LogRing;
    ///
    /// let ring = LogRing::try_new(1024 * 1024).expect("1 MiB ring allocation");
    /// assert_eq!(ring.len(), 0);
    /// assert!(ring.capacity() >= 2);
    /// ```
    pub fn try_new(ring_bytes: usize) -> Result<Self, AllocError> {
        let cap = (ring_bytes / MAX_ENTRY_BYTES).max(MIN_RING_CAPACITY);
        let inner = Ring::try_with_capacity(cap)?;
        Ok(Self {
            inner: Mutex::new(inner),
            capacity: cap,
        })
    }

    /// The number of live entries.
    ///
    /// ```rust
    /// use reovim_kernel::log::ring::LogRing;
    ///
    /// let ring = LogRing::try_new(4096).unwrap();
    /// assert_eq!(ring.len(), 0);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    /// Whether the ring holds no entries.
    ///
    /// ```rust
    /// use reovim_kernel::log::ring::LogRing;
    ///
    /// let ring = LogRing::try_new(4096).unwrap();
    /// assert!(ring.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }

    /// The fixed slot capacity of the ring.
    ///
    /// ```rust
    /// use reovim_kernel::log::ring::LogRing;
    ///
    /// let ring = LogRing::try_new(1024 * 1024).unwrap();
    /// assert!(ring.capacity() >= 2);
    /// ```
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Renders `event` via the LOG2 renderer, pushes it into the ring, then
    /// calls the LOG8 stderr gate in `sink` and updates the panic-mirror region.
    ///
    /// If rendering or allocation fails the event is silently dropped —
    /// a ring push is best-effort (the ring is an in-memory cache; a failed
    /// render is not a `log.sink.fail` event — that is the file sink's concern).
    ///
    /// After rendering, `sink::stderr_echo` is called and the §9.1 panic-mirror
    /// region is updated via `flush::update_after_push`. Both happen before the
    /// ring push so the ring lock is not held during the flush update. This lets
    /// `update_after_push` call `for_each` (which takes the ring lock) safely
    /// during compaction — there is no lock re-entrancy. The current event's
    /// rendered bytes are always written into the mirror on both the normal and
    /// overflow paths before the ring push stores the entry.
    ///
    /// Then the entry is pushed into the ring. The only call direction from ring
    /// to sink is via `stderr_echo`; the sink's replay path calls `for_each` —
    /// those two paths do not execute concurrently and do not form a runtime cycle.
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::log::ring::LogRing;
    /// use reovim_kernel::event_bus::{DS12Event, BootStageFields, EVT_BOOT_STAGE_OK};
    /// use reovim_kernel::BootClock;
    /// use reovim_uapi_abi::error::LogLevel;
    ///
    /// let ring = LogRing::try_new(1024 * 1024).unwrap();
    /// let clock = BootClock::capture();
    /// let event = DS12Event {
    ///     ts_nanos: clock.elapsed_nanos(),
    ///     level: LogLevel::Info,
    ///     event: EVT_BOOT_STAGE_OK,
    ///     fields: BootStageFields { stage: 1, error_code: None },
    /// };
    /// ring.push_event(&event);
    /// assert_eq!(ring.len(), 1);
    /// ```
    pub fn push_event(&self, event: &DS12Event) {
        let subsystem = kernel_subsystem_from_event(event.event);
        let input = RenderInput {
            ts_nanos: event.ts_nanos,
            emitter_pkg: "kernel",
            emitter_addr: EmitterAddress::Kernel { subsystem },
            instance: InstanceAddress::None,
            message: event.event,
            level: event.level,
        };
        let Ok(line) = render_line(&input) else {
            return;
        };
        // Write to stderr gate (LOG8) — no ring lock held.
        sink::stderr_echo(line.as_slice());

        // Update the §9.1 panic-mirror before the ring push (ring lock not held
        // yet, so `for_each` inside compaction is safe). The current `line` is
        // appended to the mirror on both the normal and overflow paths (the
        // overflow path calls `compact_into_buf` first, then appends `line`).
        flush::update_after_push(line.as_slice(), self);

        let entry = LogEntry {
            level: event.level,
            line,
        };
        // Oldest-first eviction when full (LOG6); evicted entry is dropped.
        let _ = self.inner.lock().push(entry);
    }

    /// Calls `f` with each live entry in oldest-first order while holding the
    /// ring lock.
    ///
    /// Used by the file sink to replay the ring head on open (LOG8).
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::log::ring::LogRing;
    /// use reovim_kernel::event_bus::{DS12Event, BootStageFields, EVT_BOOT_STAGE_OK};
    /// use reovim_kernel::BootClock;
    /// use reovim_uapi_abi::error::LogLevel;
    ///
    /// let ring = LogRing::try_new(1024 * 1024).unwrap();
    /// let event = DS12Event {
    ///     ts_nanos: 1_000_000,
    ///     level: LogLevel::Info,
    ///     event: EVT_BOOT_STAGE_OK,
    ///     fields: BootStageFields { stage: 1, error_code: None },
    /// };
    /// ring.push_event(&event);
    ///
    /// let mut count = 0usize;
    /// ring.for_each(|_entry| { count += 1; });
    /// assert_eq!(count, 1);
    /// ```
    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&LogEntry),
    {
        let guard = self.inner.lock();
        for entry in guard.iter() {
            f(entry);
        }
    }
}

/// Maps a dotted OBS1 event name to the kernel subsystem label for LOG2 §3.
///
/// The first segment of the dotted name is the family; we return a static
/// label for the families present in the boot-core crate. Later features
/// will add families (`config`, `pkg`, `persist`, …).
///
/// `pub(crate)` so `sink.rs` can use the same mapping without duplication.
pub(crate) fn kernel_subsystem_from_event(event: &str) -> &'static str {
    if event.starts_with("boot.") {
        "boot"
    } else if event.starts_with("log.") {
        "log"
    } else {
        "kernel"
    }
}

// L12 layout: tests in sibling ring_tests.rs, declared in log/mod.rs.
