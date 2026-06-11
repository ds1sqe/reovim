//! `DS12EventBus` — the kernel DS12 event fan-out bus (9.4, OBS1, CC6).
//!
//! ## Design
//!
//! The bus holds a subscriber list behind an `arch::sync::RwLock`. `emit()`
//! uses clone-then-invoke (CC6): it copies the current subscriber set under
//! the read lock, drops the lock, then invokes each subscriber — so no lock
//! is held across any subscriber callback. A subscriber that calls
//! `subscribe()` from inside its own callback therefore cannot deadlock; it
//! acquires the write lock independently after the calling `emit()` already
//! dropped the read lock.
//!
//! ## Built-in LOG1 seam
//!
//! Per LOG1, the log ring is the always-present built-in subscriber slot.
//! `DS12EventBus::set_builtin` receives a `Shared<LogRing>` clone and stores
//! it directly — no raw-pointer indirection. The slot is written once before
//! any event is emitted (boot is `&mut Init`, single-threaded) and is
//! thereafter read-only, so no lock is required.
//!
//! ## Subscriber ordering
//!
//! Subscribers receive events in registration order. The built-in slot always
//! fires first (when present), followed by the registered list in push order.
//!
//! ## `no_std` types
//!
//! Per the 9.4 §1 schema-view note: `ts` is derived from the kernel
//! `BootClock` (7.5 §4); `fields` is a fixed structured set (not a
//! `HashMap`). `SystemTime` and `HashMap` are `std` types absent from
//! the kernel floor.

use {
    reovim_arch::{
        ds::{Seq, Shared},
        sync::RwLock,
    },
    reovim_uapi_abi::error::LogLevel,
};

use crate::log::ring::LogRing;

/// Spec-default subscriber capacity per kernel instance (9.4 §7).
///
/// Until the config service lands, the default is the capacity:
/// `kernel.host.[limits].observe-subscriber-capacity` arrives with the
/// config service.
///
/// See [`DS12EventBus::subscribe`] for usage.
///
/// ```rust
/// use reovim_kernel::event_bus::SUBSCRIBER_CAPACITY;
/// assert_eq!(SUBSCRIBER_CAPACITY, 64);
/// ```
pub const SUBSCRIBER_CAPACITY: usize = 64;

/// Why a [`DS12EventBus::subscribe`] call was refused.
///
/// # Example
///
/// ```rust
/// use reovim_kernel::event_bus::SubscribeError;
///
/// assert_ne!(SubscribeError::Capacity, SubscribeError::Alloc);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscribeError {
    /// The bus already holds [`SUBSCRIBER_CAPACITY`] subscribers (9.4 §7).
    Capacity,
    /// The backing `Seq` allocation failed.
    Alloc,
}

// ── DS12Event ────────────────────────────────────────────────────────────────

/// Structured fields carried by boot-stage events (9.4 §5 common fields).
///
/// Boot-stage events need the stage index; failure events additionally carry
/// an error code. The `ErrorCode` field is `None` for `boot.stage.start` and
/// `boot.stage.ok`, and `Some` for `boot.stage.fail`.
///
/// This is the `no_std` realization of the §5 structured field set per the
/// 9.4 §1 schema-view note: the kernel carries the same semantic information
/// as the schema-view fields without `HashMap`.
///
/// # Example
///
/// ```rust
/// use reovim_kernel::event_bus::BootStageFields;
///
/// let fields = BootStageFields { stage: 3, error_code: None };
/// assert_eq!(fields.stage, 3);
/// assert!(fields.error_code.is_none());
/// ```
#[derive(Debug, Clone, Copy)]
pub struct BootStageFields {
    /// The 0-indexed boot stage number (2.2 §1).
    pub stage: u8,
    /// Error code present only for `boot.stage.fail` events.
    pub error_code: Option<i32>,
}

