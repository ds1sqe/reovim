use super::backend::{self, Status};
use crate::access::Reply;
use crate::{
    Connection, Contract, Error, Message, ResourceId, buffer::BufferState,
};
use alloc::boxed::Box;

/// The original save Work owns proof, reconciliation and backend cleanup.
///
/// Copying this report does not retain that Work. Close it after adoption;
/// Busy means Runtime must progress its retained cleanup before release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Report {
    pub selected: BufferState,
    pub target: ResourceId,
    pub work: ResourceId,
    pub status: Status,
    pub recovery: Box<str>,
}

/// Captures exact selected state and the Buffer's original backing/base.
///
/// Target WRITE authority is checked independently from Buffer WRITE.
/// Backend parameters are opaque; no caller base bookkeeping is accepted.
#[must_use = "Adopt proof or retain the original save Work for recovery"]
pub fn save(
    connection: &Connection,
    selected: BufferState,
    parameters: &[u8],
) -> Reply {
    let mut input = Message::reference(
        Contract::new("kernel.buffer.save", 1),
        selected.buffer,
    );
    input.data = [selected.revision.to_le_bytes().as_slice(), parameters]
        .concat()
        .into();
    connection.write(selected.buffer, &input)
}

/// Awaits reconciliation of the same save and returns its refined report.
/// Concurrent requests join that reconciliation; none republishes the state.
#[must_use = "Read the original Work's refined result after reconciliation"]
pub fn reconcile(connection: &Connection, work: ResourceId) -> Reply {
    connection.write(
        work,
        &Message::new(Contract::new("kernel.request.reconcile", 1), &[]),
    )
}

/// Decodes revision:u64, status-length:u8, ten zero-padded status bytes and
/// the UTF-8 recovery locator. References name Buffer, target and Work.
/// # Errors
/// Rejects malformed save metadata or evidence, retaining the caller's owner.
pub fn report(reply: &Message) -> Result<Report, Error> {
    if reply.contract != Contract::new("kernel.buffer.save", 1)
        || reply.resources.len() != 3
        || reply.data.len() < 20
    {
        return Err(Error::INVALID);
    }
    let revision = u64::from_le_bytes(
        reply.data[..8].try_into().map_err(|_| Error::INVALID)?,
    );
    let length = usize::from(reply.data[8]);
    let end = 9_usize.checked_add(length).ok_or(Error::INVALID)?;
    let status = backend::status(&Message::new(
        Contract::new("resource.effect", 1),
        reply.data.get(9..end).ok_or(Error::INVALID)?,
    ))?;
    if reply
        .data
        .get(end..19)
        .ok_or(Error::INVALID)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::INVALID);
    }
    let recovery =
        core::str::from_utf8(reply.data.get(19..).ok_or(Error::INVALID)?)
            .map_err(|_| Error::INVALID)?;
    if recovery.is_empty() {
        return Err(Error::INVALID);
    }
    Ok(Report {
        selected: BufferState { buffer: reply.resources[0], revision },
        target: reply.resources[1],
        work: reply.resources[2],
        status,
        recovery: recovery.into(),
    })
}
