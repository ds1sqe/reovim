//! Request-scoped factory staging supplied by the installation owner.

use alloc::rc::Rc;

use super::ResourceProvider;
use crate::{Error, resource::AccessId};

/// Staging callback that returns the supplied provider on rejection.
pub type StageResource<State, Output = ()> =
    fn(&State, ResourceProvider) -> Result<Output, (Error, ResourceProvider)>;

/// Stages a primary provider together with its new namespace object key.
pub type StageKeyed<State> = fn(
    &State,
    &[u8],
    ResourceProvider,
) -> Result<(), (Error, ResourceProvider)>;

/// Typed staging table; success transfers a provider into the request, without
/// publishing it. For example, Runtime accepts one immutable primary Resource.
pub struct CreationOps<State> {
    /// Validates and stages the primary provider. Rejection returns its owner
    /// unchanged; the supplier remains responsible for stopping it.
    pub resource: StageResource<State>,
    /// Namespace creation reserves the key and provider as one primary target.
    /// Ordinary factories and materialization leave this unsupported.
    pub keyed: Option<StageKeyed<State>>,
    /// Stages an auxiliary provider for the same result batch. Its provisional
    /// token is only valid as declared output of this request.
    pub auxiliary: Option<StageResource<State, AccessId>>,
}

trait Dispatch {
    fn keyed_resource(
        &self,
        key: &[u8],
        provider: ResourceProvider,
    ) -> Result<(), (Error, ResourceProvider)>;
    fn resource(
        &self,
        provider: ResourceProvider,
    ) -> Result<(), (Error, ResourceProvider)>;
    fn auxiliary(
        &self,
        provider: ResourceProvider,
    ) -> Result<AccessId, (Error, ResourceProvider)>;
}

struct Implementation<State> {
    state: State,
    ops: CreationOps<State>,
}

impl<State> Dispatch for Implementation<State> {
    fn keyed_resource(
        &self,
        key: &[u8],
        provider: ResourceProvider,
    ) -> Result<(), (Error, ResourceProvider)> {
        let Some(keyed) = self.ops.keyed else {
            return Err((Error::UNSUPPORTED, provider));
        };
        keyed(&self.state, key, provider)
    }

    fn resource(
        &self,
        provider: ResourceProvider,
    ) -> Result<(), (Error, ResourceProvider)> {
        (self.ops.resource)(&self.state, provider)
    }

    fn auxiliary(
        &self,
        provider: ResourceProvider,
    ) -> Result<AccessId, (Error, ResourceProvider)> {
        let Some(auxiliary) = self.ops.auxiliary else {
            return Err((Error::UNSUPPORTED, provider));
        };
        auxiliary(&self.state, provider)
    }
}

/// Factory's staging capability, retained through asynchronous preparation.
///
/// Clones refer to the same request. The installation owner fences callbacks
/// after acceptance or cancellation; retaining a clone cannot reopen staging.
#[derive(Clone)]
pub struct Creation {
    implementation: Rc<dyn Dispatch>,
}

impl Creation {
    /// Pairs the installation owner's state and callbacks. For example, state
    /// holds reserved capacity, the expected API and the request's close fence.
    #[must_use]
    pub fn new<State: 'static>(state: State, ops: CreationOps<State>) -> Self {
        Self { implementation: Rc::new(Implementation { state, ops }) }
    }

    /// Stages an owned primary Resource; it is not visible to callers yet.
    /// # Errors
    /// Closed staging, a duplicate primary, incompatible declarations or quota
    /// rejection leave the supplied provider with the factory for cleanup.
    pub fn resource(
        &self,
        provider: ResourceProvider,
    ) -> Result<(), (Error, ResourceProvider)> {
        self.implementation.resource(provider)
    }

    /// Stages a newly created namespace object with its stable opaque key.
    /// For example, reserve a device identity before publishing its name.
    /// The owner copies the key and stages the provider atomically; success
    /// transfers ownership, but does not publish the subject.
    /// # Errors
    /// Unsupported staging, conflicting keys, closed requests or exhausted
    /// capacity leave both the key and provider with the supplier.
    pub fn keyed_resource(
        &self,
        key: &[u8],
        provider: ResourceProvider,
    ) -> Result<(), (Error, ResourceProvider)> {
        self.implementation.keyed_resource(key, provider)
    }

    /// Stages an auxiliary Resource and returns a provisional output token.
    /// It cannot be invoked or captured before the creation batch commits.
    /// # Errors
    /// Unsupported staging, closed requests, incompatible declarations and
    /// exhausted capacity leave the supplied provider owned by the factory.
    pub fn auxiliary(
        &self,
        provider: ResourceProvider,
    ) -> Result<AccessId, (Error, ResourceProvider)> {
        self.implementation.auxiliary(provider)
    }
}
