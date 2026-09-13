use super::{Session, Snapshot};
use crate::{
    Runtime,
    namespace::{Capture, Entry, Kind},
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
    edit: Contract,
    access: Option<Rc<Capture>>,
    view: View,
    watch: Option<super::watch::Reader>,
}

pub struct Retired {
    _view: View,
    _access: Option<Rc<Capture>>,
    watch: Option<super::watch::Reader>,
    fence: bool,
}

impl Drop for Retired {
    fn drop(&mut self) {
        if let Some(watch) = self.watch.as_ref().filter(|_| self.fence) {
            crate::watch::close(watch);
        }
    }
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
    if method == Method::Read && super::watch::following(&input.contract) {
        return Ok(Box::new([
            Permissions::READ,
            Permissions::ALL,
            Permissions::READ,
        ]));
    }
    if method == Method::Read && input.contract == participant.read {
        return Ok(Box::new([Permissions::READ, Permissions::ALL]));
    }
    if method == Method::Write && input.contract == participant.edit {
        return Ok(Box::new([Permissions::WRITE, Permissions::ALL]));
    }
    if method == Method::Write
        && input.contract == Contract::new("kernel.session.seek", 1)
    {
        return Ok(Box::new([]));
    }
    Err(Error::UNSUPPORTED)
}

impl Participation {
    pub(super) fn update_session(
        &self,
        shared: &crate::Shared,
        owner: u64,
    ) -> Result<(u64, Rc<RefCell<Session>>), Error> {
        if self.owner != owner
            || !self.access.as_ref().is_some_and(|access| {
                access.rights.contains(Permissions::WRITE)
            })
        {
            return Err(Error::DENIED);
        }
        if self.settled() {
            return Err(Error::CLOSED);
        }
        Ok((self.session.0, live_session(shared, self.session)?))
    }

    pub(crate) const fn settled(&self) -> bool {
        matches!(self.view, View::Detached)
    }

    // Release backing and wake fenced readers outside all owner guards.
    pub(crate) const fn detach(&mut self) -> Retired {
        self.select(View::Detached, None, true)
    }

    // Session close ends its feed after dependants settle, preserving queued
    // delivery. Explicit detach instead fences the old stream immediately.
    pub(super) const fn end(&mut self) -> Retired {
        self.select(View::Detached, None, false)
    }

    const fn select(
        &mut self,
        view: View,
        watch: Option<super::watch::Reader>,
        fence: bool,
    ) -> Retired {
        Retired {
            _access: if matches!(view, View::Detached) {
                self.access.take()
            } else {
                None
            },
            _view: core::mem::replace(&mut self.view, view),
            watch: core::mem::replace(&mut self.watch, watch),
            fence,
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
        scope.captures.retain(|capture| capture.strong_count() != 0);
        if scope.captures.len() == shared.limits.resources {
            return Err(Error::PRESSURE);
        }
        scope.captures.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        let access = Rc::new(Capture {
            key: record.subject,
            rights: record.rights,
            _target: Kind::Session(session.clone()),
            _view: None,
        });
        let participant = Participation {
            session: ResourceId(record.subject),
            owner: scope.owner,
            read: session.borrow().model.borrow().read.clone(),
            edit: session.borrow().model.borrow().edit.clone(),
            access: Some(access.clone()),
            view: View::Live,
            watch: None,
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
        scope.captures.push(Rc::downgrade(&access));
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
        let following = super::watch::following(&request.input.contract);
        drop(request);
        if following {
            return self.follow_participation(record, participant);
        }
        self.read_participation(record, participant)
    }

    fn follow_participation(
        &self,
        record: &Rc<RefCell<Record>>,
        participant: &Rc<RefCell<Participation>>,
    ) -> Result<Message, Error> {
        let subject = participant.borrow().session;
        let session = live_session(&self.shared.borrow(), subject)?;
        let (reply, reader) =
            self.observe_session_at(record, &session, subject)?;
        let retired =
            participant.borrow_mut().select(View::Live, Some(reader), true);
        drop(retired);
        Ok(reply)
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
        let subject = participant.session;
        let session = live_session(&self.shared.borrow(), subject)?;
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
        super::references::grant_session(
            &shared,
            &mut scope,
            snapshot.session,
            Permissions::READ,
        )?;
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

fn live_session(
    shared: &crate::Shared,
    subject: ResourceId,
) -> Result<Rc<RefCell<Session>>, Error> {
    let entry = &shared.entries[shared.entry_index(subject.0)?];
    let Kind::Session(session) = &entry.kind else {
        return Err(Error::CLOSED);
    };
    crate::modules::require_ready(entry.activation.as_deref())?;
    Ok(session.clone())
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
    let retired = participant.borrow_mut().select(
        View::Historical(snapshot.clone()),
        None,
        true,
    );
    drop(retired);
    Ok(Message::new(request.input.contract.clone(), &[]))
}
