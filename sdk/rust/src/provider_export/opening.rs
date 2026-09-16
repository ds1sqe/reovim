use super::{calls, creation, owner};
use alloc::boxed::Box;
use core::ffi::c_void;
use reovim_kernel_interface::{
    Error, abi,
    provider::{Creation, Open},
};

pub(super) static OPS: abi::OpenOps =
    abi::OpenOps { bytes: size_of::<abi::OpenOps>(), begin: Some(begin) };
unsafe extern "C" fn begin(
    context: *mut c_void,
    staging: *const abi::Open,
    output: *mut abi::Pending,
    error: *mut abi::Error,
) -> u32 {
    if staging.is_null() || output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Native execution retains and serializes this provider owner.
    let Ok(owner) = (unsafe { owner(context) }) else {
        return Error::INVALID.code.0;
    };
    // SAFETY: Native begin supplies a readable Open record, with its context
    // retained through pending finish. The lease fences all safe clones.
    let raw = unsafe { *staging };
    if raw.stage.is_none() {
        return Error::INVALID.code.0;
    }
    let raw = abi::Creation {
        context: raw.context,
        resource: raw.stage,
        keyed: None,
        auxiliary: None,
        buffer: None,
        retained: None,
    };
    // SAFETY: This resource callback has the same signature and ownership rule
    // as Open.stage, with the original context and pending lifetime unchanged.
    let (staging, lease) = unsafe { creation::import(raw, owner.state.limits) };
    let opened = Open::new(staging, Creation::resource);
    let result = owner
        .state
        .provider
        .try_borrow_mut()
        .map_err(|_| Error::BUSY)
        .and_then(|mut provider| provider.open(&opened));
    // SAFETY: Output pointers were checked before invoking the provider. The
    // accepted pending owner retains the staging lease until successful finish.
    unsafe {
        calls::complete_owned(
            &owner.state,
            result,
            output,
            error,
            Some(Box::new(lease)),
        )
    }
}
