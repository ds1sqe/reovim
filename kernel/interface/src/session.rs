//! Retained context through the common immutable model and Resource contracts.

use crate::{Connection, Contract, Error, Message, RequestError, ResourceId};
use alloc::vec::Vec;
mod participation;
mod watch;
pub use participation::{attach, attached, detach, seek};
pub use watch::{Change, Watching, change, next, observe, watching};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub session: ResourceId,
    pub snapshot: ResourceId,
    pub value: Message,
}

/// Creates retained context using the selected model's initialization schema.
/// # Errors
/// Validation/admission failures and ordinary retained request recovery apply.
#[allow(clippy::future_not_send)] // Connections belong to one executor.
pub async fn create(
    connection: &Connection,
    model: ResourceId,
    initial: &Message,
) -> Result<Observation, RequestError> {
    let input = encode_initial(initial).map_err(RequestError::Rejected)?;
    observation(connection.write(model, &input).await?)
        .map_err(RequestError::Failed)
}

/// Decodes an observation; its two references have independent lifetimes.
/// # Errors
/// Rejects malformed reference slots without guessing a schema.
pub fn observation(mut reply: Message) -> Result<Observation, Error> {
    let [session, snapshot, ..] = reply.resources.as_ref() else {
        return Err(Error::INVALID);
    };
    let (session, snapshot) = (*session, *snapshot);
    reply.resources = reply.resources[2..].into();
    Ok(Observation { session, snapshot, value: reply })
}

/// Uses a retained snapshot as an opaque update precondition.
/// # Errors
/// Semantic reference slots follow the two currentness references unchanged.
pub fn update(
    observed: &Observation,
    mut proposed: Message,
) -> Result<Message, Error> {
    let mut resources = Vec::new();
    resources
        .try_reserve(2 + proposed.resources.len())
        .map_err(|_| Error::PRESSURE)?;
    resources.extend_from_slice(&[observed.session, observed.snapshot]);
    resources.extend_from_slice(&proposed.resources);
    proposed.resources = resources.into();
    Ok(proposed)
}

/// Closes owned live context; separately retained snapshots survive.
/// # Errors
/// Busy preserves retained work; only the Session owner may close it.
#[allow(clippy::future_not_send)] // Connections belong to one executor.
pub async fn close(
    connection: &Connection,
    session: ResourceId,
) -> Result<(), RequestError> {
    control(connection, session, "kernel.session.close").await
}

/// Releases an owned snapshot, preserving already admitted work.
/// # Errors
/// Missing/foreign ownership and ordinary request failures apply.
#[allow(clippy::future_not_send)] // Connections belong to one executor.
pub async fn release(
    connection: &Connection,
    snapshot: ResourceId,
) -> Result<(), RequestError> {
    control(connection, snapshot, "kernel.session.release").await
}

#[allow(clippy::future_not_send)] // Connections belong to one executor.
async fn control(
    connection: &Connection,
    subject: ResourceId,
    name: &str,
) -> Result<(), RequestError> {
    let contract = Contract::new(name, 1);
    let reply =
        connection.write(subject, &Message::new(contract.clone(), &[])).await?;
    if reply.contract != contract
        || !reply.data.is_empty()
        || !reply.resources.is_empty()
    {
        return Err(RequestError::Failed(Error::PROVIDER));
    }
    Ok(())
}

/// Identifies the common wrapper independently of the model's create schema.
#[must_use]
pub fn is_create(contract: &Contract) -> bool {
    contract.name.as_ref() == "kernel.session.create" && contract.version == 1
}

/// Decodes version:u32, name length:u32, UTF-8 name, then model payload.
/// # Errors
/// Rejects unsupported wrappers and malformed names; Runtime validates slots.
pub fn decode_initial(input: &Message) -> Result<Message, Error> {
    if !is_create(&input.contract) {
        return Err(Error::INVALID);
    }
    let prefix = input.data.get(..8).ok_or(Error::INVALID)?;
    let version =
        u32::from_le_bytes(prefix[..4].try_into().map_err(|_| Error::INVALID)?);
    let length = usize::try_from(u32::from_le_bytes(
        prefix[4..].try_into().map_err(|_| Error::INVALID)?,
    ))
    .map_err(|_| Error::INVALID)?;
    let end = 8_usize.checked_add(length).ok_or(Error::INVALID)?;
    let name =
        core::str::from_utf8(input.data.get(8..end).ok_or(Error::INVALID)?)
            .map_err(|_| Error::INVALID)?;
    Ok(Message {
        contract: Contract::new(name, version),
        data: input.data[end..].into(),
        resources: input.resources.clone(),
    })
}

fn encode_initial(initial: &Message) -> Result<Message, Error> {
    let length = u32::try_from(initial.contract.name.len())
        .map_err(|_| Error::PRESSURE)?;
    let capacity = 8_usize
        .checked_add(initial.contract.name.len())
        .and_then(|n| n.checked_add(initial.data.len()))
        .ok_or(Error::PRESSURE)?;
    let mut data = Vec::new();
    data.try_reserve_exact(capacity).map_err(|_| Error::PRESSURE)?;
    data.extend_from_slice(&initial.contract.version.to_le_bytes());
    data.extend_from_slice(&length.to_le_bytes());
    data.extend_from_slice(initial.contract.name.as_bytes());
    data.extend_from_slice(&initial.data);
    Ok(Message {
        contract: Contract::new("kernel.session.create", 1),
        data: data.into(),
        resources: initial.resources.clone(),
    })
}
