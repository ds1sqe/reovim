use alloc::boxed::Box;
use reovim_kernel_interface::{Contract, Error, Message, ResourceId, session};

/// Focus metadata carries read access, not Buffer mutation or close ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditingSessionState {
    pub active: Option<ResourceId>,
    pub known: Box<[ResourceId]>,
}

/// Encodes initial context, validating identities and active membership.
/// # Errors
/// Rejects duplicate references, missing active Buffer or native slot overflow.
pub fn initial(value: &EditingSessionState) -> Result<Message, Error> {
    message("editing.context.initial", value)
}
/// Encodes an update against the exact retained Session snapshot.
/// # Errors
/// Rejects invalid semantic values or failed envelope allocation.
pub fn update(
    observed: &session::Observation,
    value: &EditingSessionState,
) -> Result<Message, Error> {
    session::update(observed, message("editing.context.update", value)?)
}
/// Decodes an observed semantic value, retaining caller-local identities.
/// # Errors
/// Rejects another schema or malformed references/current focus.
pub fn decode(value: &Message) -> Result<EditingSessionState, Error> {
    if value.contract != Contract::new("editing.context", 1) {
        return Err(Error::UNSUPPORTED);
    }
    decode_value(value)
}
pub(crate) fn decode_value(
    value: &Message,
) -> Result<EditingSessionState, Error> {
    let data = value.data.as_ref().try_into().map_err(|_| Error::INVALID)?;
    let index = u64::from_le_bytes(data);
    let active = if index == u64::MAX {
        None
    } else {
        Some(
            *value
                .resources
                .get(usize::try_from(index).map_err(|_| Error::INVALID)?)
                .ok_or(Error::INVALID)?,
        )
    };
    let state = EditingSessionState { active, known: value.resources.clone() };
    validate(&state)?;
    Ok(state)
}
pub(crate) fn message(
    name: &str,
    value: &EditingSessionState,
) -> Result<Message, Error> {
    validate(value)?;
    let index = value.active.map_or(Ok(u64::MAX), |active| {
        value
            .known
            .iter()
            .position(|id| *id == active)
            .map(|index| index as u64)
            .ok_or(Error::INVALID)
    })?;
    Ok(Message {
        contract: Contract::new(name, 1),
        data: index.to_le_bytes().into(),
        resources: value.known.clone(),
    })
}
fn validate(value: &EditingSessionState) -> Result<(), Error> {
    // The current native Model reference view admits at most 256 slots.
    if value.known.len() > 256 {
        return Err(Error::PRESSURE);
    }
    for (index, id) in value.known.iter().enumerate() {
        if value.known[..index].contains(id) {
            return Err(Error::INVALID);
        }
    }
    if value.active.is_some_and(|id| !value.known.contains(&id)) {
        return Err(Error::INVALID);
    }
    Ok(())
}
