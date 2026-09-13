//! Conditional publication through an independently retained evidence Resource.
//!
//! These calls are for publication coordinators, not caller-managed Buffer base
//! bookkeeping. The coordinator retains selected Model data and original base,
//! validates target WRITE authority, and owns its recovery before handoff.
//! The backend owns data/base profiles; Kernel does not parse them.
//!
//! `resource.publish` v1 WRITE takes backend-defined data and two READ slots:
//! immutable selected data, then the original base. Before effects, the backend
//! reserves a retained evidence Resource and captures needed inputs. Its reply
//! carries one READ|WRITE evidence reference and an opaque release key in data.
//! Recover lost delivery through the original request. Do not publish again.
//! A missing/malformed result after invocation does not prove no effect. Only
//! explicit evidence establishes certainty; keep the original recovery owner.
//!
//! Evidence supplies `resource.effect` v1 READ and
//! `resource.effect.reconcile` v1 WRITE, both empty-input/data-only results.
//! Reconcile examines or progresses only the original effect. Unknown/partial
//! evidence must remain owned and accessible until settlement or transfer.
//! It is not made safe to release by an ordinary successful request envelope.
//!
//! Applied evidence offers an independent `resource.base` v1 read view of its
//! exact committed base. It must not acquire a later source version. A caller
//! retains that view independently before releasing evidence. Acquisition can
//! fail without changing the known external outcome; adoption remains owned.
//!
//! `resource.effect.release` v1 WRITE on the original target takes the opaque
//! release key and no references. It rejects unresolved or captured evidence.
//! Release keys never retarget replacement records. Generic access close must
//! not destroy unresolved evidence or fence its only reconciliation path.

use crate::access::Reply;
use crate::{Connection, Contract, Error, ErrorCode, Message, ResourceId};
use alloc::{boxed::Box, vec::Vec};

/// Metadata for one backend-owned record; copies are not access leases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub evidence: ResourceId,
    pub release: Box<[u8]>,
}

/// Certainty reported by an evidence Resource, not delivery certainty.
///
/// The record itself is the reconciliation subject. A coordinator supplies its
/// own retained Work locator when exposing a public unknown/partial Effect.
/// Cause codes belong to the backend. This profile has no Error.id slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Applied,
    DefinitelyNotApplied(ErrorCode),
    OutcomeUnknown(ErrorCode),
    PartiallyApplied { cause: ErrorCode, atomicity_violated: bool },
}

/// Starts one conditional effect over immutable selected-data and base inputs.
#[must_use = "Adopt the evidence or recover the original request"]
pub fn publish(
    connection: &Connection,
    target: ResourceId,
    selected: ResourceId,
    base: ResourceId,
    data: &[u8],
) -> Reply {
    let mut input = Message::new(Contract::new("resource.publish", 1), data);
    input.resources = Box::new([selected, base]);
    connection.write(target, &input)
}

/// Decodes the backend's retained record and opaque release key.
/// # Errors
/// Rejects a foreign contract, absent release key or malformed reference slots.
pub fn receipt(reply: &Message) -> Result<Receipt, Error> {
    if reply.contract != Contract::new("resource.publish", 1)
        || reply.resources.len() != 1
        || reply.data.is_empty()
    {
        return Err(Error::INVALID);
    }
    Ok(Receipt { evidence: reply.resources[0], release: reply.data.clone() })
}

#[must_use = "Observe the retained effect evidence"]
pub fn result(connection: &Connection, evidence: ResourceId) -> Reply {
    connection.read(evidence, &Message::new(effect(), &[]))
}

#[must_use = "Retain the original effect until reconciliation settles"]
pub fn reconcile(connection: &Connection, evidence: ResourceId) -> Reply {
    connection.write(evidence, &Message::new(reconciliation(), &[]))
}

#[must_use = "Retry rejected evidence release without repeating the effect"]
pub fn release(
    connection: &Connection,
    target: ResourceId,
    receipt: &Receipt,
) -> Reply {
    connection.write(
        target,
        &Message::new(
            Contract::new("resource.effect.release", 1),
            &receipt.release,
        ),
    )
}

/// Encodes data-only effect evidence; allocate this before an external cut.
///
/// Integers are little-endian. Tags: 0 applied; 1 definitely not applied plus
/// cause:u64; 2 unknown plus cause:u64; 3 partially applied plus cause:u64 and
/// a boolean byte specifying an atomicity violation.
/// # Errors
/// Rejects another contract or allocation failure without altering the effect.
pub fn encode(contract: Contract, status: Status) -> Result<Message, Error> {
    if contract != effect() && contract != reconciliation() {
        return Err(Error::INVALID);
    }
    let mut data = Vec::new();
    data.try_reserve_exact(10).map_err(|_| Error::PRESSURE)?;
    match status {
        Status::Applied => data.push(0),
        Status::DefinitelyNotApplied(cause) => cause_data(&mut data, 1, cause),
        Status::OutcomeUnknown(cause) => cause_data(&mut data, 2, cause),
        Status::PartiallyApplied { cause, atomicity_violated } => {
            cause_data(&mut data, 3, cause);
            data.push(u8::from(atomicity_violated));
        }
    }
    Ok(Message { contract, data: data.into(), resources: Box::new([]) })
}

/// # Errors
/// Rejects malformed evidence, unknown flags and incidental reference slots.
pub fn status(reply: &Message) -> Result<Status, Error> {
    if (reply.contract != effect() && reply.contract != reconciliation())
        || !reply.resources.is_empty()
    {
        return Err(Error::INVALID);
    }
    if reply.data.as_ref() == [0] {
        return Ok(Status::Applied);
    }
    let cause = reply.data.get(1..9).ok_or(Error::INVALID)?;
    let cause = ErrorCode(u64::from_le_bytes(
        cause.try_into().map_err(|_| Error::INVALID)?,
    ));
    match (reply.data[0], reply.data.len()) {
        (1, 9) => Ok(Status::DefinitelyNotApplied(cause)),
        (2, 9) => Ok(Status::OutcomeUnknown(cause)),
        (3, 10) if reply.data[9] <= 1 => Ok(Status::PartiallyApplied {
            cause,
            atomicity_violated: reply.data[9] == 1,
        }),
        _ => Err(Error::INVALID),
    }
}

fn cause_data(data: &mut Vec<u8>, tag: u8, cause: ErrorCode) {
    data.push(tag);
    data.extend_from_slice(&cause.0.to_le_bytes());
}

fn effect() -> Contract {
    Contract::new("resource.effect", 1)
}
fn reconciliation() -> Contract {
    Contract::new("resource.effect.reconcile", 1)
}
