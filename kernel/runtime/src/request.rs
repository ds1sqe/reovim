use alloc::{boxed::Box, rc::Rc};
use core::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use reovim_kernel_interface::access::Method;
use reovim_kernel_interface::buffer::StateReply;
use reovim_kernel_interface::provider::{Permissions, ProviderReply};
use reovim_kernel_interface::{Error, Message, RequestError};

use crate::{
    Shared,
    namespace::{Kind, Scope},
};

pub enum Work {
    Queued,
    Resource(ProviderReply),
    Model(StateReply),
}

pub struct Record {
    pub id: u64,
    pub parent: Option<u64>,
    pub scope: Rc<RefCell<Scope>>,
    pub subject: u64,
    pub method: Method,
    pub input: Message,
    // Futures/results are destroyed before the captured implementation.
    pub work: Option<Work>,
    pub outcome: Option<Result<Message, Error>>,
    pub target: Kind,
    pub output: Box<[Permissions]>,
    pub cancelled: bool,
    pub observed: bool,
    pub recovery: Box<str>,
    pub waker: Option<Waker>,
}

pub struct Wait {
    pub shared: Rc<RefCell<Shared>>,
    pub record: Option<Rc<RefCell<Record>>>,
    pub generation: u64,
    pub recovery: bool,
}

impl Future for Wait {
    type Output = Result<Message, RequestError>;

    fn poll(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let Some(record) = self.record.clone() else {
            return Poll::Ready(Err(RequestError::Rejected(Error::CLOSED)));
        };
        let result = self.poll_record(&record, context);
        if result.is_ready() {
            self.record = None;
        }
        result
    }
}

impl Wait {
    fn poll_record(
        &self,
        record: &Rc<RefCell<Record>>,
        context: &Context<'_>,
    ) -> Poll<Result<Message, RequestError>> {
        let mut record = record.borrow_mut();
        if record.scope.borrow().generation != self.generation {
            return Poll::Ready(Err(RequestError::Unknown {
                cause: Error::STALE,
                recovery: record.recovery.clone(),
            }));
        }
        if record.observed {
            return Poll::Ready(Err(RequestError::Rejected(
                Error::UNAVAILABLE,
            )));
        }
        let Some(outcome) = record.outcome.clone() else {
            record.waker = Some(context.waker().clone());
            return Poll::Pending;
        };
        let outcome = self.deliver(&record, outcome);
        if matches!(outcome, Err(RequestError::Unknown { .. })) {
            return Poll::Ready(outcome);
        }
        record.observed = true;
        let id = record.id;
        drop(record);
        let retired = self.shared.borrow_mut().retire(id);
        drop(retired);
        Poll::Ready(outcome)
    }
}

impl Wait {
    fn deliver(
        &self,
        record: &Record,
        outcome: Result<Message, Error>,
    ) -> Result<Message, RequestError> {
        let outcome = self.translate(record, outcome)?;
        if self.recovery {
            return reovim_kernel_interface::requests::encode(outcome).map_err(
                |cause| RequestError::Unknown {
                    cause,
                    recovery: record.recovery.clone(),
                },
            );
        }
        outcome.map_err(RequestError::Failed)
    }

    fn translate(
        &self,
        record: &Record,
        outcome: Result<Message, Error>,
    ) -> Result<Result<Message, Error>, RequestError> {
        let mut shared = self.shared.borrow_mut();
        let mut scope = record.scope.borrow_mut();
        let unknown = |cause| RequestError::Unknown {
            cause,
            recovery: record.recovery.clone(),
        };
        match outcome {
            Ok(mut message) => {
                translate_refs(&mut shared, &mut scope, &mut message)
                    .map_err(unknown)?;
                Ok(Ok(message))
            }
            Err(mut error) => {
                error.id = error.id.filter(|id| scope.visible(&shared, id.0));
                error.id = error
                    .id
                    .map(|id| scope.export(&mut shared, id.0))
                    .transpose()
                    .map_err(unknown)?;
                Ok(Err(error))
            }
        }
    }
}

impl Shared {
    pub fn retire(&mut self, id: u64) -> Option<Rc<RefCell<Record>>> {
        let index =
            self.requests.iter().position(|record| record.borrow().id == id)?;
        let record = self.requests.swap_remove(index);
        self.entries.retain(|entry| {
            !matches!(entry.kind, Kind::Work { request, .. } if request == id)
        });
        Some(record)
    }
}

fn translate_refs(
    shared: &mut Shared,
    scope: &mut Scope,
    message: &mut Message,
) -> Result<(), Error> {
    for resource in &mut message.resources {
        *resource = scope.export(shared, resource.0)?;
    }
    Ok(())
}
