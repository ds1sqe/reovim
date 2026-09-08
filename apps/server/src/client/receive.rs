use reovim_kernel_interface::buffer::plan::BufferCorrelationIdentity;
use reovim_kernel_interface::buffer::{
    BufferHistoryDirection, BufferStateStamp,
};
use reovim_kernel_interface::interaction::history::InteractionHistoryOutcome;
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::intent::InteractionIntent;
use reovim_kernel_interface::interaction::notification::{
    BufferChangeNotification, BufferChangeSubscription,
};
use reovim_kernel_interface::interaction::outcome::{
    InteractionAcceptedState, InteractionOutcome,
};
use reovim_kernel_interface::interaction::session::{
    InteractionRequestStamp, InteractionSessionHandle,
};
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateIdentityStamp,
};

use super::{Client, ClientError};

/// Exact pre-submission evidence for pairing a returned edit outcome.
pub struct EditExpectation {
    request: InteractionRequestStamp,
    before: BufferStateStamp,
}

#[derive(Clone, PartialEq, Eq)]
struct Change {
    origin: InteractionSessionHandle,
    request: InteractionRequestIdentity,
    correlation: BufferCorrelationIdentity,
    before: BufferStateStamp,
    accepted: InteractionAcceptedState,
}

#[derive(Default)]
pub(super) struct Pending {
    change: Option<Change>,
    update: Option<RepresentationUpdate>,
    last: Option<Change>,
}

impl Client {
    /// Captures exact request evidence before ownership passes to the service.
    ///
    /// # Errors
    /// Rejects another Session, attachment or observed state.
    pub fn expect_edit(
        &self,
        intent: &InteractionIntent,
    ) -> Result<EditExpectation, ClientError> {
        if !intent.stamp().session().matches(self.session.stamp())
            || intent.stamp().attachment() != self.attachment.handle()
            || self.state != Some(*intent.semantic().stamp().target())
        {
            return Err(ClientError::Binding);
        }
        Ok(EditExpectation {
            request: intent.stamp(),
            before: *intent.semantic().stamp().target(),
        })
    }

    /// Pairs a real edit outcome with the previously captured request.
    ///
    /// # Errors
    /// Rejects changed request evidence or bounded rendezvous mismatch.
    pub fn accept_edit_outcome(
        &mut self,
        expected: &EditExpectation,
        outcome: &InteractionOutcome,
    ) -> Result<Option<RepresentationUpdateIdentityStamp>, ClientError> {
        if !expected.request.same_request(outcome.request())
            || !expected.request.session().matches(self.session.stamp())
        {
            return Err(ClientError::Binding);
        }
        let Some(accepted) = outcome.accepted_state() else {
            return Ok(None);
        };
        self.pair_change(Change {
            origin: outcome.request().session(),
            request: outcome.request().request(),
            before: expected.before,
            accepted,
            correlation: outcome.request().semantic().correlation(),
        })
    }

    /// Pairs a real History outcome without a fabricated semantic edit.
    ///
    /// # Errors
    /// Rejects another Session or inconsistent accepted state.
    pub fn accept_history_outcome(
        &mut self,
        outcome: &InteractionHistoryOutcome,
    ) -> Result<Option<RepresentationUpdateIdentityStamp>, ClientError> {
        let request = outcome.request();
        if request.validate(&self.session, &self.plans).is_err() {
            return Err(ClientError::Binding);
        }
        let Some(accepted) = outcome.accepted_state() else {
            return Ok(None);
        };
        self.pair_change(Change {
            origin: request.session(),
            request: request.request(),
            before: request.target(),
            accepted,
            correlation: request.plan().correlation(),
        })
    }

    /// Accepts a real scoped change independently of snapshot delivery order.
    ///
    /// # Errors
    /// Rejects stale scope/frontier or a second different pending change.
    pub fn accept_change(
        &mut self,
        notice: &BufferChangeNotification,
    ) -> Result<Option<RepresentationUpdateIdentityStamp>, ClientError> {
        if !self.session.scope().contains(notice.before())
            || !self.session.scope().contains(notice.accepted().result())
        {
            return Err(ClientError::Binding);
        }
        if self
            .subscription_frontier
            .is_some_and(|frontier| frontier != notice.before())
        {
            return Err(ClientError::Stale);
        }
        let result = self.pair_change(Change {
            origin: notice.origin(),
            request: notice.request(),
            before: notice.before(),
            accepted: notice.accepted(),
            correlation: notice.correlation(),
        })?;
        if self.subscription.is_some() {
            self.subscription_frontier = Some(notice.accepted().result());
        }
        Ok(result)
    }

