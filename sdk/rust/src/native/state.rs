//! Retained native immutable state and rejected snapshot cleanup.

use alloc::rc::{Rc, Weak};
use core::{any::Any, cell::RefCell, mem::MaybeUninit};
use reovim_kernel_interface::{
    Error, abi, declaration::ValueLimits, provider::State,
};

use super::{Instance, import_provider};
use crate::raw;

struct NativeState {
    state: abi::State,
    origin: Option<Weak<Instance>>,
    ops: abi::StateOps,
    limits: ValueLimits,
    rejected: RefCell<Option<abi::Provider>>,
    code: Option<Rc<dyn Any>>,
}

/// Retains a borrowed native state for immutable installation and snapshots.
///
/// The supplying owner keeps its reference. Rust clones share the newly
/// retained reference; the final release runs with native code still pinned.
/// # Errors
/// Invalid records retain nothing and invoke no native callbacks.
/// # Safety
/// State and table pointers must be aligned/readable for their ABI extents.
/// Callbacks obey the state/provider ABI without unwinding and are serialized.
/// `code` retains all native dependencies through release and snapshot cleanup.
/// The supplied owner must remain live until this function returns.
pub unsafe fn import_state(
    state: *const abi::State,
    code: Rc<dyn Any>,
    limits: ValueLimits,
) -> Result<State, Error> {
    // SAFETY: Forward the caller's state and code promises without binding it
    // to an editing instance. Standalone imports cannot select native edits.
    unsafe { import(state, code, limits, None) }
}

pub(super) unsafe fn import_bound(
    state: *const abi::State,
    instance: Rc<Instance>,
    limits: ValueLimits,
) -> Result<State, Error> {
    let origin = Rc::downgrade(&instance);
    // SAFETY: The caller pairs valid state with its originating implementation;
    // the instance retains its native context and all code dependencies.
    unsafe { import(state, instance, limits, Some(origin)) }
}

unsafe fn import(
    state: *const abi::State,
    code: Rc<dyn Any>,
    limits: ValueLimits,
    origin: Option<Weak<Instance>>,
) -> Result<State, Error> {
    if state.is_null() {
        return Err(Error::INVALID);
    }
    // SAFETY: The caller supplies a readable aligned State record. Copying it
    // borrows its native ownership until the checked retain callback below.
    let state = unsafe { state.read() };
    // SAFETY: The caller supplies the readable aligned table and exact extent;
    // table checks its header before reading the remaining callback slots.
    let ops = unsafe { raw::table(state.ops)? };
    let retain = ops.retain.ok_or(Error::INVALID)?;
    if ops.release.is_none() || ops.snapshot.is_none() {
        return Err(Error::INVALID);
    }
    let value = Rc::new(NativeState {
        state,
        origin,
        ops,
        limits,
        rejected: RefCell::new(None),
        code: Some(code.clone()),
    });
    // SAFETY: All required callbacks were checked. The caller's live native
    // owner and code pin make this retain valid; value owns its new reference.
    unsafe { retain(state.data) };
    Ok(State::new(value, state.cost, |state| snapshot(&state), code))
}

fn snapshot(
    state: &Rc<NativeState>,
) -> Result<reovim_kernel_interface::provider::ResourceProvider, Error> {
    let mut rejected =
        state.rejected.try_borrow_mut().map_err(|_| Error::BUSY)?;
    cleanup(&mut rejected)?;
    let mut output = MaybeUninit::<abi::Provider>::uninit();
    let mut error = abi::Error::default();
    // SAFETY: Import validated this callback. The retained state/code are live,
    // outputs are writable, and success initializes the complete Provider.
    let result = unsafe {
        (state.ops.snapshot.unwrap())(
            state.state.data,
            output.as_mut_ptr(),
            &raw mut error,
        )
    };
    if result != 0 {
        return Err(raw::error(error).unwrap_or(Error::PROVIDER));
    }
    // SAFETY: Successful snapshot transfers a fully initialized ABI Provider.
    let output = unsafe { output.assume_init() };
    // SAFETY: Snapshot supplies valid native record extents and this retained
    // state pins their dependencies. Import takes ownership only on success.
    let imported = unsafe {
        import_provider(&raw const output, state.clone(), state.limits)
    };
    if imported.is_err() {
        *rejected = Some(output);
        // Preserve the import error. Failed cleanup retains the transferred
        // provider and fences further construction until cleanup succeeds.
        let _ = cleanup(&mut rejected);
    }
    imported
}

