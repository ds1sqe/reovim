use crate::{remote, session::Session};
use reovim_client_tui::operation::{Change, Operation};
use reovim_kernel_interface::{Contract, Message, ResourceId};
use reovim_module_text::api;
use std::io;

pub(super) fn owner(session: &Session, factory: ResourceId) -> io::Result<()> {
    owners(session)?;
    stage(session, factory)?;
    let Some(request) = &session.remote.pending else { return Ok(()) };
    let saved = request.checkpoint();
    let fields: Vec<_> =
        saved.frame.split('\t').skip(4).map(str::to_owned).collect();
    request_fields(
        session,
        factory,
        &fields,
        saved.adopted,
        saved.response.as_deref(),
    )?;
    let has_result_owner = match session.operation {
        Operation::Attach(_) => session.access.is_some(),
        Operation::Observe { .. } => session.observed.is_some(),
        Operation::Change(_) => session.accepted.is_some(),
        _ => false,
    };
    if saved.adopted || has_result_owner {
        adopted(
            session,
            saved.response.as_deref().ok_or_else(super::invalid)?,
        )?;
    }
    Ok(())
}

fn owners(session: &Session) -> io::Result<()> {
    let mut owned = session.retiring.clone();
    owned.extend(session.access);
    if let Some(value) = session.observed {
        if Some(value.observed().representation) != session.access
            || value.observed().buffer.buffer != session.buffer
        {
            return Err(super::invalid());
        }
        owned.extend([value.snapshot(), value.watch()]);
    }
    owned.sort_unstable_by_key(|id| id.0);
    if owned.windows(2).any(|pair| pair[0] == pair[1])
        || session.accepted.is_some_and(|state| state.buffer != session.buffer)
    {
        return Err(super::invalid());
    }
    Ok(())
}

fn stage(session: &Session, factory: ResourceId) -> io::Result<()> {
    let valid = match session.operation {
        Operation::Idle => {
            session.remote.pending.is_none()
                && session.remote.reserved.is_none()
                && session.retiring.is_empty()
        }
        Operation::Attach(id) => {
            id == factory
                && session.observed.is_none()
                && session.retiring.is_empty()
                && session.accepted.is_none()
        }
        Operation::Retire { .. } => {
            session.observed.is_none() && session.access.is_some()
        }
        Operation::Observe { .. } => {
            session.access.is_some() && session.retiring.is_empty()
        }
        Operation::Context { reading, .. } => {
            session.observed.is_some()
                && session.retiring.is_empty()
                && (reading.is_some() || session.remote.pending.is_none())
        }
        Operation::Change(_) => {
            session.observed.is_some() && session.retiring.is_empty()
        }
        Operation::Closing => {
            session.observed.is_none() && session.access.is_none()
        }
    };
    if valid { Ok(()) } else { Err(super::invalid()) }
}

fn request_fields(
    session: &Session,
    factory: ResourceId,
    fields: &[String],
    adopted: bool,
    response: Option<&str>,
) -> io::Result<()> {
    let expected = match session.operation {
        Operation::Attach(_) => remote::fields(
            "write",
            factory,
            &Message::reference(
                Contract::new("text.attach", 1),
                session.buffer,
            ),
        ),
        Operation::Observe { .. } => remote::fields(
            "read",
            session.access.ok_or_else(super::invalid)?,
            &Message::new(Contract::new("text.observe", 1), &[]),
        ),
        Operation::Context { reading: Some((start, count)), .. } => {
            let observed = session.observed.ok_or_else(super::invalid)?;
            remote::fields(
                "read",
                observed.snapshot(),
                &Message::new(
                    Contract::new("text.window", 1),
                    &[start.to_le_bytes(), count.to_le_bytes()].concat(),
                ),
            )
        }
        Operation::Change(change) => {
            return change_fields(session, change, fields);
        }
        Operation::Retire { .. } | Operation::Closing => {
            return close_fields(
                session,
                fields,
                adopted && response == Some("ok"),
            );
        }
        _ => return Err(super::invalid()),
    };
    if fields == expected { Ok(()) } else { Err(super::invalid()) }
}

