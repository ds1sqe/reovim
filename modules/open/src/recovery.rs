use super::{Connection, Error, Message, Pending, RequestError, code};
use reovim_kernel_interface::requests;

pub const LIMIT: usize = 16;

pub async fn opening(
    command: &super::Command,
    mut work: super::ResourceId,
) -> Result<(super::Rc<Pending>, super::Box<str>), RequestError> {
    for _ in 0..LIMIT {
        if let Ok(job) = command.find(work) {
            return Ok(job);
        }
        let result = requests::result(&command.kernel, work).await?;
        let Err(RequestError::Unknown { recovery, .. }) = result else {
            return Err(Error::UNAVAILABLE.into());
        };
        work = command.kernel.resolve(&recovery).await?;
    }
    Err(Error::PRESSURE.into())
}

/// Resolve a bounded chain without resubmitting any original operation.
pub async fn settle(
    kernel: &Connection,
    pending: &Pending,
    result: Result<Message, RequestError>,
    own_work: bool,
) -> Result<Message, RequestError> {
    let result = follow(kernel, pending, result, own_work).await;
    if let Err(RequestError::Unknown { recovery, .. }) = &result {
        expose(kernel, pending, recovery);
    }
    result
}

fn expose(kernel: &Connection, pending: &Pending, recovery: &str) {
    let Ok(id) = kernel.resolve(recovery).into_inner() else { return };
    // This locator is handed to the caller. Cleanup must not retire its result.
    pending.work.borrow_mut().retain(|work| *work != id);
}

async fn follow(
    kernel: &Connection,
    pending: &Pending,
    mut result: Result<Message, RequestError>,
    own_work: bool,
) -> Result<Message, RequestError> {
    for depth in 0..=LIMIT {
        let Err(error @ RequestError::Unknown { .. }) = result else {
            pending.recovery.borrow_mut().take();
            return result;
        };
        let RequestError::Unknown { recovery, .. } = &error else {
            unreachable!();
        };
        *pending.recovery.borrow_mut() = Some(recovery.clone());
        if depth == LIMIT {
            return Err(error);
        }
        let Ok(work) = kernel.resolve(recovery).await else {
            return Err(error);
        };
        if own_work && !retain(pending, work) {
            return Err(error);
        }
        let Ok(recovered) = requests::result(kernel, work).await else {
            return Err(error);
        };
        result = recovered;
    }
    unreachable!()
}

fn retain(pending: &Pending, id: super::ResourceId) -> bool {
    let mut work = pending.work.borrow_mut();
    if work.contains(&id) {
        return true;
    }
    if work.len() == LIMIT {
        return false;
    }
    work.push(id);
    true
}

pub fn release(kernel: &Connection, pending: &Pending) -> Result<(), Error> {
    loop {
        let id = pending.work.borrow().last().copied();
        let Some(id) = id else { return Ok(()) };
        kernel.close(id).into_inner().map_err(|error| code(&error))?;
        pending.work.borrow_mut().pop();
    }
}
