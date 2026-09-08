use reovim_kernel_interface::bound::{BoundError, BoundUnit, CheckedCount};
use reovim_kernel_interface::buffer::BufferVersion;
use reovim_kernel_interface::interaction::notification::{
    BufferChangeDelivery, BufferChangeError, BufferChangeNotification,
    BufferChangePort, BufferChangeSubscription,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};

use super::Session;
use crate::first_edit::Family;

impl<Model: Family> BufferChangePort for Session<Model>
where
    Model::State: Clone,
{
    fn subscribe_changes(
        &mut self,
        capacity: CheckedCount,
    ) -> Result<BufferChangeSubscription, BufferChangeError> {
        if self.closed {
            return Err(BufferChangeError::Closed);
        }
        if !self.ready() || !self.is_negotiated() {
            return Err(BufferChangeError::Denied);
        }
        if let Some(retained) = &self.subscription
            && !self.subscription_closed
            && retained.capacity == capacity
        {
            return Ok(retained.clone());
        }
        let generation = match &self.subscription {
            None => 1,
            Some(old) => {
                let previous = old.lifecycle.generation().value();
                previous.checked_add(1).ok_or(BufferChangeError::Bound(
                    BoundError::ArithmeticOverflow {
                        unit: BoundUnit::Count,
                        left: previous,
                        right: 1,
                    },
                ))?
            }
        };
        let id = self.fixture.session.stamp().client().client().value();
        let subscription = BufferChangeSubscription::new(
            &self.fixture.session,
            LifecycleIdentity::new(
                id + 40,
                id + 41,
                Generation::new(generation),
            ),
            self.current_stamp(),
            capacity,
        )?;
        self.subscription = Some(subscription.clone());
        self.subscription_closed = false;
        self.notice = None;
        self.notice_gap = None;
        Ok(subscription)
    }

    fn take_change(
        &mut self,
        subscription: &BufferChangeSubscription,
        after: BufferVersion,
    ) -> Result<BufferChangeDelivery, BufferChangeError> {
        self.check_subscription(subscription)?;
        if self.subscription_closed {
            return Ok(BufferChangeDelivery::closed(subscription, after));
        }
        if after.value() < subscription.checkpoint.version().value()
            || after.value() > self.current_stamp().version().value()
        {
            return Err(BufferChangeError::CheckpointMismatch);
        }
        if let Some(checkpoint) = self.notice_gap {
            return BufferChangeDelivery::gap(subscription, after, checkpoint);
        }
        let Some(notice) = self.notice.as_ref() else {
            return Ok(BufferChangeDelivery::pending(subscription, after));
        };
        let delivery =
            BufferChangeDelivery::changed(subscription, after, notice.clone())?;
        self.notice = None;
        Ok(delivery)
    }

    fn resynchronize_changes(
        &mut self,
        subscription: &BufferChangeSubscription,
    ) -> Result<BufferChangeSubscription, BufferChangeError> {
        self.check_subscription(subscription)?;
        if self.subscription_closed {
            return Err(BufferChangeError::Closed);
        }
        self.subscription_closed = true;
        self.subscribe_changes(subscription.capacity)
    }

    fn unsubscribe_changes(
        &mut self,
        subscription: &BufferChangeSubscription,
    ) -> Result<(), BufferChangeError> {
        self.check_subscription(subscription)?;
        self.subscription_closed = true;
        self.notice = None;
        self.notice_gap = None;
        Ok(())
    }
}

impl<Model: Family> Session<Model> {
    fn check_subscription(
        &self,
        subscription: &BufferChangeSubscription,
    ) -> Result<(), BufferChangeError> {
        subscription.validate(&self.fixture.session)?;
        if self.closed {
            return Err(BufferChangeError::Closed);
        }
        if self.subscription.as_ref() != Some(subscription) {
            return Err(BufferChangeError::StaleSubscription);
        }
        Ok(())
    }

    /// Scripted authority fan-out; each endpoint has its own one-record slot.
    pub const fn publish_change(&mut self, notice: BufferChangeNotification) {
        if self.closed
            || self.subscription_closed
            || self.subscription.is_none()
        {
            return;
        }
        if self.notice.is_some() || self.notice_gap.is_some() {
            self.notice_gap = Some(notice.accepted().result());
            self.notice = None;
        } else {
            self.notice = Some(notice);
        }
    }
}
