//! Product-facing scheduler and clock control vocabulary.
//!
//! Upper layers receive these function tables from composition roots. Concrete
//! implementations live below the system-kernel bridge.

#![no_std]

use {
    core::sync::atomic::AtomicU32,
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr},
};

/// Failure to spawn a detached thread through an injected scheduler service.
///
/// ```rust
/// use reovim_uapi_sched::SpawnError;
///
/// assert_eq!(SpawnError::OutOfMemory, SpawnError::OutOfMemory);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    /// The service could not allocate or retain the thread entry state.
    OutOfMemory,
    /// The lower scheduler refused the spawn. The code is diagnostic only and
    /// is intentionally not interpreted by this up-face crate.
    Refused(i32),
}

/// Product-facing scheduler syscall error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SchedulerError(i32);

impl SchedulerError {
    /// The current process blocked with retained replayable syscall state.
    pub const BUSY: Self = Self(SyscallError::BUSY.code());

    /// Builds a scheduler error from a bridge/provider code.
    #[must_use]
    pub const fn new(code: i32) -> Self {
        Self(code)
    }

    /// Returns the diagnostic bridge/provider code.
    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }
}

/// Bounded scheduler tick count for cooperative sleeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SchedulerTicks(usize);

impl SchedulerTicks {
    /// Builds a scheduler tick duration.
    #[must_use]
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Returns the raw Reovim scheduler tick count.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }

    /// Returns whether the duration has no ticks.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Scheduler control backed by the raw Reovim syscall transport.
///
/// This adapter keeps scheduler semantics in `uapi::sched`: callers request
/// cooperative yield or sleep, and only this wrapper packs raw syscall numbers
/// and scalar arguments.
#[derive(Debug, Clone, Copy)]
pub struct SyscallSchedulerControl {
    raw: RawSyscall,
}

impl SyscallSchedulerControl {
    /// Creates a scheduler control adapter over the raw syscall transport.
    #[must_use]
    pub const fn new(raw: RawSyscall) -> Self {
        Self { raw }
    }

    /// Cooperatively yields the current process.
    ///
    /// Returns `true` when another ready process ran before the caller resumed,
    /// and `false` when no peer was ready.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when there is no current process, the current
    /// process is not running, or the raw transport reports failure.
    pub fn yield_now(self) -> Result<bool, SchedulerError> {
        self.raw
            .invoke(SyscallNr::YIELD_NOW, SyscallArgs::EMPTY)
            .decode()
            .and_then(|value| match value {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(SyscallError::INVALID_ARGUMENT),
            })
            .map_err(scheduler_error_from_syscall)
    }

    /// Sleeps the current process for a bounded number of scheduler ticks.
    ///
    /// The returned value is the global scheduler tick count observed after the
    /// process wakes and resumes.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when `ticks` is zero, there is no current
    /// process, the current process is not running, or the raw transport reports
    /// failure.
    pub fn sleep_for_ticks(self, ticks: SchedulerTicks) -> Result<usize, SchedulerError> {
        if ticks.is_zero() {
            return Err(scheduler_error_from_syscall(SyscallError::INVALID_ARGUMENT));
        }
        self.raw
            .invoke(SyscallNr::SLEEP, SyscallArgs::new([ticks.raw(), 0, 0, 0, 0, 0]))
            .decode()
            .map_err(scheduler_error_from_syscall)
    }

    /// Charges one explicit scheduler tick to the current running process.
    ///
    /// The returned value is the global scheduler tick count observed after the
    /// tick is charged and any due sleeping processes are woken.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when there is no current process, the current
    /// process is not running, or the raw transport reports failure.
    pub fn tick_current(self) -> Result<usize, SchedulerError> {
        self.raw
            .invoke(SyscallNr::SCHED_TICK, SyscallArgs::EMPTY)
            .decode()
            .map_err(scheduler_error_from_syscall)
    }
}

/// Product-facing detached thread spawner.
///
/// The trait is generic so callers can pass normal Rust closures without
/// exposing raw thread-entry pointers or allocator details above the
/// system-kernel bridge.
///
/// ```rust
/// use reovim_uapi_sched::{DetachedThreadSpawner, SpawnError};
///
/// #[derive(Clone, Copy)]
/// struct InlineSpawner;
///
/// impl DetachedThreadSpawner for InlineSpawner {
///     fn spawn_detached<F>(self, f: F) -> Result<(), SpawnError>
///     where
///         F: FnOnce() + Send + 'static,
///     {
///         f();
///         Ok(())
///     }
/// }
///
/// InlineSpawner.spawn_detached(|| {}).unwrap();
/// ```
pub trait DetachedThreadSpawner: Copy + Send + Sync + 'static {
    /// Spawns a detached thread running `f`.
    ///
    /// # Errors
    ///
    /// Returns [`SpawnError`] when the scheduler cannot allocate or start the
    /// detached thread.
    fn spawn_detached<F>(self, f: F) -> Result<(), SpawnError>
    where
        F: FnOnce() + Send + 'static;
}

