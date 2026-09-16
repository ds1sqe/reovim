use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
};
use core::{any::Any, ffi::c_void};
use reovim_kernel_interface::{
    Error, abi, declaration::ValueLimits, provider::Registrar,
};

use crate::raw;

enum Lease {
    Owned(Rc<dyn Any>),
    Borrowed(Weak<dyn Any>),
}

impl Lease {
    fn acquire(&self) -> Result<Rc<dyn Any>, Error> {
        match self {
            Self::Owned(owner) => Ok(owner.clone()),
            Self::Borrowed(owner) => owner.upgrade().ok_or(Error::CLOSED),
        }
    }
}

struct State {
    registrar: Registrar,
    code: Lease,
    limits: ValueLimits,
}

/// Native publication capability for one module activation.
///
/// Example: supply `raw()` in the module context and retain this owner through
/// the module's last callback. Copied records borrow the same stable context;
/// the Runtime registrar fences staging on publication or rollback.
/// All calls are serialized on that Runtime's executor thread.
pub struct ExportedRegistrar {
    state: Box<State>,
}

impl ExportedRegistrar {
    /// Retains the scoped registrar and the module's complete code/data lease.
    /// Example: providers staged from this module keep that lease through
    /// native stop, pending/result cleanup and final destruction.
    #[must_use]
    pub fn new(
        registrar: Registrar,
        code: Rc<dyn Any>,
        limits: ValueLimits,
    ) -> Self {
        Self {
            state: Box::new(State {
                registrar,
                code: Lease::Owned(code),
                limits,
            }),
        }
    }

    pub(super) fn borrowed(
        registrar: Registrar,
        code: Weak<dyn Any>,
        limits: ValueLimits,
    ) -> Self {
        Self {
            state: Box::new(State {
                registrar,
                code: Lease::Borrowed(code),
                limits,
            }),
        }
    }

    /// Borrows the C table, e.g. for the module's Starting phase.
    /// Calling it requires retaining this owner, readable provider/path storage
    /// and the module's serialized execution discipline. Native provider data
    /// and callbacks must remain valid under the code/data lease given to new.
    #[must_use]
    pub fn raw(&self) -> abi::Registrar {
        abi::Registrar {
            bytes: size_of::<abi::Registrar>(),
            context: (&raw const *self.state).cast_mut().cast(),
            stage: Some(stage),
        }
    }
}

unsafe extern "C" fn stage(
    context: *mut c_void,
    path: abi::Bytes,
    provider: *const abi::Provider,
) -> u32 {
    if context.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: raw supplies this Box-stable State address. Its owner remains
    // alive through every serialized native callback, including late attempts
    // after the underlying activation has been fenced.
    let state = unsafe { &*context.cast::<State>() };
    // SAFETY: The native caller supplies immutable readable path storage for
    // this call. elements checks nullness, extent and the namespace byte bound.
    let path = match unsafe { raw::elements(path.ptr, path.len, 4096) } {
        Ok(path) => path,
        Err(error) => return error.code.0,
    };
    let code = match state.code.acquire() {
        Ok(code) => code,
        Err(error) => return error.code.0,
    };
    // SAFETY: The supplier retains readable tables and owns the native context
    // until staging succeeds. The retained module lease keeps its instance
    // data, callbacks and destructors valid through provider cleanup.
    let prepared = unsafe { super::prepare(provider, code, state.limits) };
    let (provider, instance) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => return error.code.0,
    };
    match state.registrar.stage(path, provider) {
        Ok(()) => {
            instance.owned.set(true);
            0
        }
        Err((error, _provider)) => error.code.0,
    }
}
