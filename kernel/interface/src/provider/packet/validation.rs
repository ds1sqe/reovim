use super::{Oversize, Receive};
use crate::{
    Error, Value,
    declaration::{ValueLimits, ValueType},
};

pub(super) fn receive(
    request: Receive,
    packet: &ValueType,
    value: &Value,
    limits: ValueLimits,
    check: impl FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
) -> Result<(), Error> {
    if value.data.len() > limits.bytes
        || value.references.len() > limits.references
    {
        return Err(Error::PRESSURE);
    }
    let (tag, data) = value.data.split_at_checked(4).ok_or(Error::INVALID)?;
    match u32::from_le_bytes(tag.try_into().unwrap()) {
        0 => item(request, packet, data, value.references.len(), limits, check),
        1 if value.references.is_empty() => dropped(request, data, limits),
        2 if value.references.is_empty() && data.is_empty() => {
            if limits.elements < 2 {
                return Err(Error::PRESSURE);
            }
            Ok(())
        }
        _ => Err(Error::INVALID),
    }
}

fn item(
    request: Receive,
    packet: &ValueType,
    data: &[u8],
    references: usize,
    mut limits: ValueLimits,
    check: impl FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
) -> Result<(), Error> {
    // Choice, Item record, Bool, original Option and its Unit/U64 payload.
    // The packet keeps its own full grammar depth inside the fixed envelope.
    limits.elements = limits.elements.checked_sub(5).ok_or(Error::PRESSURE)?;
    let tail = packet.prefix(data, references, limits, check)?;
    let (truncated, tail) = tail.split_first().ok_or(Error::INVALID)?;
    if *truncated > 1
        || (*truncated == 1 && request.oversize != Oversize::Truncate)
    {
        return Err(Error::INVALID);
    }
    if original(tail)?
        .is_some_and(|size| (size > request.maximum) != (*truncated == 1))
    {
        return Err(Error::INVALID);
    }
    Ok(())
}

fn dropped(
    request: Receive,
    data: &[u8],
    limits: ValueLimits,
) -> Result<(), Error> {
    if limits.elements < 4 {
        return Err(Error::PRESSURE);
    }
    if request.oversize != Oversize::Discard
        || original(data)?.is_some_and(|size| size <= request.maximum)
    {
        return Err(Error::INVALID);
    }
    Ok(())
}

fn original(data: &[u8]) -> Result<Option<u64>, Error> {
    let (tag, data) = data.split_at_checked(4).ok_or(Error::INVALID)?;
    match (u32::from_le_bytes(tag.try_into().unwrap()), data) {
        (0, []) => Ok(None),
        (1, bytes) => Ok(Some(u64::from_le_bytes(
            bytes.try_into().map_err(|_| Error::INVALID)?,
        ))),
        _ => Err(Error::INVALID),
    }
}
