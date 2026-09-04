use reovim_kernel_interface::bound::{
    BoundDescriptor, BoundUnit, BoundValue, CheckedByteCount, CheckedCount,
    ZeroPolicy,
};
use reovim_kernel_interface::cancellation::ExecutionOutcome;
use reovim_kernel_interface::identity::InstanceId;
use reovim_kernel_interface::lifecycle::{
    Generation, LifecycleIdentity, RollbackOutcome,
};
use reovim_kernel_interface::operation::{
    DeliveryClass, EffectClass, OperationDescriptor,
};
use reovim_kernel_interface::resource::access::{
    ResourceAccessDescriptor, ResourceApplicabilityOutcome,
    ResourceChangeCheckpoint, ResourceCloseError, ResourceCloseOutcome,
    ResourceFreshness, ResourceIncompleteClose, ResourceNegotiationError,
    ResourceObservationFailure, ResourceOpenFailure, ResourceOpenOutcome,
    ResourceOpenStatus, ResourceReadyError, ResourceReadyState,
};
use reovim_kernel_interface::resource::bounds::{
    ResourceAccessBounds, ResourceBoundField, ResourceBoundsError,
    ResourceCompletionScope, ResourcePayloadAccounting, ResourceStateBounds,
    ResourceTransferBounds,
};
use reovim_kernel_interface::resource::capability::{
    ChangeDiscovery, DestinationCreation, HistoricalAvailability,
    ResourceCapabilities, ResourceCapabilityError, ResourceMutationGrammar,
    SnapshotConsistency, SourceAccessAuthority, SourceStability,
};
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceIdentity, ResourceRevision,
    ResourceSnapshotIdentity, ResourceSnapshotStamp, ResourceValueSchemas,
};

use super::{selection, snapshot};
use crate::{error_traits, traits};

pub const fn const_cases() -> bool {
    derive_cases()
        && capability_variant_cases()
        && outcome_variant_cases()
        && bound_variant_cases()
}

pub fn exercise() -> bool {
    capability_cases()
        && bound_cases()
        && descriptor_cases()
        && ready_cases()
        && terminal_cases()
}

const fn derive_cases() -> bool {
    derive_capabilities() && derive_bounds() && derive_access()
}

const fn derive_capabilities() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::resource::capability::
            SourceAccessAuthority,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::capability::SourceStability,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::capability::SnapshotConsistency,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::capability::
            HistoricalAvailability,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::capability::
            ResourceMutationGrammar,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::capability::DestinationCreation,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::capability::ChangeDiscovery,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::capability::
            ResourceCapabilityError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::capability::ResourceCapabilities,
    >()
}

const fn derive_bounds() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::resource::bounds::ResourcePayloadAccounting,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::bounds::ResourceCompletionScope,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::bounds::ResourceBoundField,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::bounds::ResourceBoundsError,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::resource::bounds::ResourceTransferBounds,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::resource::bounds::ResourceStateBounds,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::resource::bounds::ResourceAccessBounds,
    >()
}

const fn derive_access() -> bool {
    crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::resource::access::ResourceAccessDescriptor,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::
            ResourceApplicabilityOutcome,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::
            ResourceNegotiationError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::ResourceFreshness,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::ResourceChangeCheckpoint,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::ResourceReadyState,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::ResourceReadyError,
    >() && derive_terminal_access()
}

const fn derive_terminal_access() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::ResourceOpenFailure,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::resource::access::ResourceOpenOutcome,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::ResourceOpenStatus,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::ResourceObservationStatus,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::ResourceObservedStateError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::ResourceObservationFailure,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::ResourceCloseError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::ResourceIncompleteClose,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::access::ResourceCloseOutcome,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::resource::access::ResourceObservedState,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::resource::access::ResourceObservationOutcome,
    >()
}

