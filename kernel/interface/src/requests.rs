//! Retained-result envelope; retrieval and original failure are distinct.

use crate::buffer::integer;
use crate::{Connection, Contract, Error, Message, RequestError, ResourceId};
use alloc::vec::Vec;

const NAME: &str = "kernel.request.result";

/// Retrieves the original outcome without repeating its effect.
///
/// Reads refer to the same work and returned Resource ownership. Publication
/// reconciliation may refine effect evidence; earlier Messages stay unchanged.
/// Close Work through [`Connection::close`] after adopting its result.
///
/// # Errors
/// Outer errors concern retrieval/decoding; inner errors belong to the work.
#[allow(clippy::future_not_send)] // Connections are local to one executor.
pub async fn result(
    connection: &Connection,
    work: ResourceId,
) -> Result<Result<Message, RequestError>, RequestError> {
    let envelope = connection
        .read(work, &Message::new(Contract::new(NAME, 1), &[]))
        .await?;
    decode(envelope).map_err(RequestError::Failed)
}

/// Encodes one settled outcome. Reference slots keep their ordinary identity.
///
/// # Errors
/// Allocation/size exhaustion leaves responsibility with the retained owner.
pub fn encode(outcome: Result<Message, Error>) -> Result<Message, Error> {
    let mut data = Vec::new();
    let resources = match outcome {
        Ok(message) => {
            let name = message.contract.name.as_bytes();
            let size = 13_usize
                .checked_add(name.len())
                .and_then(|size| size.checked_add(message.data.len()))
                .ok_or(Error::PRESSURE)?;
            data.try_reserve_exact(size).map_err(|_| Error::PRESSURE)?;
            data.push(0);
            data.extend_from_slice(&message.contract.version.to_le_bytes());
            data.extend_from_slice(&(name.len() as u64).to_le_bytes());
            data.extend_from_slice(name);
            data.extend_from_slice(&message.data);
            message.resources
        }
        Err(error) => {
            data.try_reserve_exact(9).map_err(|_| Error::PRESSURE)?;
            data.push(1);
            data.extend_from_slice(&error.code.0.to_le_bytes());
            error.id.into_iter().collect()
        }
    };
    Ok(Message {
        contract: Contract::new(NAME, 1),
        data: data.into(),
        resources,
    })
}

/// Encodes an observed outcome, including an original lost-reply locator.
///
/// Tag 2 carries cause:u64 followed by its UTF-8 locator; reference slots carry
/// the optional cause diagnostic. Tags 0 and 1 retain their settled encodings.
/// # Errors
/// Allocation or size exhaustion retains responsibility with the work owner.
pub fn encode_outcome(
    outcome: Result<Message, RequestError>,
) -> Result<Message, Error> {
    match outcome {
        Ok(message) => encode(Ok(message)),
        Err(RequestError::Rejected(error) | RequestError::Failed(error)) => {
            encode(Err(error))
        }
        Err(RequestError::Unknown { cause, recovery }) => {
            if recovery.is_empty() {
                return Err(Error::INVALID);
            }
            let mut data = Vec::new();
            let length =
                recovery.len().checked_add(9).ok_or(Error::PRESSURE)?;
            data.try_reserve_exact(length).map_err(|_| Error::PRESSURE)?;
            data.push(2);
            data.extend_from_slice(&cause.code.0.to_le_bytes());
            data.extend_from_slice(recovery.as_bytes());
            Ok(Message {
                contract: Contract::new(NAME, 1),
                data: data.into(),
                resources: cause.id.into_iter().collect(),
            })
        }
    }
}

/// Decodes the common envelope, keeping retrieval errors outside its value.
///
/// # Errors
/// Rejects an unknown profile, malformed payload or invalid error references.
pub fn decode(
    envelope: Message,
) -> Result<Result<Message, RequestError>, Error> {
    if envelope.contract != Contract::new(NAME, 1) {
        return Err(Error::INVALID);
    }
    match envelope.data.first() {
        Some(0) => success(envelope).map(Ok),
        Some(1) => {
            failure(&envelope).map(|error| Err(RequestError::Failed(error)))
        }
        Some(2) => unknown(&envelope).map(Err),
        _ => Err(Error::INVALID),
    }
}
fn unknown(envelope: &Message) -> Result<RequestError, Error> {
    if envelope.resources.len() > 1 {
        return Err(Error::INVALID);
    }
    let data = envelope.data.get(1..9).ok_or(Error::INVALID)?;
    let recovery = core::str::from_utf8(&envelope.data[9..])
        .map_err(|_| Error::INVALID)?;
    if recovery.is_empty() {
        return Err(Error::INVALID);
    }
    let mut cause = Error::new(integer(data)?);
    cause.id = envelope.resources.first().copied();
    Ok(RequestError::Unknown { cause, recovery: recovery.into() })
}
fn success(envelope: Message) -> Result<Message, Error> {
    let prefix = envelope.data.get(..13).ok_or(Error::INVALID)?;
    let version = u32::from_le_bytes(
        prefix[1..5].try_into().map_err(|_| Error::INVALID)?,
    );
    let length = usize::try_from(integer(&prefix[5..13])?)
        .map_err(|_| Error::INVALID)?;
    let end = 13_usize.checked_add(length).ok_or(Error::INVALID)?;
    let name =
        core::str::from_utf8(envelope.data.get(13..end).ok_or(Error::INVALID)?)
            .map_err(|_| Error::INVALID)?;
    let contract = Contract::new(name, version);
    Ok(Message {
        contract,
        data: envelope.data[end..].into(),
        resources: envelope.resources,
    })
}
fn failure(envelope: &Message) -> Result<Error, Error> {
    if envelope.data.len() != 9 || envelope.resources.len() > 1 {
        return Err(Error::INVALID);
    }
    let mut error = Error::new(integer(&envelope.data[1..])?);
    error.id = envelope.resources.first().copied();
    Ok(error)
}
