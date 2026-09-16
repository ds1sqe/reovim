//! Immutable state ownership exported through the native ABI.

use alloc::rc::Rc;
use core::{cell::RefCell, ffi::c_void};
use reovim_kernel_interface::{
    Error, abi,
    declaration::ValueLimits,
    provider::{ResourceProvider, State},
};

use crate::{ExportedProvider, caller_export::native_error};

struct Owner {
    state: State,
    limits: ValueLimits,
    rejected: RefCell<Option<ResourceProvider>>,
}

/// Native immutable state, e.g. a Rust factory's initial Buffer revision.
///
/// Borrowed records may be retained through the ABI before this wrapper drops.
/// Native callers serialize callbacks and keep the adapter's code mapped.
pub struct ExportedState {
    owner: Rc<Owner>,
}

impl ExportedState {
    /// Owns state without constructing a snapshot or invoking provider code.
    #[must_use]
    pub fn new(state: State, limits: ValueLimits) -> Self {
        Self {
            owner: Rc::new(Owner {
                state,
                limits,
                rejected: RefCell::new(None),
            }),
        }
    }

    /// Borrows the native state, e.g. during factory staging.
    /// The borrow expires with this owner unless native retain succeeds first.
    #[must_use]
    pub fn raw(&self) -> abi::State {
        abi::State {
            data: Rc::as_ptr(&self.owner).cast(),
            ops: &raw const OPS,
            cost: self.owner.state.cost(),
        }
    }

    /// Transfers one reference, requiring exactly one native release.
    #[must_use]
    pub fn into_raw(self) -> abi::State {
        let raw = self.raw();
        let _ = Rc::into_raw(self.owner);
        raw
    }
}

static OPS: abi::StateOps = abi::StateOps {
    bytes: size_of::<abi::StateOps>(),
    retain: Some(retain),
    release: Some(release),
    snapshot: Some(snapshot),
};

unsafe extern "C" fn retain(data: *const c_void) {
    // SAFETY: Native retain borrows a live record from raw/into_raw or an
    // earlier retained owner. It is serialized with release of those owners.
    unsafe { Rc::increment_strong_count(data.cast::<Owner>()) };
}

unsafe extern "C" fn release(data: *const c_void) {
    // SAFETY: Native release consumes exactly one reference transferred by
    // into_raw or acquired with retain. Code remains live until return.
    unsafe { Rc::decrement_strong_count(data.cast::<Owner>()) };
}

unsafe extern "C" fn snapshot(
    data: *const c_void,
    output: *mut abi::Provider,
    error: *mut abi::Error,
) -> u32 {
    if data.is_null() || output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Native caller retains this Rc-backed record through the call;
    // its callbacks are serialized and code remains mapped.
    let owner = unsafe { &*data.cast::<Owner>() };
    match create(owner) {
        Ok(provider) => {
            // SAFETY: Caller supplies aligned writable Provider storage.
            // Only success writes it and transfers the exported owner.
            unsafe { output.write(provider.into_raw()) };
            0
        }
        Err(failure) => {
            // SAFETY: Caller supplies aligned writable Error storage. The
            // provider output remains untouched and transfers no ownership.
            unsafe { error.write(native_error(failure)) };
            failure.code.0
        }
    }
}

fn create(owner: &Owner) -> Result<ExportedProvider, Error> {
    let mut rejected =
        owner.rejected.try_borrow_mut().map_err(|_| Error::BUSY)?;
    cleanup(&mut rejected)?;
    match ExportedProvider::new(owner.state.snapshot()?, owner.limits) {
        Ok(provider) => Ok(provider),
        Err((error, provider)) => {
            *rejected = Some(provider);
            // Retain failed cleanup and retry before any new construction.
            let _ = cleanup(&mut rejected);
            Err(error)
        }
    }
}

fn cleanup(slot: &mut Option<ResourceProvider>) -> Result<(), Error> {
    if let Some(provider) = slot {
        provider.stop()?;
    }
    *slot = None;
    Ok(())
}

impl Drop for Owner {
    fn drop(&mut self) {
        let rejected = self.rejected.get_mut();
        if cleanup(rejected).is_err() {
            // An unresolved native shutdown must keep the provider, state and
            // code pinned. Its snapshot owner already retains all three.
            core::mem::forget(rejected.take());
        }
    }
}

// The native caller supplies an original ExportedState data pointer belonging
// to the selected editing implementation, retained through this borrow.
pub unsafe fn selected(data: *const c_void) -> Result<State, Error> {
    if data.is_null() {
        return Err(Error::INVALID);
    }
    // SAFETY: The caller guarantees a retained ExportedState owner. Its state
    // clone retains private data/code without retaining the native Edit loan.
    Ok(unsafe { &*data.cast::<Owner>() }.state.clone())
}
