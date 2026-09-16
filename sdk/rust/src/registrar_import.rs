use crate::{ExportedProvider, raw};
use alloc::rc::Rc;
use core::{any::Any, cell::Cell};
use reovim_kernel_interface::{
    Error, abi,
    declaration::ValueLimits,
    provider::{Registrar, RegistrarOps, ResourceProvider},
};

struct State {
    raw: abi::Registrar,
    live: Cell<bool>,
    limits: ValueLimits,
    _code: Rc<dyn Any>,
}

/// Owns the lifetime fence for an imported native registrar.
///
/// A Rust module keeps this owner through its final callback. Dropping it makes
/// retained Registrar clones return Closed without entering native code.
pub struct ImportedRegistrar {
    state: Rc<State>,
    registrar: Registrar,
}

impl ImportedRegistrar {
    /// Borrows a native activation scope, e.g. from a module context.
    /// # Errors
    /// Rejects a mismatched extent or missing stage callback.
    /// # Safety
    /// The native context/callback remains valid until this owner is dropped;
    /// code retains its executable dependencies. Calls are serialized, stage
    /// takes provider ownership exactly on OK, and callbacks never unwind.
    pub unsafe fn new(
        raw: abi::Registrar,
        code: Rc<dyn Any>,
        limits: ValueLimits,
    ) -> Result<Self, Error> {
        if raw.bytes != size_of::<abi::Registrar>() || raw.stage.is_none() {
            return Err(Error::INVALID);
        }
        let state =
            Rc::new(State { raw, live: Cell::new(true), limits, _code: code });
        let registrar = Registrar::new(state.clone(), RegistrarOps { stage });
        Ok(Self { state, registrar })
    }

    /// Returns a safe scoped capability, e.g. retained during module startup.
    /// Cloning it cannot extend the borrowed native context's lifetime.
    #[must_use]
    pub fn registrar(&self) -> Registrar {
        self.registrar.clone()
    }
}

impl Drop for ImportedRegistrar {
    fn drop(&mut self) {
        self.state.live.set(false);
    }
}

fn stage(
    state: &Rc<State>,
    path: &[u8],
    provider: ResourceProvider,
) -> Result<(), (Error, ResourceProvider)> {
    if !state.live.get() {
        return Err((Error::CLOSED, provider));
    }
    if path.len() > 4096 {
        return Err((Error::PRESSURE, provider));
    }
    let exported = ExportedProvider::new(provider, state.limits)?;
    let provider = exported.raw();
    // SAFETY: ImportedRegistrar's lifetime fence is live, so construction's
    // native context/code promise still holds. Path and provider records remain
    // readable during this call. Only native OK transfers the exported owner.
    let result = unsafe {
        state.raw.stage.unwrap()(
            state.raw.context,
            raw::bytes(path),
            &raw const provider,
        )
    };
    if result != 0 {
        return Err((Error::new(result), exported.into_provider()));
    }
    let _ = exported.into_raw();
    Ok(())
}
