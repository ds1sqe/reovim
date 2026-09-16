//! Staging an implementation for one opened access, without a new identity.
use super::{ResourceProvider, StageResource};
use crate::{Error, Pending};
use alloc::rc::Rc;

/// Initializes per-open state, e.g. a fresh stream cursor with no data rights.
pub type OpenFn<State> = fn(&mut State, &Open) -> Result<Pending, Error>;
trait Dispatch {
    fn stage(
        &self,
        provider: ResourceProvider,
    ) -> Result<(), (Error, ResourceProvider)>;
}
struct Implementation<State> {
    state: State,
    stage: StageResource<State>,
}
impl<State> Dispatch for Implementation<State> {
    fn stage(
        &self,
        provider: ResourceProvider,
    ) -> Result<(), (Error, ResourceProvider)> {
        (self.stage)(&self.state, provider)
    }
}
/// Request-scoped opened staging, e.g. a cursor implementation for one Acquire.
///
/// Clones share one target. Its owner validates, retains and fences staging;
/// it installs no Resource identity and grants no operation rights.
#[derive(Clone)]
pub struct Open {
    implementation: Rc<dyn Dispatch>,
}
impl Open {
    /// Pairs installation state with its ownership-transfer callback.
    /// Example: state holds the admitted subject and an empty opened slot.
    #[must_use]
    pub fn new<State: 'static>(
        state: State,
        callback: StageResource<State>,
    ) -> Self {
        Self {
            implementation: Rc::new(Implementation { state, stage: callback }),
        }
    }
    /// Transfers a staged opened implementation only when the owner accepts it.
    /// # Errors
    /// Closed staging, duplicates, incompatible tables and exhausted capacity
    /// return the provider unchanged; the supplier remains its cleanup owner.
    pub fn stage(
        &self,
        provider: ResourceProvider,
    ) -> Result<(), (Error, ResourceProvider)> {
        self.implementation.stage(provider)
    }
}
