//! Metadata of the exact opened subject. Native flags use driven custody.

use alloc::{boxed::Box, vec::Vec};
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value, call::Completion, caller::Call,
    provider::ResourceProvider, resource::AccessId,
};

use super::{access::Scope, execution::Task};

pub(super) fn info(
    scope: &Scope,
    subject: AccessId,
    input: &[u8],
) -> Result<Outcome, Error> {
    if !input.is_empty() {
        return Err(Error::INVALID);
    }
    let handle = scope.live(subject)?;
    let entry = &handle.entry;
    let implementation = &handle.implementation;
    // Native flag observation fills this prepared output during driving.
    // No Host clock is supplied.
    let receive = implementation
        .receive_type
        .as_ref()
        .map_or(&[][..], |packet| packet.bytes());
    let send = implementation
        .send_type
        .as_ref()
        .map_or(&[][..], |packet| packet.bytes());
    let prefix: usize = if implementation.io.is_some() { 84 } else { 52 };
    let prefix = prefix + receive.len() + send.len();
    // Record/Identity/scalars, Times choices, Io choice and API sequence.
    let nodes = if implementation.io.is_some() { 20 } else { 14 };
    let size =
        implementation.interfaces.iter().try_fold(prefix, |size, api| {
            size.checked_add(4 + api.bytes().len()).ok_or(Error::PRESSURE)
        })?;
    if size > scope.limits.values.bytes
        || nodes + implementation.interfaces.len()
            > scope.limits.values.elements
        || implementation.interfaces.len() > 256
    {
        return Err(Error::PRESSURE);
    }
    let mut output = Vec::new();
    output.try_reserve_exact(size).map_err(|_| Error::PRESSURE)?;
    output.extend_from_slice(&entry.identity.epoch);
    output.extend_from_slice(&entry.identity.serial.to_le_bytes());
    output.extend_from_slice(&handle.opened.borrow().held.to_le_bytes());
    output.extend_from_slice(
        &super::namespace::flags(scope, entry)?.to_le_bytes(),
    );
    output.extend_from_slice(&[0; 12]); // Three None timestamp choices.
    output.extend_from_slice(
        &u32::from(implementation.io.is_some()).to_le_bytes(),
    );
    if let Some(io) = implementation.io {
        output.extend_from_slice(&io.read_modes.to_le_bytes());
        output.extend_from_slice(&io.write_modes.to_le_bytes());
        output.extend_from_slice(&io.alignment.to_le_bytes());
        output.extend_from_slice(&io.transfer_limit.to_le_bytes());
        append_type(&mut output, receive)?;
        append_type(&mut output, send)?;
    }
    let count = u32::try_from(implementation.interfaces.len())
        .map_err(|_| Error::PRESSURE)?;
    output.extend_from_slice(&count.to_le_bytes());
    for api in &implementation.interfaces {
        super::discovery::append(&mut output, api)?;
    }
    Ok(Outcome::Ok(Value {
        data: output.into_boxed_slice(),
        references: Box::new([]),
    }))
}

pub(super) struct Metadata {
    output: Option<Value>,
}
pub(super) fn admit(scope: &Scope, call: &Call) -> Result<Option<Task>, Error> {
    let input = super::codec::no_refs(&call.input)?;
    let handle = scope.live(call.subject)?;
    if handle.entry.directory
        || handle
            .implementation
            .provider
            .try_borrow()
            .map_err(|_| Error::BUSY)?
            .namespace()
            .is_none()
    {
        return Ok(None);
    }
    let Outcome::Ok(output) = info(scope, call.subject, input)? else {
        unreachable!()
    };
    Ok(Some(Task::metadata(
        handle.entry.clone(),
        Metadata { output: Some(output) },
    )))
}
impl Metadata {
    pub fn begin(provider: &mut ResourceProvider) -> Result<Pending, Error> {
        provider.namespace_flags()
    }
    pub fn validate(
        &mut self,
        completion: &mut Completion,
    ) -> Result<Option<Error>, Error> {
        if completion.activity != 0 {
            return Err(Error::PROVIDER);
        }
        if matches!(
            completion.outcome,
            Outcome::Unknown { progress: Some(_), .. }
        ) {
            return Err(Error::PROVIDER);
        }
        let Outcome::Ok(value) = &completion.outcome else {
            return Ok(None);
        };
        let flags = super::codec::no_refs(value)
            .and_then(super::codec::u32_value)
            .map_err(|_| Error::PROVIDER)?;
        if !matches!(flags, 1 | 3) {
            return Err(Error::PROVIDER);
        }
        let mut output = self.output.take().unwrap();
        output.data[28..32].copy_from_slice(&flags.to_le_bytes());
        completion.outcome = Outcome::Ok(output);
        Ok(None)
    }
}

fn append_type(output: &mut Vec<u8>, packet: &[u8]) -> Result<(), Error> {
    let size = u32::try_from(packet.len()).map_err(|_| Error::PRESSURE)?;
    output.extend_from_slice(&size.to_le_bytes());
    output.extend_from_slice(packet);
    Ok(())
}
