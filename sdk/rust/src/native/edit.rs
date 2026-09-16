//! Native editing preparation with exact instance-bound state.

use alloc::{rc::Rc, vec::Vec};
use core::{ffi::c_void, ptr};
use reovim_kernel_interface::{
    Error, Pending, Value, abi,
    declaration::{Operation, ValueLimits},
    provider::Edit,
};

use super::{Imported, Instance, complete_begin, state};
use crate::raw;

struct Context {
    edit: Edit,
    instance: Rc<Instance>,
    limits: ValueLimits,
}
struct Export {
    _context: Rc<Context>,
    raw: abi::Edit,
}

pub(super) fn prepare(
    imported: &mut Imported,
    interface: usize,
    operation: Operation,
    edit: &Edit,
    input: &Value,
) -> Result<Pending, Error> {
    let offered = imported.offered.get(interface).ok_or(Error::UNSUPPORTED)?;
    let callback =
        offered.edit.and_then(|ops| ops.prepare).ok_or(Error::UNSUPPORTED)?;
    let before = state::bound(edit.before(), &imported.instance)?;
    let context = Rc::new(Context {
        edit: edit.clone(),
        instance: imported.instance.clone(),
        limits: imported.limits,
    });
    let owner = Rc::new(Export {
        raw: abi::Edit {
            before: before.data,
            context: Rc::as_ptr(&context).cast_mut().cast(),
            propose: Some(propose),
        },
        _context: context,
    });
    let mut refs = Vec::new();
    refs.try_reserve_exact(input.references.len())
        .map_err(|_| Error::PRESSURE)?;
    refs.extend(input.references.iter().map(|access| access.0));
    let args = abi::Args {
        data: raw::bytes(&input.data),
        refs: abi::Refs { ptr: refs.as_ptr(), len: refs.len() },
    };
    let command = match operation {
        Operation::Exec => 0,
        Operation::Ioctl(command) => command,
    };
    let mut pending =
        abi::Pending { context: ptr::null_mut(), ops: ptr::null() };
    let mut error = abi::Error::default();
    // SAFETY: Import checked the selected callback. Bound verifies that before
    // belongs to this exact instance; owner pins state and the stable Edit
    // record through pending cleanup. Native begin copies borrowed input.
    let result = unsafe {
        callback(
            offered.context,
            command,
            &raw const owner.raw,
            &raw const args,
            &raw mut pending,
            &raw mut error,
        )
    };
    complete_begin(imported, pending, result, error, Some(owner))
}

unsafe extern "C" fn propose(
    context: *mut c_void,
    candidate: *const abi::State,
) -> u32 {
    if context.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Export owns this Rc-stable Context and pending cleanup retains
    // it through callbacks. The candidate is borrowed only for this call.
    let context = unsafe { &*context.cast::<Context>() };
    // SAFETY: The originating native editing callback supplies a valid state
    // for its binding. Import retains an independent reference under its code
    // lease before exposing safe clones to the staging owner.
    let candidate = unsafe {
        state::import_bound(candidate, context.instance.clone(), context.limits)
    };
    candidate
        .and_then(|candidate| context.edit.propose(&candidate))
        .map_or_else(|error| error.code.0, |()| 0)
}
