use super::{calls, owner};
use crate::raw;
use core::ffi::c_void;
use reovim_kernel_interface::{Error, abi, provider::Receive};

pub(super) unsafe extern "C" fn receive(
    context: *mut c_void,
    input: *const abi::Args,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    // SAFETY: The serialized native caller retains provider context and all
    // readable input/writable output records through this callback.
    unsafe { invoke(context, input, output, error, true) }
}

pub(super) unsafe extern "C" fn send(
    context: *mut c_void,
    input: *const abi::Args,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    // SAFETY: Forward the native caller's retained context and borrowed records
    // unchanged; invoke copies input before beginning pending work.
    unsafe { invoke(context, input, output, error, false) }
}

unsafe fn invoke(
    context: *mut c_void,
    input: *const abi::Args,
    output: *mut abi::Pending,
    error: *mut abi::Error,
    receive: bool,
) -> u32 {
    if input.is_null() || output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: The native call retains and serializes this exported Owner.
    let Ok(owner) = (unsafe { owner(context) }) else {
        return Error::INVALID.code.0;
    };
    // SAFETY: The caller supplies readable Args and nested immutable spans;
    // raw::value checks bounds and copies them into Rust-owned storage.
    let result =
        unsafe { raw::value(*input, owner.state.limits) }.and_then(|input| {
            let mut provider = owner
                .state
                .provider
                .try_borrow_mut()
                .map_err(|_| Error::BUSY)?;
            if !receive {
                return provider.send(&input);
            }
            if !input.references.is_empty() {
                return Err(Error::INVALID);
            }
            provider.receive(Receive::decode(&input.data)?)
        });
    // SAFETY: Both writable output records were checked before dispatch;
    // complete retains admitted pending work and leaves rejection unowned.
    unsafe { calls::complete(&owner.state, result, output, error) }
}
