use super::{Release, Save, cause, message};
use crate::{
    Editing, Shared,
    namespace::{Kind, Scope},
    opening::Base,
};
use alloc::rc::Rc;
use core::cell::RefCell;
use reovim_kernel_interface::publication::backend::{self, Status};
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
};

#[cfg(test)]
mod tests;

fn context(
    save: &RefCell<Save>,
) -> Result<(Rc<RefCell<Scope>>, Connection), Error> {
    save.borrow().context.clone().ok_or(Error::NOT_READY)
}

fn local(
    shared: &RefCell<Shared>,
    scope: &RefCell<Scope>,
    key: u64,
) -> Result<ResourceId, Error> {
    scope.borrow_mut().export(&mut shared.borrow_mut(), key)
}

#[allow(clippy::future_not_send)] // Runtime drives the coordinator locally.
pub(super) async fn run(
    shared: &Rc<RefCell<Shared>>,
    save: &Rc<RefCell<Save>>,
    reconcile: bool,
) -> Result<Message, RequestError> {
    let result = execute(shared, save, reconcile).await;
    if let Err(error) = result {
        let mut save = save.borrow_mut();
        // Backend diagnostics are private scoped references; expose the code
        // while the original retained evidence remains authoritative.
        save.last_error = Some(Error::new(cause(&error).code.0));
        if !save.handed {
            save.status =
                Some(Status::DefinitelyNotApplied(cause(&error).code));
            save.settled = true;
        }
        // An observation/adoption failure cannot erase known application.
        else if save.status.is_none() {
            save.status = Some(Status::OutcomeUnknown(cause(&error).code));
        }
    }
    message(&mut save.borrow_mut()).map_err(Into::into)
}

#[allow(clippy::future_not_send)]
async fn execute(
    shared: &Rc<RefCell<Shared>>,
    save: &Rc<RefCell<Save>>,
    reconcile: bool,
) -> Result<(), RequestError> {
    if !save.borrow().handed && save.borrow().cancelled {
        let mut save = save.borrow_mut();
        save.status = Some(Status::DefinitelyNotApplied(Error::CANCELLED.code));
        save.settled = true;
        return Ok(());
    }
    if save.borrow().receipt.is_none() {
        publish(shared, save).await?;
    }
    if save.borrow().receipt.is_none() {
        return Ok(());
    }
    let (scope, connection) = context(save)?;
    let key = save.borrow().evidence.as_ref().ok_or(Error::NOT_READY)?.0;
    let publisher = save.borrow().publisher;
    if let Some(work) = publisher {
        connection.close(local(shared, &scope, work)?).into_inner()?;
        save.borrow_mut().publisher = None;
    }
    let evidence = local(shared, &scope, key)?;
    if save.borrow().status == Some(Status::Applied) {
        adopt(shared, save, &scope, evidence)?;
        return Ok(());
    }
    let result = if reconcile && !save.borrow().settled {
        backend::reconcile(&connection, evidence)
    } else {
        backend::result(&connection, evidence)
    };
    let status = backend::status(&result.await?)?;
    let status = refine(save.borrow().status, status)?;
    save.borrow_mut().status = Some(status);
    match status {
        Status::Applied => adopt(shared, save, &scope, evidence)?,
        Status::DefinitelyNotApplied(_) => save.borrow_mut().settled = true,
        _ => (),
    }
    Ok(())
}

const fn refine(
    previous: Option<Status>,
    next: Status,
) -> Result<Status, Error> {
    match (previous, next) {
        (
            Some(Status::PartiallyApplied { .. }),
            Status::DefinitelyNotApplied(_) | Status::OutcomeUnknown(_),
        )
        | (
            Some(Status::PartiallyApplied { atomicity_violated: true, .. }),
            Status::Applied
            | Status::PartiallyApplied { atomicity_violated: false, .. },
        ) => Err(Error::PROVIDER),
        _ => Ok(next),
    }
}

#[allow(clippy::future_not_send)]
async fn publish(
    shared: &Rc<RefCell<Shared>>,
    save: &Rc<RefCell<Save>>,
) -> Result<(), RequestError> {
    let (scope, connection) = context(save)?;
    let prior = save.borrow().publisher;
    let reply = if let Some(work) = prior {
        let work = local(shared, &scope, work)?;
        reovim_kernel_interface::requests::result(&connection, work).await??
    } else {
        let input = {
            let save = save.borrow();
            let target = local(shared, &scope, save.source)?;
            let base = local(shared, &scope, save.base.key)?;
            (target, base, save.parameters.clone())
        };
        let selected = connection.root().into_inner()?;
        let before = shared.borrow().requests.len();
        let pending =
            backend::publish(&connection, input.0, selected, input.1, &input.2);
        let child = retain_child(&mut shared.borrow_mut(), before);
        save.borrow_mut().publisher = child;
        save.borrow_mut().handed = child.is_some();
        save.borrow_mut().release =
            if child.is_some() { Release::Held } else { Release::Done };
        pending.await?
    };
    let receipt = backend::receipt(&reply)?;
    let key = scope.borrow().key(receipt.evidence)?;
    let target = {
        let shared = shared.borrow();
        shared.entries[shared.entry_index(key)?].kind.clone()
    };
    save.borrow_mut().evidence = Some((key, target));
    save.borrow_mut().receipt = Some(receipt);
    Ok(())
}

