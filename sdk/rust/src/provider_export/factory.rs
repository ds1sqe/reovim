use super::{Binding, calls, creation};
use crate::raw;
use alloc::boxed::Box;
use core::ffi::c_void;
use reovim_kernel_interface::{Error, abi, declaration::Operation};

pub(super) static OPS: abi::FactoryOps = abi::FactoryOps {
    bytes: size_of::<abi::FactoryOps>(),
    create: Some(create),
};
unsafe extern "C" fn create(
    context: *mut c_void,
    command: u32,
    staging: *const abi::Creation,
    input: *const abi::Args,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    if context.is_null()
        || staging.is_null()
        || input.is_null()
        || output.is_null()
        || error.is_null()
    {
        return Error::INVALID.code.0;
    }
    // SAFETY: The provider owner retains this interface binding and stable
    // State allocation for every serialized callback and pending/result use.
    let binding = unsafe { &*context.cast::<Binding>() };
    // SAFETY: The binding borrows its provider owner's State box.
    let state = unsafe { &*binding.state };
    // SAFETY: Native begin supplies readable Creation, retaining its callbacks
    // and context through finish. The lease fences retained Rust clones.
    let (creation, lease) = unsafe { creation::import(*staging, state.limits) };
    let operation =
        if command == 0 { Operation::Exec } else { Operation::Ioctl(command) };
    // SAFETY: Native input spans stay readable through begin; value copies them
    // before the Rust factory can retain any input beyond this callback.
    let result =
        unsafe { raw::value(*input, state.limits) }.and_then(|input| {
            state.provider.try_borrow_mut().map_err(|_| Error::BUSY)?.create(
                binding.index,
                operation,
                &input,
                &creation,
            )
        });
    // SAFETY: Writable outputs were checked before factory entry. Rejection
    // expires the lease; accepted pending retains it until successful finish.
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
