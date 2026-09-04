#![forbid(unsafe_code)]
//! Executed public-boundary journey for Kernel Interface contracts.

use reovim_kernel_interface::bound::{
    BoundDescriptor, BoundValue, CheckedByteCount, CheckedCount, ZeroPolicy,
};
use reovim_kernel_interface::buffer::bounds::BufferPlanBounds;
use reovim_kernel_interface::buffer::carrier::{
    BufferCarrierContract, BufferCarrierContracts, BufferCarrierRole,
    BufferPlanCarrier,
};
use reovim_kernel_interface::buffer::plan::{
    BufferCancellationIdentity, BufferCorrelationIdentity, BufferDeadline,
    BufferHistoryTransitionIdentity, BufferPlanBindingOutcome,
    BufferPlanDescriptor, BufferPlanStamp, BufferTransactionIdentity,
};
use reovim_kernel_interface::buffer::{
    BufferHistoryPosition, BufferHistoryStamp, BufferIdentity,
    BufferIncarnationStamp, BufferStateStamp, BufferVersion,
};
use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, InstallationOutcome,
};
use reovim_kernel_interface::carrier::{
    BorrowModel, CarrierSafetyDescriptor, ConcurrencySafety, CopyModel,
    DropModel, InvocationModel, InvocationSafety, LifetimeModel, MemorySafety,
    OwnershipModel, SharingModel, ThreadTransfer, TypeMismatch, UnsafeBoundary,
    UnwindModel,
};
use reovim_kernel_interface::contract::{
    CompatibleVersionRange, ContractId, ContractVersion,
};
use reovim_kernel_interface::identity::InstanceId;
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::operation::ContractSelection;
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceIdentity, ResourceRevision,
    ResourceSnapshotIdentity, ResourceSnapshotStamp,
};
use reovim_kernel_interface::semantic::domain::{
    DomainPolicyPort, DomainValidationCandidate, DomainValidationReceipt,
    DomainValidationRequest,
};
use reovim_kernel_interface::semantic::identity::{
    SemanticIntentIdentity, SemanticRequestIdentity,
};
use reovim_kernel_interface::semantic::intent::SemanticIntentBindingOutcome;
use reovim_kernel_interface::semantic::representation::{
    RepresentationLoweringCandidate, RepresentationLoweringRequest,
    RepresentationPolicyPort,
};
use reovim_kernel_interface::semantic::stamp::SemanticIntentStamp;

#[test]
fn complete_public_contract_journey() {
    assert!(reovim_kernel_interface_public_consumer::exercise_public_api());
}

#[test]
fn buffer_carrier_debug_is_payload_redacted() {
    let descriptor = buffer_descriptor();
    let stamp = descriptor.carriers().origin().selected_stamp();
    let preparation = descriptor
        .prepare_carrier(BufferCarrierRole::Origin, stamp, 1, 6)
        .expect("valid origin preparation");
    let carrier = preparation
        .bind(&descriptor, b"secret".as_slice().into())
        .expect("valid origin allocation");
    let rendered = format!("{carrier:?}");
    assert!(rendered.contains("BufferPlanCarrier"));
    assert!(rendered.contains("Origin"));
    assert!(!rendered.contains("secret"));

    let rejection = descriptor
        .prepare_carrier(BufferCarrierRole::Origin, stamp, 1, 7)
        .expect("valid carrier preparation")
        .bind(&descriptor, b"secret".as_slice().into())
        .expect_err("allocation length must remain exact");
    let rendered = format!("{rejection:?}");
    assert!(rendered.contains("BufferCarrierRejection"));
    assert!(!rendered.contains("secret"));
}

#[test]
fn buffer_plan_outcome_debug_is_payload_redacted() {
    let descriptor = buffer_descriptor();
    let (origin, operations, preconditions) = plan_carriers(&descriptor);
    let accepted = descriptor.bind_plan(
        plan_stamp(1),
        buffer_state(),
        origin,
        operations,
        preconditions,
    );
    let rendered = format!("{accepted:?}");
    assert!(rendered.contains("Accepted"));
    assert!(rendered.contains("BufferTransactionPlan"));
    assert!(!rendered.contains("secret"));
    assert!(!rendered.contains("apply"));

    let (origin, operations, preconditions) = plan_carriers(&descriptor);
    let rejected = descriptor.bind_plan(
        plan_stamp(2),
        buffer_state(),
        origin,
        operations,
        preconditions,
    );
    let rendered = format!("{rejected:?}");
    assert!(rendered.contains("Rejected"));
    assert!(rendered.contains("BufferPlanRejection"));
    assert!(!rendered.contains("secret"));
    assert!(!rendered.contains("apply"));
    assert!(matches!(rejected, BufferPlanBindingOutcome::Rejected(_)));
}

