use super::{calls, owner, packet};
use crate::raw;
use core::ffi::c_void;
use reovim_kernel_interface::{
    Error, abi,
    provider::{IoInfo, Read, ResourceProvider, Write},
};

pub(super) fn table(info: IoInfo, provider: &ResourceProvider) -> abi::IoOps {
    abi::IoOps {
        bytes: size_of::<abi::IoOps>(),
        read_modes: info.read_modes,
        write_modes: info.write_modes,
        alignment: info.alignment,
        transfer_limit: info.transfer_limit,
        receive_type: raw::bytes(
            provider.receive_type().map_or(&[], |t| t.bytes()),
        ),
        send_type: raw::bytes(provider.send_type().map_or(&[], |t| t.bytes())),
        read: if info.read_modes == 0 { None } else { Some(read) },
        write: if info.write_modes == 0 { None } else { Some(write) },
        receive: provider.receive_type().map(|_| packet::receive as _),
        send: provider.send_type().map(|_| packet::send as _),
    }
}
const fn offset(positional: u32, offset: u64) -> Result<Option<u64>, Error> {
    match (positional, offset) {
        (0, 0) => Ok(None),
        (1, offset) => Ok(Some(offset)),
        _ => Err(Error::INVALID),
    }
}
unsafe extern "C" fn read(
    context: *mut c_void,
    input: *const abi::Read,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    if input.is_null() || output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: The serialized native call retains the provider's owner.
    let Ok(owner) = (unsafe { owner(context) }) else {
        return Error::INVALID.code.0;
    };
    // SAFETY: The native caller supplies a readable aligned Read record.
    let request = unsafe { &*input };
    let result =
        offset(request.positional, request.offset).and_then(|offset| {
            owner
                .state
                .provider
                .try_borrow_mut()
                .map_err(|_| Error::BUSY)?
                .read(Read { offset, maximum: request.maximum })
        });
    // SAFETY: Writable output/error records were checked before begin.
    unsafe { calls::complete(&owner.state, result, output, error) }
}
unsafe extern "C" fn write(
    context: *mut c_void,
    input: *const abi::Write,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    if input.is_null() || output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Native execution retains and serializes the provider owner.
    let Ok(owner) = (unsafe { owner(context) }) else {
        return Error::INVALID.code.0;
    };
    // SAFETY: Input is a readable aligned record with borrowed immutable bytes.
    let request = unsafe { &*input };
    let result =
        offset(request.positional, request.offset).and_then(|offset| {
            // SAFETY: The readable span remains borrowed through begin.
            let data = unsafe {
                raw::elements(
                    request.data.ptr,
                    request.data.len,
                    owner.state.limits.bytes,
                )?
            };
            owner
                .state
                .provider
                .try_borrow_mut()
                .map_err(|_| Error::BUSY)?
                .write(Write { offset, data })
        });
    // SAFETY: Writable output/error records were checked before begin.
    unsafe { calls::complete(&owner.state, result, output, error) }
}
