use crate::{remote, session::Session};
use reovim_client_tui::{
    custody::Submitted,
    input::Action,
    operation::{Change, Operation},
};
use reovim_kernel_interface::{Contract, Message};
use reovim_module_text::api::Observation;
use std::io;

pub(super) fn validate(
    session: &Session,
    submitted: Option<&Submitted>,
    planned: Option<u64>,
) -> io::Result<()> {
    let Some(submitted) = submitted else {
        return if matches!(session.operation, Operation::Change(_)) {
            Err(super::invalid())
        } else {
            Ok(())
        };
    };
    let cursor = submitted.action.cursor_after().ok_or_else(super::invalid)?;
    if submitted.basis.buffer != session.buffer
        || planned.is_some_and(|value| value != cursor)
        || !matches!(
            submitted.action,
            Action::Insert(..) | Action::Undo | Action::Redo
        )
    {
        return Err(super::invalid());
    }
    let Operation::Change(change) = session.operation else { return Ok(()) };
    let observed = session.observed.ok_or_else(super::invalid)?;
    if observed.observed().buffer != submitted.basis {
        return Err(super::invalid());
    }
    let expected = request(&submitted.action, change, &observed, cursor)?;
    let Some(receipt) = &session.remote.pending else { return Ok(()) };
    let saved = receipt.checkpoint();
    if !saved.frame.split('\t').skip(4).eq(expected.iter().map(String::as_str))
    {
        return Err(super::invalid());
    }
    Ok(())
}

fn request(
    action: &Action,
    change: Change,
    observed: &Observation,
    cursor: u64,
) -> io::Result<Vec<String>> {
    match (action, change) {
        (
            Action::Insert(start, end, text, window),
            Change::Replace { origin },
        ) => {
            replacement((*start, *end), text, *window, origin, cursor, observed)
        }
        (Action::Undo, Change::History { redo: false }) => {
            Ok(history(observed, false))
        }
        (Action::Redo, Change::History { redo: true }) => {
            Ok(history(observed, true))
        }
        _ => Err(super::invalid()),
    }
}

fn replacement(
    range: (u64, u64),
    text: &str,
    window: u64,
    origin: u64,
    cursor: u64,
    observed: &Observation,
) -> io::Result<Vec<String>> {
    if origin != window.max(cursor.saturating_sub(512)) {
        return Err(super::invalid());
    }
    let mut data = [range.0.to_le_bytes(), range.1.to_le_bytes()].concat();
    data.extend(text.as_bytes());
    let input = Message {
        contract: Contract::new("text.replace", 1),
        data: data.into(),
        resources: Box::new([observed.snapshot()]),
    };
    Ok(remote::fields("write", observed.observed().representation, &input))
}

fn history(observed: &Observation, redo: bool) -> Vec<String> {
    let name = if redo { "kernel.history.redo" } else { "kernel.history.undo" };
    let input = Message {
        contract: Contract::new(name, 1),
        data: observed.observed().buffer.revision.to_le_bytes().into(),
        resources: Box::new([observed.observed().buffer.buffer]),
    };
    remote::fields("write", observed.history(), &input)
}
