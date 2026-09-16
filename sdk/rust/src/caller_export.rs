use alloc::{boxed::Box, rc::Rc};
use core::{cell::Cell, ffi::c_void};
use reovim_kernel_interface::{
    Error, ResourceId, abi,
    call::Cancellation,
    caller::{Call, Caller, Expected, Operation, Status},
    declaration::ValueLimits,
    resource::AccessId,
};

use crate::raw;

mod view;
pub mod wake;

struct State {
    caller: Caller,
    limits: ValueLimits,
    views: Rc<Cell<usize>>,
    capacity: usize,
}

/// Native caller table over a retained Interface Caller.
///
/// Example: module assembly supplies `raw()` as that module's granted imports.
/// Native calls run serialized on the owning executor thread. Keep this owner
/// alive through all calls using its borrowed context; delivery views retain
/// their own Caller and never release original request custody on drop.
pub struct ExportedCaller {
    state: Box<State>,
}

impl ExportedCaller {
    /// Bounds live delivery views and distinct wake registrations per view.
    /// Example: capacity 64 permits 64 views, each retaining up to 64 wakes.
    /// # Errors
    /// Zero capacity is invalid; pressure rejects before submitting an effect.
    pub fn new(
        caller: Caller,
        limits: ValueLimits,
        capacity: usize,
    ) -> Result<Self, Error> {
        if capacity == 0 {
            return Err(Error::INVALID);
        }
        Ok(Self {
            state: Box::new(State {
                caller,
                limits,
                views: Rc::new(Cell::new(0)),
                capacity,
            }),
        })
    }

    /// Borrow native imports, e.g. until the module's callbacks have drained.
    /// Calling these pointers requires this owner to remain alive and calls
    /// to stay on its executor thread, with valid ABI storage and wake owners.
    #[must_use]
    pub fn raw(&self) -> abi::Caller {
        abi::Caller {
            context: (&raw const *self.state).cast_mut().cast(),
            ops: &raw const OPS,
        }
    }
}

static OPS: abi::CallerOps = abi::CallerOps {
    bytes: size_of::<abi::CallerOps>(),
    reserve: Some(reserve),
    submit: Some(submit),
    result: Some(result),
    cancel: Some(cancel),
    reconcile: Some(reconcile),
    adopt: Some(adopt),
    release: Some(release),
};

unsafe fn state<'a>(context: *mut c_void) -> Result<&'a State, Error> {
    if context.is_null() {
        return Err(Error::INVALID);
    }
    // SAFETY: ExportedCaller::raw points at its stable Box<State>. The native
    // caller retains that owner on the executor thread through this callback.
    Ok(unsafe { &*context.cast::<State>() })
}

unsafe extern "C" fn reserve(context: *mut c_void, output: *mut u64) -> u32 {
    if output.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: The borrowed context follows ExportedCaller::raw's lifetime.
    let result = unsafe { state(context) }.and_then(|s| s.caller.reserve());
    match result {
        Ok(id) => {
            // SAFETY: The native caller supplies initialized writable u64
            // output storage; rejection never writes it.
            unsafe { output.write(id) };
            0
        }
        Err(error) => checked_error(error).code.0,
    }
}

unsafe extern "C" fn submit(
    context: *mut c_void,
    input: *const abi::Call,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    if input.is_null() || output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Context and input are retained, initialized native storage for
    // the callback. Capture validates fields and copies bounded input spans.
    let result = unsafe { capture_and_submit(context, &*input) };
    // SAFETY: Both result destinations were checked nonnull and are writable
    // under the callback's ABI contract. Only the selected destination is set.
    unsafe { deliver(result, output, error) }
}

unsafe fn capture_and_submit(
    context: *mut c_void,
    input: &abi::Call,
) -> Result<Box<view::View>, Error> {
    // SAFETY: This function inherits submit's retained context/span contract.
    let state = unsafe { state(context)? };
    // SAFETY: The input record and its immutable spans remain borrowed until
    // the capture completes, before caller admission or another callback.
    let call = unsafe { capture(input, state.limits)? };
    let view = view::View::new(state, call.request)?;
    state.caller.submit(call)?;
    Ok(view)
}

