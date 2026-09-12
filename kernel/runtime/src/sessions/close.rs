use crate::{Runtime, namespace::Kind, request::Record};
use alloc::rc::Rc;
use core::cell::RefCell;
use reovim_kernel_interface::{Error, Message};

impl Runtime {
    pub(crate) fn close_session(
        &self,
        record: &Rc<RefCell<Record>>,
    ) -> Result<Message, Error> {
        let record = record.borrow();
        if !record.input.data.is_empty() {
            return Err(Error::INVALID);
        }
        let owner = match &record.target {
            Kind::Session(session) => session.borrow().owner,
            Kind::ClosedSession { owner } => *owner,
            _ => return Err(Error::UNSUPPORTED),
        };
        if record.scope.borrow().owner != owner {
            return Err(Error::DENIED);
        }
        let mut shared = self.shared.borrow_mut();
        let busy = shared.requests.iter().any(|other| {
            let other = other.borrow();
            other.id != record.id
                && (other.subject == record.subject
                    || matches!(&other.target, Kind::Participation(p)
                        if p.borrow().session.0 == record.subject))
                && !(other.method
                    == reovim_kernel_interface::access::Method::Write
                    && super::closing(&other.input.contract))
        });
        let children = shared.entries.iter().any(|entry| {
            entry.key != record.subject
                && shared.descends(entry.key, record.subject)
                && !matches!(entry.kind, Kind::Directory | Kind::Snapshot(_))
        });
        if busy || children {
            return Err(Error::BUSY);
        }
        let index = shared.entry_index(record.subject)?;
        let participants: alloc::vec::Vec<_> = shared
            .entries
            .iter()
            .filter_map(|entry| match &entry.kind {
                Kind::Participation(p)
                    if p.borrow().session.0 == record.subject =>
                {
                    Some(p.clone())
                }
                _ => None,
            })
            .collect();
        let feed = match &record.target {
            Kind::Session(session) => Some(session.borrow().feed.clone()),
            _ => None,
        };
        let retired = core::mem::replace(
            &mut shared.entries[index].kind,
            Kind::ClosedSession { owner },
        );
        shared.entries[index].activation = None;
        let reply = Message::new(record.input.contract.clone(), &[]);
        drop(shared);
        drop(retired);
        drop(record);
        for participant in participants {
            let retired = participant.borrow_mut().end();
            drop(retired);
        }
        if let Some(feed) = feed {
            crate::watch::end(&feed);
        }
        Ok(reply)
    }

    pub(crate) fn snapshot_request(
        &self,
        record: &Rc<RefCell<Record>>,
        snapshot: &Rc<super::Snapshot>,
    ) -> Result<Message, Error> {
        use reovim_kernel_interface::access::Method;
        let record = record.borrow();
        if !record.input.data.is_empty() {
            return Err(Error::INVALID);
        }
        if record.method == Method::Read {
            crate::binding::grant_outputs(
                &self.shared.borrow(),
                &mut record.scope.borrow_mut(),
                &snapshot.value.rights,
                &snapshot.value.message,
            )?;
            return Ok(snapshot.value.message.clone());
        }
        if record.scope.borrow().owner != snapshot.owner {
            return Err(Error::DENIED);
        }
        let mut shared = self.shared.borrow_mut();
        let index = shared.entry_index(record.subject)?;
        let retired = shared.entries.remove(index);
        drop(shared);
        drop(retired);
        Ok(Message::new(record.input.contract.clone(), &[]))
    }
}
