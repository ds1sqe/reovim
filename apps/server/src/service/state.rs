//! Assembly creates shared state through public calls.

use alloc::{boxed::Box, format};
use core::{future::Future, task::Poll};
use reovim_kernel_interface::{
    Connection, RequestError, ResourceId, buffer, session,
};
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
    selected: &crate::boot::Initial,
) -> io::Result<ResourceId> {
    let owner = reovim_sdk::local(binding.connection());
    let model = owner
        .resolve(&selected.model)
        .into_inner()
        .map_err(io::Error::other)?;
    let observed =
        finish(runtime, session::create(&owner, model, &selected.initial))?;
    let resource = named(binding, &owner, "sessions", observed.session)?;
    finish(runtime, session::release(&owner, observed.snapshot))?;
    Ok(resource)
}

pub(super) fn buffer(
    runtime: &mut Runtime,
    binding: &Binding,
    selected: &crate::boot::Initial,
) -> io::Result<ResourceId> {
    let owner = reovim_sdk::local(binding.connection());
    let model = owner
        .resolve(&selected.model)
        .into_inner()
        .map_err(io::Error::other)?;
    let reply = finish(runtime, owner.write(model, &selected.initial))?;
    let observed = buffer::observation(reply).map_err(io::Error::other)?;
    named(binding, &owner, "buffers", observed.state.buffer)
}

fn named(
    binding: &Binding,
    owner: &Connection,
    container: &str,
    id: ResourceId,
) -> io::Result<ResourceId> {
    let parent =
        owner.resolve(container).into_inner().map_err(io::Error::other)?;
    let entry = owner
        .children(parent)
        .into_inner()
        .map_err(io::Error::other)?
        .into_iter()
        .find(|entry| entry.resource == id)
        .ok_or_else(|| io::Error::other("shared state is not named"))?;
    binding
        .resource(&format!("{container}/{}", entry.name))
        .map_err(io::Error::other)
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
