use super::ExportedProvider;
use alloc::rc::Rc;
use core::cell::Cell;
use reovim_kernel_interface::{
    Error, abi,
    declaration::ValueLimits,
    provider::{Creation, CreationOps, ResourceProvider},
    resource::AccessId,
};

struct State {
    raw: abi::Creation,
    live: Cell<bool>,
    limits: ValueLimits,
}

pub(super) struct Lease(Rc<State>);
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.live.set(false);
    }
}

/// The native begin caller retains raw through pending finish. Lease expiration
/// fences every safe clone before that borrowed native context can disappear.
pub(super) unsafe fn import(
    raw: abi::Creation,
    limits: ValueLimits,
) -> (Creation, Lease) {
    let state = Rc::new(State { raw, live: Cell::new(true), limits });
    let creation = Creation::new(
        state.clone(),
        CreationOps {
            resource,
            keyed: Some(keyed),
            auxiliary: Some(auxiliary),
        },
    );
    (creation, Lease(state))
}

fn resource(
    state: &Rc<State>,
    provider: ResourceProvider,
) -> Result<(), (Error, ResourceProvider)> {
    if !state.live.get() {
        return Err((Error::CLOSED, provider));
    }
    let Some(callback) = state.raw.resource else {
        return Err((Error::UNSUPPORTED, provider));
    };
    let export = ExportedProvider::new(provider, state.limits)?;
    let raw = export.raw();
    // SAFETY: The live lease precedes native pending finish. The native caller
    // retains its staging context; export retains readable tables/provider
    // ownership until the callback reports acceptance.
    let result = unsafe { callback(state.raw.context, &raw const raw) };
    if result != 0 {
        return Err((Error::new(result), export.into_provider()));
    }
    let _ = export.into_raw();
    Ok(())
}

fn keyed(
    state: &Rc<State>,
    key: &[u8],
    provider: ResourceProvider,
) -> Result<(), (Error, ResourceProvider)> {
    if !state.live.get() {
        return Err((Error::CLOSED, provider));
    }
    let Some(callback) = state.raw.keyed else {
        return Err((Error::UNSUPPORTED, provider));
    };
    if key.len() > state.limits.bytes {
        return Err((Error::PRESSURE, provider));
    }
    let export = ExportedProvider::new(provider, state.limits)?;
    let raw = export.raw();
    // SAFETY: The live lease retains native staging. Key and descriptor stay
    // readable through the callback; only acceptance transfers ownership.
    let result = unsafe {
        callback(state.raw.context, crate::raw::bytes(key), &raw const raw)
    };
    if result != 0 {
        return Err((Error::new(result), export.into_provider()));
    }
    let _ = export.into_raw();
    Ok(())
}

fn auxiliary(
    state: &Rc<State>,
    provider: ResourceProvider,
) -> Result<AccessId, (Error, ResourceProvider)> {
    if !state.live.get() {
        return Err((Error::CLOSED, provider));
    }
    let Some(callback) = state.raw.auxiliary else {
        return Err((Error::UNSUPPORTED, provider));
    };
    let export = ExportedProvider::new(provider, state.limits)?;
    let raw = export.raw();
    let mut token = 0;
    // SAFETY: The lease retains native staging availability; raw borrows this
    // export; token is writable. Native OK transfers the provider and returns
    // its valid provisional token, as required by the Creation ABI contract.
    let result =
        unsafe { callback(state.raw.context, &raw const raw, &raw mut token) };
    if result != 0 {
        return Err((Error::new(result), export.into_provider()));
    }
    let _ = export.into_raw();
    Ok(AccessId(token))
}