const fn capability_variant_cases() -> bool {
    source_access_code(SourceAccessAuthority::ObserveOnly) == 0
        && source_access_code(SourceAccessAuthority::ObserveAndPublish) == 1
        && stability_code(SourceStability::Stable) == 0
        && stability_code(SourceStability::ExternallyEvolving) == 1
        && history_code(HistoricalAvailability::CurrentOnly) == 0
        && history_code(HistoricalAvailability::BoundedRetention) == 1
}

const fn source_access_code(value: SourceAccessAuthority) -> u8 {
    match value {
        SourceAccessAuthority::ObserveOnly => 0,
        SourceAccessAuthority::ObserveAndPublish => 1,
    }
}

const fn stability_code(value: SourceStability) -> u8 {
    match value {
        SourceStability::Stable => 0,
        SourceStability::ExternallyEvolving => 1,
    }
}

const fn history_code(value: HistoricalAvailability) -> u8 {
    match value {
        HistoricalAvailability::CurrentOnly => 0,
        HistoricalAvailability::BoundedRetention => 1,
    }
}

const fn outcome_variant_cases() -> bool {
    freshness_code(ResourceFreshness::Fresh) == 0
        && freshness_code(ResourceFreshness::Stale) == 1
        && freshness_code(ResourceFreshness::Indeterminate) == 2
        && applicability_code(ResourceApplicabilityOutcome::Applicable) == 0
        && applicability_code(ResourceApplicabilityOutcome::Denied) == 1
        && applicability_code(ResourceApplicabilityOutcome::Unsupported) == 2
        && applicability_code(ResourceApplicabilityOutcome::Incompatible) == 3
}

const fn freshness_code(value: ResourceFreshness) -> u8 {
    match value {
        ResourceFreshness::Fresh => 0,
        ResourceFreshness::Stale => 1,
        ResourceFreshness::Indeterminate => 2,
    }
}

const fn applicability_code(value: ResourceApplicabilityOutcome) -> u8 {
    match value {
        ResourceApplicabilityOutcome::Applicable => 0,
        ResourceApplicabilityOutcome::Denied => 1,
        ResourceApplicabilityOutcome::Unsupported => 2,
        ResourceApplicabilityOutcome::Incompatible => 3,
    }
}

const fn bound_variant_cases() -> bool {
    payload_code(ResourcePayloadAccounting::OwnedByAccess) == 0
        && payload_code(ResourcePayloadAccounting::SharedBySnapshot) == 1
        && completion_code(ResourceCompletionScope::AccessIncarnation) == 0
        && completion_code(ResourceCompletionScope::AdapterInstance) == 1
        && bound_field_code(ResourceBoundField::Regions) == 0
        && bound_field_code(ResourceBoundField::CarrierBytes) == 1
        && bound_field_code(ResourceBoundField::PayloadBytes) == 2
        && bound_field_code(ResourceBoundField::OwnedBytes) == 3
        && bound_field_code(ResourceBoundField::InFlight) == 4
        && bound_field_code(ResourceBoundField::Queue) == 5
        && bound_field_code(ResourceBoundField::Snapshots) == 6
        && bound_field_code(ResourceBoundField::Terminals) == 7
}

const fn payload_code(value: ResourcePayloadAccounting) -> u8 {
    match value {
        ResourcePayloadAccounting::OwnedByAccess => 0,
        ResourcePayloadAccounting::SharedBySnapshot => 1,
    }
}

const fn completion_code(value: ResourceCompletionScope) -> u8 {
    match value {
        ResourceCompletionScope::AccessIncarnation => 0,
        ResourceCompletionScope::AdapterInstance => 1,
    }
}

const fn bound_field_code(value: ResourceBoundField) -> u8 {
    match value {
        ResourceBoundField::Regions => 0,
        ResourceBoundField::CarrierBytes => 1,
        ResourceBoundField::PayloadBytes => 2,
        ResourceBoundField::OwnedBytes => 3,
        ResourceBoundField::InFlight => 4,
        ResourceBoundField::Queue => 5,
        ResourceBoundField::Snapshots => 6,
        ResourceBoundField::Terminals => 7,
    }
}

