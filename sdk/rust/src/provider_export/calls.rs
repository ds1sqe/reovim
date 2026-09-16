use super::{Binding, State};
use crate::{caller_export::native_error, pending_export::export_owned, raw};
use alloc::boxed::Box;
use core::any::Any;
use core::ffi::c_void;
use reovim_kernel_interface::{Error, Pending, abi, declaration::Operation};

pub(super) static EXEC: abi::ExecOps =
    abi::ExecOps { bytes: size_of::<abi::ExecOps>(), exec: Some(exec) };
pub(super) static CONTROL: abi::ControlOps =
    abi::ControlOps { bytes: size_of::<abi::ControlOps>(), ioctl: Some(ioctl) };

unsafe extern "C" fn exec(
    context: *mut c_void,
    input: *const abi::Args,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    // SAFETY: The callback borrows its bound context, readable input and
    // writable outputs under raw's native serialized-call contract.
    unsafe { invoke(context, Operation::Exec, input, output, error) }
}
unsafe extern "C" fn ioctl(
    context: *mut c_void,
    command: u32,
    input: *const abi::Args,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    // SAFETY: Forward the same borrowed records; no API selector is added.
    unsafe { invoke(context, Operation::Ioctl(command), input, output, error) }
}
unsafe fn invoke(
    context: *mut c_void,
    operation: Operation,
    input: *const abi::Args,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    if context.is_null()
        || input.is_null()
        || output.is_null()
        || error.is_null()
    {
        return Error::INVALID.code.0;
    }
    // SAFETY: Owner retains the Box-stable Binding array for all native uses;
    // its state owns the original provider and bounded input allocation policy.
    let binding = unsafe { &*context.cast::<Binding>() };
    // SAFETY: Binding points to its Owner's stable State box; the provider
    // owner keeps both allocations alive through this serialized callback.
    let state = unsafe { &*binding.state };
    // SAFETY: The native caller supplies readable Args and nested input spans.
    let result =
        unsafe { raw::value(*input, state.limits) }.and_then(|input| {
            state.provider.try_borrow_mut().map_err(|_| Error::BUSY)?.begin(
                binding.index,
                operation,
                &input,
            )
        });
    // SAFETY: Checked pointers refer to native writable output records.
    unsafe { complete(state, result, output, error) }
}

pub(super) unsafe fn complete(
    state: &State,
    result: Result<Pending, Error>,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    // SAFETY: Forward the same checked writable records without extra custody.
    unsafe { complete_owned(state, result, output, error, None) }
}

pub(super) unsafe fn complete_owned(
    state: &State,
    result: Result<Pending, Error>,
    output: *mut abi::Pending,
    error: *mut abi::Error,
    owner: Option<Box<dyn Any>>,
) -> u32 {
    match result {
        Ok(pending) => {
            // SAFETY: The caller checked writable output. The exported owner
            // keeps pending work alive; input borrows ended at begin return.
            unsafe { output.write(export_owned(pending, state.limits, owner)) };
            0
        }
        Err(cause) => {
            let cause = native_error(cause);
            // SAFETY: The caller checked writable error storage. Rejected begin
            // transfers no pending owner and leaves its output untouched.
            unsafe { error.write(cause) };
            cause.code
        }
    }
}
