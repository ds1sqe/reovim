use reovim_kernel_interface::bound::{
    BoundValue, CheckedByteCount, CheckedCount, ZeroPolicy,
};
use reovim_kernel_interface::buffer::BufferVersion;
use reovim_kernel_interface::interaction::notification::{
    BufferChangeDelivery, BufferChangeError, BufferChangeNotification,
    BufferChangePort, BufferChangeSubscription,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};

use super::{Phase, Session};

impl BufferChangePort for Session {
    fn subscribe_changes(
        &mut self,
        capacity: CheckedCount,
    ) -> Result<BufferChangeSubscription, BufferChangeError> {
        if !matches!(self.phase, Phase::Negotiated | Phase::Attached) {
            return Err(BufferChangeError::Denied);
        }
        if let Some(existing) = &self.subscription
            && !self.subscription_closed
        {
            return (existing.capacity == capacity)
                .then(|| existing.clone())
                .ok_or(BufferChangeError::StaleSubscription);
        }
        let previous = self
            .subscription
            .as_ref()
            .map_or(0, |s| s.lifecycle.generation().value());
        let generation =
            previous.checked_add(1).ok_or(BufferChangeError::SizeOverflow)?;
        let owner = self.binding.session.stamp().lifecycle();
        let lifecycle = LifecycleIdentity::new(
            owner.instance(),
            owner.incarnation(),
            Generation::new(generation),
        );
        let checkpoint = self.current.ok_or(BufferChangeError::Denied)?;
        let subscription = BufferChangeSubscription::new(
            &self.binding.session,
            lifecycle,
            checkpoint,
            capacity,
        )?;
        self.reserve_subscription(&subscription)?;
        self.notices.clear();
        self.notice_gap = None;
        self.notice_frontier = Some(checkpoint);
        self.subscription_closed = false;
        self.subscription = Some(subscription.clone());
        Ok(subscription)
    }

    fn take_change(
        &mut self,
        subscription: &BufferChangeSubscription,
        after: BufferVersion,
    ) -> Result<BufferChangeDelivery, BufferChangeError> {
        self.check_subscription(subscription)?;
        if self.subscription_closed || self.ending() {
            return Ok(BufferChangeDelivery::closed(subscription, after));
        }
        let current = self.current.ok_or(BufferChangeError::Denied)?;
        if self.notice_frontier.is_none_or(|value| value.version() != after)
            || after.value() < subscription.checkpoint.version().value()
            || after.value() > current.version().value()
        {
            return Err(BufferChangeError::CheckpointMismatch);
        }
        if let Some(checkpoint) = self.notice_gap {
            return BufferChangeDelivery::gap(subscription, after, checkpoint);
        }
        let Some(notice) = self.notices.first() else {
            return Ok(BufferChangeDelivery::pending(subscription, after));
        };
        if self.notice_frontier != Some(notice.before()) {
            return Err(BufferChangeError::CheckpointMismatch);
        }
        let delivery =
            BufferChangeDelivery::changed(subscription, after, notice.clone())?;
        self.notice_frontier = Some(notice.accepted().result());
        self.notices.remove(0);
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
        self.notices.clear();
        self.notice_gap = None;
        Ok(())
    }
}

impl Session {
    fn check_subscription(
        &self,
        subscription: &BufferChangeSubscription,
    ) -> Result<(), BufferChangeError> {
        subscription.validate(&self.binding.session)?;
        if self.subscription.as_ref() != Some(subscription) {
            return Err(BufferChangeError::StaleSubscription);
        }
        Ok(())
    }

    fn reserve_subscription(
        &mut self,
        subscription: &BufferChangeSubscription,
    ) -> Result<(), BufferChangeError> {
        let updates = self.binding.session.bounds().updates();
        let count = CheckedCount::new(
            self.updates.len() as u64 + u64::from(self.initial.is_some()),
            ZeroPolicy::Allow,
        )
        .map_err(BufferChangeError::Bound)?;
        updates
            .retained_updates()
            .admit_aggregate(
                BoundValue::Count(count),
                BoundValue::Count(subscription.capacity),
            )
            .map_err(BufferChangeError::Bound)?;
        let bytes = subscription.reserved_bytes()?.value();
        let data = self
            .updates
            .iter()
            .flatten()
            .filter_map(|value| {
                value.carrier().map(|carrier| carrier.shape().bytes().value())
            })
            .sum::<u64>();
        let data = data
            + self
                .initial
                .as_ref()
                .and_then(|update| update.carrier())
                .map_or(0, |value| value.shape().bytes().value());
        let data = CheckedByteCount::new(data, ZeroPolicy::Allow)
            .map_err(BufferChangeError::Bound)?;
        updates
            .retained_bytes()
            .admit_aggregate(
                BoundValue::Bytes(data),
                BoundValue::Bytes(
                    CheckedByteCount::new(bytes, ZeroPolicy::Reject)
                        .map_err(BufferChangeError::Bound)?,
                ),
            )
            .map_err(BufferChangeError::Bound)?;
        let capacity = usize::try_from(subscription.capacity.value())
            .map_err(|_| BufferChangeError::SizeOverflow)?;
        self.notices
            .try_reserve_exact(capacity.saturating_sub(self.notices.len()))
            .map_err(|_| BufferChangeError::SizeOverflow)
    }

    pub(crate) fn publish_change(&mut self, notice: &BufferChangeNotification) {
        let Some(subscription) = &self.subscription else {
            return;
        };
        if self.subscription_closed || self.ending() {
            return;
        }
        if self.notice_gap.is_some()
            || self.notices.len() as u64 >= subscription.capacity.value()
        {
            self.notices.clear();
            self.notice_gap = Some(notice.accepted().result());
            return;
        }
        self.notices.push(notice.clone());
    }
}