/// A DS12 event (9.4 §1 schema view, `no_std` realization — see module doc `no_std` types note).
///
/// The schema-view `SystemTime`/`HashMap` fields are replaced by
/// `BootClock`-derived timestamps and a fixed structured field set. Every
/// emitted event satisfies its family schema (OBS1).
///
/// # Example
///
/// ```rust
/// use reovim_kernel::event_bus::{DS12Event, BootStageFields};
/// use reovim_kernel::LauncherArgs;
/// use reovim_kernel::BootClock;
/// use reovim_uapi_abi::error::LogLevel;
///
/// let clock = BootClock::capture();
/// let event = DS12Event {
///     ts_nanos: clock.elapsed_nanos(),
///     level: LogLevel::Info,
///     event: "boot.stage.start",
///     fields: BootStageFields { stage: 0, error_code: None },
/// };
/// assert_eq!(event.event, "boot.stage.start");
/// assert_eq!(event.fields.stage, 0);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct DS12Event {
    /// Nanoseconds since the boot clock anchor (7.5 §4).
    /// Replaces `SystemTime` in the schema view (see module doc `no_std` types note).
    pub ts_nanos: u64,
    /// Log severity level (6.3 §2.4).
    pub level: LogLevel,
    /// Dotted family.subject string from the OBS1 vocabulary (9.4 §2).
    pub event: &'static str,
    /// Structured boot-stage fields (9.4 §5, `no_std` fixed set — see module doc).
    pub fields: BootStageFields,
}

// ── DS12 event string constants (OBS1 vocabulary) ───────────────────────────

/// OBS1 `boot.stage.start` — emitted at the beginning of each boot stage.
///
/// ```rust
/// use reovim_kernel::event_bus::EVT_BOOT_STAGE_START;
/// assert!(EVT_BOOT_STAGE_START.starts_with("boot."));
/// ```
pub const EVT_BOOT_STAGE_START: &str = "boot.stage.start";

/// OBS1 `boot.stage.ok` — emitted on successful stage completion.
///
/// ```rust
/// use reovim_kernel::event_bus::EVT_BOOT_STAGE_OK;
/// assert!(EVT_BOOT_STAGE_OK.starts_with("boot."));
/// ```
pub const EVT_BOOT_STAGE_OK: &str = "boot.stage.ok";

/// OBS1 `boot.stage.fail` — emitted when a stage fails (aborts boot).
///
/// ```rust
/// use reovim_kernel::event_bus::EVT_BOOT_STAGE_FAIL;
/// assert!(EVT_BOOT_STAGE_FAIL.starts_with("boot."));
/// ```
pub const EVT_BOOT_STAGE_FAIL: &str = "boot.stage.fail";

// ── Subscriber type ──────────────────────────────────────────────────────────

/// A DS12 subscriber callback: a function pointer invoked for every event
/// emitted on the bus.
///
/// Function pointers are `Copy + Send + Sync`, satisfying the `Seq<T: Send>`
/// bound and the `RwLock<T: Send + Sync>` bound.
///
/// # Example
///
/// ```rust
/// use reovim_kernel::event_bus::{Subscriber, DS12Event};
///
/// fn my_handler(event: &DS12Event) {
///     let _ = event.event;
/// }
///
/// let _sub: Subscriber = my_handler;
/// ```
pub type Subscriber = fn(&DS12Event);

// ── DS12EventBus ─────────────────────────────────────────────────────────────

/// The kernel DS12 event fan-out bus (9.4, CC6).
///
/// Holds a subscriber set behind an `arch::sync::RwLock<Seq<Subscriber>>`.
/// `emit()` clones the subscriber set under the read lock (CC6: copy-then-
/// invoke, never holding the lock across a callback). The built-in LOG1 slot
/// (`builtin_ring`) fires first when set.
///
/// Created by `DS12EventBus::new()` in `Init::boot` before any stage runs.
/// Kept in `Kernel::event_bus` after the handoff.
///
/// # Example
///
/// ```rust
/// use reovim_kernel::event_bus::{DS12EventBus, DS12Event, BootStageFields, EVT_BOOT_STAGE_START};
/// use reovim_kernel::BootClock;
/// use reovim_uapi_abi::error::LogLevel;
///
/// let bus = DS12EventBus::new();
/// let clock = BootClock::capture();
/// let event = DS12Event {
///     ts_nanos: clock.elapsed_nanos(),
///     level: LogLevel::Info,
///     event: EVT_BOOT_STAGE_START,
///     fields: BootStageFields { stage: 0, error_code: None },
/// };
/// bus.emit(&event); // no subscribers yet — no-op
/// ```
pub struct DS12EventBus {
    /// Registered subscribers in push order (CC6: cloned under read lock,
    /// invoked after lock drop).
    subscribers: RwLock<Seq<Subscriber>>,

