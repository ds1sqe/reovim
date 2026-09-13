use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use reovim_kernel_interface::access::Method;
use reovim_kernel_interface::buffer::StateReply;
use reovim_kernel_interface::driver::Lifecycle;
use reovim_kernel_interface::provider::Permissions;
use reovim_kernel_interface::{Error, Message, RequestError};

use crate::{
    Shared,
    namespace::{Kind, Scope},
};

mod wake;
pub use wake::Schedule;

pub enum Work {
    Queued,
    ModuleUnload,
    Resource(reovim_kernel_interface::access::Reply),
    Model(StateReply),
}

impl Work {
    pub fn provider(
        reply: reovim_kernel_interface::provider::ProviderReply,
    ) -> Self {
        Self::Resource(Box::pin(async move { reply.await.map_err(Into::into) }))
    }
}

pub struct Record {
    pub _capture: Option<Rc<crate::namespace::Capture>>,
    pub id: u64,
    pub parent: Option<u64>,
    pub scope: Rc<RefCell<Scope>>,
    pub receiver: Option<Rc<RefCell<Scope>>>,
    pub subject: u64,
    pub method: Method,
    pub rights: Permissions,
    pub input: Message,
    // Futures/results are destroyed before the captured implementation.
    pub work: Option<Work>,
    pub outcome: Option<Result<Message, RequestError>>,
    pub target: Kind,
    pub references: Vec<Kind>,
    pub activation: Option<Rc<Cell<Lifecycle>>>,
    pub output: Box<[Permissions]>,
    pub snapshot: Option<Rc<crate::sessions::Snapshot>>,
    pub backing: Option<crate::opening::Association>,
    pub cancelled: bool,
    pub observed: bool,
    pub readers: usize,
    pub recovery_work: Option<(u64, Permissions)>,
    pub recovery: Box<str>,
    pub wakers: Vec<(u64, Waker)>,
    pub schedule: Schedule,
}

impl Drop for Record {
    fn drop(&mut self) {
        // Pending destructors may still need their borrowed reference grants.
        drop(self.work.take());
        if let Some(receiver) = &self.receiver {
            receiver
                .borrow_mut()
                .loans
                .retain(|(owner, _, _)| *owner != self.id);
        }
    }
}

impl Record {
    /// The admitted Session remains distinct from a participation subject.
    pub fn session(&self) -> Option<&Rc<RefCell<crate::sessions::Session>>> {
        self.snapshot.as_ref()?;
        match self.references.first()? {
            Kind::Session(session) => Some(session),
            _ => None,
        }
    }
}

pub struct Wait {
    pub id: u64,
    pub shared: Rc<RefCell<Shared>>,
    pub scope: Rc<RefCell<Scope>>,
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
            self.release();
        }
        result
    }
}

impl Wait {
    fn release(&mut self) {
        let Some(record) = self.record.take() else { return };
        let mut record = record.borrow_mut();
        // Keep native wake owners pinned until their admitted work settles.
        if self.recovery {
            record.readers -= 1;
        }
    }

    fn poll_record(
        &self,
        record: &Rc<RefCell<Record>>,
        context: &Context<'_>,
    ) -> Poll<Result<Message, RequestError>> {
        let mut record = record.borrow_mut();
        if self.scope.borrow().generation != self.generation {
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
            let limit = self.shared.borrow().limits.requests.saturating_add(1);
            record.listen(self.id, context.waker(), limit).map_err(
                |cause| RequestError::Unknown {
                    cause,
                    recovery: record.recovery.clone(),
                },
            )?;
            return Poll::Pending;
        };
        let outcome = self.deliver(&record, outcome);
        if self.recovery
            || matches!(outcome, Err(RequestError::Unknown { .. }))
            || self.shared.borrow().retained_work(record.id)
        {
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

impl Record {
    pub fn references(&self, key: reovim_kernel_interface::ResourceId) -> bool {
        self.recovery_work.is_some_and(|(work, _)| work == key.0)
            || self.input.resources.contains(&key)
            || self
                .outcome
                .as_ref()
                .and_then(|outcome| outcome.as_ref().ok())
                .is_some_and(|message| message.resources.contains(&key))
    }

    fn listen(
        &mut self,
        id: u64,
        waker: &Waker,
        limit: usize,
    ) -> Result<(), Error> {
        if let Some((_, waiting)) =
            self.wakers.iter_mut().find(|(owner, _)| *owner == id)
        {
            waiting.clone_from(waker);
            return Ok(());
        }
        if self.wakers.len() == limit {
            return Err(Error::PRESSURE);
        }
        self.wakers.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        self.wakers.push((id, waker.clone()));
        Ok(())
    }
}

impl Drop for Wait {
    fn drop(&mut self) {
        self.release();
    }
}

impl Wait {
    fn deliver(
        &self,
        record: &Record,
        outcome: Result<Message, RequestError>,
    ) -> Result<Message, RequestError> {
        let outcome = self.translate(record, outcome)?;
        if self.recovery {
            return reovim_kernel_interface::requests::encode_outcome(outcome)
                .map_err(|cause| RequestError::Unknown {
                    cause,
                    recovery: record.recovery.clone(),
                });
        }
        match outcome {
            Err(RequestError::Unknown { cause, .. }) => {
                Err(RequestError::Unknown {
                    cause,
                    recovery: record.recovery.clone(),
                })
            }
            other => other,
        }
    }

    fn translate(
        &self,
        record: &Record,
        outcome: Result<Message, RequestError>,
    ) -> Result<Result<Message, RequestError>, RequestError> {
        let mut shared = self.shared.borrow_mut();
        let mut scope = self.scope.borrow_mut();
        let unknown = |cause| RequestError::Unknown {
            cause,
            recovery: record.recovery.clone(),
        };
        match outcome {
            Ok(mut message) => {
                crate::binding::grant_outputs(
                    &shared,
                    &mut scope,
                    &record.output,
                    &message,
                )
                .map_err(unknown)?;
                translate_refs(&mut shared, &mut scope, &mut message)
                    .map_err(unknown)?;
                Ok(Ok(message))
            }
            Err(RequestError::Unknown { cause, recovery }) => {
                crate::binding::recovery(&shared, record, &mut scope)
                    .map_err(unknown)?;
                let cause = translate_error(&mut shared, &mut scope, cause)
                    .map_err(unknown)?;
                Ok(Err(RequestError::Unknown { cause, recovery }))
            }
            Err(
                RequestError::Rejected(error) | RequestError::Failed(error),
            ) => {
                let error = translate_error(&mut shared, &mut scope, error)
                    .map_err(unknown)?;
                Ok(Err(RequestError::Failed(error)))
            }
        }
    }
}

impl Shared {
    fn retained_work(&self, id: u64) -> bool {
        self.entries.iter().any(|entry| {
            matches!(entry.kind,
            Kind::Work { request, retained: true, .. } if request == id)
        })
    }

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

pub fn translate_error(
    shared: &mut Shared,
    scope: &mut Scope,
    mut error: Error,
) -> Result<Error, Error> {
    error.id = error.id.filter(|id| scope.visible(shared, id.0));
    error.id = error.id.map(|id| scope.export(shared, id.0)).transpose()?;
    Ok(error)
}
