use crate::{Runtime, namespace::Kind, request::Record};
use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;
use reovim_kernel_interface::{Contract, Error, Message, buffer::integer};

pub fn contract(value: &Contract) -> bool {
    value.name.as_ref() == "kernel.buffer.close" && value.version == 1
}

impl Runtime {
    pub(super) fn close_buffer(
        &self,
        record: &Rc<RefCell<Record>>,
    ) -> Result<Message, Error> {
        let record = record.borrow();
        let expected =
            integer(record.input.data.get(..8).ok_or(Error::INVALID)?)?;
        let policy = record.input.data.get(8).copied().ok_or(Error::INVALID)?;
        if record.input.data.len() != 9
            || policy > 1
            || record.input.resources.first().map(|id| id.0)
                != Some(record.subject)
        {
            return Err(Error::INVALID);
        }
        let (owner, revision) = state(&record.target)?;
        if owner != record.scope.borrow().owner {
            return Err(Error::DENIED);
        }
        if expected != revision {
            return Err(Error::STALE);
        }
        let reply = Message::new(record.input.contract.clone(), b"closed");
        if matches!(record.target, Kind::ClosedBuffer { .. }) {
            return Ok(reply);
        }
        // Every currently supported Buffer is unbound/unexported.
        if policy == 0 {
            return Err(Error::BUSY);
        }
        let mut shared = self.shared.borrow_mut();
        let busy = shared.requests.iter().any(|other| {
            let other = other.borrow();
            other.id != record.id
                && shared.descends(other.subject, record.subject)
        });
        if busy {
            return Err(Error::BUSY);
        }
        let index = shared.entry_index(record.subject)?;
        let mut retired = Vec::new();
        retired
            .try_reserve_exact(shared.entries.len())
            .map_err(|_| Error::PRESSURE)?;
        let closed = Kind::ClosedBuffer { owner, revision };
        retired
            .push(core::mem::replace(&mut shared.entries[index].kind, closed));
        // The admitted close pins state while registry entries retire.
        remove_history(&mut shared.entries, record.subject, &mut retired);
        drop(shared);
        drop(retired);
        Ok(reply)
    }
}
fn state(target: &Kind) -> Result<(u64, u64), Error> {
    match target {
        Kind::Buffer(buffer) => {
            let buffer = buffer.borrow();
            Ok((buffer.owner, buffer.state.revision()))
        }
        Kind::ClosedBuffer { owner, revision } => Ok((*owner, *revision)),
        _ => Err(Error::UNSUPPORTED),
    }
}
fn remove_history(
    entries: &mut Vec<crate::namespace::Entry>,
    parent: u64,
    retired: &mut Vec<Kind>,
) {
    let Some(index) = entries.iter().position(|entry| {
        entry.parent == parent && matches!(entry.kind, Kind::History(_))
    }) else {
        return;
    };
    retired.push(entries.remove(index).kind);
}
