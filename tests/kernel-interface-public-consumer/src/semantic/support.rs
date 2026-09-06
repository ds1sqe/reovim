use reovim_kernel_interface::bound::{
    BoundDescriptor, BoundValue, CheckedByteCount, CheckedCount, ZeroPolicy,
};
use reovim_kernel_interface::buffer::carrier::{
    BufferCarrierContract, BufferCarrierContracts,
};
use reovim_kernel_interface::buffer::plan::{
    BufferCancellationIdentity, BufferCorrelationIdentity, BufferDeadline,
    BufferHistoryTransitionIdentity, BufferPlanDescriptor, BufferPlanStamp,
    BufferTransactionIdentity,
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
use reovim_kernel_interface::identity::{IncarnationId, InstanceId};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::operation::{
    CallbackClass, CompletionClass, DeliveryClass, EffectClass,
    InvocationClass, ReentrancyClass,
};
use reovim_kernel_interface::semantic::bounds::{
    SemanticIntentBounds, SemanticPolicyCallBounds,
};
use reovim_kernel_interface::semantic::descriptor::{
    DomainPolicyDescriptor, RepresentationBehavior,
    RepresentationPolicyDescriptor, SemanticBufferPlanContract,
    SemanticCoordinateSpaceDescriptor, SemanticCorrespondence,
    SemanticDescriptorError, SemanticEditability, SemanticFidelity,
    SemanticPolicyAffinity, SemanticPolicyCallDescriptor,
    SemanticPolicyConcurrency, SemanticPolicyExecutionContext,
    SemanticPolicyInterruptionObservation, SemanticPolicyOutwardCalls,
    SemanticPolicyProgress,
};
use reovim_kernel_interface::semantic::identity::{
    CoordinateSpaceIdentity, DomainIdentity, ProjectionGeneration,
    ProjectionIdentity, RepresentationIdentity, SemanticIntentIdentity,
    SemanticRequestIdentity,
};
use reovim_kernel_interface::semantic::intent::{
    SemanticIntent, SemanticIntentBindingOutcome, SemanticIntentContract,
};
use reovim_kernel_interface::semantic::stamp::{
    CoordinateSpaceStamp, DomainPolicyStamp, ProjectionStamp,
    RepresentationPolicyStamp, SemanticCallerStamp, SemanticIntentStamp,
};

pub const INTENT_BYTES: &[u8] = &[0x91, 0x02, 0x7f, 0xa4];
pub const INTENT_TYPE: u128 = 1_040;
pub const INTENT_OWNER: u128 = 1_041;
pub const DOMAIN_CALL_OWNER: u128 = 1_042;
pub const REPRESENTATION_CALL_OWNER: u128 = 1_043;

pub struct SemanticFixture {
    pub domain: DomainPolicyDescriptor,
    pub representation: RepresentationPolicyDescriptor,
    pub buffer: BufferPlanDescriptor,
}

pub fn fixture() -> Option<SemanticFixture> {
    let buffer = crate::buffer::descriptor()?;
    let domain_stamp = domain_stamp(1)?;
    let domain_call = call_descriptor(DOMAIN_CALL_OWNER, 16)?;
    let domain = DomainPolicyDescriptor::new(domain_stamp, &domain_call);
    let representation_stamp = representation_stamp(domain_stamp, 1)?;
    let coordinate = coordinate_descriptor()?;
    let intent = intent_contract()?;
    let behavior = RepresentationBehavior::new(
        SemanticFidelity::Faithful,
        SemanticEditability::Transactional,
        SemanticCorrespondence::Bidirectional,
    )
    .ok()?;
    let lowering = call_descriptor(REPRESENTATION_CALL_OWNER, 24)?;
    let plan = SemanticBufferPlanContract::from_descriptor(&buffer);
    let representation = RepresentationPolicyDescriptor::new(
        &domain,
        representation_stamp,
        &coordinate,
        &intent,
        behavior,
        plan,
        &lowering,
    )
    .ok()?;
    Some(SemanticFixture { domain, representation, buffer })
}

pub fn descriptor_cases() -> bool {
    let Some(fixture) = fixture() else { return false };
    let validation = fixture.domain.validation();
    let lowering = fixture.representation.lowering();
    descriptor_identity(&fixture)
        && operation_shape(validation)
        && operation_shape(lowering)
        && fixture.representation.fidelity() == SemanticFidelity::Faithful
        && fixture.representation.editability()
            == SemanticEditability::Transactional
        && fixture.representation.correspondence()
            == SemanticCorrespondence::Bidirectional
        && fixture.representation.plan().matches_descriptor(&fixture.buffer)
        && behavior_errors()
        && serialized_concurrency_error()
        && representation_owner_error(&fixture)
        && intent_safety_error()
}

pub fn make_intent(
    fixture: &SemanticFixture,
    bytes: &[u8],
) -> Option<SemanticIntent> {
    make_intent_with(fixture, bytes, 1_060, 1_061)
}

pub fn make_intent_with(
    fixture: &SemanticFixture,
    bytes: &[u8],
    intent: u128,
    request: u128,
) -> Option<SemanticIntent> {
    let size = u64::try_from(bytes.len()).ok()?;
    let stamp = intent_stamp(fixture, intent, request)?;
    let preparation = stamp
        .prepare_intent(
            &fixture.representation,
            fixture.representation.intent().selected_stamp(),
            1,
            0,
            size,
        )
        .ok()?;
    match preparation.bind(&fixture.representation, bytes.into()) {
        SemanticIntentBindingOutcome::Bound(intent) => Some(intent),
        SemanticIntentBindingOutcome::Rejected(_) => None,
    }
}

pub fn intent_stamp(
    fixture: &SemanticFixture,
    intent: u128,
    request: u128,
) -> Option<SemanticIntentStamp> {
    intent_stamp_for(
        intent,
        request,
        fixture.representation.stamp(),
        fixture.representation.coordinate().stamp(),
    )
}

pub fn intent_stamp_for(
    intent: u128,
    request: u128,
    representation: RepresentationPolicyStamp,
    coordinate: CoordinateSpaceStamp,
) -> Option<SemanticIntentStamp> {
    let caller = SemanticCallerStamp::new(
        LifecycleIdentity::new(1_062, 1_063, Generation::new(1)),
        crate::resource::selection(1_064)?,
    );
    let projection = ProjectionStamp::new(
        ProjectionIdentity::new(1_065),
        LifecycleIdentity::new(1_066, 1_067, Generation::new(1)),
        representation,
        ProjectionGeneration::new(3),
    );
    Some(SemanticIntentStamp::new(
        SemanticIntentIdentity::new(intent),
        caller,
        SemanticRequestIdentity::new(request),
        crate::buffer::state(1, 0, 0)?,
        plan_stamp(),
        projection,
        coordinate,
    ))
}

pub fn domain_descriptor(generation: u64) -> Option<DomainPolicyDescriptor> {
    let stamp = domain_stamp(generation)?;
    let call = call_descriptor(DOMAIN_CALL_OWNER, 16)?;
    Some(DomainPolicyDescriptor::new(stamp, &call))
}

pub fn replacement_representation(
    fixture: &SemanticFixture,
) -> Option<RepresentationPolicyDescriptor> {
    let stamp = representation_stamp(fixture.domain.stamp(), 2)?;
    rebuild_representation(fixture, stamp, editable_behavior()?)
}

pub fn read_only_representation(
    fixture: &SemanticFixture,
) -> Option<RepresentationPolicyDescriptor> {
    let behavior = RepresentationBehavior::new(
        SemanticFidelity::Faithful,
        SemanticEditability::ReadOnly,
        SemanticCorrespondence::Bidirectional,
    )
    .ok()?;
    rebuild_representation(fixture, fixture.representation.stamp(), behavior)
}

pub fn buffer_descriptor(
    contract: u128,
    owner_generation: u64,
) -> Option<BufferPlanDescriptor> {
    Some(BufferPlanDescriptor::new(
        crate::resource::selection(contract)?,
        crate::buffer::owner(owner_generation),
        &crate::buffer::carrier::contracts()?,
        &crate::buffer::bounds()?,
    ))
}

pub fn alternate_buffer_descriptor() -> Option<BufferPlanDescriptor> {
    let origin = buffer_carrier_contract(1_100)?;
    let operations = buffer_carrier_contract(1_101)?;
    let preconditions = buffer_carrier_contract(1_102)?;
    let carriers =
        BufferCarrierContracts::new(&origin, &operations, &preconditions)
            .ok()?;
    Some(BufferPlanDescriptor::new(
        crate::resource::selection(800)?,
        crate::buffer::owner(1),
        &carriers,
        &crate::buffer::bounds()?,
    ))
}

pub fn call_bounds(
    owner: u128,
    work_maximum: u64,
) -> Option<SemanticPolicyCallBounds> {
    call_bounds_with_concurrency(owner, work_maximum, 1)
}

pub fn call_bounds_with_concurrency(
    owner: u128,
    work_maximum: u64,
    concurrent_maximum: u64,
) -> Option<SemanticPolicyCallBounds> {
    SemanticPolicyCallBounds::new(
        InstanceId::new(owner),
        count_bound(owner, work_maximum, ZeroPolicy::Reject)?,
        byte_bound(owner, 32, ZeroPolicy::Allow)?,
        byte_bound(owner, 64, ZeroPolicy::Allow)?,
        count_bound(owner, concurrent_maximum, ZeroPolicy::Reject)?,
    )
    .ok()
}

pub fn domain_stamp(generation: u64) -> Option<DomainPolicyStamp> {
    Some(DomainPolicyStamp::new(
        DomainIdentity::new(1_000),
        crate::resource::selection(1_001)?,
        LifecycleIdentity::new(1_002, 1_003, Generation::new(generation)),
    ))
}

pub fn representation_stamp(
    domain: DomainPolicyStamp,
    generation: u64,
) -> Option<RepresentationPolicyStamp> {
    Some(RepresentationPolicyStamp::new(
        RepresentationIdentity::new(1_010),
        crate::resource::selection(1_011)?,
        LifecycleIdentity::new(1_012, 1_013, Generation::new(generation)),
        domain,
    ))
}

pub fn coordinate_stamp(incarnation: u128) -> Option<CoordinateSpaceStamp> {
    Some(CoordinateSpaceStamp::new(
        CoordinateSpaceIdentity::new(1_020),
        crate::resource::selection(1_021)?,
        IncarnationId::new(incarnation),
    ))
}

const fn descriptor_identity(fixture: &SemanticFixture) -> bool {
    fixture.domain.stamp().domain().value() == 1_000
        && fixture.representation.stamp().representation().value() == 1_010
        && fixture.representation.coordinate().stamp().space().value() == 1_020
        && fixture.representation.intent().schema().identity() == INTENT_TYPE
}

fn behavior_errors() -> bool {
    let derived = RepresentationBehavior::new(
        SemanticFidelity::DerivedReadOnly,
        SemanticEditability::Transactional,
        SemanticCorrespondence::Bidirectional,
    );
    let constrained = RepresentationBehavior::new(
        SemanticFidelity::ConstrainedEditable,
        SemanticEditability::ReadOnly,
        SemanticCorrespondence::Bidirectional,
    );
    let no_reverse = RepresentationBehavior::new(
        SemanticFidelity::Faithful,
        SemanticEditability::Transactional,
        SemanticCorrespondence::SourceOnly,
    );
    let no_source = RepresentationBehavior::new(
        SemanticFidelity::Faithful,
        SemanticEditability::ReadOnly,
        SemanticCorrespondence::None,
    );
    descriptor_error_code(derived) == 1
        && descriptor_error_code(constrained) == 2
        && descriptor_error_code(no_reverse) == 3
        && descriptor_error_code(no_source) == 4
}

const fn descriptor_error_code(
    result: Result<RepresentationBehavior, SemanticDescriptorError>,
) -> u8 {
    match result {
        Err(SemanticDescriptorError::DerivedEditable) => 1,
        Err(SemanticDescriptorError::ConstrainedReadOnly) => 2,
        Err(
            SemanticDescriptorError::TransactionWithoutReverseCorrespondence,
        ) => 3,
        Err(SemanticDescriptorError::FaithfulWithoutCorrespondence) => 4,
        _ => 0,
    }
}

fn serialized_concurrency_error() -> bool {
    let Some(bounds) = call_bounds_with_concurrency(DOMAIN_CALL_OWNER, 16, 2)
    else {
        return false;
    };
    let result = SemanticPolicyCallDescriptor::new(
        SemanticPolicyConcurrency::Serialized,
        &bounds,
    );
    let code = match result {
        Err(SemanticDescriptorError::SerializedConcurrencyExceeded {
            actual: 2,
        }) => 0,
        _ => 1,
    };
    code == 0
}

fn representation_owner_error(fixture: &SemanticFixture) -> bool {
    let Some(other_domain) = domain_stamp(2) else { return false };
    let Some(stamp) = representation_stamp(other_domain, 1) else {
        return false;
    };
    let Some(behavior) = editable_behavior() else { return false };
    let result = RepresentationPolicyDescriptor::new(
        &fixture.domain,
        stamp,
        fixture.representation.coordinate(),
        fixture.representation.intent(),
        behavior,
        fixture.representation.plan(),
        fixture.representation.lowering(),
    );
    let code = match result {
        Err(SemanticDescriptorError::DomainGenerationMismatch) => 0,
        _ => 1,
    };
    code == 0
}

fn intent_safety_error() -> bool {
    let Some(schema) = crate::resource::selection(INTENT_TYPE) else {
        return false;
    };
    let Some(safety) =
        carrier_safety_with_transfer(INTENT_TYPE, ThreadTransfer::ThreadAffine)
    else {
        return false;
    };
    let result = SemanticIntentContract::new(schema, safety);
    let code = match result {
        Err(
            reovim_kernel_interface::semantic::intent::
                SemanticIntentError::IncoherentSafety,
        ) => 0,
        _ => 1,
    };
    code == 0
}

fn operation_shape(descriptor: &SemanticPolicyCallDescriptor) -> bool {
    let operation = descriptor.operation();
    let safety = descriptor.safety();
    operation.invocation() == InvocationClass::NonBlockingInvocation
        && operation.completion() == CompletionClass::DeferredTerminal
        && operation.delivery() == DeliveryClass::Single
        && operation.effect() == EffectClass::LocalOnly
        && operation.callback() == CallbackClass::MayInvokeUserCallback
        && operation.reentrancy() == ReentrancyClass::NonReentrant
        && safety.invocation() == InvocationModel::UserCallback
        && safety.unwind() == UnwindModel::Contained
        && descriptor.progress() == SemanticPolicyProgress::BoundedToCompletion
        && descriptor.concurrency() == SemanticPolicyConcurrency::Serialized
        && descriptor.context() == SemanticPolicyExecutionContext::RuntimeWorker
        && descriptor.affinity() == SemanticPolicyAffinity::AnyRuntimeWorker
        && descriptor.outward_calls() == SemanticPolicyOutwardCalls::None
        && descriptor.interruption()
            == SemanticPolicyInterruptionObservation::RuntimeBoundaries
}

fn coordinate_descriptor() -> Option<SemanticCoordinateSpaceDescriptor> {
    Some(SemanticCoordinateSpaceDescriptor::new(
        coordinate_stamp(1_022)?,
        crate::resource::selection(1_023)?,
        crate::resource::selection(1_024)?,
        crate::resource::selection(1_025)?,
        &intent_bounds()?,
    ))
}

fn editable_behavior() -> Option<RepresentationBehavior> {
    RepresentationBehavior::new(
        SemanticFidelity::Faithful,
        SemanticEditability::Transactional,
        SemanticCorrespondence::Bidirectional,
    )
    .ok()
}

fn rebuild_representation(
    fixture: &SemanticFixture,
    stamp: RepresentationPolicyStamp,
    behavior: RepresentationBehavior,
) -> Option<RepresentationPolicyDescriptor> {
    RepresentationPolicyDescriptor::new(
        &fixture.domain,
        stamp,
        fixture.representation.coordinate(),
        fixture.representation.intent(),
        behavior,
        fixture.representation.plan(),
        fixture.representation.lowering(),
    )
    .ok()
}

fn intent_bounds() -> Option<SemanticIntentBounds> {
    SemanticIntentBounds::new(
        InstanceId::new(INTENT_OWNER),
        count_bound(INTENT_OWNER, 4, ZeroPolicy::Reject)?,
        count_bound(INTENT_OWNER, 4, ZeroPolicy::Allow)?,
        byte_bound(INTENT_OWNER, 64, ZeroPolicy::Reject)?,
    )
    .ok()
}

fn intent_contract() -> Option<SemanticIntentContract> {
    let schema = crate::resource::selection(INTENT_TYPE)?;
    let safety = carrier_safety(INTENT_TYPE)?;
    SemanticIntentContract::new(schema, safety).ok()
}

fn call_descriptor(
    owner: u128,
    work_maximum: u64,
) -> Option<SemanticPolicyCallDescriptor> {
    SemanticPolicyCallDescriptor::new(
        SemanticPolicyConcurrency::Serialized,
        &call_bounds(owner, work_maximum)?,
    )
    .ok()
}

const fn plan_stamp() -> BufferPlanStamp {
    BufferPlanStamp::new(
        crate::buffer::owner(1),
        BufferTransactionIdentity::new(1_070),
        BufferHistoryTransitionIdentity::new(1_071),
        BufferCancellationIdentity::new(1_072),
        BufferCorrelationIdentity::new(1_073),
        BufferDeadline::None,
    )
}

pub fn count_bound(
    owner: u128,
    maximum: u64,
    zero: ZeroPolicy,
) -> Option<BoundDescriptor> {
    let maximum = CheckedCount::new(maximum, zero).ok()?;
    BoundDescriptor::new(
        BoundValue::Count(maximum),
        BoundValue::Count(maximum),
        zero,
        InstanceId::new(owner),
    )
    .ok()
}

pub fn byte_bound(
    owner: u128,
    maximum: u64,
    zero: ZeroPolicy,
) -> Option<BoundDescriptor> {
    let maximum = CheckedByteCount::new(maximum, zero).ok()?;
    BoundDescriptor::new(
        BoundValue::Bytes(maximum),
        BoundValue::Bytes(maximum),
        zero,
        InstanceId::new(owner),
    )
    .ok()
}

pub fn carrier_safety(identity: u128) -> Option<CarrierSafetyDescriptor> {
    carrier_safety_with_transfer(identity, ThreadTransfer::Transferable)
}

fn carrier_safety_with_transfer(
    identity: u128,
    thread_transfer: ThreadTransfer,
) -> Option<CarrierSafetyDescriptor> {
    let versions = CompatibleVersionRange::new(
        ContractVersion::new(1),
        ContractVersion::new(1),
    )
    .ok()?;
    let memory = MemorySafety::new(
        OwnershipModel::Owned,
        BorrowModel::Shared,
        CopyModel::MoveOnly,
        DropModel::ReleasesOwnedState,
        LifetimeModel::Value,
    );
    let concurrency =
        ConcurrencySafety::new(thread_transfer, SharingModel::Immutable);
    let invocation = InvocationSafety::new(
        InvocationModel::DataOnly,
        UnwindModel::NotInvocable,
    );
    CarrierSafetyDescriptor::new(
        ContractId::new(identity),
        versions,
        memory,
        concurrency,
        invocation,
        TypeMismatch::Reject,
        UnsafeBoundary::None,
    )
    .ok()
}

fn buffer_carrier_contract(identity: u128) -> Option<BufferCarrierContract> {
    BufferCarrierContract::new(
        crate::resource::selection(identity)?,
        carrier_safety(identity)?,
    )
    .ok()
}
