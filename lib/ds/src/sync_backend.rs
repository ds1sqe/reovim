//! Sync park/unpark backend for `Mutex`, `RwLock`, and `Condvar`.
//!
//! The sync algorithms remain in `lib/ds`, but their blocking primitive is
//! injected as an up-face `SyncControl` table instead of reaching directly into
//! the down-face platform handle.

use core::{
    cell::UnsafeCell,
    sync::atomic::{
        AtomicU8, AtomicU32,
        Ordering::{AcqRel, Acquire, Release},
    },
};

use reovim_uapi_sched::SyncControl;

const UNINSTALLED: u8 = 0;
const INSTALLING: u8 = 1;
const INSTALLED: u8 = 2;

/// Error returned when the sync backend was already installed.
///
/// ```rust
/// use reovim_lib_ds::sync_backend::SyncBackendInstallError;
///
/// assert_eq!(
///     SyncBackendInstallError::AlreadyInstalled,
///     SyncBackendInstallError::AlreadyInstalled
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncBackendInstallError {
    /// A control table has already been installed.
    AlreadyInstalled,
}

struct ControlCell(UnsafeCell<SyncControl>);

// SAFETY: writes happen only during the single successful install transition
// (`UNINSTALLED -> INSTALLING -> INSTALLED`). Readers copy the table after an
// Acquire load observes `INSTALLED`.
unsafe impl Sync for ControlCell {}

static CONTROL: ControlCell = ControlCell(UnsafeCell::new(SyncControl::noop()));
static STATE: AtomicU8 = AtomicU8::new(UNINSTALLED);

/// Installs the process-wide sync backend used by futex-backed DS primitives.
///
/// # Errors
///
/// Returns [`SyncBackendInstallError::AlreadyInstalled`] if another backend was
/// already installed.
///
/// ```rust,no_run
/// use reovim_lib_ds::sync_backend::install;
/// use reovim_uapi_sched::SyncControl;
///
/// // no_run: mutates the process-wide sync backend.
/// let _ = install(SyncControl::noop());
/// ```
pub fn install(control: SyncControl) -> Result<(), SyncBackendInstallError> {
    if STATE
        .compare_exchange(UNINSTALLED, INSTALLING, AcqRel, Acquire)
        .is_err()
    {
        return Err(SyncBackendInstallError::AlreadyInstalled);
    }

    // SAFETY: this is the only writer after the state transition above, and
    // readers do not use the cell as installed until the Release store below.
    unsafe {
        *CONTROL.0.get() = control;
    }
    STATE.store(INSTALLED, Release);
    Ok(())
}

pub(crate) fn park(word: &AtomicU32, expected: u32) {
    control().park(word, expected);
}

pub(crate) fn unpark(word: &AtomicU32) {
    control().unpark(word);
}

pub(crate) fn unpark_all(word: &AtomicU32) {
    control().unpark_all(word);
}

fn control() -> SyncControl {
    if STATE.load(Acquire) == INSTALLED {
        // SAFETY: observing `INSTALLED` pairs with the install Release store,
        // so the table write is visible and no later writer exists.
        unsafe { *CONTROL.0.get() }
    } else {
        SyncControl::noop()
    }
}
