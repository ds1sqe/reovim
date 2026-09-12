//! Source-owned byte units and codecs over the common Resource call path.
#![allow(clippy::future_not_send)] // Connection uses its owning executor.

use alloc::{boxed::Box, vec::Vec};
use core::ops::Range;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    buffer::{self, Observation, RetainedState},
};

/// Owned bytes from an exact retained state and declared source-byte range.
pub struct Window {
    snapshot: RetainedState,
    range: Range<u64>,
    bytes: Box<[u8]>,
}

impl Window {
    #[must_use]
    pub const fn snapshot(&self) -> &RetainedState {
        &self.snapshot
    }
    #[must_use]
    pub const fn range(&self) -> &Range<u64> {
        &self.range
    }
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Creates independent working bytes, with no backing association.
/// # Errors
/// Reports provider, input-bound or request-delivery failure.
pub async fn create(
    connection: &Connection,
    model: ResourceId,
    bytes: &[u8],
) -> Result<Observation, RequestError> {
    let reply = connection
        .write(model, &Message::new(Contract::new("source.initial", 1), bytes))
        .await?;
    descriptor(reply).map_err(Into::into)
}

/// Opens working source ranges from an immutable snapshot Resource.
/// # Errors
/// Rejects unavailable access, malformed snapshot metadata or failed delivery.
pub async fn open(
    connection: &Connection,
    model: ResourceId,
    snapshot: ResourceId,
) -> Result<Observation, RequestError> {
    let reply = connection
        .write(
            model,
            &Message::reference(Contract::new("source.open", 1), snapshot),
        )
        .await?;
    descriptor(reply).map_err(Into::into)
}

/// Reads only the selected window through its independently owned state access.
/// # Errors
/// Rejects expired access, invalid ranges, bad replies or failed delivery.
pub async fn read(
    connection: &Connection,
    snapshot: &RetainedState,
    range: Range<u64>,
) -> Result<Window, RequestError> {
    let request =
        Message::new(Contract::new("source.bytes", 1), &encode_range(&range));
    let reply = connection.read(snapshot.id, &request).await?;
    if reply.contract != request.contract
        || !reply.resources.is_empty()
        || reply.data.len() < 16
        || decode_range(&reply.data[..16])? != range
        || u64::try_from(reply.data.len() - 16).ok()
            != range.end.checked_sub(range.start)
    {
        return Err(Error::PROVIDER.into());
    }
    Ok(Window { snapshot: *snapshot, range, bytes: reply.data[16..].into() })
}

/// Replaces exact source bytes under the snapshot's Buffer currentness.
/// # Errors
/// Reports stale state, invalid ranges, pressure or failed delivery.
pub async fn splice(
    connection: &Connection,
    snapshot: &RetainedState,
    range: Range<u64>,
    bytes: &[u8],
) -> Result<Observation, RequestError> {
    let size = bytes.len().checked_add(16).ok_or(Error::PRESSURE)?;
    let mut data = Vec::new();
    data.try_reserve_exact(size).map_err(|_| Error::PRESSURE)?;
    data.extend_from_slice(&encode_range(&range));
    data.extend_from_slice(bytes);
    let request = buffer::edit(
        snapshot.state,
        Message::new(Contract::new("source.splice", 1), &data),
    );
    let reply = connection.write(snapshot.state.buffer, &request).await?;
    descriptor(reply).map_err(Into::into)
}

/// Returns source length from a model observation's finite descriptor.
/// # Errors
/// Rejects a foreign contract or malformed descriptor.
pub fn length(observed: &Observation) -> Result<u64, Error> {
    if observed.value.contract != Contract::new("source.bytes", 1)
        || !observed.value.resources.is_empty()
    {
        return Err(Error::INVALID);
    }
    buffer::integer(&observed.value.data)
}

fn descriptor(reply: Message) -> Result<Observation, Error> {
    let observed = buffer::observation(reply)?;
    length(&observed)?;
    Ok(observed)
}

pub(crate) fn encode_range(range: &Range<u64>) -> [u8; 16] {
    let mut data = [0; 16];
    data[..8].copy_from_slice(&range.start.to_le_bytes());
    data[8..].copy_from_slice(&range.end.to_le_bytes());
    data
}

pub(crate) fn decode_range(data: &[u8]) -> Result<Range<u64>, Error> {
    if data.len() != 16 {
        return Err(Error::INVALID);
    }
    Ok(buffer::integer(&data[..8])?..buffer::integer(&data[8..])?)
}
