//! Scheduler, clock, and thread-identity bridge.
//!
//! Upper layers receive `uapi/sched` function tables from composition roots.
//! This bridge owns the current handle-backed scheduler, clock, thread-id, and
//! sync implementations so pure data-structure code does not expose World
//! service helpers upward.

use {
    core::{alloc::Layout, sync::atomic::AtomicU32},
    reovim_kabi_platform::handle,
    reovim_uapi_sched::{
        ClockControl, DetachedThreadSpawner, SpawnError, SyncControl, ThreadControl,
    },
};

/// Returns the up-face clock control table backed by this bridge.
///
/// ```rust,no_run
/// use reovim_system_kernel::sched::clock_control;
///
/// let clock = clock_control();
/// let _ = clock.monotonic();
/// ```
#[must_use]
pub const fn clock_control() -> ClockControl {
    ClockControl::new(monotonic, realtime)
}

/// Returns the up-face thread identity control table backed by this bridge.
///
/// ```rust,no_run
/// use reovim_system_kernel::sched::thread_control;
///
/// let thread = thread_control();
/// let _ = thread.current_id();
/// ```
#[must_use]
pub const fn thread_control() -> ThreadControl {
    ThreadControl::new(current_id)
}

/// Returns the up-face sync park/unpark control table backed by this bridge.
///
/// ```rust,no_run
/// use core::sync::atomic::AtomicU32;
/// use reovim_system_kernel::sched::sync_control;
///
/// let word = AtomicU32::new(0);
/// sync_control().unpark(&word);
/// ```
#[must_use]
pub const fn sync_control() -> SyncControl {
    SyncControl::new(park, unpark, unpark_all)
}

/// Installs this bridge's sync control table into `lib/ds`.
///
/// # Errors
///
/// Returns [`reovim_lib_ds::sync_backend::SyncBackendInstallError`] when the
/// process already installed a sync backend.
pub fn install_lib_ds_sync_backend()
-> Result<(), reovim_lib_ds::sync_backend::SyncBackendInstallError> {
    reovim_lib_ds::sync_backend::install(sync_control())
}

/// System-kernel implementation of the product-facing detached thread spawner.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemThreadSpawner;

impl DetachedThreadSpawner for SystemThreadSpawner {
    fn spawn_detached<F>(self, f: F) -> Result<(), SpawnError>
    where
        F: FnOnce() + Send + 'static,
    {
        spawn_detached(f)
    }
}

/// Returns the up-face detached thread spawner backed by this bridge.
///
/// ```rust,no_run
/// use reovim_system_kernel::sched::thread_spawner;
/// use reovim_uapi_sched::DetachedThreadSpawner;
///
/// thread_spawner().spawn_detached(|| {}).expect("spawn succeeds");
/// ```
#[must_use]
pub const fn thread_spawner() -> SystemThreadSpawner {
    SystemThreadSpawner
}

fn monotonic() -> i64 {
    handle().clock()
}

fn realtime() -> i64 {
    handle().realtime()
}

fn current_id() -> i64 {
    i64::from(handle().thread_id())
}

fn park(word: &AtomicU32, expected: u32) {
    reovim_kabi_platform::handle().park(word, expected);
}

fn unpark(word: &AtomicU32) {
    reovim_kabi_platform::handle().unpark(word);
}

fn unpark_all(word: &AtomicU32) {
    handle().unpark_all(word);
}

unsafe extern "C" fn trampoline<F: FnOnce()>(arg: *mut u8) {
    let boxed = arg.cast::<F>();
    // SAFETY: `arg` is the closure handle `spawn_detached` produced: either a
    // heap box for a non-ZST `F`, or a dangling-aligned pointer for a ZST. The
    // spawning thread never touches it after a successful spawn.
    let f = unsafe { core::ptr::read(boxed) };
    if core::mem::size_of::<F>() != 0 {
        // SAFETY: for a non-ZST `F`, `boxed` is the unique live allocation that
        // held the closure. The value was moved out above, so only the backing
        // storage remains to free.
        unsafe {
            handle()
                .dealloc(core::ptr::NonNull::new_unchecked(boxed.cast::<u8>()), Layout::new::<F>());
        }
    }
    f();
}

fn spawn_detached<F>(f: F) -> Result<(), SpawnError>
where
    F: FnOnce() + Send + 'static,
{
    if core::mem::size_of::<F>() == 0 {
        let arg = core::ptr::NonNull::<F>::dangling().as_ptr().cast::<u8>();
        let _ = f;
        // SAFETY: `trampoline::<F>` reconstructs a zero-sized `F` from the
        // aligned dangling pointer and frees no storage.
        let result = unsafe { handle().thread_spawn(trampoline::<F>, arg) };
        return result.map(|_| ()).map_err(map_spawn_errno);
    }

    let layout = Layout::new::<F>();
    let boxed = handle()
        .alloc(layout)
        .map_err(|_| SpawnError::OutOfMemory)?
        .cast::<F>();
    // SAFETY: `boxed` is fresh storage of the correct size and alignment for
    // `F`; ownership transfers to the child thread after a successful spawn.
    unsafe {
        core::ptr::write(boxed.as_ptr(), f);
    }

    // SAFETY: `trampoline::<F>` consumes exactly the closure box passed as
    // `arg`; the child thread owns that box on success.
    let result = unsafe { handle().thread_spawn(trampoline::<F>, boxed.as_ptr().cast::<u8>()) };
    match result {
        Ok(_tid) => Ok(()),
        Err(errno) => {
            // SAFETY: the thread did not start, so this thread still uniquely
            // owns the initialized closure box and must reclaim it.
            unsafe {
                core::ptr::drop_in_place(boxed.as_ptr());
                handle().dealloc(boxed.cast::<u8>(), layout);
            }
            Err(map_spawn_errno(errno))
        }
    }
}

const fn map_spawn_errno(errno: reovim_kabi_platform::Errno) -> SpawnError {
    SpawnError::Refused(errno.code())
}
