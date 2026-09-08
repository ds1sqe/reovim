use crate::bound::{BoundValue, CheckedByteCount, CheckedCount, ZeroPolicy};
use crate::buffer::BufferStateStamp;
use crate::lifecycle::LifecycleIdentity;

use crate::interaction::descriptor::{
    InteractionCapabilitySupport, InteractionSessionDescriptor,
};
use crate::interaction::session::{
    InteractionResourceScope, InteractionSessionHandle,
};

use super::{BufferChangeError, BufferChangeNotification};

/// Exact observer lifetime, authorized scope and atomic notification frontier.
///
/// Construction validates descriptor consistency, not permission to subscribe.
/// The Session owner separately checks grants and reserves aggregate capacity,
/// then retains this exact binding for all lookup/revoke operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferChangeSubscription {
    /// Destination observer Session, independent of each change's origin.
    pub session: InteractionSessionHandle,
    /// Exact Resource access and Buffer incarnation visible to the observer.
    pub scope: InteractionResourceScope,
    /// Subscription identity and replacement fence, owned by the Session.
    pub lifecycle: LifecycleIdentity,
    /// Exact current state at the subscription/snapshot race cut.
    pub checkpoint: BufferStateStamp,
    /// Number of independently retained notifications reserved for this reader.
    pub capacity: CheckedCount,
}

impl BufferChangeSubscription {
    /// Binds one authorized descriptor to its immutable initial checkpoint.
    ///
    /// # Errors
    ///
    /// Rejects scope, unsupported observation or per-subscription bounds.
    pub fn new(
        session: &InteractionSessionDescriptor,
        lifecycle: LifecycleIdentity,
        checkpoint: BufferStateStamp,
        capacity: CheckedCount,
    ) -> Result<Self, BufferChangeError> {
        let value = Self {
            session: session.stamp().handle(),
            scope: session.scope(),
            lifecycle,
            checkpoint,
            capacity,
        };
        value.validate(session)?;
        Ok(value)
    }

    /// Rechecks static binding; the provider also matches its live record.
    ///
    /// # Errors
    ///
    /// Rejects Session/scope replacement, unsupported observation or bounds.
    pub fn validate(
        &self,
        session: &InteractionSessionDescriptor,
    ) -> Result<(), BufferChangeError> {
        if !self.session.matches(session.stamp()) {
            return Err(BufferChangeError::StaleSubscription);
        }
        if !self.scope.same_scope(session.scope())
            || !self.scope.contains(self.checkpoint)
        {
            return Err(BufferChangeError::ScopeMismatch);
        }
        if session.capabilities().updates()
            != InteractionCapabilitySupport::Supported
        {
            return Err(BufferChangeError::Unsupported);
        }
        let bounds = session.bounds().updates();
        bounds
            .retained_updates()
            .admit_per_object(BoundValue::Count(self.capacity))
            .map_err(BufferChangeError::Bound)?;
        bounds
            .retained_bytes()
            .admit_per_object(BoundValue::Bytes(self.reserved_bytes()?))
            .map_err(BufferChangeError::Bound)?;
        bounds
            .require_control_reserve()
            .map_err(BufferChangeError::FlowBound)?;
        Ok(())
    }

    /// Returns native retained-record storage, excluding transport encoding.
    ///
    /// Runtime separately charges subscription/control state and global totals.
    ///
    /// # Errors
    ///
    /// Rejects zero capacity or checked storage multiplication overflow.
    pub fn reserved_bytes(
        &self,
    ) -> Result<CheckedByteCount, BufferChangeError> {
        let size =
            u64::try_from(core::mem::size_of::<BufferChangeNotification>())
                .map_err(|_| BufferChangeError::SizeOverflow)?;
        let bytes = size
            .checked_mul(self.capacity.value())
            .ok_or(BufferChangeError::SizeOverflow)?;
        CheckedByteCount::new(bytes, ZeroPolicy::Reject)
            .map_err(BufferChangeError::Bound)
    }
}
