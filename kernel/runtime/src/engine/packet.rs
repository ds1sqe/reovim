use super::{
    access::Scope,
    execution::Task,
    loans::Loans,
    outputs::{Outputs, Schema},
};
use alloc::rc::Rc;
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Outcome, Pending,
    call::Completion,
    caller::Call,
    declaration::{ValueLimits, ValueType},
    provider::{Receive, ResourceProvider},
};

pub(super) fn send(
    scope: &mut Scope,
    current: &Rc<RefCell<Scope>>,
    call: &Call,
) -> Result<Task, Error> {
    let handle = scope.live(call.subject)?;
    let packet =
        handle.implementation.send_type.as_ref().ok_or(Error::UNSUPPORTED)?;
    if handle.opened.borrow().held & 2 == 0 {
        return Err(Error::DENIED);
    }
    // Reserve a receipt large enough for Record{accepted_bytes:U64} before
    // the provider can accept the packet. Packet metadata stays type-owned.
    if scope.limits.values.bytes < 8 || scope.limits.values.elements < 2 {
        return Err(Error::PRESSURE);
    }
    let rights = scope.value_input(packet, &call.input)?;
    let entry = handle.entry.clone();
    let loans = Loans::admit(scope, current, &entry, &call.input, &rights)?;
    Ok(Task::send(entry, loans))
}

pub(super) fn validate_send(
    completion: &Completion,
    limits: ValueLimits,
) -> Result<Option<Error>, Error> {
    if completion.activity & !2 != 0 {
        return Err(Error::PROVIDER);
    }
    match &completion.outcome {
        Outcome::Ok(value) => {
            if value.data.len() != 8
                || !value.references.is_empty()
                || value.data.len() > limits.bytes
            {
                return Err(Error::PROVIDER);
            }
        }
        Outcome::Unknown { progress: Some(_), .. } => {
            return Err(Error::PROVIDER);
        }
        Outcome::Unknown { progress: None, .. } | Outcome::Failed(_) => {}
    }
    Ok(None)
}

pub(super) struct Received {
    packet: ValueType,
    request: Receive,
}

pub(super) fn receive(
    scope: &mut Scope,
    current: &Rc<RefCell<Scope>>,
    call: &Call,
) -> Result<Task, Error> {
    let handle = scope.live(call.subject)?;
    let packet = handle
        .implementation
        .receive_type
        .as_ref()
        .ok_or(Error::UNSUPPORTED)?;
    if handle.opened.borrow().held & 1 == 0 {
        return Err(Error::DENIED);
    }
    let request = Receive::decode(super::codec::no_refs(&call.input)?)?;
    if request.maximum > scope.limits.values.bytes as u64
        || scope.limits.values.bytes < 4
        || scope.limits.values.elements < 4
    {
        return Err(Error::PRESSURE);
    }
    let count = packet.reference_bound(scope.limits.values.references);
    let received = Received { packet: packet.clone(), request };
    let entry = handle.entry.clone();
    let output =
        Outputs::reserve_count(scope, current, &entry, count, call.request)?;
    Ok(Task::receive(entry, received, output))
}

impl Received {
    pub fn begin(
        &self,
        provider: &mut ResourceProvider,
    ) -> Result<Pending, Error> {
        provider.receive(self.request)
    }

    pub fn validate(
        &self,
        completion: &mut Completion,
        limits: ValueLimits,
        output: &mut Outputs,
    ) -> Result<Option<Error>, Error> {
        if completion.activity & !1 != 0 {
            return Err(Error::PROVIDER);
        }
        match &mut completion.outcome {
            Outcome::Ok(value) => output.transfer(
                value,
                Schema::Receive(&self.packet, self.request),
                limits,
            ),
            Outcome::Unknown { progress: Some(_), .. } => Err(Error::PROVIDER),
            Outcome::Failed(_) => {
                output.release();
                Ok(None)
            }
            Outcome::Unknown { progress: None, .. } => Ok(None),
        }
    }
}