#[test]
fn domain_policy_panic_preserves_runtime_intent() {
    let inputs =
        reovim_kernel_interface_public_consumer::semantic_panic_inputs()
            .expect("semantic panic fixture");
    let provider = PanickingDomain { descriptor: &inputs.domain };
    let request = DomainValidationRequest::new(
        &inputs.domain,
        &inputs.representation,
        &inputs.intent,
        inputs.domain_charge,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .expect("valid Domain request");
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        provider.validate(request)
    }));
    assert!(unwind.is_err());
    assert_eq!(inputs.intent.bytes(), [0x91, 0x02, 0x7f, 0xa4]);
}

#[test]
fn representation_policy_panic_preserves_runtime_intent() {
    let inputs =
        reovim_kernel_interface_public_consumer::semantic_panic_inputs()
            .expect("semantic panic fixture");
    let domain_request = DomainValidationRequest::new(
        &inputs.domain,
        &inputs.representation,
        &inputs.intent,
        inputs.domain_charge,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .expect("valid Domain request");
    let receipt = domain_request
        .finish(
            &inputs.domain,
            domain_request.validated_candidate(),
            InstallationOutcome::Current,
        )
        .receipt()
        .expect("exact Domain receipt");
    let request = RepresentationLoweringRequest::new(
        &inputs.representation,
        &inputs.intent,
        &receipt,
        &inputs.buffer,
        inputs.representation_charge,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .expect("valid Representation request");
    let provider =
        PanickingRepresentation { descriptor: &inputs.representation };
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        provider.lower(request)
    }));
    assert!(unwind.is_err());
    assert_eq!(inputs.intent.bytes(), [0x91, 0x02, 0x7f, 0xa4]);
}

#[test]
fn semantic_owning_debug_is_payload_redacted() {
    let inputs =
        reovim_kernel_interface_public_consumer::semantic_panic_inputs()
            .expect("semantic debug fixture");
    let intent_debug = format!("{:?}", inputs.intent);
    assert!(intent_debug.contains("SemanticIntent"));
    assert!(!intent_debug.contains("[145, 2, 127, 164]"));

    let binding = semantic_binding_rejection(&inputs);
    let binding_debug = format!("{binding:?}");
    assert!(binding_debug.contains("Rejected"));
    assert!(!binding_debug.contains("secret"));
    let SemanticIntentBindingOutcome::Rejected(rejection) = binding else {
        panic!("expected exact semantic binding rejection");
    };
    let rejection_debug = format!("{rejection:?}");
    assert!(rejection_debug.contains("SemanticIntentRejection"));
    assert!(!rejection_debug.contains("secret"));

    let receipt = semantic_receipt(&inputs);
    let request = semantic_lowering_request(&inputs, &receipt);
    let candidate = semantic_plan_candidate(&inputs);
    let candidate_debug = format!("{candidate:?}");
    assert!(candidate_debug.contains("RepresentationLoweringCandidate"));
    assert!(!candidate_debug.contains("secret"));
    assert!(!candidate_debug.contains("apply"));
    let outcome = candidate.finish(
        request,
        &inputs.representation,
        InstallationOutcome::Current,
    );
    let outcome_debug = format!("{outcome:?}");
    assert!(outcome_debug.contains("RepresentationLoweringOutcome"));
    assert!(!outcome_debug.contains("secret"));
    assert!(!outcome_debug.contains("apply"));
}

fn semantic_binding_rejection(
    inputs: &reovim_kernel_interface_public_consumer::SemanticPanicInputs,
) -> SemanticIntentBindingOutcome {
    let retained = inputs.intent.stamp();
    let stamp = SemanticIntentStamp::new(
        SemanticIntentIdentity::new(9_980),
        retained.caller(),
        SemanticRequestIdentity::new(9_981),
        *retained.target(),
        *retained.plan(),
        *retained.projection(),
        retained.coordinate(),
    );
    let preparation = stamp
        .prepare_intent(
            &inputs.representation,
            inputs.representation.intent().selected_stamp(),
            1,
            0,
            7,
        )
        .expect("valid semantic preparation");
    let preparation_debug = format!("{preparation:?}");
    assert!(preparation_debug.contains("SemanticIntentPreparation"));
    assert!(!preparation_debug.contains("secret"));
    preparation.bind(&inputs.representation, b"secret".as_slice().into())
}