/// Function pointer for blocking while a sync word still equals `expected`.
///
/// ```rust
/// use core::sync::atomic::AtomicU32;
/// use reovim_uapi_sched::ParkFn;
///
/// fn park(_: &AtomicU32, _: u32) {}
/// let f: ParkFn = park;
/// f(&AtomicU32::new(0), 0);
/// ```
pub type ParkFn = fn(&AtomicU32, u32);

/// Function pointer for waking one waiter parked on a sync word.
///
/// ```rust
/// use core::sync::atomic::AtomicU32;
/// use reovim_uapi_sched::UnparkFn;
///
/// fn unpark(_: &AtomicU32) {}
/// let f: UnparkFn = unpark;
/// f(&AtomicU32::new(0));
/// ```
pub type UnparkFn = fn(&AtomicU32);

/// Function pointer for waking all waiters parked on a sync word.
///
/// ```rust
/// use core::sync::atomic::AtomicU32;
/// use reovim_uapi_sched::UnparkAllFn;
///
/// fn unpark_all(_: &AtomicU32) {}
/// let f: UnparkAllFn = unpark_all;
/// f(&AtomicU32::new(0));
/// ```
pub type UnparkAllFn = fn(&AtomicU32);

/// Product-facing sync park/unpark control table.
///
/// ```rust
/// use core::sync::atomic::AtomicU32;
/// use reovim_uapi_sched::SyncControl;
///
/// fn park(_: &AtomicU32, _: u32) {}
/// fn unpark(_: &AtomicU32) {}
/// fn unpark_all(_: &AtomicU32) {}
///
/// let word = AtomicU32::new(0);
/// let sync = SyncControl::new(park, unpark, unpark_all);
/// sync.park(&word, 0);
/// sync.unpark(&word);
/// sync.unpark_all(&word);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct SyncControl {
    /// Blocks while the word still equals the expected value.
    pub park_fn: ParkFn,
    /// Wakes one waiter.
    pub unpark_fn: UnparkFn,
    /// Wakes every waiter.
    pub unpark_all_fn: UnparkAllFn,
}

impl SyncControl {
    /// Creates a no-op sync control table.
    ///
    /// ```rust
    /// use core::sync::atomic::AtomicU32;
    /// use reovim_uapi_sched::SyncControl;
    ///
    /// let word = AtomicU32::new(0);
    /// SyncControl::noop().park(&word, 0);
    /// ```
    #[must_use]
    pub const fn noop() -> Self {
        Self::new(noop_park, noop_unpark, noop_unpark)
    }

    /// Creates a sync control table.
    ///
    /// ```rust
    /// use core::sync::atomic::AtomicU32;
    /// use reovim_uapi_sched::SyncControl;
    ///
    /// fn park(_: &AtomicU32, _: u32) {}
    /// fn unpark(_: &AtomicU32) {}
    /// fn unpark_all(_: &AtomicU32) {}
    ///
    /// let _sync = SyncControl::new(park, unpark, unpark_all);
    /// ```
    #[must_use]
    pub const fn new(park_fn: ParkFn, unpark_fn: UnparkFn, unpark_all_fn: UnparkAllFn) -> Self {
        Self {
            park_fn,
            unpark_fn,
            unpark_all_fn,
        }
    }

    /// Blocks while `word` still equals `expected`.
    ///
    /// ```rust
    /// use core::sync::atomic::AtomicU32;
    /// use reovim_uapi_sched::SyncControl;
    ///
    /// let word = AtomicU32::new(0);
    /// SyncControl::noop().park(&word, 0);
    /// ```
    pub fn park(self, word: &AtomicU32, expected: u32) {
        (self.park_fn)(word, expected);
    }

    /// Wakes one waiter parked on `word`.
    ///
    /// ```rust
    /// use core::sync::atomic::AtomicU32;
    /// use reovim_uapi_sched::SyncControl;
    ///
    /// let word = AtomicU32::new(0);
    /// SyncControl::noop().unpark(&word);
    /// ```
    pub fn unpark(self, word: &AtomicU32) {
        (self.unpark_fn)(word);
    }

    /// Wakes all waiters parked on `word`.
    ///
    /// ```rust
    /// use core::sync::atomic::AtomicU32;
    /// use reovim_uapi_sched::SyncControl;
    ///
    /// let word = AtomicU32::new(0);
    /// SyncControl::noop().unpark_all(&word);
    /// ```
    pub fn unpark_all(self, word: &AtomicU32) {
        (self.unpark_all_fn)(word);
    }
}

impl Default for SyncControl {
    fn default() -> Self {
        Self::noop()
    }
}

/// Function pointer for a monotonic nanosecond clock.
///
/// ```rust
/// use reovim_uapi_sched::MonotonicNowFn;
///
/// fn mono() -> i64 { 1 }
/// let f: MonotonicNowFn = mono;
/// assert_eq!(f(), 1);
/// ```
pub type MonotonicNowFn = fn() -> i64;

