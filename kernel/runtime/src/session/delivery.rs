use reovim_kernel_interface::interaction::delivery::{intent, update};
use reovim_kernel_interface::interaction::update::RepresentationUpdate;
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use super::{Phase, RequestBody, Session};

impl Session {
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
        if self.issued.len() as u64
            >= self.binding.attachment.initial_credit().value()
        {
            return Delivery::unavailable(Missing::NoCredit);
        }
        let slot = if self.control.is_some() {
            &mut self.control
        } else if let Some(first) = self.updates.first_mut() {
            first
        } else {
            return Delivery::unavailable(Missing::Pending);
        };
        let candidate = slot.as_ref().expect("queue retains occupied slots");
        let identity = candidate.stamp().identity();
        if request.expected().value() < identity.cursor().value() {
            return Delivery::unavailable(Missing::CursorBehind);
        }
        if request.expected().value() > identity.cursor().value() {
            return Delivery::unavailable(Missing::CursorAhead);
        }
        let delivery = match Delivery::try_bind(&request, slot) {
            Ok(delivery) => delivery,
            Err(cause) => {
                return Delivery::unavailable(
                    Missing::ProviderContractViolation(cause),
                );
            }
        };
        self.transferred(delivery.update().expect("bound update retained"));
        if self.updates.first().is_some_and(Option::is_none) {
            self.updates.remove(0);
        }
        self.issued.push(identity);
        delivery
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
        if self.acknowledged == Some(request.stamp()) {
            return Result::AlreadyRecorded;
        }
        let Some(index) =
            self.issued.iter().position(|stamp| *stamp == request.stamp())
        else {
            return Result::UnknownUpdate;
        };
        self.issued.remove(index);
        self.acknowledged = Some(request.stamp());
        Result::Accepted
    }
}
