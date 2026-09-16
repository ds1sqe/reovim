//! Activation-scoped publication supplied by the owning Runtime.

use alloc::rc::Rc;

use super::ResourceProvider;
use crate::Error;

/// Stages one relative export, e.g. `devices/queue`, without publishing it.
pub type StageExport<State> = fn(
    &State,
    &[u8],
    ResourceProvider,
) -> Result<(), (Error, ResourceProvider)>;

/// Publication owner's table, e.g. validation against a reserved module mount.
pub struct RegistrarOps<State> {
    /// Copies the relative path and stages the provider only on success.
    /// Rejection returns ownership; no provider callback may run under a guard.
    pub stage: StageExport<State>,
}

trait Dispatch {
    fn stage(
        &self,
        path: &[u8],
        provider: ResourceProvider,
    ) -> Result<(), (Error, ResourceProvider)>;
}

struct Implementation<State> {
    state: State,
    ops: RegistrarOps<State>,
}

impl<State> Dispatch for Implementation<State> {
    fn stage(
        &self,
        path: &[u8],
        provider: ResourceProvider,
    ) -> Result<(), (Error, ResourceProvider)> {
        (self.ops.stage)(&self.state, path, provider)
    }
}

/// Provisional module exports, e.g. two providers committed together at Ready.
///
/// Clones share one activation. The publication owner enforces the mount,
/// grants and quotas, and fences all copies on publication or rollback.
/// A retained copy cannot publish later device arrivals outside activation.
#[derive(Clone)]
pub struct Registrar {
    implementation: Rc<dyn Dispatch>,
}

impl Registrar {
    /// Pairs owner state with its staging table; for example, a fenced batch.
    #[must_use]
    pub fn new<State: 'static>(state: State, ops: RegistrarOps<State>) -> Self {
        Self { implementation: Rc::new(Implementation { state, ops }) }
    }

    /// Stages an owned provider at a relative path below the authorized mount.
    /// Example: `stage(b"queue", provider)` transfers only on success.
    /// # Errors
    /// Closed activation, invalid paths, conflicts and pressure return the
    /// provider unchanged. The supplying module retains cleanup responsibility.
    pub fn stage(
        &self,
        path: &[u8],
        provider: ResourceProvider,
    ) -> Result<(), (Error, ResourceProvider)> {
        self.implementation.stage(path, provider)
    }
}
