use super::{Session, Snapshot};
use crate::{
    Runtime,
    namespace::{Entry, Kind},
    request::Record,
};
use alloc::{boxed::Box, format, rc::Rc};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Contract, Error, Message, ResourceId, access::Method, provider::Permissions,
};

pub struct Participation {
    pub(super) session: ResourceId,
    pub(crate) owner: u64,
    read: Contract,
    view: View,
}

enum View {
    Live,
    Historical(Rc<Snapshot>),
    Detached,
}

pub fn attaching(contract: &Contract) -> bool {
    contract == &Contract::new("kernel.session.attach", 1)
}

pub fn surface(
    participant: &Participation,
    method: Method,
    input: &Message,
) -> Result<Box<[Permissions]>, Error> {
    if method == Method::Write
        && input.contract == Contract::new("kernel.session.detach", 1)
    {
        return Ok(Box::new([]));
    }
    if matches!(participant.view, View::Detached) {
        return Err(Error::CLOSED);
    }
    if method == Method::Read && input.contract == participant.read {
        return Ok(Box::new([Permissions::READ, Permissions::ALL]));
    }
    if method == Method::Write
        && input.contract == Contract::new("kernel.session.seek", 1)
    {
        return Ok(Box::new([]));
    }
    Err(Error::UNSUPPORTED)
}

impl Participation {
    pub(crate) const fn settled(&self) -> bool {
        matches!(self.view, View::Detached)
    }

    // Return the retained value so callers drop it outside registry guards.
    pub(crate) fn detach(&mut self) -> Option<Rc<Snapshot>> {
        match core::mem::replace(&mut self.view, View::Detached) {
            View::Historical(snapshot) => Some(snapshot),
            _ => None,
        }
    }
}

impl Runtime {
    pub(super) fn attach_session(
        &self,
        record: &Rc<RefCell<Record>>,
        session: &Rc<RefCell<Session>>,
    ) -> Result<Message, Error> {
        let record = record.borrow();
        if !record.input.data.is_empty() {
            return Err(Error::INVALID);
        }
        let mut shared = self.shared.borrow_mut();
        let mut scope = record.scope.borrow_mut();
        let parent = crate::binding::state_container(
            &shared,
            &scope,
            "participations",
            1,
        )?;
        scope.grants.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        let participant = Participation {
            session: ResourceId(record.subject),
            owner: scope.owner,
            read: session.borrow().model.borrow().read.clone(),
            view: View::Live,
        };
        let key = shared.id()?;
        let parent = parent.unwrap_or_else(|| {
            shared.insert_at(scope.root, "participations", Kind::Directory)
        });
        shared.entries.push(Entry {
            key,
            parent,
            name: format!("{key}").into(),
            kind: Kind::Participation(Rc::new(RefCell::new(participant))),
            activation: None,
            receiver: None,
            group: None,
            ownership: None,
        });
        scope.grants.push((key, Permissions::ALL));
        Ok(Message::reference(record.input.contract.clone(), ResourceId(key)))
    }

    pub(crate) fn participation_request(
        &self,
        record: &Rc<RefCell<Record>>,
        participant: &Rc<RefCell<Participation>>,
    ) -> Result<Message, Error> {
        let request = record.borrow();
        if !request.input.data.is_empty() {
            return Err(Error::INVALID);
        }
        if request.scope.borrow().owner != participant.borrow().owner {
            return Err(Error::DENIED);
        }
        let detach = request.method == Method::Write
            && request.input.contract
                == Contract::new("kernel.session.detach", 1);
        if detach {
            return detach_request(self, &request, participant);
        }
        if participant.borrow().settled() {
            return Err(Error::CLOSED);
        }
        if request.method == Method::Write {
            return seek_request(&request, participant);
        }
        drop(request);
        self.read_participation(record, participant)
    }

    fn read_participation(
        &self,
        record: &Rc<RefCell<Record>>,
        participant: &Rc<RefCell<Participation>>,
    ) -> Result<Message, Error> {
        let participant = participant.borrow();
        if let View::Historical(snapshot) = &participant.view {
            return self.copy_session_snapshot(record, snapshot);
        }
        let shared = self.shared.borrow();
        let entry =
            &shared.entries[shared.entry_index(participant.session.0)?];
        let Kind::Session(session) = &entry.kind else {
            return Err(Error::CLOSED);
        };
        crate::modules::require_ready(entry.activation.as_deref())?;
        let session = session.clone();
        let subject = participant.session;
        drop(shared);
        drop(participant);
        self.read_session_at(record, &session, subject)
    }

    fn copy_session_snapshot(
        &self,
        record: &Rc<RefCell<Record>>,
        snapshot: &Snapshot,
    ) -> Result<Message, Error> {
        let value = snapshot.value.clone();
        super::references::set_output(
            record,
            &value,
            self.shared.borrow().limits.resources,
        )?;
        let record = record.borrow();
        let mut shared = self.shared.borrow_mut();
        super::reserve_snapshot(&shared)?;
        let mut scope = record.scope.borrow_mut();
        scope
            .grants
            .try_reserve(1 + value.rights.len())
            .map_err(|_| Error::PRESSURE)?;
        crate::binding::grant_outputs(
            &shared,
            &mut scope,
            &value.rights,
            &value.message,
        )?;
        Ok(super::snapshot(
            &mut shared,
            &mut scope,
            snapshot.session,
            snapshot.revision,
            value,
        ))
    }
}

fn detach_request(
    runtime: &Runtime,
    request: &Record,
    participant: &Rc<RefCell<Participation>>,
) -> Result<Message, Error> {
    let busy = runtime.shared.borrow().requests.iter().any(|other| {
        let other = other.borrow();
        other.id != request.id
            && other.subject == request.subject
            && other.input.contract != request.input.contract
    });
    if busy {
        return Err(Error::BUSY);
    }
    let retired = participant.borrow_mut().detach();
    drop(retired);
    Ok(Message::new(request.input.contract.clone(), &[]))
}

fn seek_request(
    request: &Record,
    participant: &Rc<RefCell<Participation>>,
) -> Result<Message, Error> {
    let Some(Kind::Snapshot(snapshot)) = request.references.first() else {
        return Err(Error::INVALID);
    };
    if snapshot.session != participant.borrow().session {
        return Err(Error::STALE);
    }
    let retired = core::mem::replace(
        &mut participant.borrow_mut().view,
        View::Historical(snapshot.clone()),
    );
    drop(retired);
    Ok(Message::new(request.input.contract.clone(), &[]))
}
