use reovim_kernel_interface::bound::{
    BoundDescriptor, BoundValue, CheckedByteCount, ZeroPolicy,
};
use reovim_kernel_interface::cancellation::{CancellationState, DeadlineState};
use reovim_kernel_interface::interaction::bounds::{
    InteractionRequestBounds, InteractionSessionBounds,
    RepresentationResynchronizationBounds, RepresentationUpdateBounds,
    RepresentationUpdateFlowBounds, RepresentationUpdateShapeBounds,
};
use reovim_kernel_interface::interaction::{
    descriptor::InteractionSessionDescriptor,
    intent::{InteractionIntent, InteractionIntentBindingOutcome},
};
use reovim_kernel_interface::semantic::identity::{
    SemanticIntentIdentity, SemanticRequestIdentity,
};
use reovim_kernel_interface::semantic::intent::SemanticIntentBindingOutcome;
use reovim_kernel_interface::semantic::stamp::{
    SemanticCallerStamp, SemanticIntentStamp,
};

use super::Session;
use crate::first_edit::{Family, native::support as native};
use crate::interaction::support::{self, InteractionFixture};

pub fn reserve_history(fixture: &mut InteractionFixture) {
    let old = &fixture.session;
    let owner = old.bounds().charge_owner();
    let requests = old.bounds().requests();
    // Named in-process storage reservations, not wire-payload expectations.
    let bytes = BoundDescriptor::new(
        BoundValue::Bytes(
            CheckedByteCount::new(2048, ZeroPolicy::Reject).unwrap(),
        ),
        BoundValue::Bytes(
            CheckedByteCount::new(8192, ZeroPolicy::Reject).unwrap(),
        ),
        ZeroPolicy::Reject,
        owner,
    )
    .unwrap();
    let requests = InteractionRequestBounds::new(
        owner,
        requests.pending_intents(),
        bytes,
        requests.retained_outcomes(),
        bytes,
    )
    .expect("edit/history request and terminal reservation");
    let updates = old.bounds().updates();
    let retained_bytes = BoundDescriptor::new(
        BoundValue::Bytes(native::bytes(2048)),
        BoundValue::Bytes(native::bytes(8192)),
        ZeroPolicy::Allow,
        owner,
    )
    .unwrap();
    let flow = RepresentationUpdateFlowBounds::new(
        updates.retained_updates(),
        retained_bytes,
        updates.credits(),
        updates.control(),
        RepresentationResynchronizationBounds::new(
            updates.resynchronizations(),
            updates.resynchronization_work(),
            updates.resynchronization_bytes(),
        ),
    );
    let shape = RepresentationUpdateShapeBounds::new(
        updates.items(),
        updates.depth(),
        updates.bytes(),
    );
    let updates = RepresentationUpdateBounds::new(owner, shape, flow).unwrap();
    let bounds = InteractionSessionBounds::new(
        owner,
        requests,
        updates,
        old.bounds().close_work(),
    )
    .unwrap();
    fixture.session = InteractionSessionDescriptor::new(
        old.stamp(),
        old.scope(),
        old.capabilities(),
        bounds,
        old.update_contract().clone(),
        old.operations(),
    )
    .unwrap();
}

impl<Model: Family> Session<Model> {
    pub fn intent(&self, request: u128, payload: &[u8]) -> InteractionIntent {
        self.intent_at(request, payload, &self.current_stamp())
    }

    pub fn intent_at(
        &self,
        request: u128,
        payload: &[u8],
        target: &reovim_kernel_interface::buffer::BufferStateStamp,
    ) -> InteractionIntent {
        let base = crate::semantic::support::intent_stamp(
            &self.fixture.semantic,
            request + 1,
            request + 2,
        )
        .expect("selected semantics");
        let caller = SemanticCallerStamp::new(
            self.fixture.session.stamp().lifecycle(),
            self.fixture.session.stamp().client().role(),
        );
        let stamp = SemanticIntentStamp::new(
            SemanticIntentIdentity::new(request + 1),
            caller,
            SemanticRequestIdentity::new(request + 2),
            *target,
            native::plan_stamp(request + 3),
            *base.projection(),
            base.coordinate(),
        );
        let descriptor = &self.fixture.semantic.representation;
        let prepared = stamp
            .prepare_intent(
                descriptor,
                descriptor.intent().selected_stamp(),
                1,
                0,
                u64::try_from(payload.len()).unwrap(),
            )
            .unwrap();
        let SemanticIntentBindingOutcome::Bound(semantic) =
            prepared.bind(descriptor, payload.into())
        else {
            panic!("semantic binding")
        };
        let stamp =
            support::request_stamp(&self.fixture, &semantic, request).unwrap();
        let charge = self
            .fixture
            .session
            .bounds()
            .requests()
            .prepare_intent(semantic.shape().bytes().value())
            .unwrap();
        let InteractionIntentBindingOutcome::Bound(intent) =
            InteractionIntent::bind(
                &self.fixture.session,
                &self.fixture.attachment,
                stamp,
                charge,
                semantic,
                CancellationState::Open,
                DeadlineState::Pending,
            )
        else {
            panic!("Interaction binding")
        };
        intent
    }
}
