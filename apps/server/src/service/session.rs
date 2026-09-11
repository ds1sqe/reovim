//! Assembly selects one shared editing context; clients keep private roots.

use alloc::{boxed::Box, format};
use core::{future::Future, task::Poll};
use reovim_kernel_interface::{RequestError, ResourceId, session};
use reovim_kernel_runtime::{Binding, Runtime};
use std::{
    io,
    sync::Arc,
    task::{Context, Waker},
    thread,
};

pub(super) fn create(
    runtime: &mut Runtime,
    binding: &Binding,
    selected: &crate::boot::Session,
) -> io::Result<ResourceId> {
    let owner = reovim_sdk::local(binding.connection());
    let model = owner
        .resolve(&selected.model)
        .into_inner()
        .map_err(io::Error::other)?;
    let observed =
        finish(runtime, session::create(&owner, model, &selected.initial))?;
    let parent =
        owner.resolve("sessions").into_inner().map_err(io::Error::other)?;
    let entry = owner
        .children(parent)
        .into_inner()
        .map_err(io::Error::other)?
        .into_iter()
        .find(|entry| entry.resource == observed.session)
        .ok_or_else(|| io::Error::other("shared Session is not named"))?;
    let resource = binding
        .resource(&format!("sessions/{}", entry.name))
        .map_err(io::Error::other)?;
    finish(runtime, session::release(&owner, observed.snapshot))?;
    Ok(resource)
}
pub(super) fn finish<T>(
    runtime: &mut Runtime,
    future: impl Future<Output = Result<T, RequestError>>,
) -> io::Result<T> {
    let waker = Waker::from(Arc::new(super::OwnerWake(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        if let Poll::Ready(result) = future.as_mut().poll(&mut context) {
            return result.map_err(io::Error::other);
        }
        let progress =
            runtime.advance(&mut context).map_err(io::Error::other)?;
        if progress != reovim_kernel_interface::driver::Progress::More {
            thread::park_timeout(std::time::Duration::from_millis(1));
        }
    }
}