fn capability_cases() -> bool {
    let Some(stable) = stable_capabilities() else { return false };
    let publication = ResourceCapabilities::new(
        SourceAccessAuthority::ObserveAndPublish,
        SourceStability::ExternallyEvolving,
        SnapshotConsistency::NativeRegion,
        HistoricalAvailability::BoundedRetention,
        ResourceMutationGrammar::AddOnly,
        DestinationCreation::Granted,
        ChangeDiscovery::OrderedResumableFeed,
    );
    let invalid_mutation = ResourceCapabilities::new(
        SourceAccessAuthority::ObserveOnly,
        SourceStability::ExternallyEvolving,
        SnapshotConsistency::NativeRegion,
        HistoricalAvailability::CurrentOnly,
        ResourceMutationGrammar::Replacement,
        DestinationCreation::NotGranted,
        ChangeDiscovery::None,
    );
    let invalid_stability = ResourceCapabilities::new(
        SourceAccessAuthority::ObserveOnly,
        SourceStability::Stable,
        SnapshotConsistency::WholeResource,
        HistoricalAvailability::CurrentOnly,
        ResourceMutationGrammar::None,
        DestinationCreation::NotGranted,
        ChangeDiscovery::Invalidation,
    );
    let Ok(publication) = publication else { return false };
    traits(stable)
        && traits(publication)
        && stable.source_access() == SourceAccessAuthority::ObserveOnly
        && stable.source_stability() == SourceStability::Stable
        && stable.snapshot_consistency() == SnapshotConsistency::WholeResource
        && stable.history() == HistoricalAvailability::CurrentOnly
        && stable.mutation() == ResourceMutationGrammar::None
        && stable.creation() == DestinationCreation::NotGranted
        && stable.change_discovery() == ChangeDiscovery::None
        && mutation_grammar_cases()
        && snapshot_consistency_cases()
        && change_discovery_cases()
        && capability_error_cases(invalid_mutation, invalid_stability)
}

fn mutation_grammar_cases() -> bool {
    traits(ResourceMutationGrammar::None)
        && traits(ResourceMutationGrammar::Replacement)
        && traits(ResourceMutationGrammar::AddOnly)
        && traits(ResourceMutationGrammar::Resize)
        && traits(DestinationCreation::NotGranted)
        && traits(DestinationCreation::Granted)
}

fn snapshot_consistency_cases() -> bool {
    traits(SnapshotConsistency::WholeResource)
        && traits(SnapshotConsistency::NativeRegion)
}

fn change_discovery_cases() -> bool {
    traits(ChangeDiscovery::None)
        && traits(ChangeDiscovery::Invalidation)
        && traits(ChangeDiscovery::OrderedResumableFeed)
}

fn capability_error_cases(
    mutation: Result<ResourceCapabilities, ResourceCapabilityError>,
    stability: Result<ResourceCapabilities, ResourceCapabilityError>,
) -> bool {
    let mutation_error = match mutation {
        Err(ResourceCapabilityError::MutationWithoutPublicationAuthority) => {
            true
        }
        Err(ResourceCapabilityError::ChangeDiscoveryForStableResource)
        | Ok(_) => false,
    };
    let stability_error = match stability {
        Err(ResourceCapabilityError::ChangeDiscoveryForStableResource) => true,
        Err(ResourceCapabilityError::MutationWithoutPublicationAuthority)
        | Ok(_) => false,
    };
    let error = ResourceCapabilityError::MutationWithoutPublicationAuthority;
    mutation_error
        && stability_error
        && error_traits(&error)
        && traits(ResourceCapabilityError::ChangeDiscoveryForStableResource)
}

fn stable_capabilities() -> Option<ResourceCapabilities> {
    ResourceCapabilities::new(
        SourceAccessAuthority::ObserveOnly,
        SourceStability::Stable,
        SnapshotConsistency::WholeResource,
        HistoricalAvailability::CurrentOnly,
        ResourceMutationGrammar::None,
        DestinationCreation::NotGranted,
        ChangeDiscovery::None,
    )
    .ok()
}

