use super::{Call, Cancellation, Context, Error, Rc, Status};

/// Caller-bound operations over private state, e.g. Runtime custody.
pub struct CallerOps<State> {
    /// Reserve a nonzero monotonic request before effects.
    pub reserve: fn(&State) -> Result<u64, Error>,
    /// Admit captured input once; exact retries join the original work.
    pub submit: fn(&State, Call) -> Result<(), Error>,
    /// Observe custody; optional context registers wake interest.
    pub result:
        fn(&State, u64, Option<&mut Context<'_>>) -> Result<Status, Error>,
    /// Ask the original execution to cancel; acknowledgement is not rollback.
    pub cancel: fn(&State, u64) -> Result<Cancellation, Error>,
    /// Refine original UNKNOWN without another submission.
    pub reconcile: fn(&State, u64) -> Result<(), Error>,
    /// Transfer successful returned access once, idempotently.
    pub adopt: fn(&State, u64) -> Result<(), Error>,
    /// Retire receipt custody; pending execution remains Runtime-owned.
    pub release: fn(&State, u64) -> Result<(), Error>,
}

trait Dispatch {
    fn reserve(&self) -> Result<u64, Error>;
    fn submit(&self, call: Call) -> Result<(), Error>;
    fn result(
        &self,
        id: u64,
        cx: Option<&mut Context<'_>>,
    ) -> Result<Status, Error>;
    fn cancel(&self, id: u64) -> Result<Cancellation, Error>;
    fn reconcile(&self, id: u64) -> Result<(), Error>;
    fn adopt(&self, id: u64) -> Result<(), Error>;
    fn release(&self, id: u64) -> Result<(), Error>;
}

struct Implementation<State> {
    state: State,
    ops: CallerOps<State>,
}

impl<State> Dispatch for Implementation<State> {
    fn reserve(&self) -> Result<u64, Error> {
        (self.ops.reserve)(&self.state)
    }
    fn submit(&self, call: Call) -> Result<(), Error> {
        (self.ops.submit)(&self.state, call)
    }
    fn result(
        &self,
        id: u64,
        cx: Option<&mut Context<'_>>,
    ) -> Result<Status, Error> {
        (self.ops.result)(&self.state, id, cx)
    }
    fn cancel(&self, id: u64) -> Result<Cancellation, Error> {
        (self.ops.cancel)(&self.state, id)
    }
    fn reconcile(&self, id: u64) -> Result<(), Error> {
        (self.ops.reconcile)(&self.state, id)
    }
    fn adopt(&self, id: u64) -> Result<(), Error> {
        (self.ops.adopt)(&self.state, id)
    }
    fn release(&self, id: u64) -> Result<(), Error> {
        (self.ops.release)(&self.state, id)
    }
}

/// Authenticated caller facade; cloning shares authority and receipt custody.
/// Example: a transport reconnect retains this Caller to recover old results.
#[derive(Clone)]
pub struct Caller(Rc<dyn Dispatch>);

impl Caller {
    /// Pair private caller state with its typed operations at composition.
    /// Example: Runtime supplies its scoped custody without exposing its type.
    #[must_use]
    pub fn new<State: 'static>(state: State, ops: CallerOps<State>) -> Self {
        Self(Rc::new(Implementation { state, ops }))
    }

    /// Reserve custody before submission, e.g. before sending a service frame.
    /// # Errors
    /// Closed or Pressure leaves no new reservation.
    pub fn reserve(&self) -> Result<u64, Error> {
        self.0.reserve()
    }

    /// Submit once or join an identical call, e.g. after losing its reply.
    /// # Errors
    /// Rejection proves no new admission; changed retries return Conflict.
    pub fn submit(&self, call: Call) -> Result<(), Error> {
        self.0.submit(call)
    }

    /// Observe retained work and register interest, e.g. after subject close.
    /// # Errors
    /// Released or foreign IDs provide no recovery access.
    pub fn result(
        &self,
        id: u64,
        context: &mut Context<'_>,
    ) -> Result<Status, Error> {
        self.0.result(id, Some(context))
    }

    /// Inspect custody without a wake registration, e.g. a service Result.
    /// # Errors
    /// Released or foreign IDs provide no recovery access.
    pub fn snapshot(&self, id: u64) -> Result<Status, Error> {
        self.0.result(id, None)
    }

    /// Request cancellation, e.g. when the user stops waiting.
    /// # Errors
    /// Failure preserves the original effect's custody.
    pub fn cancel(&self, id: u64) -> Result<Cancellation, Error> {
        self.0.cancel(id)
    }

    /// Refine UNKNOWN, e.g. query evidence for an already issued pulse.
    /// # Errors
    /// Unsupported never authorizes repeating the effect.
    pub fn reconcile(&self, id: u64) -> Result<(), Error> {
        self.0.reconcile(id)
    }

    /// Adopt successful output once, e.g. after SDK handle construction.
    /// # Errors
    /// Pending or UNKNOWN results cannot be adopted as success.
    pub fn adopt(&self, id: u64) -> Result<(), Error> {
        self.0.adopt(id)
    }

    /// Retire receipt custody, e.g. after adopting returned access.
    /// # Errors
    /// Foreign IDs reject; admitted cleanup remains owned after release.
    pub fn release(&self, id: u64) -> Result<(), Error> {
        self.0.release(id)
    }
}
