//! Request-scoped candidate preparation, without installation authority.

use alloc::rc::Rc;

use super::State;
use crate::Error;

/// Retains a candidate only after the request owner validates its proposal.
/// The owner checks WRITE, binding, quota and an empty staging slot.
pub type Propose<Context> = fn(&Context, &State) -> Result<(), Error>;

trait Dispatch {
    fn propose(&self, candidate: &State) -> Result<(), Error>;
}

struct Implementation<Context> {
    context: Context,
    propose: Propose<Context>,
}

impl<Context> Dispatch for Implementation<Context> {
    fn propose(&self, candidate: &State) -> Result<(), Error> {
        (self.propose)(&self.context, candidate)
    }
}

/// Preparation capability for one request and selected immutable state.
///
/// Clones share the same request. Its installation owner validates proposals,
/// retains at most one candidate and fences the capability after settlement.
/// Providers cannot install state or History through this capability.
#[derive(Clone)]
pub struct Edit {
    before: State,
    implementation: Rc<dyn Dispatch>,
}

impl Edit {
    /// Pairs selected state with the request owner's staging callback.
    /// Context holds captured authority, binding and reserved capacity.
    #[must_use]
    pub fn new<Context: 'static>(
        before: State,
        context: Context,
        propose: Propose<Context>,
    ) -> Self {
        Self {
            before,
            implementation: Rc::new(Implementation { context, propose }),
        }
    }

    /// Borrows the selected immutable state for its originating provider.
    /// Runtime/Core retain this value without interpreting its private data.
    #[must_use]
    pub const fn before(&self) -> &State {
        &self.before
    }

    /// Proposes a candidate without changing the selected working state.
    /// Success retains one candidate; the supplier keeps its original owner.
    /// # Errors
    /// Closed staging, insufficient authority, a foreign binding, a duplicate
    /// proposal or exhausted capacity leave candidate ownership unchanged.
    pub fn propose(&self, candidate: &State) -> Result<(), Error> {
        self.implementation.propose(candidate)
    }
}
