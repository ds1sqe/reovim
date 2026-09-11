#![no_std]
#![forbid(unsafe_code)]
//! Editing providers and optional caller helpers over Resource requests.

extern crate alloc;
pub mod native;

use alloc::{rc::Rc, vec};
use reovim_kernel_interface::{
    Connection, Contract, Message, RequestError, ResourceId,
    buffer::{self, Observation},
    driver::{DriverOps, Lifecycle, Progress},
    module::{Factory, Module},
    provider::Contribution,
};

/// Supplies heading and keyed-state models to the owning Runtime.
/// ```
/// let factory = reovim_module_editing::factory();
/// // Assembly registers this declaration; activation drives its constructor.
/// ```
#[must_use]
pub fn factory() -> Factory {
    Factory::new((), |(), _| Ok(construct()), Rc::new(()))
}

fn construct() -> Module {
    Module::new(
        Lifecycle::Constructed,
        DriverOps {
            status: |state| *state,
            advance: |state, _| {
                if *state == Lifecycle::Closed {
                    return Ok(Progress::Stopped);
                }
                *state = Lifecycle::Ready;
                Ok(Progress::Idle)
            },
            close: |state| {
                *state = Lifecycle::Closed;
                Ok(())
            },
        },
        vec![
            Contribution::Model {
                path: "models/heading".into(),
                provider: reovim_representation_basic::heading(),
            },
            Contribution::Model {
                path: "models/keys".into(),
                provider: reovim_representation_basic::keyed(),
            },
        ],
        Rc::new(()),
    )
}

/// Creates a Buffer using an explicitly selected heading model.
/// ```
/// # async fn example(kernel: &reovim_kernel_interface::Connection)
/// # -> Result<(), reovim_kernel_interface::RequestError> {
/// let model = kernel.resolve("models/heading").await?;
/// let opened = reovim_module_editing::open(kernel, model, b"# Notes").await?;
/// reovim_kernel_interface::buffer::close(kernel, opened.state,
///     reovim_kernel_interface::buffer::ClosePolicy::DiscardLocal).await?;
/// # Ok(()) }
/// ```
/// # Errors
/// Preserves ordinary request failures and recovery for unknown outcomes.
#[allow(clippy::future_not_send)] // Granted access belongs to one executor.
pub async fn open(
    kernel: &Connection,
    model: ResourceId,
    source: &[u8],
) -> Result<Observation, RequestError> {
    let reply = kernel
        .write(
            model,
            &Message::new(Contract::new("heading.initial", 1), source),
        )
        .await?;
    buffer::observation(reply).map_err(RequestError::Failed)
}

/// Replaces the heading using captured currentness; leaves other source intact.
/// ```
/// # async fn example(kernel: &reovim_kernel_interface::Connection,
/// # before: reovim_kernel_interface::buffer::Observation)
/// # -> Result<(), reovim_kernel_interface::RequestError> {
/// let changed =
///     reovim_module_editing::replace(kernel, &before, b"Draft").await?;
/// // Keep changed.state for the next edit or explicit Buffer close.
/// # Ok(()) }
/// ```
/// # Errors
/// Stale edits fail; uncertain outcomes retain their original recovery access.
#[allow(clippy::future_not_send)] // Granted access belongs to one executor.
pub async fn replace(
    kernel: &Connection,
    before: &Observation,
    title: &[u8],
) -> Result<Observation, RequestError> {
    let input = buffer::edit(
        before.state,
        Message::new(Contract::new("heading.replace", 1), title),
    );
    let reply = kernel.write(before.state.buffer, &input).await?;
    buffer::observation(reply).map_err(RequestError::Failed)
}