fn retain_child(shared: &mut Shared, index: usize) -> Option<u64> {
    let id = shared.requests.get(index)?.borrow().id;
    let entry = shared.entries.iter_mut().find(|entry| {
        matches!(entry.kind, Kind::Work { request, .. } if request == id)
    })?;
    if let Kind::Work { retained, .. } = &mut entry.kind {
        *retained = true;
    }
    Some(entry.key)
}

fn adopt(
    shared: &Rc<RefCell<Shared>>,
    save: &RefCell<Save>,
    scope: &Rc<RefCell<Scope>>,
    evidence: ResourceId,
) -> Result<(), Error> {
    if save.borrow().settled {
        return Ok(());
    }
    let applicable = {
        let save = save.borrow();
        applies(&shared.borrow(), &save, &save.buffer.borrow())
    };
    if !applicable {
        let mut save = save.borrow_mut();
        save.settled = true;
        return Ok(());
    }
    let (key, access) =
        crate::read_views::acquire_root(shared, scope, evidence)?;
    crate::opening::require_base(&shared.borrow(), key)?;
    let new_base = Rc::new(Base { key, _access: access });
    let mut save = save.borrow_mut();
    {
        let mut buffer = save.buffer.borrow_mut();
        let applicable = applies(&shared.borrow(), &save, &buffer);
        if let Some(association) =
            buffer.backing.as_mut().filter(|_| applicable)
        {
            association.base = new_base;
            association.published = save.node;
        }
    }
    save.settled = true;
    Ok(())
}

fn applies(shared: &Shared, save: &Save, buffer: &Editing) -> bool {
    let live = shared.entry_index(buffer.resource).is_ok_and(|index| {
        matches!(shared.entries[index].kind, Kind::Buffer(_))
    });
    live && buffer.backing.as_ref().is_some_and(|a| {
        a.source == save.source && Rc::ptr_eq(&a.base, &save.base)
    })
}

#[allow(clippy::future_not_send)]
pub(super) async fn release(
    shared: &Rc<RefCell<Shared>>,
    save: &Rc<RefCell<Save>>,
) -> Result<Message, RequestError> {
    let (scope, connection) = context(save)?;
    let receipt = save.borrow().receipt.clone().ok_or(Error::NOT_READY)?;
    let target = local(shared, &scope, save.borrow().source)?;
    // Base ownership was transferred independently before this release.
    save.borrow_mut().evidence.take();
    if save.borrow().release != Release::Acknowledged {
        release_record(shared, save, &scope, &connection, target, &receipt)
            .await;
    }
    if save.borrow().release == Release::Acknowledged {
        let _ = finish_release(shared, save, &scope, &connection);
    }
    message(&mut save.borrow_mut()).map_err(Into::into)
}

#[allow(clippy::future_not_send)]
async fn release_record(
    shared: &Rc<RefCell<Shared>>,
    save: &RefCell<Save>,
    scope: &RefCell<Scope>,
    connection: &Connection,
    target: ResourceId,
    receipt: &backend::Receipt,
) {
    let previous = save.borrow().release_work;
    let result = if let Some(work) = previous {
        match local(shared, scope, work) {
            Ok(work) => {
                reovim_kernel_interface::requests::result(connection, work)
                    .await
                    .and_then(core::convert::identity)
            }
            Err(error) => Err(error.into()),
        }
    } else {
        let before = shared.borrow().requests.len();
        let pending = backend::release(connection, target, receipt);
        save.borrow_mut().release_work =
            retain_child(&mut shared.borrow_mut(), before);
        pending.await
    };
    match result {
        Ok(reply)
            if reply.contract
                == Contract::new("resource.effect.release", 1)
                && reply.data.is_empty()
                && reply.resources.is_empty() =>
        {
            save.borrow_mut().release = Release::Acknowledged;
        }
        Err(RequestError::Rejected(error) | RequestError::Failed(error))
            if error == Error::BUSY =>
        {
            retry_release(shared, save, scope, connection);
        }
        _ => (),
    }
}

fn finish_release(
    shared: &RefCell<Shared>,
    save: &RefCell<Save>,
    scope: &RefCell<Scope>,
    connection: &Connection,
) -> Result<(), Error> {
    let work = save.borrow().release_work;
    if let Some(work) = work {
        connection
            .close(local(shared, scope, work)?)
            .into_inner()
            .map_err(|error| cause(&error))?;
    }
    let mut save = save.borrow_mut();
    save.release_work = None;
    save.receipt = None;
    save.release = Release::Done;
    Ok(())
}

fn retry_release(
    shared: &RefCell<Shared>,
    save: &RefCell<Save>,
    scope: &RefCell<Scope>,
    connection: &Connection,
) {
    let work = save.borrow().release_work;
    let closed = work.is_none_or(|work| {
        local(shared, scope, work)
            .is_ok_and(|local| connection.close(local).into_inner().is_ok())
    });
    if closed {
        save.borrow_mut().release_work = None;
    }
}