fn semantic_receipt(
    inputs: &reovim_kernel_interface_public_consumer::SemanticPanicInputs,
) -> DomainValidationReceipt {
    let request = DomainValidationRequest::new(
        &inputs.domain,
        &inputs.representation,
        &inputs.intent,
        inputs.domain_charge,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .expect("valid Domain request");
    request
        .finish(
            &inputs.domain,
            request.validated_candidate(),
            InstallationOutcome::Current,
        )
        .receipt()
        .expect("exact Domain receipt")
}

fn semantic_lowering_request<'a>(
    inputs: &'a reovim_kernel_interface_public_consumer::SemanticPanicInputs,
    receipt: &'a DomainValidationReceipt,
) -> RepresentationLoweringRequest<'a> {
    RepresentationLoweringRequest::new(
        &inputs.representation,
        &inputs.intent,
        receipt,
        &inputs.buffer,
        inputs.representation_charge,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .expect("valid Representation request")
}

fn semantic_plan_candidate(
    inputs: &reovim_kernel_interface_public_consumer::SemanticPanicInputs,
) -> RepresentationLoweringCandidate {
    let (origin, operations, preconditions) = plan_carriers(&inputs.buffer);
    let outcome = inputs.buffer.bind_plan(
        *inputs.intent.stamp().plan(),
        *inputs.intent.stamp().target(),
        origin,
        operations,
        preconditions,
    );
    let BufferPlanBindingOutcome::Accepted(plan) = outcome else {
        panic!("semantic fixture must produce a complete plan");
    };
    plan.into()
}

struct PanickingDomain<'a> {
    descriptor: &'a reovim_kernel_interface::semantic::descriptor::
        DomainPolicyDescriptor,
}

impl DomainPolicyPort for PanickingDomain<'_> {
    fn descriptor(
        &self,
    ) -> &reovim_kernel_interface::semantic::descriptor::DomainPolicyDescriptor
    {
        self.descriptor
    }

    fn validate(
        &self,
        _request: DomainValidationRequest<'_>,
    ) -> DomainValidationCandidate {
        panic!("scripted Domain policy panic")
    }
}

struct PanickingRepresentation<'a> {
    descriptor: &'a reovim_kernel_interface::semantic::descriptor::
        RepresentationPolicyDescriptor,
}

impl RepresentationPolicyPort for PanickingRepresentation<'_> {
    fn descriptor(
        &self,
    ) -> &reovim_kernel_interface::semantic::descriptor::
    RepresentationPolicyDescriptor{
        self.descriptor
    }

    fn lower(
        &self,
        _request: RepresentationLoweringRequest<'_>,
    ) -> RepresentationLoweringCandidate {
        panic!("scripted Representation policy panic")
    }
}

fn plan_carriers(
    descriptor: &BufferPlanDescriptor,
) -> (BufferPlanCarrier, BufferPlanCarrier, BufferPlanCarrier) {
    (
        plan_carrier(descriptor, BufferCarrierRole::Origin, 1, b"secret"),
        plan_carrier(descriptor, BufferCarrierRole::Operations, 1, b"apply"),
        plan_carrier(descriptor, BufferCarrierRole::Preconditions, 0, b""),
    )
}

fn plan_carrier(
    descriptor: &BufferPlanDescriptor,
    role: BufferCarrierRole,
    items: u64,
    bytes: &[u8],
) -> BufferPlanCarrier {
    let contract = match role {
        BufferCarrierRole::Origin => descriptor.carriers().origin(),
        BufferCarrierRole::Operations => descriptor.carriers().operations(),
        BufferCarrierRole::Preconditions => {
            descriptor.carriers().preconditions()
        }
    };
    let size = u64::try_from(bytes.len()).expect("portable byte count");
    descriptor
        .prepare_carrier(role, contract.selected_stamp(), items, size)
        .expect("valid carrier preparation")
        .bind(descriptor, bytes.into())
        .expect("valid carrier allocation")
}

