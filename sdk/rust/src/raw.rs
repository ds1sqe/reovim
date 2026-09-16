use alloc::vec::Vec;
use core::{mem::size_of, slice};
use reovim_kernel_interface::{
    Error, ResourceId, Value, abi, declaration::ValueLimits, resource::AccessId,
};

// Caller supplies a repr(C) ABI table whose first field is usize. Its aligned
// size header is readable, and a matching advertised extent contains a valid T.
pub const unsafe fn table<T: Copy>(pointer: *const T) -> Result<T, Error> {
    if pointer.is_null() {
        return Err(Error::INVALID);
    }
    // SAFETY: The caller guarantees the aligned readable usize header. Reading
    // only that header does not require the rest of a rejected table to exist.
    if unsafe { pointer.cast::<usize>().read() } != size_of::<T>() {
        return Err(Error::INVALID);
    }
    // SAFETY: The header matches T's extent; the caller guarantees valid,
    // initialized ABI fields in that extent. T is Copy, so no owner is moved.
    Ok(unsafe { pointer.read() })
}

// Caller guarantees non-ZST initialized elements in one aligned allocation,
// immutable for the returned borrow. Nonzero pointers remain a native promise.
pub const unsafe fn elements<'a, T>(
    pointer: *const T,
    length: usize,
    limit: usize,
) -> Result<&'a [T], Error> {
    if length > limit || length > isize::MAX as usize / size_of::<T>() {
        return Err(Error::PRESSURE);
    }
    if length == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() {
        return Err(Error::INVALID);
    }
    // SAFETY: Null, admission and isize extent checks passed. The caller
    // guarantees initialized aligned elements in one allocation for 'a.
    Ok(unsafe { slice::from_raw_parts(pointer, length) })
}

pub unsafe fn value(
    value: abi::Args,
    limits: ValueLimits,
) -> Result<Value, Error> {
    // SAFETY: The caller retains the native outcome owner while these immutable
    // spans are copied. elements checks nullness and the configured byte bound.
    let bytes =
        unsafe { elements(value.data.ptr, value.data.len, limits.bytes)? };
    // SAFETY: The same owner retains aligned initialized u64 token storage;
    // elements checks its extent before exposing the borrowed slice.
    let slots =
        unsafe { elements(value.refs.ptr, value.refs.len, limits.references)? };
    let mut references = Vec::new();
    references.try_reserve_exact(slots.len()).map_err(|_| Error::PRESSURE)?;
    references.extend(slots.iter().map(|slot| AccessId(*slot)));
    Value::copy(bytes, &references)
}

pub fn error(value: abi::Error) -> Result<Error, Error> {
    if value.code == 0 {
        return Err(Error::PROVIDER);
    }
    if value.detail == abi::Identity::default() {
        return Ok(Error::new(value.code));
    }
    if value.detail.serial == 0 {
        return Err(Error::PROVIDER);
    }
    Ok(Error::with_id(
        value.code,
        ResourceId { epoch: value.detail.epoch, serial: value.detail.serial },
    ))
}

pub const fn bytes(value: &[u8]) -> abi::Bytes {
    abi::Bytes { ptr: value.as_ptr(), len: value.len() }
}

pub const fn empty() -> abi::Args {
    abi::Args {
        data: abi::Bytes { ptr: core::ptr::null(), len: 0 },
        refs: abi::Refs { ptr: core::ptr::null(), len: 0 },
    }
}