fn close_fields(
    session: &Session,
    fields: &[String],
    closed: bool,
) -> io::Result<()> {
    let [method, target] = fields else { return Err(super::invalid()) };
    let id = remote::resource(target)?;
    let owner = if closed {
        !session.retiring.contains(&id)
    } else {
        session.retiring.last() == Some(&id)
    };
    if method == "close" && owner { Ok(()) } else { Err(super::invalid()) }
}

fn change_fields(
    session: &Session,
    change: Change,
    fields: &[String],
) -> io::Result<()> {
    let value = session.observed.ok_or_else(super::invalid)?;
    let [method, _, _, version, data, _] = fields else {
        return Err(super::invalid());
    };
    if method != "write" || version != "1" {
        return Err(super::invalid());
    }
    let bytes = reovim_service::unhex(data).ok_or_else(super::invalid)?;
    match change {
        Change::History { redo } => {
            history_fields(session, &value, fields, &bytes, redo)
        }
        Change::Replace { .. } => replace_fields(&value, fields, &bytes),
    }
}

fn history_fields(
    session: &Session,
    value: &api::Observation,
    fields: &[String],
    bytes: &[u8],
    redo: bool,
) -> io::Result<()> {
    let [_, target, name, _, _, references] = fields else {
        return Err(super::invalid());
    };
    let contract =
        if redo { "kernel.history.redo" } else { "kernel.history.undo" };
    if name != contract
        || remote::resource(target)? != value.history()
        || references != &format!("@{}", session.buffer.0)
        || bytes != value.observed().buffer.revision.to_le_bytes()
    {
        return Err(super::invalid());
    }
    Ok(())
}

fn replace_fields(
    value: &api::Observation,
    fields: &[String],
    bytes: &[u8],
) -> io::Result<()> {
    let [_, target, name, _, _, references] = fields else {
        return Err(super::invalid());
    };
    if name != "text.replace"
        || remote::resource(target)? != value.observed().representation
        || references != &format!("@{}", value.snapshot().0)
        || bytes.len() < 16
    {
        return Err(super::invalid());
    }
    let start = reovim_kernel_interface::buffer::integer(&bytes[..8])
        .map_err(io::Error::other)?;
    let end = reovim_kernel_interface::buffer::integer(&bytes[8..16])
        .map_err(io::Error::other)?;
    if start > end || std::str::from_utf8(&bytes[16..]).is_err() {
        return Err(super::invalid());
    }
    Ok(())
}

fn adopted(session: &Session, reply: &str) -> io::Result<()> {
    if reply.starts_with("error\t") {
        let unexpected = match session.operation {
            Operation::Attach(_) => session.access.is_some(),
            Operation::Observe { .. } => session.observed.is_some(),
            Operation::Change(_) => session.accepted.is_some(),
            _ => false,
        };
        return if unexpected { Err(super::invalid()) } else { Ok(()) };
    }
    let valid = match session.operation {
        Operation::Attach(_) => {
            session.access
                == Some(
                    api::attachment(&remote::message(reply)?, true)
                        .map_err(io::Error::other)?,
                )
        }
        Operation::Observe { .. } => {
            session.observed
                == Some(
                    api::observation(
                        &remote::message(reply)?,
                        session.access.ok_or_else(super::invalid)?,
                    )
                    .map_err(io::Error::other)?,
                )
        }
        Operation::Change(change) => {
            session.accepted
                == Some(crate::session::change_state(
                    change,
                    reply,
                    session.buffer,
                )?)
        }
        Operation::Context { reading: Some((start, count)), .. } => {
            api::window(
                &remote::message(reply)?,
                &session.observed.ok_or_else(super::invalid)?,
                start,
                count,
            )
            .map_err(io::Error::other)?;
            true
        }
        Operation::Retire { .. } | Operation::Closing => reply == "ok",
        _ => false,
    };
    if valid { Ok(()) } else { Err(super::invalid()) }
}
