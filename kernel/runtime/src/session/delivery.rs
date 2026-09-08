use reovim_kernel_interface::interaction::delivery::{intent, update};
use reovim_kernel_interface::interaction::update::{
    RepresentationGapCause, RepresentationUpdate,
    RepresentationUpdateIdentityStamp, RepresentationUpdateLineage,
};
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use super::{CreditClass, IssuedUpdate, Phase, RequestBody, Session};

impl Session {
    fn update_acknowledged(
        &self,
        identity: RepresentationUpdateIdentityStamp,
    ) -> bool {
        self.acknowledged == Some(identity)
            || self
                .gap
                .as_ref()
                .is_some_and(|gap| gap.identity == identity && gap.acknowledged)
    }

    pub(super) fn deliver_outcome(
        &mut self,
        request: &intent::InteractionOutcomeRequest,
    ) -> intent::InteractionOutcomeDelivery {
        use intent::{
            InteractionOutcomeDelivery as Delivery,
            InteractionOutcomeUnavailable as Missing,
        };
        if !request.session().matches(self.binding.session.stamp()) {
            return Delivery::unavailable(Missing::Replaced);
        }
        let Some(record) = self
            .requests
            .iter_mut()
            .find(|record| record.matches_edit(&request.request()))
        else {
            return Delivery::unavailable(Missing::UnknownRequest);
        };
        if record.delivered {
            return Delivery::unavailable(Missing::AlreadyDelivered);
        }
        let RequestBody::Edit { outcome, .. } = &record.body else {
            unreachable!("exact edit lookup only selects an edit");
        };
        let Some(outcome) = outcome.first().copied() else {
            return Delivery::unavailable(Missing::Pending);
        };
        record.delivered = true;
        Delivery::delivered(outcome)
    }

    pub(super) fn deliver_update(
        &mut self,
        request: update::RepresentationUpdateRequest,
    ) -> update::RepresentationUpdateDelivery {
        use update::{
            RepresentationUpdateDelivery as Delivery,
            RepresentationUpdateUnavailable as Missing,
        };
        if request.stream() != self.binding.attachment.stream().handle() {
            return Delivery::unavailable(Missing::Replaced);
        }
        if self.phase == Phase::Closed {
            return Delivery::unavailable(Missing::Closed);
        }
        self.replay_gap(request.expected());
        let Some(candidate) = self.delivery_candidate() else {
            return Delivery::unavailable(Missing::Pending);
        };
        let identity = candidate.stamp().identity();
        let class = if candidate.carrier().is_some() {
            CreditClass::Data
        } else {
            CreditClass::Control
        };
        let gap = matches!(
            candidate.lineage(),
            RepresentationUpdateLineage::Gap { .. }
        );
        if request.expected().value() < identity.cursor().value() && !gap {
            return Delivery::unavailable(Missing::CursorBehind);
        }
        if request.expected().value() > identity.cursor().value() {
            return Delivery::unavailable(Missing::CursorAhead);
        }
        if !self.credit_available(class, identity) {
            return Delivery::unavailable(Missing::NoCredit);
        }
        let slot =
            self.delivery_slot().expect("inspected occupied delivery slot");
        let delivery = match Delivery::try_bind(&request, slot) {
            Ok(delivery) => delivery,
            Err(cause) => {
                return Delivery::unavailable(
                    Missing::ProviderContractViolation(cause),
                );
            }
        };
        self.transferred(delivery.update().expect("bound update retained"));
        self.mark_gap_transferred(
            delivery.update().expect("bound update retained"),
        );
        self.recovery_transferred(
            delivery.update().expect("bound update retained"),
        );
        if self.updates.first().is_some_and(Option::is_none) {
            self.updates.remove(0);
        }
        if !self.update_acknowledged(identity)
            && !self.issued.iter().any(|issued| issued.stamp == identity)
        {
            self.issued.push(IssuedUpdate { stamp: identity, class });
        }
        delivery
    }

    fn delivery_candidate(&self) -> Option<&RepresentationUpdate> {
        self.recovery
            .as_ref()
            .and_then(|work| work.completion.as_ref())
            .or(self.control.as_ref())
            .or_else(|| self.updates.first().and_then(Option::as_ref))
    }

    fn delivery_slot(&mut self) -> Option<&mut Option<RepresentationUpdate>> {
        if self.recovery.as_ref().is_some_and(|work| work.completion.is_some())
        {
            return Some(
                &mut self
                    .recovery
                    .as_mut()
                    .expect("retained completion")
                    .completion,
            );
        }
        if self.control.is_some() {
            return Some(&mut self.control);
        }
        self.updates.first_mut()
    }

    fn credit_available(
        &self,
        class: CreditClass,
        identity: RepresentationUpdateIdentityStamp,
    ) -> bool {
        if self.update_acknowledged(identity)
            || self.issued.iter().any(|issued| issued.stamp == identity)
        {
            return true;
        }
        let limit = match class {
            CreditClass::Data => {
                self.binding.attachment.initial_credit().value()
            }
            CreditClass::Control => self
                .binding
                .session
                .bounds()
                .updates()
                .control()
                .aggregate()
                .value(),
        };
        (self.issued.iter().filter(|issued| issued.class == class).count()
            as u64)
            < limit
    }

    pub(super) const fn transferred(&mut self, update: &RepresentationUpdate) {
        self.next_delivery_cursor =
            update.stamp().cursor().value().checked_add(1);
        if let Some(result) = update.lineage().result() {
            let prior = self.transferred_projection;
            self.transferred_projection = ProjectionStamp::new(
                prior.projection(),
                prior.lifecycle(),
                prior.representation(),
                result,
            );
        }
    }

    pub(super) fn acknowledge(
        &mut self,
        request: update::RepresentationUpdateAcknowledgement,
    ) -> update::RepresentationUpdateAcknowledgementResult {
        use update::RepresentationUpdateAcknowledgementResult as Result;
        if request.stamp().stream() != self.binding.attachment.stream().handle()
        {
            return Result::StaleStream;
        }
        if self.update_acknowledged(request.stamp()) {
            return Result::AlreadyRecorded;
        }
        let Some(index) = self
            .issued
            .iter()
            .position(|issued| issued.stamp == request.stamp())
        else {
            return Result::UnknownUpdate;
        };
        let issued = self.issued.remove(index);
        self.acknowledged = Some(request.stamp());
        if let Some(gap) = &mut self.gap {
            gap.acknowledged |= gap.identity == request.stamp();
        }
        if issued.class == CreditClass::Data
            && matches!(
                request.application(),
                update::RepresentationUpdateApplication::MissingPredecessor
            )
            && !self.ending()
        {
            let _ = self.queue_gap(RepresentationGapCause::MissingPredecessor);
        }
        Result::Accepted
    }
}