const fn plan_stamp(generation: u64) -> BufferPlanStamp {
    BufferPlanStamp::new(
        LifecycleIdentity::new(904, 905, Generation::new(generation)),
        BufferTransactionIdentity::new(920),
        BufferHistoryTransitionIdentity::new(921),
        BufferCancellationIdentity::new(922),
        BufferCorrelationIdentity::new(923),
        BufferDeadline::None,
    )
}

const fn buffer_state() -> BufferStateStamp {
    let access = ResourceAccessStamp::new(
        ResourceIdentity::new(930),
        LifecycleIdentity::new(931, 932, Generation::new(1)),
    );
    let source = ResourceSnapshotStamp::new(
        access,
        ResourceRevision::new(selection(933), 934),
        ResourceSnapshotIdentity::new(935),
    );
    let buffer = BufferIncarnationStamp::new(
        BufferIdentity::new(936),
        LifecycleIdentity::new(937, 938, Generation::new(1)),
    );
    let history = BufferHistoryStamp::new(
        LifecycleIdentity::new(939, 940, Generation::new(1)),
        BufferHistoryPosition::INITIAL,
    );
    BufferStateStamp::new(source, buffer, BufferVersion::INITIAL, history)
}

fn buffer_descriptor() -> BufferPlanDescriptor {
    let origin = carrier_contract(901);
    let operations = carrier_contract(902);
    let preconditions = carrier_contract(903);
    let carriers =
        BufferCarrierContracts::new(&origin, &operations, &preconditions)
            .expect("distinct carrier identities");
    BufferPlanDescriptor::new(
        selection(900),
        LifecycleIdentity::new(904, 905, Generation::new(1)),
        &carriers,
        &buffer_bounds(),
    )
}

fn buffer_bounds() -> BufferPlanBounds {
    BufferPlanBounds::new(
        InstanceId::new(906),
        count_bound(4, ZeroPolicy::Reject),
        count_bound(4, ZeroPolicy::Allow),
        byte_bound(8, ZeroPolicy::Reject),
        byte_bound(16, ZeroPolicy::Reject),
        byte_bound(8, ZeroPolicy::Allow),
        byte_bound(20, ZeroPolicy::Reject),
    )
    .expect("coherent Buffer-plan bounds")
}

fn count_bound(maximum: u64, zero: ZeroPolicy) -> BoundDescriptor {
    let maximum = CheckedCount::new(maximum, zero).expect("valid count");
    BoundDescriptor::new(
        BoundValue::Count(maximum),
        BoundValue::Count(maximum),
        zero,
        InstanceId::new(906),
    )
    .expect("coherent count bound")
}

fn byte_bound(maximum: u64, zero: ZeroPolicy) -> BoundDescriptor {
    let maximum = CheckedByteCount::new(maximum, zero).expect("valid bytes");
    BoundDescriptor::new(
        BoundValue::Bytes(maximum),
        BoundValue::Bytes(maximum),
        zero,
        InstanceId::new(906),
    )
    .expect("coherent byte bound")
}

fn carrier_contract(identity: u128) -> BufferCarrierContract {
    BufferCarrierContract::new(selection(identity), carrier_safety(identity))
        .expect("coherent Buffer carrier")
}

fn carrier_safety(identity: u128) -> CarrierSafetyDescriptor {
    let versions = CompatibleVersionRange::new(
        ContractVersion::new(1),
        ContractVersion::new(1),
    )
    .expect("valid version range");
    CarrierSafetyDescriptor::new(
        ContractId::new(identity),
        versions,
        MemorySafety::new(
            OwnershipModel::Owned,
            BorrowModel::Shared,
            CopyModel::MoveOnly,
            DropModel::ReleasesOwnedState,
            LifetimeModel::Value,
        ),
        ConcurrencySafety::new(
            ThreadTransfer::Transferable,
            SharingModel::Immutable,
        ),
        InvocationSafety::new(
            InvocationModel::DataOnly,
            UnwindModel::NotInvocable,
        ),
        TypeMismatch::Reject,
        UnsafeBoundary::None,
    )
    .expect("valid carrier safety")
}

const fn selection(identity: u128) -> ContractSelection {
    match ContractSelection::new(identity, 1, 1, 1) {
        Ok(value) => value,
        Err(_) => panic!("valid contract selection"),
    }
}
