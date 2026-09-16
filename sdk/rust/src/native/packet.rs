use super::{Imported, complete_begin, raw};
use alloc::vec::Vec;
use core::{ffi::c_void, ptr};
use reovim_kernel_interface::{
    Error, Pending, Value, abi, declaration::ValueType, provider::Receive,
};

type Callback = unsafe extern "C" fn(
    *mut c_void,
    *const abi::Args,
    *mut abi::Pending,
    *mut abi::Error,
) -> u32;

pub(super) unsafe fn descriptor(
    bytes: abi::Bytes,
    present: bool,
) -> Result<Option<ValueType>, Error> {
    if (bytes.len != 0) != present {
        return Err(Error::INVALID);
    }
    // SAFETY: Import retains readable native descriptor storage; the extent
    // check bounds the copy before decoding. No ownership is taken on error.
    let bytes = unsafe { raw::elements(bytes.ptr, bytes.len, 65536)? };
    present.then(|| ValueType::decode(bytes)).transpose()
}

pub(super) fn receive(
    state: &mut Imported,
    request: Receive,
) -> Result<Pending, Error> {
    let callback =
        state.io.and_then(|io| io.receive).ok_or(Error::UNSUPPORTED)?;
    let data = request.encode();
    let args = abi::Args { data: raw::bytes(&data), ..raw::empty() };
    invoke(state, &args, callback)
}

pub(super) fn send(
    state: &mut Imported,
    input: &Value,
) -> Result<Pending, Error> {
    let callback = state.io.and_then(|io| io.send).ok_or(Error::UNSUPPORTED)?;
    let mut refs = Vec::new();
    refs.try_reserve_exact(input.references.len())
        .map_err(|_| Error::PRESSURE)?;
    refs.extend(input.references.iter().map(|access| access.0));
    let args = abi::Args {
        data: raw::bytes(&input.data),
        refs: abi::Refs { ptr: refs.as_ptr(), len: refs.len() },
    };
    invoke(state, &args, callback)
}

fn invoke(
    state: &Imported,
    args: &abi::Args,
    callback: Callback,
) -> Result<Pending, Error> {
    if state.instance.stopped.get() {
        return Err(Error::CLOSED);
    }
    let mut pending =
        abi::Pending { context: ptr::null_mut(), ops: ptr::null() };
    let mut error = abi::Error::default();
    // SAFETY: Import validated the callback and keeps context/code alive.
    // Args and initialized outputs remain valid through this serialized call;
    // pending native work must capture borrowed bytes/references before return.
    let result = unsafe {
        callback(
            state.instance.provider.context,
            ptr::from_ref(args),
            &raw mut pending,
            &raw mut error,
        )
    };
    complete_begin(state, pending, result, error, None)
}
