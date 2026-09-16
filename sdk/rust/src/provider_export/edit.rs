//! Rust editing callbacks and request-scoped native proposal loans.

use alloc::{boxed::Box, rc::Rc};
use core::{cell::Cell, ffi::c_void};
use reovim_kernel_interface::{
    Error, abi,
    declaration::{Operation, ValueLimits},
    provider::{Edit, State},
};

use super::{Binding, calls};
use crate::{ExportedState, raw, state_export};

struct Context {
    raw: abi::Edit,
    live: Cell<bool>,
    limits: ValueLimits,
}
struct Lease(Rc<Context>);
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.live.set(false);
    }
}

pub(super) static OPS: abi::EditOps = abi::EditOps {
    bytes: size_of::<abi::EditOps>(),
    prepare: Some(prepare),
    write: None,
};

unsafe extern "C" fn prepare(
    context: *mut c_void,
    command: u32,
    edit: *const abi::Edit,
    input: *const abi::Args,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    if context.is_null()
        || edit.is_null()
        || input.is_null()
        || output.is_null()
        || error.is_null()
    {
        return Error::INVALID.code.0;
    }
    // SAFETY: The caller retains the exported provider and its stable binding
    // through serialized invocation and the resulting pending/result cleanup.
    let binding = unsafe { &*context.cast::<Binding>() };
    // SAFETY: Binding borrows its provider owner's stable state allocation.
    let state = unsafe { &*binding.state };
    // SAFETY: Native preparation supplies a readable Edit whose selected data
    // originated from ExportedState for this implementation. Its proposal
    // capability remains borrowed through pending cleanup.
    let imported = unsafe { import(*edit, state.limits) };
    let (edit, lease) = match imported {
        Ok(imported) => imported,
        Err(cause) => {
            // SAFETY: Output pointers are writable; failure transfers nothing.
            return unsafe {
                calls::complete(state, Err(cause), output, error)
            };
        }
    };
    let operation =
        if command == 0 { Operation::Exec } else { Operation::Ioctl(command) };
    // SAFETY: Native input spans remain readable through begin and are copied
    // before Rust preparation can retain them beyond this call.
    let result =
        unsafe { raw::value(*input, state.limits) }.and_then(|input| {
            state.provider.try_borrow_mut().map_err(|_| Error::BUSY)?.prepare(
                binding.index,
                operation,
                &edit,
                &input,
            )
        });
    // SAFETY: Outputs are writable. Accepted pending retains the lease through
    // successful finish; rejected begin expires it before returning to native.
    unsafe {
        calls::complete_owned(
            state,
            result,
            output,
            error,
            Some(Box::new(lease)),
        )
    }
}

unsafe fn import(
    raw: abi::Edit,
    limits: ValueLimits,
) -> Result<(Edit, Lease), Error> {
    if raw.propose.is_none() {
        return Err(Error::INVALID);
    }
    // SAFETY: The native caller selects this implementation's exported state
    // and retains its reference until preparation settles. Cloning detaches the
    // Rust selected-state lifetime from the borrowed native Edit record.
    let before = unsafe { state_export::selected(raw.before)? };
    let context = Rc::new(Context { raw, limits, live: Cell::new(true) });
    let edit = Edit::new(before, context.clone(), propose);
    Ok((edit, Lease(context)))
}

fn propose(context: &Rc<Context>, candidate: &State) -> Result<(), Error> {
    if !context.live.get() {
        return Err(Error::CLOSED);
    }
    let export = ExportedState::new(candidate.clone(), context.limits);
    let raw = export.raw();
    // SAFETY: The live lease keeps this borrowed staging capability valid.
    // Export retains readable state records through the call. Successful native
    // staging retains its own reference; the supplier still owns this one.
    let result = unsafe {
        (context.raw.propose.unwrap())(context.raw.context, &raw const raw)
    };
    if result != 0 {
        return Err(Error::new(result));
    }
    Ok(())
}
