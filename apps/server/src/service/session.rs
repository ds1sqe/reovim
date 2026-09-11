//! Assembly selects one shared editing context; clients keep private roots.

use alloc::format;
use core::future::Future;
use reovim_kernel_interface::{
    Connection, Contract, Message, RequestError, ResourceId,
    provider::Permissions, session,
};
use reovim_kernel_runtime::Runtime;
use std::io;

pub(super) fn create(
    runtime: &mut Runtime,
) -> io::Result<(Connection, ResourceId)> {
    let model = runtime
        .register_model(
            "models/session",
            reovim_representation_basic::heading(),
        )
        .map_err(|(error, _)| io::Error::other(error))?;
    let owner = reovim_sdk::local(
        runtime
            .connect(&[(model, Permissions::ALL)])
            .map_err(io::Error::other)?,
    );
    let model = owner
        .resolve("models/session")
        .into_inner()
        .map_err(io::Error::other)?;
    let observed = finish(
        runtime,
        session::create(
            &owner,
            model,
            &Message::new(
                Contract::new("heading.initial", 1),
                b"# Shared session\n",
            ),
        ),
    )?;
    let parent =
        owner.resolve("sessions").into_inner().map_err(io::Error::other)?;
    let entry = owner
        .children(parent)
        .into_inner()
        .map_err(io::Error::other)?
        .into_iter()
        .find(|entry| entry.resource == observed.session)
        .ok_or_else(|| io::Error::other("shared Session is not named"))?;
    let resource = runtime
        .resource(&format!("sessions/{}", entry.name))
        .map_err(io::Error::other)?;
    finish(runtime, session::release(&owner, observed.snapshot))?;
    Ok((owner, resource))
}
fn finish<T>(
    runtime: &mut Runtime,
    future: impl Future<Output = Result<T, RequestError>>,
) -> io::Result<T> {
    reovim_host_reference::run(future, |context| {
        runtime.advance(context).map(|_| ())
    })
    .map_err(io::Error::other)?
    .map_err(io::Error::other)
}
