//! Captured caller submissions, shared by Runtime and transport adapters.
//!
//! Example: reserve a request ID, capture a `Call`, validate its envelope,
//! then admit it under the authenticated caller's access and custody rules.

use alloc::rc::Rc;
use core::task::Context;

use crate::{
    Error, Outcome, ResourceId, Value, call::Cancellation, resource::AccessId,
};

mod dispatch;
pub use dispatch::{Caller, CallerOps};

/// Passive custody evidence; observing it never executes a provider.
/// Example: a lost reply is recovered as the same retained Outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Status {
    /// No submission has been admitted for this reserved ID.
    Reserved,
    /// Original work is awaiting evidence, including reconciliation.
    Pending,
    /// Immutable generation; earlier UNKNOWN readers retain their evidence.
    Outcome(Rc<Outcome>),
}

/// Closed common operation set; module commands belong to `Ioctl`.
/// Example: `Exec` invokes the bound executable without a command number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Operation {
    /// Resolve a locator with zero initial operation rights.
    Acquire = 1,
    /// Enumerate offered declarations.
    Interfaces = 2,
    /// Qualify existing opened access with an exact declaration.
    Bind = 3,
    /// Read common Resource metadata.
    Info = 4,
    /// Enumerate container children.
    Entries = 5,
    /// Read through opened I/O.
    Read = 6,
    /// Write through opened I/O.
    Write = 7,
    /// Read at an explicit byte offset.
    ReadAt = 8,
    /// Write at an explicit byte offset.
    WriteAt = 9,
    /// Invoke the bound executable.
    Exec = 10,
    /// Invoke a nonzero command local to the bound control interface.
    Ioctl = 11,
    /// Fence and clean up opened access.
    Close = 12,
    /// Create through the parent's default factory.
    Create = 13,
    /// Delete a child name.
    Delete = 14,
    /// Copy into a destination name.
    Copy = 15,
    /// Link a source into a destination name.
    Link = 16,
    /// Rename a child into a destination parent.
    Rename = 17,
    /// Receive one declared packet or end indication.
    Receive = 18,
    /// Send one declared packet for local acceptance.
    Send = 19,
    /// Capture opened state with an independent close fence.
    Capture = 20,
    /// Change this opened handle's held rights within caller grants.
    SetAccess = 21,
}

impl TryFrom<u32> for Operation {
    type Error = Error;

    fn try_from(code: u32) -> Result<Self, Error> {
        match code {
            1 => Ok(Self::Acquire),
            2 => Ok(Self::Interfaces),
            3 => Ok(Self::Bind),
            4 => Ok(Self::Info),
            5 => Ok(Self::Entries),
            6 => Ok(Self::Read),
            7 => Ok(Self::Write),
            8 => Ok(Self::ReadAt),
            9 => Ok(Self::WriteAt),
            10 => Ok(Self::Exec),
            11 => Ok(Self::Ioctl),
            12 => Ok(Self::Close),
            13 => Ok(Self::Create),
            14 => Ok(Self::Delete),
            15 => Ok(Self::Copy),
            16 => Ok(Self::Link),
            17 => Ok(Self::Rename),
            18 => Ok(Self::Receive),
            19 => Ok(Self::Send),
            20 => Ok(Self::Capture),
            21 => Ok(Self::SetAccess),
            _ => Err(Error::UNSUPPORTED),
        }
    }
}

/// Expected immutable state, e.g. the Buffer revision an edit was based on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Expected {
    /// Qualified Resource identity; this grants no access.
    pub subject: ResourceId,
    /// Revision to revalidate before installation.
    pub revision: u64,
}

/// Owned submission; exact equality joins original work, including Ref tokens.
/// Example: changing input under a previously submitted ID causes Conflict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Call {
    /// Nonzero ID previously reserved in this caller's custody.
    pub request: u64,
    /// Common operation, e.g. Exec or `SetAccess`.
    pub operation: Operation,
    /// Caller-owned access; zero only for Acquire.
    pub subject: AccessId,
    /// Nonzero only for Ioctl, local to its bound interface.
    pub command: u32,
    /// Optional captured state; never inferred from payload length.
    pub expected: Option<Expected>,
    /// Captured canonical bytes and ordered access slots.
    pub input: Value,
}

impl Call {
    /// Checks envelope shape before lookup or provider execution.
    ///
    /// Runtime must still check reservation, liveness, grants, declaration,
    /// input values and expected-state applicability. Example: an Exec with
    /// a command number is rejected here before any executable is selected.
    /// # Errors
    /// Invalid identifies a zero request, invalid subject or unused command.
    pub fn validate(&self) -> Result<(), Error> {
        if self.request == 0
            || (self.subject.0 == 0) != (self.operation == Operation::Acquire)
            || (self.command != 0) != (self.operation == Operation::Ioctl)
            || self.expected.is_some_and(|value| value.subject.serial == 0)
        {
            return Err(Error::INVALID);
        }
        Ok(())
    }
}
