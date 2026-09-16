//! Declared packet directions; protocol metadata belongs to the packet Type.
//!
//! Example: a device can offer receive without byte reads or packet sends.

use super::ResourceProvider;

mod validation;
use crate::{Error, Pending, Value, declaration::ValueType};

/// Oversized packet policy, e.g. Reject leaves the next packet unconsumed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Oversize {
    /// Reject without consuming the packet.
    Reject,
    /// Consume and return a truncated Item.
    Truncate,
    /// Consume and report Dropped instead of an Item.
    Discard,
}

/// One receive request; e.g. maximum 1024 with `Oversize::Reject`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Receive {
    /// Maximum packet bytes accepted by this request; zero permits empty items.
    pub maximum: u64,
    /// Policy applied to a packet exceeding maximum.
    pub oversize: Oversize,
}

/// Receives one packet under the implementation's declared queue policy.
/// Example: return Pending while an empty queue waits for arrival.
pub type ReceiveFn<State> = fn(&mut State, Receive) -> Result<Pending, Error>;

/// Sends one admitted canonical packet; pending work captures borrowed input.
/// Example: a datagram provider returns a local acceptance receipt.
pub type SendFn<State> = fn(&mut State, &Value) -> Result<Pending, Error>;

/// A receive direction with its packet Type, independent of byte read support.
/// Example: Bytes(1024) declares packet data without protocol metadata.
pub struct ReceiveOps<State> {
    /// Canonical Type of the Item value, excluding generic receive metadata.
    pub packet: ValueType,
    /// Returns Item, Dropped or End through ordinary pending custody.
    pub begin: ReceiveFn<State>,
}

/// A send direction with its packet Type, independent of byte write support.
/// Example: a Record can carry a destination plus opaque packet contents.
pub struct SendOps<State> {
    /// Canonical Type of the complete input packet.
    pub packet: ValueType,
    /// Returns a canonical U64 local accepted-byte receipt.
    pub begin: SendFn<State>,
}

impl ResourceProvider {
    /// Returns the received packet's checked Type without provider execution.
    /// Example: None advertises no receive; Unit advertises an empty value.
    #[must_use]
    pub fn receive_type(&self) -> Option<&ValueType> {
        self.implementation.receive_type()
    }

    /// Returns the sent packet's checked Type, e.g. a datagram Record.
    #[must_use]
    pub fn send_type(&self) -> Option<&ValueType> {
        self.implementation.send_type()
    }

    /// Begins an admitted receive, e.g. waiting for one complete message.
    /// # Errors
    /// Closed or unsupported directions invoke no provider callback. Accepted
    /// work retains its code and original recovery/cleanup obligation.
    pub fn receive(&mut self, request: Receive) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        let mut pending = self.implementation.receive(request)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Sends an admitted canonical packet, e.g. a destination/data Record.
    /// Runtime validates its Type, references and grants before this entry.
    /// # Errors
    /// Closed or unsupported directions invoke no callback. Pending work owns
    /// any retained input and describes local acceptance, not peer delivery.
    pub fn send(&mut self, packet: &Value) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        let mut pending = self.implementation.send(packet)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }
}

impl Receive {
    /// Encodes the common native/service options; e.g. Reject has choice zero.
    #[must_use]
    pub fn encode(self) -> [u8; 12] {
        let mut bytes = [0; 12];
        bytes[..8].copy_from_slice(&self.maximum.to_le_bytes());
        let oversize: u32 = match self.oversize {
            Oversize::Reject => 0,
            Oversize::Truncate => 1,
            Oversize::Discard => 2,
        };
        bytes[8..].copy_from_slice(&oversize.to_le_bytes());
        bytes
    }

    /// Decodes options, e.g. a 12-byte maximum/oversize record.
    /// # Errors
    /// Rejects trailing bytes, short input and undeclared oversize choices.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let (maximum, oversize) =
            bytes.split_first_chunk::<8>().ok_or(Error::INVALID)?;
        let maximum = u64::from_le_bytes(*maximum);
        let oversize: [u8; 4] =
            oversize.try_into().map_err(|_| Error::INVALID)?;
        let oversize = match u32::from_le_bytes(oversize) {
            0 => Oversize::Reject,
            1 => Oversize::Truncate,
            2 => Oversize::Discard,
            _ => return Err(Error::INVALID),
        };
        Ok(Self { maximum, oversize })
    }

    /// Checks one received Item/Dropped/End and its declared packet refs.
    /// Example: Reject disallows a truncated Item; empty packets remain Items.
    /// Reference checks inspect metadata only, as in `ValueType::validate`.
    /// # Errors
    /// Malformed values, impossible oversize reports and exceeded bounds reject
    /// before reference ownership can transfer. Callback errors are preserved.
    pub fn validate(
        self,
        packet: &ValueType,
        value: &Value,
        limits: crate::declaration::ValueLimits,
        check: impl FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
    ) -> Result<(), Error> {
        validation::receive(self, packet, value, limits, check)
    }
}
