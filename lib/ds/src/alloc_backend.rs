//! Allocation backend for heap-owning DS containers.
//!
//! The container algorithms remain in `lib/ds`, but their allocation primitive
//! is injected as an up-face `AllocControl` table instead of reaching directly
//! into the down-face platform handle.

use core::{
    alloc::Layout,
    cell::UnsafeCell,
    ptr::NonNull,
    sync::atomic::{
        AtomicU8,
        Ordering::{AcqRel, Acquire, Release},
    },
};

use reovim_uapi_mm::AllocControl;

pub use reovim_uapi_mm::AllocError;

const UNINSTALLED: u8 = 0;
const INSTALLING: u8 = 1;
const INSTALLED: u8 = 2;

/// Error returned when the allocation backend was already installed.
///
/// ```rust
/// use reovim_lib_ds::alloc_backend::AllocBackendInstallError;
///
/// assert_eq!(
///     AllocBackendInstallError::AlreadyInstalled,
///     AllocBackendInstallError::AlreadyInstalled
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocBackendInstallError {
    /// A control table has already been installed.
    AlreadyInstalled,
}

struct ControlCell(UnsafeCell<AllocControl>);

// SAFETY: writes happen only during the single successful install transition
// (`UNINSTALLED -> INSTALLING -> INSTALLED`). Readers copy the table after an
// Acquire load observes `INSTALLED`.
unsafe impl Sync for ControlCell {}

static CONTROL: ControlCell = ControlCell(UnsafeCell::new(AllocControl::noop()));
static STATE: AtomicU8 = AtomicU8::new(UNINSTALLED);

/// Installs the process-wide allocation backend used by heap-owning DS
/// containers.
///
/// # Errors
///
/// Returns [`AllocBackendInstallError::AlreadyInstalled`] if another backend
/// was already installed.
///
/// ```rust,no_run
/// use reovim_lib_ds::alloc_backend::install;
/// use reovim_uapi_mm::AllocControl;
///
/// // no_run: mutates the process-wide allocation backend.
/// let _ = install(AllocControl::noop());
/// ```
pub fn install(control: AllocControl) -> Result<(), AllocBackendInstallError> {
    if STATE
        .compare_exchange(UNINSTALLED, INSTALLING, AcqRel, Acquire)
        .is_err()
    {
        return Err(AllocBackendInstallError::AlreadyInstalled);
    }

    // SAFETY: this is the only writer after the state transition above, and
    // readers do not use the cell as installed until the Release store below.
    unsafe {
        *CONTROL.0.get() = control;
    }
    STATE.store(INSTALLED, Release);
    Ok(())
}

pub(crate) fn alloc(layout: Layout) -> Result<NonNull<u8>, AllocError> {
    control().allocate(layout)
}

/// Deallocates a block previously returned by [`alloc`].
///
/// # Safety
///
/// `ptr` and `layout` must name a live allocation returned by [`alloc`], and
/// the allocation must not be used after this call.
pub(crate) unsafe fn dealloc(ptr: NonNull<u8>, layout: Layout) {
    control().deallocate(ptr, layout);
}

fn control() -> AllocControl {
    if STATE.load(Acquire) == INSTALLED {
        // SAFETY: observing `INSTALLED` pairs with the install Release store,
        // so the table write is visible and no later writer exists.
        unsafe { *CONTROL.0.get() }
    } else {
        AllocControl::noop()
    }
}
