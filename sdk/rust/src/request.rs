use alloc::rc::Rc;
use core::{
    future::poll_fn,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Error, Outcome, Value,
    call::Cancellation,
    caller::{Call, Caller, Expected, Operation, Status},
    resource::AccessId,
};

/// Caller-owned receipt and optional exact submission for retry.
///
/// Example: reserve, submit, await `outcome`, adopt successful returned access,
/// then release. Dropping this object or an outcome future does not release
/// Runtime custody; save `id()` to recover through the same Caller.
pub struct Request {
    caller: Caller,
    id: u64,
    call: Option<Call>,
}

impl Request {
    /// Reserve custody and capture the exact call before transmission.
    /// Example: Exec uses command zero and the bound access token.
    /// # Errors
    /// Invalid envelope or exhausted custody admits no provider work.
    pub fn reserve(
        caller: &Caller,
        operation: Operation,
        subject: AccessId,
        command: u32,
        expected: Option<Expected>,
        input: Value,
    ) -> Result<Self, Error> {
        let mut call =
            Call { request: 1, operation, subject, command, expected, input };
        call.validate()?;
        let id = caller.reserve()?;
        call.request = id;
        Ok(Self { caller: caller.clone(), id, call: Some(call) })
    }

    /// Recover existing custody without reconstructing or resending input.
    /// Example: use the saved ID after a transport reconnect to this Caller.
    /// # Errors
    /// Zero is invalid; result lookup separately checks live caller custody.
    pub fn recover(caller: &Caller, id: u64) -> Result<Self, Error> {
        if id == 0 {
            return Err(Error::INVALID);
        }
        Ok(Self { caller: caller.clone(), id, call: None })
    }

    /// Return the caller-local receipt number, e.g. for reconnect recovery.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Submit the captured call or join its exact original work.
    /// Example: a lost acknowledgement permits retrying this same capture.
    /// # Errors
    /// Rejection retains this releasable reservation. Recovery-only requests
    /// have no captured input and cannot submit an inferred replacement.
    pub fn submit(&self) -> Result<(), Error> {
        let call = self.call.as_ref().ok_or(Error::INVALID)?;
        self.caller.submit(call.clone())
    }

    /// Observe Reserved/Pending/Outcome without driving any provider.
    /// Example: Reserved proves that this receipt has no admitted call.
    /// # Errors
    /// Closed/foreign receipts and waiter pressure remain explicit.
    pub fn status(&self, context: &mut Context<'_>) -> Result<Status, Error> {
        self.caller.result(self.id, context)
    }

    /// Await one immutable outcome generation; UNKNOWN remains an Outcome.
    /// Example: retain the returned Rc while reconciling the original request.
    /// Dropping this future leaves execution and result custody intact.
    /// # Errors
    /// A still-reserved receipt returns `NotReady`; unavailable evidence never
    /// authorizes submitting a new effect under a different receipt.
    #[allow(clippy::future_not_send)]
    pub async fn outcome(&self) -> Result<Rc<Outcome>, Error> {
        poll_fn(|context| match self.status(context) {
            Ok(Status::Pending) => Poll::Pending,
            Ok(Status::Reserved) => Poll::Ready(Err(Error::NOT_READY)),
            Ok(Status::Outcome(outcome)) => Poll::Ready(Ok(outcome)),
            Err(error) => Poll::Ready(Err(error)),
        })
        .await
    }

    /// Ask original work to cancel, e.g. after a user stops waiting.
    /// # Errors
    /// Failure does not settle or discard the original effect.
    pub fn cancel(&self) -> Result<Cancellation, Error> {
        self.caller.cancel(self.id)
    }

    /// Refine the original UNKNOWN, e.g. inspect an already issued pulse.
    /// # Errors
    /// Unsupported or Closed never licenses repeating the effect.
    pub fn reconcile(&self) -> Result<(), Error> {
        self.caller.reconcile(self.id)
    }

    /// Adopt successful access after constructing its SDK owners.
    /// Example: repeated adoption transfers no additional handles.
    /// # Errors
    /// Pending and UNKNOWN cannot be adopted as success.
    pub fn adopt(&self) -> Result<(), Error> {
        self.caller.adopt(self.id)
    }

    /// Release receipt custody, e.g. after adopting successful returned access.
    /// # Errors
    /// Failure leaves the ID available for explicit recovery/cleanup.
    pub fn release(&self) -> Result<(), Error> {
        self.caller.release(self.id)
    }
}
