//! Behavioral tests for `uapi/sched` function-table dispatch.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use {
    reovim_uapi_sched::{
        ClockControl, DetachedThreadSpawner, SchedulerError, SchedulerTicks, SpawnError,
        SyncControl, SyscallSchedulerControl, ThreadControl,
    },
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr, SyscallRet},
};

#[derive(Clone, Copy)]
struct InlineSpawner;

impl DetachedThreadSpawner for InlineSpawner {
    fn spawn_detached<F>(self, f: F) -> Result<(), SpawnError>
    where
        F: FnOnce() + Send + 'static,
    {
        f();
        Ok(())
    }
}

fn monotonic() -> i64 {
    11
}

fn realtime() -> i64 {
    22
}

fn current_id() -> i64 {
    33
}

fn park(_: &AtomicU32, _: u32) {
    PARKS.fetch_add(1, Ordering::Relaxed);
}

fn unpark(_: &AtomicU32) {
    UNPARKS.fetch_add(1, Ordering::Relaxed);
}

fn unpark_all(_: &AtomicU32) {
    UNPARK_ALLS.fetch_add(1, Ordering::Relaxed);
}

static PARKS: AtomicUsize = AtomicUsize::new(0);
static UNPARKS: AtomicUsize = AtomicUsize::new(0);
static UNPARK_ALLS: AtomicUsize = AtomicUsize::new(0);

#[test]
fn clock_control_dispatches_function_pointers() {
    let clock = ClockControl::new(monotonic, realtime);
    assert_eq!(clock.monotonic(), 11);
    assert_eq!(clock.realtime(), 22);
}

#[test]
fn thread_control_dispatches_current_id() {
    let thread = ThreadControl::new(current_id);
    assert_eq!(thread.current_id(), 33);
}

#[test]
fn sync_control_dispatches_function_pointers() {
    let word = AtomicU32::new(0);
    let sync = SyncControl::new(park, unpark, unpark_all);

    PARKS.store(0, Ordering::Relaxed);
    UNPARKS.store(0, Ordering::Relaxed);
    UNPARK_ALLS.store(0, Ordering::Relaxed);

    sync.park(&word, 0);
    sync.unpark(&word);
    sync.unpark_all(&word);

    assert_eq!(PARKS.load(Ordering::Relaxed), 1);
    assert_eq!(UNPARKS.load(Ordering::Relaxed), 1);
    assert_eq!(UNPARK_ALLS.load(Ordering::Relaxed), 1);
}

#[test]
fn default_controls_are_noops() {
    assert_eq!(ClockControl::default().monotonic(), 0);
    assert_eq!(ClockControl::default().realtime(), 0);
    assert_eq!(ThreadControl::default().current_id(), 0);
    SyncControl::default().park(&AtomicU32::new(0), 0);
}

#[test]
fn detached_thread_spawner_dispatches_closure() {
    static RAN: AtomicBool = AtomicBool::new(false);

    RAN.store(false, Ordering::Relaxed);
    InlineSpawner
        .spawn_detached(|| RAN.store(true, Ordering::Relaxed))
        .expect("inline spawn succeeds");

    assert!(RAN.load(Ordering::Relaxed));
}

#[test]
fn scheduler_syscall_scalars_are_stable() {
    assert_eq!(SchedulerTicks::new(5).raw(), 5);
    assert!(!SchedulerTicks::new(5).is_zero());
    assert!(SchedulerTicks::new(0).is_zero());
    assert_eq!(SchedulerError::new(6).code(), 6);
}

#[test]
fn syscall_scheduler_control_lowers_yield_sleep_and_tick_through_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        match nr {
            SyscallNr::YIELD_NOW => {
                assert_eq!(args, SyscallArgs::EMPTY);
                SyscallRet::success(1)
            }
            SyscallNr::SLEEP => {
                assert_eq!(args.get(0), Some(3));
                SyscallRet::success(11)
            }
            SyscallNr::SCHED_TICK => {
                assert_eq!(args, SyscallArgs::EMPTY);
                SyscallRet::success(12)
            }
            _ => SyscallRet::failure(SyscallError::INVALID_ARGUMENT),
        }
    }

    let sched = SyscallSchedulerControl::new(RawSyscall::new(syscall));

    assert_eq!(sched.yield_now(), Ok(true));
    assert_eq!(sched.sleep_for_ticks(SchedulerTicks::new(3)), Ok(11));
    assert_eq!(sched.tick_current(), Ok(12));
}

#[test]
fn syscall_scheduler_control_maps_transport_errors() {
    fn syscall(_: SyscallNr, _: SyscallArgs) -> SyscallRet {
        SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS)
    }

    let sched = SyscallSchedulerControl::new(RawSyscall::new(syscall));

    assert_eq!(
        sched.yield_now(),
        Err(SchedulerError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        sched.sleep_for_ticks(SchedulerTicks::new(1)),
        Err(SchedulerError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        sched.sleep_for_ticks(SchedulerTicks::new(0)),
        Err(SchedulerError::new(SyscallError::INVALID_ARGUMENT.code())),
    );
    assert_eq!(
        sched.tick_current(),
        Err(SchedulerError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
}