    /// Retains one validated update until its actual acceptance is known.
    ///
    /// # Errors
    /// Rejects wrong schema, cursor, predecessor, scope or occupied capacity.
    pub fn accept_update(
        &mut self,
        update: RepresentationUpdate,
    ) -> Result<Option<RepresentationUpdateIdentityStamp>, ClientError> {
        self.validate_incoming(&update)?;
        if self.take_control(&update)? {
            self.retain_gap(update);
            return Err(ClientError::Invalidated);
        }
        if self.pending.update.is_some() {
            return Err(ClientError::Pressure);
        }
        if let Some(change) = &self.pending.change {
            self.match_update(change, &update)?;
        }
        self.pending.update = Some(update);
        self.install_pair()
    }

    /// Binds an explicitly requested subscription at this exact observation.
    ///
    /// # Errors
    /// Rejects a different scope, current checkpoint or occupied rendezvous.
    pub fn bind_subscription(
        &mut self,
        subscription: BufferChangeSubscription,
    ) -> Result<(), ClientError> {
        subscription
            .validate(&self.session)
            .map_err(|_| ClientError::Binding)?;
        if Some(subscription.checkpoint) != self.state {
            return Err(ClientError::Stale);
        }
        if self.pending.change.is_some() || self.pending.update.is_some() {
            return Err(ClientError::Pressure);
        }
        self.subscription_frontier = Some(subscription.checkpoint);
        self.subscription = Some(subscription);
        Ok(())
    }

    /// Returns the opt-in observer binding, separate from update credit.
    #[must_use]
    pub const fn subscription(&self) -> Option<&BufferChangeSubscription> {
        self.subscription.as_ref()
    }

    pub(crate) const fn change_pending(&self) -> bool {
        self.pending.change.is_some()
    }

    pub(crate) const fn change_frontier(&self) -> Option<BufferStateStamp> {
        self.subscription_frontier
    }

    pub(crate) const fn update_pending(&self) -> bool {
        self.pending.update.is_some()
    }

    pub(super) fn clear_pairs(&mut self) {
        self.pending = Pending::default();
    }

    fn pair_change(
        &mut self,
        change: Change,
    ) -> Result<Option<RepresentationUpdateIdentityStamp>, ClientError> {
        if self.pending.last.as_ref() == Some(&change) {
            return Ok(None);
        }
        self.check_change(&change)?;
        if self.pending.change.as_ref().is_some_and(|prior| prior != &change) {
            return Err(ClientError::Pressure);
        }
        if let Some(update) = &self.pending.update {
            self.match_update(&change, update)?;
        }
        self.pending.change = Some(change);
        self.install_pair()
    }

    fn check_change(&self, change: &Change) -> Result<(), ClientError> {
        let result = change.accepted.result();
        if self.state != Some(change.before)
            || !self.session.scope().contains(result)
            || result.source() != change.before.source()
            || result.history().lifecycle()
                != change.before.history().lifecycle()
            || Some(result.version().value())
                != change.before.version().value().checked_add(1)
        {
            return Err(ClientError::Stale);
        }
        let direction = change
            .accepted
            .history_direction()
            .unwrap_or(BufferHistoryDirection::Redo);
        let edge = change.accepted.history_edge();
        if edge.from(direction) != change.before.history().node()
            || edge.to(direction) != result.history().node()
        {
            return Err(ClientError::Mismatch);
        }
        Ok(())
    }

    fn match_update(
        &self,
        change: &Change,
        update: &RepresentationUpdate,
    ) -> Result<(), ClientError> {
        let target = update
            .stamp()
            .resolve_target(&self.session, &self.attachment)
            .map_err(|_| ClientError::Binding)?;
        if target != change.accepted.result() {
            return Err(ClientError::Mismatch);
        }
        Ok(())
    }

    fn install_pair(
        &mut self,
    ) -> Result<Option<RepresentationUpdateIdentityStamp>, ClientError> {
        let (Some(change), Some(update)) =
            (&self.pending.change, &self.pending.update)
        else {
            return Ok(None);
        };
        self.match_update(change, update)?;
        let prior = change.before;
        let projection = self.projection;
        let update = self.pending.update.take().ok_or(ClientError::NotReady)?;
        let identity = self.commit_update(update)?;
        self.follow_selection(&prior, &projection);
        self.pending.last = self.pending.change.take();
        Ok(Some(identity))
    }
}
