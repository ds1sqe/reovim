use crate::buffer::{BufferStateStamp, BufferVersion};
use crate::lifecycle::LifecycleIdentity;

use super::{
    BufferChangeError, BufferChangeNotification, BufferChangeSubscription,
};

/// Non-waiting notification result, not a request-terminal disposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferChangeDeliveryStatus {
    /// One exact next accepted transition is included.
    Changed,
    /// No later accepted transition is currently available.
    Pending,
    /// Missing continuity requires a fresh subscription/snapshot checkpoint.
    Gap,
    /// This subscription cannot produce additional records.
    Closed,
}

/// Subscription-bound delivery, without describing a client's follow policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferChangeDelivery {
    /// Exact subscription incarnation that produced this delivery.
    subscription: LifecycleIdentity,
    /// Last received Buffer version from the lookup.
    after: BufferVersion,
    /// Changed, pending, gap or closed.
    status: BufferChangeDeliveryStatus,
    /// Present only for one delivered accepted transition.
    notification: Option<BufferChangeNotification>,
    /// Accepted frontier for a change, or known current frontier after a gap.
    checkpoint: Option<BufferStateStamp>,
}

impl BufferChangeDelivery {
    /// Returns the exact subscription lifetime that produced this result.
    #[must_use]
    pub const fn subscription(&self) -> LifecycleIdentity {
        self.subscription
    }

    /// Returns the requested last-received Buffer version.
    #[must_use]
    pub const fn after(&self) -> BufferVersion {
        self.after
    }

    /// Returns changed, pending, gap or closed.
    #[must_use]
    pub const fn status(&self) -> BufferChangeDeliveryStatus {
        self.status
    }

    /// Borrows the accepted change only for a Changed result.
    #[must_use]
    pub const fn notification(&self) -> Option<&BufferChangeNotification> {
        self.notification.as_ref()
    }

    /// Returns the change or gap frontier, never reconstructed native data.
    #[must_use]
    pub const fn checkpoint(&self) -> Option<BufferStateStamp> {
        self.checkpoint
    }

    /// Transfers ownership of the received notification when present.
    #[must_use]
    pub const fn into_notification(self) -> Option<BufferChangeNotification> {
        self.notification
    }

    /// Binds a contiguous accepted change to this exact observer stream.
    ///
    /// # Errors
    ///
    /// Rejects another scope, transition incoherence or a skipped frontier.
    pub fn changed(
        subscription: &BufferChangeSubscription,
        after: BufferVersion,
        notification: BufferChangeNotification,
    ) -> Result<Self, BufferChangeError> {
        notification.validate()?;
        check_scope(subscription, &notification.before)?;
        check_scope(subscription, &notification.accepted.result())?;
        if notification.before.version() != after
            || after.value() < subscription.checkpoint.version().value()
        {
            return Err(BufferChangeError::CheckpointMismatch);
        }
        if after == subscription.checkpoint.version()
            && !notification.before.same_state(subscription.checkpoint)
        {
            return Err(BufferChangeError::CheckpointMismatch);
        }
        Ok(Self {
            subscription: subscription.lifecycle,
            after,
            status: BufferChangeDeliveryStatus::Changed,
            checkpoint: Some(notification.accepted.result()),
            notification: Some(notification),
        })
    }

    /// Reports lost continuity without inventing the missing transitions.
    ///
    /// # Errors
    ///
    /// Rejects another scope or a checkpoint older than the requested frontier.
    pub fn gap(
        subscription: &BufferChangeSubscription,
        after: BufferVersion,
        checkpoint: BufferStateStamp,
    ) -> Result<Self, BufferChangeError> {
        check_scope(subscription, &checkpoint)?;
        if checkpoint.version().value() < after.value()
            || after.value() < subscription.checkpoint.version().value()
        {
            return Err(BufferChangeError::CheckpointMismatch);
        }
        Ok(Self {
            subscription: subscription.lifecycle,
            after,
            status: BufferChangeDeliveryStatus::Gap,
            notification: None,
            checkpoint: Some(checkpoint),
        })
    }

    /// Reports no next record; does not advance the observer's frontier.
    #[must_use]
    pub const fn pending(
        subscription: &BufferChangeSubscription,
        after: BufferVersion,
    ) -> Self {
        Self::without_change(
            subscription,
            after,
            BufferChangeDeliveryStatus::Pending,
        )
    }

    /// Reports terminal delivery closure, not global Buffer shutdown.
    #[must_use]
    pub const fn closed(
        subscription: &BufferChangeSubscription,
        after: BufferVersion,
    ) -> Self {
        Self::without_change(
            subscription,
            after,
            BufferChangeDeliveryStatus::Closed,
        )
    }

    const fn without_change(
        subscription: &BufferChangeSubscription,
        after: BufferVersion,
        status: BufferChangeDeliveryStatus,
    ) -> Self {
        Self {
            subscription: subscription.lifecycle,
            after,
            status,
            notification: None,
            checkpoint: None,
        }
    }
}

fn check_scope(
    subscription: &BufferChangeSubscription,
    checkpoint: &BufferStateStamp,
) -> Result<(), BufferChangeError> {
    if !subscription.scope.contains(*checkpoint)
        || subscription.checkpoint.history().lifecycle()
            != checkpoint.history().lifecycle()
    {
        return Err(BufferChangeError::ScopeMismatch);
    }
    Ok(())
}