fn evolving_capabilities(
    change: ChangeDiscovery,
) -> Option<ResourceCapabilities> {
    ResourceCapabilities::new(
        SourceAccessAuthority::ObserveOnly,
        SourceStability::ExternallyEvolving,
        SnapshotConsistency::NativeRegion,
        HistoricalAvailability::BoundedRetention,
        ResourceMutationGrammar::None,
        DestinationCreation::NotGranted,
        change,
    )
    .ok()
}

fn bound_cases() -> bool {
    let Some(stable) = stable_capabilities() else { return false };
    let Some(bounds) = access_bounds(stable, 0, 1, 1, 1) else {
        return false;
    };
    let transfer = bounds.transfer();
    let state = bounds.state();
    bounds == bounds.clone()
        && transfer.regions().unit() == BoundUnit::Count
        && transfer.carrier_bytes().unit() == BoundUnit::Bytes
        && transfer.payload_bytes().unit() == BoundUnit::Bytes
        && state.owned_bytes().unit() == BoundUnit::Bytes
        && state.in_flight().aggregate().value() == 1
        && state.queue().aggregate().value() == 0
        && state.snapshots().aggregate().value() == 1
        && state.terminals().aggregate().value() == 1
        && bounds.payload_accounting()
            == ResourcePayloadAccounting::SharedBySnapshot
        && bounds.completion_scope()
            == ResourceCompletionScope::AccessIncarnation
        && wrong_unit_cases()
        && missing_capacity_cases()
}

fn access_bounds(
    capabilities: ResourceCapabilities,
    queue: u64,
    snapshots: u64,
    in_flight: u64,
    terminals: u64,
) -> Option<ResourceAccessBounds> {
    let transfer = ResourceTransferBounds::new(
        count_bound(301, 1, 8)?,
        byte_bound(302, 64, 256)?,
        byte_bound(303, 1024, 4096)?,
    )
    .ok()?;
    let state = ResourceStateBounds::new(
        byte_bound(304, 4096, 16384)?,
        count_bound_allow_zero(305, in_flight, in_flight)?,
        count_bound_allow_zero(306, queue, queue)?,
        count_bound_allow_zero(307, snapshots, snapshots)?,
        count_bound_allow_zero(308, terminals, terminals)?,
    )
    .ok()?;
    ResourceAccessBounds::new(
        &transfer,
        &state,
        ResourcePayloadAccounting::SharedBySnapshot,
        ResourceCompletionScope::AccessIncarnation,
        capabilities,
    )
    .ok()
}

fn count_bound(owner: u128, item: u64, total: u64) -> Option<BoundDescriptor> {
    let item = CheckedCount::new(item, ZeroPolicy::Reject).ok()?;
    let total = CheckedCount::new(total, ZeroPolicy::Reject).ok()?;
    BoundDescriptor::new(
        BoundValue::Count(item),
        BoundValue::Count(total),
        ZeroPolicy::Reject,
        InstanceId::new(owner),
    )
    .ok()
}

fn count_bound_allow_zero(
    owner: u128,
    item: u64,
    total: u64,
) -> Option<BoundDescriptor> {
    let item = CheckedCount::new(item, ZeroPolicy::Allow).ok()?;
    let total = CheckedCount::new(total, ZeroPolicy::Allow).ok()?;
    BoundDescriptor::new(
        BoundValue::Count(item),
        BoundValue::Count(total),
        ZeroPolicy::Allow,
        InstanceId::new(owner),
    )
    .ok()
}

fn byte_bound(owner: u128, item: u64, total: u64) -> Option<BoundDescriptor> {
    let item = CheckedByteCount::new(item, ZeroPolicy::Reject).ok()?;
    let total = CheckedByteCount::new(total, ZeroPolicy::Reject).ok()?;
    BoundDescriptor::new(
        BoundValue::Bytes(item),
        BoundValue::Bytes(total),
        ZeroPolicy::Reject,
        InstanceId::new(owner),
    )
    .ok()
}

