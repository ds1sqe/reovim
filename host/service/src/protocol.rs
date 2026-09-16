//! Bounded caller operation frames; authentication/opening belongs to the host.
//! Example: decode a frame, dispatch under its retained Caller, encode a reply.

use alloc::{rc::Rc, string::String};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Outcome,
    call::Cancellation,
    caller::{Call, Caller, Status},
};

mod decode;
mod encode;

/// One authenticated caller token plus its exact action.
/// Example: resuming a token selects old custody, not a new Caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    /// Host-issued credential, e.g. 16 random bytes encoded as lowercase hex.
    pub caller: [u8; 16],
    /// Parsed operation, e.g. Result for lost delivery recovery.
    pub action: Action,
}

/// Common caller mechanisms; module-local commands stay inside Call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    /// Reserve with a nonzero increasing client sequence, distinct from IDs.
    /// Repeat the current sequence to recover a lost allocation reply.
    Reserve(u64),
    /// Submit captured input, e.g. operation 10 invokes bound exec.
    Call(Call),
    /// Read original work, e.g. after a socket disconnect.
    Result(u64),
    /// Request cancellation, e.g. when the user stops waiting.
    Cancel(u64),
    /// Refine UNKNOWN, e.g. query the original issued effect.
    Reconcile(u64),
    /// Adopt successful returned access exactly once.
    Adopt(u64),
    /// Release receipt custody while Runtime retains execution cleanup.
    Release(u64),
}

/// Operation reply; missing transport evidence is never encoded as Error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reply {
    /// Fresh reservation ID, e.g. 7.
    Reserved(u64),
    /// Submission/control acknowledged; does not assert effect success.
    Accepted,
    /// Cancellation acknowledgement, not rollback evidence.
    Cancelled(Cancellation),
    /// Original custody evidence, e.g. immutable UNKNOWN progress.
    Status(Status),
    /// Rejected call/control; admitted failures use `Outcome::Failed`.
    Error(Error),
}

impl Frame {
    /// Parse one frame without its newline, e.g. from `transport::receive`.
    /// # Errors
    /// Malformed, retired and oversized records reject before dispatch.
    pub fn decode(line: &str) -> Result<Self, Error> {
        decode::frame(line)
    }

    /// Encode canonical fields, e.g. retaining the exact input Ref order.
    /// # Errors
    /// Invalid envelopes and oversized frames reject before transmission.
    pub fn encode(&self) -> Result<String, Error> {
        encode::frame(self)
    }
}

impl Reply {
    /// Parse operation evidence, e.g. unknown provider error codes survive.
    /// # Errors
    /// Invalid fields and oversized frames provide no effect evidence.
    pub fn decode(line: &str) -> Result<Self, Error> {
        decode::reply(line)
    }

    /// Encode retained evidence, e.g. UNKNOWN has no returned reference slots.
    /// # Errors
    /// Oversized results require explicit transport recovery, never a retry.
    pub fn encode(&self) -> Result<String, Error> {
        encode::reply(self)
    }
}

/// Retained authenticated endpoint; sockets may reconnect to the same owner.
///
/// Example: opening binds a credential and granted Caller before call traffic.
/// Keep this endpoint across reconnects. Serialize reservations per credential;
/// advance the persisted sequence only after receiving its allocated ID.
/// Calls/results may proceed concurrently with later reservations.
pub struct Endpoint {
    token: [u8; 16],
    caller: Caller,
    reservation: RefCell<Option<(u64, u64)>>,
}

impl Endpoint {
    /// Pair an opening-issued credential with its Caller, e.g. once at boot.
    #[must_use]
    pub const fn new(token: [u8; 16], caller: Caller) -> Self {
        Self { token, caller, reservation: RefCell::new(None) }
    }

    /// Dispatch under retained authority; result reads never drive providers.
    /// Example: a duplicate exact Call joins its original execution.
    #[must_use]
    pub fn dispatch(&self, frame: Frame) -> Reply {
        if frame.caller != self.token {
            return Reply::Error(Error::DENIED);
        }
        dispatch(self, frame.action).unwrap_or_else(Reply::Error)
    }
    fn reserve(&self, sequence: u64) -> Result<Reply, Error> {
        if sequence == 0 {
            return Err(Error::INVALID);
        }
        // A reentrant allocation cannot replace an acknowledgement in flight.
        let mut prior =
            self.reservation.try_borrow_mut().map_err(|_| Error::BUSY)?;
        if prior.is_some_and(|(last, _)| sequence < last) {
            return Err(Error::CLOSED);
        }
        if let Some((_, request)) = prior.filter(|(last, _)| sequence == *last)
        {
            return Ok(Reply::Reserved(request));
        }
        let request = self.caller.reserve()?;
        *prior = Some((sequence, request));
        Ok(Reply::Reserved(request))
    }
}

fn dispatch(endpoint: &Endpoint, action: Action) -> Result<Reply, Error> {
    let caller = &endpoint.caller;
    match action {
        Action::Reserve(sequence) => endpoint.reserve(sequence),
        Action::Call(call) => caller.submit(call).map(|()| Reply::Accepted),
        Action::Result(id) => caller.snapshot(id).map(Reply::Status),
        Action::Cancel(id) => caller.cancel(id).map(Reply::Cancelled),
        Action::Reconcile(id) => caller.reconcile(id).map(|()| Reply::Accepted),
        Action::Adopt(id) => caller.adopt(id).map(|()| Reply::Accepted),
        Action::Release(id) => caller.release(id).map(|()| Reply::Accepted),
    }
}

fn outcome(value: Outcome) -> Reply {
    Reply::Status(Status::Outcome(Rc::new(value)))
}