unsafe fn capture(
    input: &abi::Call,
    limits: ValueLimits,
) -> Result<Call, Error> {
    let expected = match input.expected.present {
        0 if input.expected == abi::Expected::default() => None,
        1 if input.expected.subject.serial != 0 => Some(Expected {
            subject: ResourceId {
                epoch: input.expected.subject.epoch,
                serial: input.expected.subject.serial,
            },
            revision: input.expected.revision,
        }),
        _ => return Err(Error::INVALID),
    };
    let mut call = Call {
        request: input.request,
        operation: Operation::try_from(input.operation)?,
        subject: AccessId(input.subject),
        command: input.command,
        expected,
        input: reovim_kernel_interface::Value::unit(),
    };
    call.validate()?;
    // SAFETY: The native caller retains both input spans through this copy.
    // raw::value validates nullness and configured extents before borrowing.
    call.input = unsafe { raw::value(input.input, limits)? };
    Ok(call)
}

unsafe extern "C" fn result(
    context: *mut c_void,
    id: u64,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    if output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Native callers retain ExportedCaller through this callback.
    let result = unsafe { state(context) }.and_then(|state| {
        if state.caller.snapshot(id)? == Status::Reserved {
            return Err(Error::NOT_READY);
        }
        view::View::new(state, id)
    });
    // SAFETY: Native output/error are writable slots for exactly these types.
    unsafe { deliver(result, output, error) }
}

unsafe fn deliver(
    result: Result<Box<view::View>, Error>,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    match result {
        Ok(view) => {
            // SAFETY: The caller supplies writable Pending storage. into_raw
            // transfers this unique view; finish destroys it exactly once.
            unsafe { output.write(view::into_raw(view)) };
            0
        }
        Err(cause) => {
            let cause = checked_error(cause);
            // SAFETY: Error storage is writable under the inherited callback
            // contract. Pending stays untouched, transferring no view.
            unsafe { error.write(native_error(cause)) };
            cause.code.0
        }
    }
}

pub fn native_error(error: Error) -> abi::Error {
    let error = checked_error(error);
    abi::Error {
        code: error.code.0,
        detail: error.id.map_or_else(abi::Identity::default, |id| {
            abi::Identity { epoch: id.epoch, serial: id.serial }
        }),
    }
}

pub fn checked_error(error: Error) -> Error {
    if error.code.0 == 0 || error.id.is_some_and(|id| id.serial == 0) {
        Error::PROVIDER
    } else {
        error
    }
}

unsafe extern "C" fn cancel(context: *mut c_void, id: u64) -> u32 {
    // SAFETY: The callback context remains owned by ExportedCaller.
    match unsafe { state(context) }.and_then(|s| s.caller.cancel(id)) {
        Ok(Cancellation::Accepted) => 0,
        Ok(Cancellation::TooLate) => 1,
        Err(error) => checked_error(error).code.0,
    }
}

unsafe extern "C" fn reconcile(context: *mut c_void, id: u64) -> u32 {
    // SAFETY: The callback context remains owned by ExportedCaller.
    unsafe { control(context, id, Caller::reconcile) }
}
unsafe extern "C" fn adopt(context: *mut c_void, id: u64) -> u32 {
    // SAFETY: The callback context remains owned by ExportedCaller.
    unsafe { control(context, id, Caller::adopt) }
}
unsafe extern "C" fn release(context: *mut c_void, id: u64) -> u32 {
    // SAFETY: The callback context remains owned by ExportedCaller.
    unsafe { control(context, id, Caller::release) }
}

unsafe fn control(
    context: *mut c_void,
    id: u64,
    action: fn(&Caller, u64) -> Result<(), Error>,
) -> u32 {
    // SAFETY: Forwarding inherits raw's retained context contract.
    unsafe { state(context) }
        .and_then(|state| action(&state.caller, id))
        .map_or_else(|error| checked_error(error).code.0, |()| 0)
}