/// Function pointer for a wall-clock nanosecond clock.
///
/// ```rust
/// use reovim_uapi_sched::RealtimeNowFn;
///
/// fn realtime() -> i64 { 2 }
/// let f: RealtimeNowFn = realtime;
/// assert_eq!(f(), 2);
/// ```
pub type RealtimeNowFn = fn() -> i64;

/// Product-facing clock control table.
///
/// ```rust
/// use reovim_uapi_sched::ClockControl;
///
/// fn mono() -> i64 { 10 }
/// fn realtime() -> i64 { 20 }
/// let clock = ClockControl::new(mono, realtime);
/// assert_eq!(clock.monotonic(), 10);
/// assert_eq!(clock.realtime(), 20);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ClockControl {
    /// Reads monotonic nanoseconds.
    pub monotonic_fn: MonotonicNowFn,
    /// Reads realtime nanoseconds since the Unix epoch.
    pub realtime_fn: RealtimeNowFn,
}

impl ClockControl {
    /// Creates a no-op clock control table.
    ///
    /// ```rust
    /// use reovim_uapi_sched::ClockControl;
    ///
    /// assert_eq!(ClockControl::noop().monotonic(), 0);
    /// assert_eq!(ClockControl::noop().realtime(), 0);
    /// ```
    #[must_use]
    pub const fn noop() -> Self {
        Self::new(noop_i64, noop_i64)
    }

    /// Creates a clock control table.
    ///
    /// ```rust
    /// use reovim_uapi_sched::ClockControl;
    ///
    /// fn mono() -> i64 { 1 }
    /// fn realtime() -> i64 { 2 }
    /// let _clock = ClockControl::new(mono, realtime);
    /// ```
    #[must_use]
    pub const fn new(monotonic_fn: MonotonicNowFn, realtime_fn: RealtimeNowFn) -> Self {
        Self {
            monotonic_fn,
            realtime_fn,
        }
    }

    /// Reads monotonic nanoseconds.
    ///
    /// ```rust
    /// use reovim_uapi_sched::ClockControl;
    ///
    /// fn mono() -> i64 { 7 }
    /// fn realtime() -> i64 { 0 }
    /// assert_eq!(ClockControl::new(mono, realtime).monotonic(), 7);
    /// ```
    #[must_use]
    pub fn monotonic(self) -> i64 {
        (self.monotonic_fn)()
    }

    /// Reads realtime nanoseconds since the Unix epoch.
    ///
    /// ```rust
    /// use reovim_uapi_sched::ClockControl;
    ///
    /// fn mono() -> i64 { 0 }
    /// fn realtime() -> i64 { 9 }
    /// assert_eq!(ClockControl::new(mono, realtime).realtime(), 9);
    /// ```
    #[must_use]
    pub fn realtime(self) -> i64 {
        (self.realtime_fn)()
    }
}

impl Default for ClockControl {
    fn default() -> Self {
        Self::noop()
    }
}

/// Function pointer for reading the current thread id.
///
/// ```rust
/// use reovim_uapi_sched::CurrentThreadIdFn;
///
/// fn current() -> i64 { 42 }
/// let f: CurrentThreadIdFn = current;
/// assert_eq!(f(), 42);
/// ```
pub type CurrentThreadIdFn = fn() -> i64;

/// Product-facing thread identity control table.
///
/// ```rust
/// use reovim_uapi_sched::ThreadControl;
///
/// fn current() -> i64 { 42 }
/// let thread = ThreadControl::new(current);
/// assert_eq!(thread.current_id(), 42);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ThreadControl {
    /// Reads the current thread id.
    pub current_id_fn: CurrentThreadIdFn,
}

impl ThreadControl {
    /// Creates a no-op thread identity control table.
    ///
    /// ```rust
    /// use reovim_uapi_sched::ThreadControl;
    ///
    /// assert_eq!(ThreadControl::noop().current_id(), 0);
    /// ```
    #[must_use]
    pub const fn noop() -> Self {
        Self::new(noop_i64)
    }

    /// Creates a thread control table.
    ///
    /// ```rust
    /// use reovim_uapi_sched::ThreadControl;
    ///
    /// fn current() -> i64 { 1 }
    /// let _thread = ThreadControl::new(current);
    /// ```
    #[must_use]
    pub const fn new(current_id_fn: CurrentThreadIdFn) -> Self {
        Self { current_id_fn }
    }

    /// Reads the current thread id.
    ///
    /// ```rust
    /// use reovim_uapi_sched::ThreadControl;
    ///
    /// fn current() -> i64 { 7 }
    /// assert_eq!(ThreadControl::new(current).current_id(), 7);
    /// ```
    #[must_use]
    pub fn current_id(self) -> i64 {
        (self.current_id_fn)()
    }
}

impl Default for ThreadControl {
    fn default() -> Self {
        Self::noop()
    }
}

fn scheduler_error_from_syscall(error: SyscallError) -> SchedulerError {
    SchedulerError::new(error.code())
}

const fn noop_i64() -> i64 {
    0
}

fn noop_park(_: &AtomicU32, _: u32) {}

fn noop_unpark(_: &AtomicU32) {}
