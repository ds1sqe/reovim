//! Owned live/historical context views through common Resource messages.
//! Read the Model's declared schema on the participation to obtain a separately
//! owned snapshot. Participation updates and follow are not yet wired.

use crate::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    access::Reply,
};

/// Creates an owned participation initially reading live Session context.
#[must_use = "Adopt or recover the returned participation"]
pub fn attach(connection: &Connection, session: ResourceId) -> Reply {
    connection.read(
        session,
        &Message::new(Contract::new("kernel.session.attach", 1), &[]),
    )
}

/// Decodes the independently owned participation handle.
/// # Errors
/// Rejects malformed attach results without guessing ownership.
pub fn attached(reply: &Message) -> Result<ResourceId, Error> {
    if reply.contract != Contract::new("kernel.session.attach", 1)
        || !reply.data.is_empty()
    {
        return Err(Error::INVALID);
    }
    let [participant] = reply.resources.as_ref() else {
        return Err(Error::INVALID);
    };
    Ok(*participant)
}

/// Selects a retained snapshot without changing live context or Buffers.
#[must_use = "Adopt or recover the selected view"]
pub fn seek(
    connection: &Connection,
    participant: ResourceId,
    snapshot: ResourceId,
) -> Reply {
    connection.write(
        participant,
        &Message::reference(Contract::new("kernel.session.seek", 1), snapshot),
    )
}

/// Ends owned participation; independently returned snapshots survive.
/// # Errors
/// Busy retains unfinished requests; foreign owners cannot detach this view.
#[allow(clippy::future_not_send)]
pub async fn detach(
    connection: &Connection,
    participant: ResourceId,
) -> Result<(), RequestError> {
    super::control(connection, participant, "kernel.session.detach").await
}
