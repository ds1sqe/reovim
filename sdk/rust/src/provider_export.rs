use crate::raw;
use alloc::{boxed::Box, vec::Vec};
use core::{cell::RefCell, ffi::c_void, ptr};
use reovim_kernel_interface::{
    Error, abi,
    declaration::{Dispatch, Operation, ValueLimits},
    provider::ResourceProvider,
};
mod calls;
mod creation;
mod edit;
mod factory;
mod io;
mod namespace;
mod opening;
mod packet;

struct State {
    provider: RefCell<ResourceProvider>,
    limits: ValueLimits,
}
struct Binding {
    state: *const State,
    index: usize,
}
struct Owner {
    state: Box<State>,
    bindings: Box<[Binding]>,
    interfaces: Box<[abi::Interface]>,
    io: Option<Box<abi::IoOps>>,
    namespace: Option<Box<abi::NamespaceOps>>,
    has_open: bool,
}

/// Owned native adapter, e.g. a Rust module stages its raw provider record.
/// Construction failure and rejected staging can return the original provider.
pub struct ExportedProvider {
    owner: Box<Owner>,
}

impl ExportedProvider {
    /// Prepares call/edit/factory, byte/packet I/O and namespace tables without
    /// calling.
    /// # Errors
    /// Unsupported families, invalid bounds and pressure return the provider.
    pub fn new(
        provider: ResourceProvider,
        limits: ValueLimits,
    ) -> Result<Self, (Error, ResourceProvider)> {
        if provider.interfaces().len() > 256 {
            return Err((Error::PRESSURE, provider));
        }
        let has_open = provider.has_open();
        let count = provider.interfaces().len();
        let mut bindings = Vec::new();
        let mut interfaces = Vec::new();
        if bindings.try_reserve_exact(count).is_err()
            || interfaces.try_reserve_exact(count).is_err()
        {
            return Err((Error::PRESSURE, provider));
        }
        let namespace =
            provider.namespace().map(|info| Box::new(namespace::table(info)));
        let io = provider.io().map(|info| Box::new(io::table(info, &provider)));
        let state =
            Box::new(State { provider: RefCell::new(provider), limits });
        for index in 0..count {
            bindings.push(Binding { state: &raw const *state, index });
        }
        let bindings = bindings.into_boxed_slice();
        for binding in &bindings {
            interfaces.push(interface(binding, &state));
        }
        Ok(Self {
            owner: Box::new(Owner {
                state,
                bindings,
                interfaces: interfaces.into_boxed_slice(),
                io,
                namespace,
                has_open,
            }),
        })
    }

    /// Borrows native records, e.g. while a scoped registrar decides admission.
    /// Native calls require this owner/code to remain live and serialized.
    /// No pointer may survive this owner unless `into_raw` transfers ownership.
    /// Native Edit callers supply selected data from this binding's
    /// `ExportedState` and retain their staging capability through finish.
    #[must_use]
    pub fn raw(&self) -> abi::Provider {
        abi::Provider {
            bytes: size_of::<abi::Provider>(),
            context: ptr::from_ref(&*self.owner).cast_mut().cast(),
            interfaces: self.owner.interfaces.as_ptr(),
            interface_count: self.owner.interfaces.len(),
            open: if self.owner.has_open {
                &raw const opening::OPS
            } else {
                ptr::null()
            },
            namespace_ops: self
                .owner
                .namespace
                .as_ref()
                .map_or(ptr::null(), |ops| &raw const **ops),
            io: self
                .owner
                .io
                .as_ref()
                .map_or(ptr::null(), |io| &raw const **io),
            stop: Some(stop),
            destroy: Some(destroy),
        }
    }

    /// Transfers the native owner, e.g. after successful registrar staging.
    /// Native destroy consumes it once after stop and all dependent work drain.
    #[must_use]
    pub fn into_raw(self) -> abi::Provider {
        let raw = self.raw();
        let _ = Box::into_raw(self.owner);
        raw
    }

    /// Returns the Rust provider, e.g. after a registrar rejects raw staging.
    /// Every borrowed native record expires; no accepted owner may retain it.
    #[must_use]
    pub fn into_provider(self) -> ResourceProvider {
        let Owner { state, bindings, .. } = *self.owner;
        drop(bindings);
        state.provider.into_inner()
    }
}

unsafe fn owner<'a>(context: *mut c_void) -> Result<&'a Owner, Error> {
    if context.is_null() {
        return Err(Error::INVALID);
    }
    // SAFETY: raw supplies this Box-stable Owner; native calls retain it until
    // destruction and serialize callbacks. Mutable state uses RefCell.
    Ok(unsafe { &*context.cast::<Owner>() })
}
unsafe extern "C" fn stop(context: *mut c_void) -> u32 {
    // SAFETY: The caller retains a live serialized provider owner.
    let result = unsafe { owner(context) }.and_then(|owner| {
        owner.state.provider.try_borrow_mut().map_err(|_| Error::BUSY)?.stop()
    });
    result
        .map_or_else(|e| crate::caller_export::checked_error(e).code.0, |()| 0)
}
unsafe extern "C" fn destroy(context: *mut c_void) {
    // SAFETY: into_raw transferred this unique Box. Native ownership requires
    // successful stop and no pending/result use before exactly one destroy.
    drop(unsafe { Box::from_raw(context.cast::<Owner>()) });
}

fn interface(binding: &Binding, state: &State) -> abi::Interface {
    let provider = state.provider.borrow();
    let api = &provider.interfaces()[binding.index];
    let exec = api.methods().iter().any(|m| {
        m.dispatch() == Dispatch::Call && m.operation() == Operation::Exec
    });
    let control = api.methods().iter().any(|m| {
        m.dispatch() == Dispatch::Call
            && matches!(m.operation(), Operation::Ioctl(_))
    });
    let edit = api.methods().iter().any(|m| m.dispatch() == Dispatch::Edit);
    let factory =
        api.methods().iter().any(|m| m.dispatch() == Dispatch::Factory);
    abi::Interface {
        bytes: size_of::<abi::Interface>(),
        context: ptr::from_ref(binding).cast_mut().cast(),
        declaration: raw::bytes(api.bytes()),
        exec: if exec { &raw const calls::EXEC } else { ptr::null() },
        control: if control { &raw const calls::CONTROL } else { ptr::null() },
        edit: if edit { &raw const edit::OPS } else { ptr::null() },
        factory: if factory { &raw const factory::OPS } else { ptr::null() },
    }
}