fn wrong_unit_cases() -> bool {
    let Some(count) = count_bound(311, 1, 2) else { return false };
    let Some(bytes) = byte_bound(312, 1, 2) else { return false };
    let transfer = ResourceTransferBounds::new(bytes, bytes, bytes);
    let state = ResourceStateBounds::new(count, count, count, count, count);
    wrong_unit(
        &transfer,
        ResourceBoundField::Regions,
        BoundUnit::Count,
        BoundUnit::Bytes,
    ) && wrong_unit(
        &state,
        ResourceBoundField::OwnedBytes,
        BoundUnit::Bytes,
        BoundUnit::Count,
    )
}

fn wrong_unit<T>(
    result: &Result<T, ResourceBoundsError>,
    field: ResourceBoundField,
    expected: BoundUnit,
    actual: BoundUnit,
) -> bool {
    match result {
        Err(ResourceBoundsError::WrongUnit {
            field: found,
            expected: expected_unit,
            actual: actual_unit,
        }) => {
            *found == field
                && *expected_unit == expected
                && *actual_unit == actual
        }
        Err(
            ResourceBoundsError::InFlightCapacityRequired
            | ResourceBoundsError::TerminalRetentionRequired
            | ResourceBoundsError::HistoricalRetentionRequired
            | ResourceBoundsError::ChangeQueueRequired,
        )
        | Ok(_) => false,
    }
}

fn missing_capacity_cases() -> bool {
    let Some(stable) = stable_capabilities() else { return false };
    let Some(history) = evolving_capabilities(ChangeDiscovery::None) else {
        return false;
    };
    let Some(changes) = evolving_capabilities(ChangeDiscovery::Invalidation)
    else {
        return false;
    };
    let Some(in_flight) = access_bounds_result(stable, 0, 1, 0, 1) else {
        return false;
    };
    let Some(terminals) = access_bounds_result(stable, 0, 1, 1, 0) else {
        return false;
    };
    let Some(retention) = access_bounds_result(history, 0, 0, 1, 1) else {
        return false;
    };
    let Some(queue) = access_bounds_result(changes, 0, 1, 1, 1) else {
        return false;
    };
    missing_capacity(&in_flight, ResourceBoundsError::InFlightCapacityRequired)
        && missing_capacity(
            &terminals,
            ResourceBoundsError::TerminalRetentionRequired,
        )
        && missing_capacity(
            &retention,
            ResourceBoundsError::HistoricalRetentionRequired,
        )
        && missing_capacity(&queue, ResourceBoundsError::ChangeQueueRequired)
}

fn access_bounds_result(
    capabilities: ResourceCapabilities,
    queue: u64,
    snapshots: u64,
    in_flight: u64,
    terminals: u64,
) -> Option<Result<ResourceAccessBounds, ResourceBoundsError>> {
    let transfer = ResourceTransferBounds::new(
        count_bound(321, 1, 8)?,
        byte_bound(322, 64, 256)?,
        byte_bound(323, 1024, 4096)?,
    )
    .ok()?;
    let state = ResourceStateBounds::new(
        byte_bound(324, 4096, 16384)?,
        count_bound_allow_zero(325, in_flight, in_flight)?,
        count_bound_allow_zero(326, queue, queue)?,
        count_bound_allow_zero(327, snapshots, snapshots)?,
        count_bound_allow_zero(328, terminals, terminals)?,
    )
    .ok()?;
    Some(ResourceAccessBounds::new(
        &transfer,
        &state,
        ResourcePayloadAccounting::OwnedByAccess,
        ResourceCompletionScope::AdapterInstance,
        capabilities,
    ))
}

fn missing_capacity(
    result: &Result<ResourceAccessBounds, ResourceBoundsError>,
    expected: ResourceBoundsError,
) -> bool {
    match result {
        Err(actual) => *actual == expected && error_traits(actual),
        Ok(_) => false,
    }
}

