use super::{Session, Snapshot};
use crate::{
    Runtime, Shared,
    namespace::{Kind, Scope},
    request::Record,
};
use alloc::rc::Rc;
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Message, ResourceId,
    access::Method,
    buffer::{State, StateReply},
    provider::Permissions,
};

pub fn capture_snapshot(
    shared: &Shared,
    scope: &Scope,
    target: &Kind,
    method: Method,
    subject: u64,
    input: &mut Message,
) -> Result<Option<Rc<Snapshot>>, Error> {
    if !matches!(target, Kind::Session(_))
        || method != Method::Write
        || super::closing(&input.contract)
    {
        return Ok(None);
    }
    let [session, snapshot, ..] = input.resources.as_ref() else {
        return Err(Error::INVALID);
    };
    if scope.key(*session)? != subject {
        return Err(Error::STALE);
    }
    let key = scope.key(*snapshot)?;
    if !scope
        .permissions(shared, key)
        .is_some_and(|p| p.contains(Permissions::READ))
    {
        return Err(Error::DENIED);
    }
    let Kind::Snapshot(snapshot) =
        &shared.entries[shared.entry_index(key)?].kind
    else {
        return Err(Error::INVALID);
    };
    if snapshot.session.0 != subject {
        return Err(Error::STALE);
    }
    let rights = match target {
        Kind::Session(session) => session.borrow().model.borrow().references,
        _ => None,
    };
    let mut resources = alloc::vec![ResourceId(subject), ResourceId(key)];
    for id in &input.resources[2..] {
        let rights = rights.ok_or(Error::INVALID)?;
        let key = scope.key(*id)?;
        if !scope.permissions(shared, key).is_some_and(|p| p.contains(rights)) {
            return Err(Error::DENIED);
        }
        resources.push(ResourceId(key));
    }
    input.resources = resources.into();
    Ok(Some(Rc::clone(snapshot)))
}

impl Runtime {
    pub(crate) fn prepare_session(
        &self,
        record: &Rc<RefCell<Record>>,
        session: &Rc<RefCell<Session>>,
    ) -> Result<StateReply, Error> {
        let mut input = record.borrow().input.clone();
        let (model, state) = {
            let session = session.borrow();
            current(&record.borrow(), &session)?;
            (Rc::clone(&session.model), session.state.clone())
        };
        input.resources = input.resources[2..].into();
        if !input.resources.is_empty() && record.borrow().receiver.is_none() {
            return Err(Error::INVALID);
        }
        let (_, input) = crate::binding::receive(
            &mut self.shared.borrow_mut(),
            &record.borrow(),
            input,
        )?;
        Ok(model.borrow_mut().prepare(&state, input))
    }

    pub(crate) fn install_session(
        &self,
        record: &Rc<RefCell<Record>>,
        session: &Rc<RefCell<Session>>,
        mut candidate: State,
    ) -> Result<Message, Error> {
        let model = Rc::clone(&session.borrow().model);
        candidate.retain(model.clone());
        let limits = self.shared.borrow().limits;
        let value = super::observe(&model, &candidate, limits)?;
        let receiver = session.borrow().receiver.clone();
        let value = super::references::capture(
            &self.shared.borrow(),
            receiver.as_ref(),
            &model.borrow(),
            value,
        )?;
        super::references::set_output(record, &value, limits.resources)?;
        let record = record.borrow();
        if record.cancelled {
            return Err(Error::CANCELLED);
        }
        let mut session = session.borrow_mut();
        current(&record, &session)?;
        let revision =
            session.revision.checked_add(1).ok_or(Error::PRESSURE)?;
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
        let reply = super::snapshot(
            &mut shared,
            &mut scope,
            ResourceId(record.subject),
            revision,
            value,
        );
        let retired = core::mem::replace(&mut session.state, candidate);
        let change = super::watch::Change {
            request: record.id,
            before: session.revision,
            after: revision,
        };
        let feed = session.feed.clone();
        session.revision = revision;
        drop(scope);
        drop(shared);
        drop(session);
        drop(record);
        drop(retired);
        crate::watch::changed(&feed, change);
        Ok(reply)
    }
}

fn current(record: &Record, session: &Session) -> Result<(), Error> {
    let expected = record.snapshot.as_ref().ok_or(Error::INVALID)?;
    if expected.revision != session.revision {
        return Err(Error::STALE);
    }
    Ok(())
}
