use super::{Imported, complete_begin, creation};
use alloc::rc::Rc;
use core::ptr;
use reovim_kernel_interface::{
    Error, Pending, abi,
    provider::{Creation, CreationOps, Open},
};

pub(super) fn begin(
    state: &mut Imported,
    opened: &Open,
) -> Result<Pending, Error> {
    let callback =
        state.open.and_then(|ops| ops.begin).ok_or(Error::UNSUPPORTED)?;
    let staging = Creation::new(
        opened.clone(),
        CreationOps { resource: Open::stage, keyed: None, auxiliary: None },
    );
    let owner =
        creation::Export::new(staging, state.instance.clone(), state.limits);
    let raw =
        abi::Open { context: owner.raw.context, stage: owner.raw.resource };
    let mut pending =
        abi::Pending { context: ptr::null_mut(), ops: ptr::null() };
    let mut error = abi::Error::default();
    // SAFETY: Import checked this callback/context. The Open record borrows
    // the same stable staging owner as its resource callback. Native pending
    // captures the record; owner survives cleanup. Outputs remain writable.
    let result = unsafe {
        callback(
            state.instance.provider.context,
            &raw const raw,
            &raw mut pending,
            &raw mut error,
        )
    };
    complete_begin(
        state,
        pending,
        result,
        error,
        Some(owner as Rc<dyn core::any::Any>),
    )
}