fn cleanup(slot: &mut Option<abi::Provider>) -> Result<(), Error> {
    let Some(provider) = *slot else {
        return Ok(());
    };
    if provider.bytes != size_of::<abi::Provider>() {
        return Err(Error::PROVIDER);
    }
    let stop = provider.stop.ok_or(Error::PROVIDER)?;
    let destroy = provider.destroy.ok_or(Error::PROVIDER)?;
    // SAFETY: Snapshot transferred this context with callable ABI slots;
    // State/code remain pinned. Serialized stop may retain ownership.
    let result = unsafe { stop(provider.context) };
    if result != 0 {
        return Err(Error::new(result));
    }
    // SAFETY: Stop succeeded and this unpublished provider has no adapter
    // requests. Its transferred context is destroyed exactly once.
    unsafe { destroy(provider.context) };
    *slot = None;
    Ok(())
}

impl Drop for NativeState {
    fn drop(&mut self) {
        if cleanup(self.rejected.get_mut()).is_err() {
            // Nonblocking Drop cannot declare unresolved cleanup complete.
            // Keep the native state reference and code together, as native
            // provider Drop does, instead of unmapping live callbacks.
            core::mem::forget(self.code.take());
            return;
        }
        // SAFETY: This is the last Rust owner of the one reference retained
        // at import. Snapshot cleanup finished and code is still retained.
        unsafe { (self.ops.release.unwrap())(self.state.data) };
    }
}

/// Only adapter-owned states bound to this exact implementation may be passed
/// back as private native data. Shared library identity is not sufficient.
pub(super) fn bound(
    state: &State,
    instance: &Rc<Instance>,
) -> Result<abi::State, Error> {
    let state = state.data::<NativeState>().ok_or(Error::INVALID)?;
    let origin = state.origin.as_ref().ok_or(Error::INVALID)?;
    if !Weak::ptr_eq(origin, &Rc::downgrade(instance)) {
        return Err(Error::INVALID);
    }
    Ok(state.state)
}

/// Imports a native state/provider pair for later factory staging.
///
/// Success retains the initial state and takes the provider; the supplier keeps
/// its original state reference. This does not install a Buffer or History.
/// # Errors
/// Rejection leaves provider ownership with the supplier. Standard byte-edit
/// tables are not yet supported by this adapter and reject before retention.
/// # Safety
/// The record and nested pointers satisfy state/provider import requirements.
/// Initial state and every proposed candidate must be valid private state for
/// this implementation's editing callbacks. Code retains all dependencies;
/// callbacks are serialized and do not unwind. Transfer occurs only on success.
pub unsafe fn import_state_init(
    init: *const abi::StateInit,
    code: Rc<dyn Any>,
    limits: ValueLimits,
) -> Result<(State, super::ResourceProvider), Error> {
    if init.is_null() {
        return Err(Error::INVALID);
    }
    // SAFETY: The caller supplies the readable aligned StateInit descriptor.
    let init = unsafe { init.read() };
    if !init.byte_edits.is_null() {
        return Err(Error::UNSUPPORTED);
    }
    // SAFETY: Nested provider records and code satisfy import requirements.
    // prepare validates without transferring native context ownership.
    let (provider, instance) = unsafe {
        super::prepare(&raw const init.implementation, code, limits)?
    };
    // SAFETY: StateInit pairs valid initial state with this implementation.
    // All provider validation completed before retaining the native state.
    let initial = unsafe {
        import_bound(&raw const init.initial, instance.clone(), limits)?
    };
    instance.owned.set(true);
    Ok((initial, provider))
}
