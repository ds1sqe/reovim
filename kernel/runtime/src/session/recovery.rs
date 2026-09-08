use reovim_kernel_interface::event::editing::EditingStage;
use reovim_kernel_interface::interaction::delivery::update::{
    RepresentationResynchronizationAdmission as Admission,
    RepresentationResynchronizationFailure as Failure,
    RepresentationResynchronizationRequest as Request,
};
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeCause;
use reovim_kernel_interface::interaction::update::{
    RepresentationGapCause, RepresentationUpdate,
    RepresentationUpdateIdentityStamp, RepresentationUpdateLineage,
};
use reovim_kernel_interface::interaction::{bounds, descriptor, identity};
use reovim_kernel_interface::semantic::identity::ProjectionGeneration;

use super::{CreditClass, Phase, Session};

mod gap;

pub struct RecoveryGap {
    pub identity: RepresentationUpdateIdentityStamp,
    pub state: reovim_kernel_interface::buffer::BufferStateStamp,
    pub projection: reovim_kernel_interface::semantic::stamp::ProjectionStamp,
    pub token: identity::RepresentationResynchronizationIdentity,
    pub expected: ProjectionGeneration,
    pub available: ProjectionGeneration,
    pub cause:
        reovim_kernel_interface::interaction::update::RepresentationGapCause,
    pub delivered: bool,
    pub acknowledged: bool,
}

pub struct RecoveryWork {
    pub request: Request,
    pub completion: Option<RepresentationUpdate>,
    pub totals: bounds::RepresentationResynchronizationTotals,
}

impl Session {
    pub(super) fn request_recovery(&mut self, request: Request) -> Admission {
        if let Some(cause) = self.recovery_refusal(&request) {
            return Admission::NotAdmitted { cause, request };
        }
        let totals = self
            .binding
            .session
            .bounds()
            .updates()
            .admit_resynchronization(0, 0, 0, request.charge())
            .expect("unchanged checked recovery reservation");
        // A delivered Gap authorizes abandoning its obsolete data prefix,
        // never another control credit or an editing terminal outcome.
        self.issued.retain(|issued| {
            issued.class == CreditClass::Control
                || issued.stamp.cursor().value() >= request.cursor().value()
        });
        let admitted = Admission::admitted(&request);
        self.recovery =
            Some(RecoveryWork { request, completion: None, totals });
        admitted
    }

    fn recovery_refusal(&self, request: &Request) -> Option<Failure> {
        if self.phase == Phase::Closed {
            return Some(Failure::Closed);
        }
        if self.phase == Phase::Closing {
            return Some(Failure::Closing);
        }
        if self.phase != Phase::Attached {
            return Some(Failure::Denied);
        }
        if self.binding.session.capabilities().resynchronization()
            != descriptor::InteractionCapabilitySupport::Supported
        {
            return Some(Failure::Unsupported);
        }
        if request.stream() != self.binding.attachment.stream().handle() {
            return Some(Failure::Replaced);
        }
        let bounds = self.binding.session.bounds().updates();
        if bounds.validate_resynchronization_charge(request.charge()).is_err()
            || request.cursor().checked_next().is_none()
        {
            return Some(Failure::Pressure);
        }
        if let Some(work) = &self.recovery {
            return Some(duplicate_failure(work, request));
        }
        let Some(gap) = &self.gap else {
            return Some(Failure::UnknownGap);
        };
        if !gap.delivered
            || gap.identity.stream() != request.stream()
            || gap.identity.update() != request.gap()
            || gap.identity.cursor() != request.cursor()
            || gap.token != request.resynchronization()
        {
            return Some(Failure::UnknownGap);
        }
        bounds
            .admit_resynchronization(0, 0, 0, request.charge())
            .err()
            .map(|_| Failure::Pressure)
    }

    pub(crate) fn recovery_pending(&self) -> bool {
        self.recovery.as_ref().is_some_and(|work| work.completion.is_none())
    }

    pub(crate) fn recovery_failed(&mut self, cause: Failure) {
        let receipt = self
            .recovery
            .as_ref()
            .expect("admitted recovery remains owned")
            .request
            .receipt();
        let cursor = receipt
            .cursor()
            .checked_next()
            .expect("admission reserves a representable terminal cursor");
        let projection = self.transferred_projection;
        let stamp = crate::snapshot::stamp(self, cursor.value(), &projection)
            .expect("admitted recovery retains its Session scope");
        let terminal = RepresentationUpdate::resynchronization_failed(
            &self.binding.session,
            &self.binding.attachment,
            stamp,
            receipt.resynchronization(),
            cause,
        )
        .expect("selected recovery failure control");
        receipt
            .validate_result(&terminal)
            .expect("exact owned recovery terminal");
        let mut context = self.update_context;
        context.observer = Some(self.binding.session.stamp().handle());
        context.update = Some(terminal.stamp().identity());
        context.cause = Some(failure_cause(cause));
        let _ = self.telemetry.emit(EditingStage::UpdateFailed, &context);
        self.note_ready(&terminal);
        self.recovery.as_mut().expect("retained owner").completion =
            Some(terminal);
        self.cursor = cursor.value();
        self.invalidated = true;
    }

    pub(super) fn recovery_transferred(
        &mut self,
        update: &RepresentationUpdate,
    ) {
        let Some(work) = &self.recovery else {
            return;
        };
        if work.request.validate_result(update).is_err() {
            return;
        }
        let token = work.request.resynchronization();
        self.recovery = None;
        if self.gap.as_ref().is_some_and(|gap| gap.token == token) {
            self.gap = None;
        }
        if matches!(
            update.lineage(),
            RepresentationUpdateLineage::ResynchronizationFailed { .. }
        ) && !self.ending()
        {
            let _ = self.queue_gap(RepresentationGapCause::MissingPredecessor);
        }
    }
}

const fn failure_cause(cause: Failure) -> InteractionOutcomeCause {
    match cause {
        Failure::Closing => InteractionOutcomeCause::Closing,
        Failure::Replaced => InteractionOutcomeCause::Replaced,
        Failure::Pressure => InteractionOutcomeCause::Pressure,
        _ => InteractionOutcomeCause::DeliveryFailure,
    }
}

fn duplicate_failure(work: &RecoveryWork, request: &Request) -> Failure {
    if work.request.receipt() == request.receipt() {
        Failure::DuplicateRequest
    } else {
        Failure::Pressure
    }
}