fn descriptor_cases() -> bool {
    let Some(descriptor) = stable_descriptor() else { return false };
    let operations = descriptor_operations(&descriptor);
    descriptor.contract().identity() == 200
        && descriptor.adapter().instance() == 210
        && descriptor.owner().instance() == 220
        && descriptor.schemas().address().identity() == 101
        && descriptor.capabilities().source_stability()
            == SourceStability::Stable
        && descriptor.bounds().state().in_flight().aggregate().value() == 1
        && descriptor.negotiate(200, 1).is_ok()
        && descriptor.negotiate(201, 1)
            == Err(ResourceNegotiationError::UnsupportedContract)
        && descriptor.negotiate(200, 3)
            == Err(ResourceNegotiationError::IncompatibleContract)
        && operations
        && error_traits(&ResourceNegotiationError::UnsupportedContract)
        && traits(ResourceNegotiationError::IncompatibleContract)
}

fn descriptor_operations(descriptor: &ResourceAccessDescriptor) -> bool {
    let inspection = descriptor.inspection_operation();
    let applicability = descriptor.applicability_operation();
    let open = descriptor.open_operation();
    let observation = descriptor.observation_operation();
    inspection == OperationDescriptor::local_inline()
        && applicability == OperationDescriptor::external_observation_inline()
        && open
            == OperationDescriptor::external_observation_deferred_admission()
        && observation
            == OperationDescriptor::external_observation_deferred_admission()
        && descriptor.completion_delivery_operation()
            == OperationDescriptor::local_inline()
        && descriptor.cancellation_operation()
            == OperationDescriptor::local_inline()
        && descriptor.change_operation().is_none()
        && descriptor.close_operation() == OperationDescriptor::local_deferred()
        && delivery_code(open.delivery()) == 0
        && effect_code(open.effect()) == 1
}

const fn delivery_code(value: DeliveryClass) -> u8 {
    match value {
        DeliveryClass::Single => 0,
        DeliveryClass::Stream => 1,
    }
}

const fn effect_code(value: EffectClass) -> u8 {
    match value {
        EffectClass::LocalOnly => 0,
        EffectClass::ExternalObservation => 1,
        EffectClass::ExternalMutation => 2,
    }
}

pub(super) fn stable_descriptor() -> Option<ResourceAccessDescriptor> {
    let capabilities = stable_capabilities()?;
    descriptor(capabilities)
}

fn descriptor(
    capabilities: ResourceCapabilities,
) -> Option<ResourceAccessDescriptor> {
    let contract = selection(200)?;
    let schemas = ResourceValueSchemas::new(
        selection(101)?,
        selection(102)?,
        selection(103)?,
        selection(104)?,
    );
    let queue = match capabilities.change_discovery() {
        ChangeDiscovery::None => 0,
        ChangeDiscovery::Invalidation
        | ChangeDiscovery::OrderedResumableFeed => 4,
    };
    let bounds = access_bounds(capabilities, queue, 4, 1, 1)?;
    let carriers = super::carrier::contracts()?;
    Some(ResourceAccessDescriptor::new(
        contract,
        LifecycleIdentity::new(210, 211, Generation::new(1)),
        LifecycleIdentity::new(220, 221, Generation::new(1)),
        schemas,
        capabilities,
        &carriers,
        &bounds,
    ))
}

fn ready_cases() -> bool {
    stable_ready_case()
        && ready_error_cases()
        && invalidation_ready_case()
        && feed_ready_case()
}

fn stable_ready_case() -> bool {
    let Some(descriptor) = stable_descriptor() else { return false };
    let Some(snapshot) = snapshot(1, 14) else { return false };
    let result = ResourceReadyState::new(
        &descriptor,
        snapshot,
        ResourceChangeCheckpoint::NotRequired,
    );
    let Ok(ready) = result else { return false };
    traits(ready)
        && ready.access().same_generation(snapshot.access())
        && ready.snapshot().same_snapshot(snapshot)
        && ready.change_checkpoint() == ResourceChangeCheckpoint::NotRequired
}

