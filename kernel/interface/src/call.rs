//! Owned declared values and provider request custody for same-build adapters.
//!
//! Example: Runtime polls `Pending`, takes one owned `Completion`, validates
//! and adopts its references, releases its provider owner, then finishes work.
//! Native modules use the C ABI instead of these Rust-owned types.

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    any::Any,
    task::{Context, Poll},
};

use crate::{Error, resource::AccessId};

/// Captured canonical bytes and access slots; no operation or schema selector.
/// Example: U32 input is four bytes with an empty references array.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    /// Canonical bytes, e.g. little-endian fixed-width numbers.
    pub data: Box<[u8]>,
    /// Caller-scoped slots; every element must occur in the declared value.
    pub references: Box<[AccessId]>,
}

impl Value {
    /// Copies bounded input storage before admission.
    /// # Errors
    /// Allocation failure returns Pressure; no provider has executed.
    /// Example: `Value::copy(&42_u32.to_le_bytes(), &[])` captures a U32.
    pub fn copy(data: &[u8], references: &[AccessId]) -> Result<Self, Error> {
        Ok(Self { data: copy(data)?, references: copy(references)? })
    }

    /// Empty canonical Unit, e.g. a no-argument control.
    #[must_use]
    pub fn unit() -> Self {
        Self { data: Box::new([]), references: Box::new([]) }
    }
}

fn copy<T: Copy>(input: &[T]) -> Result<Box<[T]>, Error> {
    let mut output = Vec::new();
    output.try_reserve_exact(input.len()).map_err(|_| Error::PRESSURE)?;
    output.extend_from_slice(input);
    Ok(output.into_boxed_slice())
}

/// Admitted effect evidence; e.g. Failed never retroactively means Rejected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// Completed declared result; e.g. accepted local send bytes.
    Ok(Value),
    /// Terminal failure without outstanding effect recovery.
    Failed(Error),
    /// Original effect remains uncertain; no returned access is permitted.
    Unknown {
        /// Machine cause, e.g. lost provider evidence.
        error: Error,
        /// Optional declared progress; absent when no trustworthy value exists.
        progress: Option<Value>,
    },
}

/// Taken provider result, e.g. spans and provisional access stay owned while
/// Runtime validates and transfers their references into caller custody.
pub struct Completion {
    /// Copied result/evidence; does not itself adopt returned access.
    pub outcome: Outcome,
    /// Known READ/WRITE/STATUS events within the bound method's allowance.
    pub activity: u8,
    /// Provider allocator/reference cleanup; release after validation/adoption.
    pub owner: Option<Box<dyn Any>>,
}

/// Cancellation response, e.g. Accepted is a request acknowledgement only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cancellation {
    /// Cancellation accepted; eventual outcome supplies effect evidence.
    Accepted,
    /// Work cannot be cancelled; Runtime continues its original custody.
    TooLate,
}

/// Typed pending table paired with private state. Example: the native adapter
/// supplies callbacks over a retained C context and executable code owner.
pub struct PendingOps<State> {
    /// Observes readiness without transferring output ownership.
    pub poll: fn(&mut State, &mut Context<'_>) -> Result<Poll<()>, Error>,
    /// Transfers one ready generation; e.g. a second take returns Closed.
    pub take: fn(&mut State) -> Result<Completion, Error>,
    /// Requests cancellation while preserving the effect's actual outcome.
    pub cancel: fn(&mut State) -> Result<Cancellation, Error>,
    /// Refines a taken UNKNOWN; returns Unsupported when refinement is absent.
    pub reconcile: fn(&mut State) -> Result<(), Error>,
    /// Returns OK only when context/output/cleanup obligations have drained.
    pub finish: fn(&mut State) -> Result<(), Error>,
}

trait Dispatch {
    fn poll(&mut self, context: &mut Context<'_>) -> Result<Poll<()>, Error>;
    fn take(&mut self) -> Result<Completion, Error>;
    fn cancel(&mut self) -> Result<Cancellation, Error>;
    fn reconcile(&mut self) -> Result<(), Error>;
    fn finish(&mut self) -> Result<(), Error>;
}

struct Implementation<State> {
    state: State,
    ops: PendingOps<State>,
}

impl<State> Dispatch for Implementation<State> {
    fn poll(&mut self, context: &mut Context<'_>) -> Result<Poll<()>, Error> {
        (self.ops.poll)(&mut self.state, context)
    }
    fn take(&mut self) -> Result<Completion, Error> {
        (self.ops.take)(&mut self.state)
    }
    fn cancel(&mut self) -> Result<Cancellation, Error> {
        (self.ops.cancel)(&mut self.state)
    }
    fn reconcile(&mut self) -> Result<(), Error> {
        (self.ops.reconcile)(&mut self.state)
    }
    fn finish(&mut self) -> Result<(), Error> {
        (self.ops.finish)(&mut self.state)
    }
}

/// Paired provider request; Runtime owns execution independently of waiters.
/// Example: finish before dropping; failed finish must remain driven/owned.
pub struct Pending {
    implementation: Box<dyn Dispatch>,
    code: Option<Rc<dyn Any>>,
}

impl Pending {
    /// Pairs state and its typed table, e.g. a device request and callbacks.
    #[must_use]
    pub fn new<State: 'static>(state: State, ops: PendingOps<State>) -> Self {
        Self {
            implementation: Box::new(Implementation { state, ops }),
            code: None,
        }
    }

    pub(crate) fn keep_code(&mut self, code: Rc<dyn Any>) {
        self.code = Some(code);
    }

    /// Observes readiness. Example: Pending retains all execution obligations.
    /// # Errors
    /// Provider faults retain original request custody.
    pub fn poll(
        &mut self,
        context: &mut Context<'_>,
    ) -> Result<Poll<()>, Error> {
        self.implementation.poll(context)
    }

    /// Takes one ready generation. Example: repeated take returns Closed.
    /// # Errors
    /// NotReady/Closed/faults transfer no output ownership.
    pub fn take(&mut self) -> Result<Completion, Error> {
        self.implementation.take()
    }

    /// Requests cancellation. Example: Accepted does not prove rollback.
    /// # Errors
    /// Provider faults do not erase existing execution obligations.
    pub fn cancel(&mut self) -> Result<Cancellation, Error> {
        self.implementation.cancel()
    }

    /// Refines original UNKNOWN work; e.g. never calls exec a second time.
    /// # Errors
    /// Missing support, Busy and terminal generations remain explicit.
    pub fn reconcile(&mut self) -> Result<(), Error> {
        self.implementation.reconcile()
    }

    /// Joins cleanup; e.g. Busy keeps the pending owner in Runtime.
    /// # Errors
    /// Failure leaves context and code retained; retry only this cleanup.
    pub fn finish(&mut self) -> Result<(), Error> {
        self.implementation.finish()
    }
}
