use alloc::{boxed::Box, collections::VecDeque, rc::Rc};
use core::{
    cell::RefCell,
    task::{Context, Poll, Waker},
};
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::{Cancellation, Completion, PendingOps},
    declaration::{Declaration, Operation},
    provider::{ProviderInterface, ResourceOps, ResourceProvider},
};

pub(super) struct Work {
    pub path: Box<[u8]>,
    pub output: Value,
}
pub(super) struct Request {
    work: Option<Work>,
    outcome: Option<Outcome>,
    taken: bool,
    cancelled: bool,
    wake: Option<Waker>,
}
pub(super) struct Queue {
    requests: VecDeque<Rc<RefCell<Request>>>,
    capacity: usize,
    closed: bool,
    wake: Option<Waker>,
}
impl Queue {
    pub fn new(capacity: usize) -> Result<Self, Error> {
        let mut requests = VecDeque::new();
        requests.try_reserve_exact(capacity).map_err(|_| Error::PRESSURE)?;
        Ok(Self { requests, capacity, closed: false, wake: None })
    }
}

pub(super) fn next(
    queue: &Rc<RefCell<Queue>>,
    cx: &Context<'_>,
) -> Option<Rc<RefCell<Request>>> {
    let mut queue = queue.borrow_mut();
    queue.wake = Some(cx.waker().clone());
    queue.requests.pop_front()
}

pub(super) fn start(request: &Rc<RefCell<Request>>) -> Option<Work> {
    let mut state = request.borrow_mut();
    let work = state.work.take();
    if state.cancelled {
        drop(state);
        complete(request, Outcome::Failed(Error::CANCELLED));
        return None;
    }
    work
}
pub(super) fn complete(request: &Rc<RefCell<Request>>, outcome: Outcome) {
    let mut state = request.borrow_mut();
    state.outcome = Some(outcome);
    let wake = state.wake.take();
    drop(state);
    if let Some(wake) = wake {
        wake.wake();
    }
}
pub(super) fn close(queue: &Rc<RefCell<Queue>>) {
    queue.borrow_mut().closed = true;
    loop {
        let request = queue.borrow_mut().requests.pop_front();
        let Some(request) = request else { break };
        let mut state = request.borrow_mut();
        state.work = None;
        let error =
            if state.cancelled { Error::CANCELLED } else { Error::CLOSED };
        drop(state);
        complete(&request, Outcome::Failed(error));
    }
    let wake = queue.borrow_mut().wake.take();
    if let Some(wake) = wake {
        wake.wake();
    }
}

pub(super) fn provider(
    queue: Rc<RefCell<Queue>>,
) -> Result<ResourceProvider, Error> {
    ResourceProvider::new(
        queue,
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |queue| {
                close(queue);
                Ok(())
            },
        },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: declaration()?,
            begin: Some(begin),
            create: None,
        }]),
        Rc::new(()),
    )
    .map_err(|(error, _)| error)
}
fn declaration() -> Result<Declaration, Error> {
    let mut bytes = alloc::vec![0xd3; 16];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    // Ordinary exec, EXEC access, WRITE activity; native path is bytes.
    bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0, 4, 0, 0, 0, 2, 5, 0]);
    bytes.extend_from_slice(b"admit");
    bytes.push(7);
    bytes.extend_from_slice(&4096_u32.to_le_bytes());
    bytes.extend_from_slice(&[13, 0]); // ResourceId result, Unit progress.
    Declaration::decode(&bytes)
}
fn begin(
    queue: &mut Rc<RefCell<Queue>>,
    _: usize,
    operation: Operation,
    input: &Value,
) -> Result<Pending, Error> {
    if operation != Operation::Exec {
        return Err(Error::UNSUPPORTED);
    }
    let (count, path) = input.data.split_at_checked(4).ok_or(Error::INVALID)?;
    let count = u32::from_le_bytes(count.try_into().unwrap());
    if count == 0
        || count > 4096
        || count as usize != path.len()
        || path.contains(&0)
        || !input.references.is_empty()
    {
        return Err(Error::INVALID);
    }
    let mut state = queue.borrow_mut();
    if state.closed {
        return Err(Error::CLOSED);
    }
    if state.requests.len() == state.capacity {
        return Err(Error::PRESSURE);
    }
    let path = Value::copy(path, &[])?.data;
    let output = Value::copy(&[0; 24], &[])?;
    let request = Rc::new(RefCell::new(Request {
        work: Some(Work { path, output }),
        outcome: None,
        taken: false,
        cancelled: false,
        wake: None,
    }));
    state.requests.push_back(request.clone());
    let wake = state.wake.take();
    drop(state);
    if let Some(wake) = wake {
        wake.wake();
    }
    Ok(pending(request))
}
fn pending(request: Rc<RefCell<Request>>) -> Pending {
    Pending::new(
        request,
        PendingOps {
            poll,
            take,
            cancel: |request| Ok(cancel(request)),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |request| {
                let mut state = request.borrow_mut();
                if state.outcome.is_none() && !state.taken {
                    return Err(Error::BUSY);
                }
                state.wake = None;
                Ok(())
            },
        },
    )
}
fn poll(
    request: &mut Rc<RefCell<Request>>,
    cx: &mut Context<'_>,
) -> Result<Poll<()>, Error> {
    let mut state = request.borrow_mut();
    if state.outcome.is_some() {
        return Ok(Poll::Ready(()));
    }
    if state.taken {
        return Err(Error::CLOSED);
    }
    state.wake = Some(cx.waker().clone());
    Ok(Poll::Pending)
}
fn take(request: &mut Rc<RefCell<Request>>) -> Result<Completion, Error> {
    let mut state = request.borrow_mut();
    let outcome = state.outcome.take().ok_or(Error::NOT_READY)?;
    state.taken = true;
    let activity = u8::from(matches!(outcome, Outcome::Ok(_))) * 2;
    Ok(Completion { outcome, activity, owner: None })
}
fn cancel(request: &Rc<RefCell<Request>>) -> Cancellation {
    let mut state = request.borrow_mut();
    if state.work.is_none() {
        return Cancellation::TooLate;
    }
    state.cancelled = true;
    Cancellation::Accepted
}