fn ready_error_cases() -> bool {
    let Some(descriptor) = stable_descriptor() else { return false };
    let Some(current) = snapshot(1, 14) else { return false };
    let Some(wrong_revision) = snapshot_with_schema(105) else {
        return false;
    };
    let revision = ResourceReadyState::new(
        &descriptor,
        wrong_revision,
        ResourceChangeCheckpoint::NotRequired,
    );
    let checkpoint = ResourceReadyState::new(
        &descriptor,
        current,
        ResourceChangeCheckpoint::Invalidation(Generation::new(1)),
    );
    ready_error(revision, ResourceReadyError::RevisionSchemaMismatch)
        && ready_error(checkpoint, ResourceReadyError::ChangeCheckpointMismatch)
}

fn snapshot_with_schema(schema: u128) -> Option<ResourceSnapshotStamp> {
    let access = ResourceAccessStamp::new(
        ResourceIdentity::new(11),
        LifecycleIdentity::new(12, 13, Generation::new(1)),
    );
    Some(ResourceSnapshotStamp::new(
        access,
        ResourceRevision::new(selection(schema)?, 14),
        ResourceSnapshotIdentity::new(15),
    ))
}

fn ready_error(
    result: Result<ResourceReadyState, ResourceReadyError>,
    expected: ResourceReadyError,
) -> bool {
    match result {
        Err(actual) => actual == expected && error_traits(&actual),
        Ok(_) => false,
    }
}

fn invalidation_ready_case() -> bool {
    let Some(capabilities) =
        evolving_capabilities(ChangeDiscovery::Invalidation)
    else {
        return false;
    };
    let Some(descriptor) = descriptor(capabilities) else { return false };
    let Some(snapshot) = snapshot(1, 14) else { return false };
    let checkpoint = ResourceChangeCheckpoint::Invalidation(Generation::new(7));
    ResourceReadyState::new(&descriptor, snapshot, checkpoint).is_ok()
        && descriptor.change_operation()
            == Some(OperationDescriptor::external_observation_stream())
}

fn feed_ready_case() -> bool {
    let Some(capabilities) =
        evolving_capabilities(ChangeDiscovery::OrderedResumableFeed)
    else {
        return false;
    };
    let Some(descriptor) = descriptor(capabilities) else { return false };
    let Some(snapshot) = snapshot(1, 14) else { return false };
    let checkpoint = ResourceChangeCheckpoint::Feed {
        subscription: LifecycleIdentity::new(231, 232, Generation::new(1)),
        cursor: 233,
    };
    let result = ResourceReadyState::new(&descriptor, snapshot, checkpoint);
    result.is_ok() && checkpoint_fields(checkpoint)
}

const fn checkpoint_fields(checkpoint: ResourceChangeCheckpoint) -> bool {
    match checkpoint {
        ResourceChangeCheckpoint::Feed { subscription, cursor } => {
            subscription.instance() == 231 && cursor == 233
        }
        ResourceChangeCheckpoint::NotRequired
        | ResourceChangeCheckpoint::Invalidation(_) => false,
    }
}

fn terminal_cases() -> bool {
    open_outcome_cases() && observation_outcome_cases() && close_outcome_cases()
}