    /// The always-present built-in subscriber slot for the log ring (LOG1).
    ///
    /// Holds the `Shared<LogRing>` directly so `emit()` can call
    /// `ring.push_event` without any raw-pointer indirection. `None` until
    /// `set_builtin` is called in boot stage 0.
    ///
    /// Written once before any event is emitted (boot is `&mut Init`,
    /// single-threaded) and thereafter read-only, so no lock is required.
    builtin_ring: Option<Shared<LogRing>>,
}

impl DS12EventBus {
    /// Creates a new empty event bus.
    ///
    /// No subscribers are registered and the built-in LOG1 slot is `None`
    /// until `set_builtin` is called. Called once in `Init::boot` before stage 1.
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::event_bus::DS12EventBus;
    ///
    /// let bus = DS12EventBus::new();
    /// // Zero subscribers — emit is a no-op.
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            subscribers: RwLock::new(Seq::new()),
            builtin_ring: None,
        }
    }

    /// Sets the always-present built-in LOG1 ring.
    ///
    /// Called once during boot (under `&mut Init`, single-threaded) before
    /// any events are emitted. After this call every `emit()` pushes the
    /// event into the ring before invoking registered subscribers.
    ///
    /// Calling it a second time replaces the previous ring; only the
    /// boot-stage-0 wiring call in `Init::boot` is the intended caller
    /// (write-once in practice).
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_arch::ds::Shared;
    /// use reovim_kernel::event_bus::DS12EventBus;
    /// use reovim_kernel::log::ring::LogRing;
    ///
    /// let ring = Shared::try_new(LogRing::try_new(1024 * 1024).unwrap()).unwrap();
    /// let mut bus = DS12EventBus::new();
    /// bus.set_builtin(ring);
    /// ```
    pub fn set_builtin(&mut self, ring: Shared<LogRing>) {
        self.builtin_ring = Some(ring);
    }

    /// Registers a new subscriber.
    ///
    /// Subscribers receive events in registration order after the built-in
    /// slot. A subscriber registered during an `emit()` callback (from a
    /// different thread or re-entrantly) takes the write lock independently
    /// after the calling `emit()` has already dropped its read lock — no
    /// deadlock is possible (CC6).
    ///
    /// # Errors
    ///
    /// Returns [`SubscribeError::Capacity`] when the bus already holds
    /// [`SUBSCRIBER_CAPACITY`] subscribers, and [`SubscribeError::Alloc`]
    /// when the backing `Seq` allocation fails. Subscribers registered
    /// before the failure are unaffected.
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::event_bus::{DS12EventBus, DS12Event, Subscriber};
    ///
    /// fn noop(_event: &DS12Event) {}
    ///
    /// let bus = DS12EventBus::new();
    /// assert!(bus.subscribe(noop).is_ok());
    /// ```
    #[must_use = "if subscriber registration fails, the subscriber is silently not added"]
    pub fn subscribe(&self, subscriber: Subscriber) -> Result<(), SubscribeError> {
        let mut subs = self.subscribers.write();
        if subs.len() >= SUBSCRIBER_CAPACITY {
            return Err(SubscribeError::Capacity);
        }
        subs.try_push(subscriber).map_err(|_| SubscribeError::Alloc)
    }

    /// Emits `event` to all subscribers (CC6 clone-then-invoke).
    ///
    /// The subscriber list is copied under the read lock; the lock is dropped
    /// before any callback is invoked. This ensures no lock is held across a
    /// subscriber callback — a subscriber that calls `subscribe()` or even
    /// `emit()` from within its own callback cannot deadlock.
    ///
    /// Fan-out order: built-in LOG1 ring (if set) first, then registered
    /// subscribers in registration order.
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::event_bus::{DS12EventBus, DS12Event, BootStageFields, EVT_BOOT_STAGE_OK};
    /// use reovim_kernel::BootClock;
    /// use reovim_uapi_abi::error::LogLevel;
    /// use core::sync::atomic::{AtomicUsize, Ordering};
    ///
    /// static COUNT: AtomicUsize = AtomicUsize::new(0);
    /// fn counter(_e: &DS12Event) { COUNT.fetch_add(1, Ordering::Relaxed); }
    ///
    /// let bus = DS12EventBus::new();
    /// bus.subscribe(counter).unwrap();
    /// let clock = BootClock::capture();
    /// let event = DS12Event {
    ///     ts_nanos: clock.elapsed_nanos(),
    ///     level: LogLevel::Info,
    ///     event: EVT_BOOT_STAGE_OK,
    ///     fields: BootStageFields { stage: 0, error_code: None },
    /// };
    /// bus.emit(&event);
    /// assert_eq!(COUNT.load(Ordering::Relaxed), 1);
    /// ```
    pub fn emit(&self, event: &DS12Event) {
        // ── Step 1: push into the built-in LOG1 ring (no lock needed) ────────
        //
        // `builtin_ring` is written once under `&mut Init` (single-threaded
        // boot) before the bus is accessible to any thread. After that it is
        // read-only, so no lock is required here.
        if let Some(ring) = &self.builtin_ring {
            ring.push_event(event);
        }

        // ── Step 2: copy the subscriber list under the read lock (CC6) ──────
        //
        // We copy fn-pointer-sized values (8 bytes each on 64-bit) into a
        // fixed-capacity stack snapshot. The read lock is held only for the
        // duration of the copy, not across any callback.
        //
        // SUBSCRIBER_CAPACITY bounds the stack allocation; subscribe()
        // enforces the same bound, so the copy is always complete (no
        // silent skip).
        let mut snapshot = [emit_noop as Subscriber; SUBSCRIBER_CAPACITY];
        let n;
        {
            let guard = self.subscribers.read();
            let subs = &*guard;
            n = subs.len().min(SUBSCRIBER_CAPACITY);
            for i in 0..n {
                snapshot[i] = subs[i];
            }
            // Guard drops here — read lock released before any callback.
        }

        // ── Step 3: invoke each subscriber with no lock held (CC6) ──────────
        for sub in &snapshot[..n] {
            sub(event);
        }
    }
}

impl Default for DS12EventBus {
    fn default() -> Self {
        Self::new()
    }
}

// `DS12EventBus` is automatically `Send + Sync`:
// - `subscribers: RwLock<Seq<Subscriber>>`: function pointers are `Send + Sync`;
//   `Seq<fn>` is `Send + Sync`; `RwLock` auto-impls `Send + Sync` for those.
// - `builtin_ring: Option<Shared<LogRing>>`: `LogRing` is `Send + Sync`
//   (its `Mutex<Ring<LogEntry>>` is `Send + Sync`); `Shared<T: Send+Sync>` is
//   `Send + Sync`; `Option<Shared<LogRing>>` inherits those bounds.
// No raw pointers with aliasing; no `UnsafeCell` outside `RwLock`/`Mutex`.
// The compiler derives `Send + Sync` automatically; no manual impl needed.

/// A no-op subscriber used to zero-initialize the stack snapshot array.
/// Never registered; only fills the `[Subscriber; N]` literal.
#[inline(always)]
pub(crate) const fn emit_noop(_event: &DS12Event) {}

// L12 layout: tests in sibling event_bus_tests.rs, declared in lib.rs.