fn open_outcome_cases() -> bool {
    let Some(descriptor) = stable_descriptor() else { return false };
    let Some(snapshot) = snapshot(1, 14) else { return false };
    let Some(ready) = ResourceReadyState::new(
        &descriptor,
        snapshot,
        ResourceChangeCheckpoint::NotRequired,
    )
    .ok() else {
        return false;
    };
    let succeeded = ResourceOpenOutcome::ready(ready);
    let failure_cases = OPEN_FAILURES.into_iter().all(|cause| {
        let outcome =
            ResourceOpenOutcome::not_ready(cause, RollbackOutcome::Reverted);
        outcome.status() == ResourceOpenStatus::NotReady
            && outcome.ready_state().is_none()
            && outcome.failure() == Some(cause)
            && outcome.rollback() == Some(RollbackOutcome::Reverted)
    });
    let incomplete = ResourceOpenOutcome::not_ready(
        ResourceOpenFailure::AdapterFailure,
        RollbackOutcome::IncompleteShutdown,
    );
    succeeded.status() == ResourceOpenStatus::Ready
        && succeeded.ready_state().is_some_and(|state| {
            state.snapshot().same_snapshot(ready.snapshot())
        })
        && succeeded.failure().is_none()
        && succeeded.rollback().is_none()
        && failure_cases
        && incomplete.rollback() == Some(RollbackOutcome::IncompleteShutdown)
}

const OPEN_FAILURES: [ResourceOpenFailure; 14] = [
    ResourceOpenFailure::Denied,
    ResourceOpenFailure::Unsupported,
    ResourceOpenFailure::Incompatible,
    ResourceOpenFailure::InvalidDescriptor,
    ResourceOpenFailure::InvalidSnapshot,
    ResourceOpenFailure::Stale,
    ResourceOpenFailure::Unavailable,
    ResourceOpenFailure::Indeterminate,
    ResourceOpenFailure::Pressure,
    ResourceOpenFailure::Cancelled,
    ResourceOpenFailure::Replaced,
    ResourceOpenFailure::Closing,
    ResourceOpenFailure::Closed,
    ResourceOpenFailure::AdapterFailure,
];

fn observation_outcome_cases() -> bool {
    OBSERVATION_FAILURES.into_iter().all(|cause| {
        let outcome =
            reovim_kernel_interface::resource::access::
                ResourceObservationOutcome::not_observed(cause);
        outcome.status()
            == reovim_kernel_interface::resource::access::
                ResourceObservationStatus::NotObserved
            && outcome.observed_state().is_none()
            && outcome.failure() == Some(cause)
            && outcome.into_observed_state().is_none()
    })
}

const OBSERVATION_FAILURES: [ResourceObservationFailure; 12] = [
    ResourceObservationFailure::InvalidRange,
    ResourceObservationFailure::UnsupportedSchema,
    ResourceObservationFailure::StaleAccess,
    ResourceObservationFailure::Unavailable,
    ResourceObservationFailure::Stale,
    ResourceObservationFailure::Indeterminate,
    ResourceObservationFailure::Pressure,
    ResourceObservationFailure::Cancelled,
    ResourceObservationFailure::Replaced,
    ResourceObservationFailure::Closing,
    ResourceObservationFailure::Closed,
    ResourceObservationFailure::AdapterFailure,
];

fn close_outcome_cases() -> bool {
    let Some(zero) = CheckedCount::new(0, ZeroPolicy::Allow).ok() else {
        return false;
    };
    let Some(one) = CheckedCount::new(1, ZeroPolicy::Reject).ok() else {
        return false;
    };
    let invalid = ResourceIncompleteClose::new(ExecutionOutcome::Stopped, zero);
    let nonterminal =
        ResourceIncompleteClose::new(ExecutionOutcome::OwnedNonterminal, zero);
    let release = ResourceIncompleteClose::new(ExecutionOutcome::Stopped, one);
    let (Ok(nonterminal), Ok(release)) = (nonterminal, release) else {
        return false;
    };
    invalid == Err(ResourceCloseError::NoRetainedOwnership)
        && error_traits(&ResourceCloseError::NoRetainedOwnership)
        && nonterminal.execution() == ExecutionOutcome::OwnedNonterminal
        && nonterminal.unresolved().value() == 0
        && release.execution() == ExecutionOutcome::Stopped
        && release.unresolved().value() == 1
        && traits(ResourceCloseOutcome::Closed)
        && traits(ResourceCloseOutcome::Incomplete(nonterminal))
        && traits(ResourceCloseOutcome::Incomplete(release))
        && traits(ResourceCloseOutcome::StaleAdapter)
}
