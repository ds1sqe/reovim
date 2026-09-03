//! Exact compiler-resolved Kernel Interface public items.

pub const fn witness() -> bool {
    bound_items();
    cancellation_items();
    carrier_items_one();
    carrier_items_two();
    clock_items();
    contract_items();
    diagnostic_items_one();
    diagnostic_items_two();
    diagnostic_items_three();
    event_items_one();
    event_items_two();
    identity_items();
    lifecycle_items();
    operation_items();
    trait_items();
    trait_method_items();
    variant_items();
    true
}

const fn bound_items() {
    let _ = (
        ::reovim_kernel_interface::bound::BoundDescriptor::admit_aggregate,
        ::reovim_kernel_interface::bound::BoundDescriptor::admit_per_object,
        ::reovim_kernel_interface::bound::BoundDescriptor::aggregate,
        ::reovim_kernel_interface::bound::BoundDescriptor::arithmetic_rule,
        ::reovim_kernel_interface::bound::BoundDescriptor::charge_owner,
        ::reovim_kernel_interface::bound::BoundDescriptor::new,
        ::reovim_kernel_interface::bound::BoundDescriptor::per_object,
        ::reovim_kernel_interface::bound::BoundDescriptor::unit,
        ::reovim_kernel_interface::bound::BoundDescriptor::validation_order,
        ::reovim_kernel_interface::bound::BoundDescriptor::zero_policy,
        ::reovim_kernel_interface::bound::BoundValue::checked_add,
        ::reovim_kernel_interface::bound::BoundValue::unit,
        ::reovim_kernel_interface::bound::BoundValue::value,
        ::reovim_kernel_interface::bound::CheckedByteCount::checked_add,
        ::reovim_kernel_interface::bound::CheckedByteCount::new,
        ::reovim_kernel_interface::bound::CheckedByteCount::value,
        ::reovim_kernel_interface::bound::CheckedCount::checked_add,
        ::reovim_kernel_interface::bound::CheckedCount::new,
        ::reovim_kernel_interface::bound::CheckedCount::value,
        ::reovim_kernel_interface::bound::CheckedDuration::checked_add,
        ::reovim_kernel_interface::bound::CheckedDuration::from_nanoseconds,
        ::reovim_kernel_interface::bound::CheckedDuration::nanoseconds,
    );
}

const fn carrier_items_one() {
    let _ = (
        ::reovim_kernel_interface::carrier::CarrierDescriptorError::invocation,
        ::reovim_kernel_interface::carrier::CarrierDescriptorError::memory,
        ::reovim_kernel_interface::carrier::CarrierDescriptorError::unwind,
        ::reovim_kernel_interface::carrier::CarrierSafetyDescriptor::
            concurrency,
        ::reovim_kernel_interface::carrier::CarrierSafetyDescriptor::invocation,
        ::reovim_kernel_interface::carrier::CarrierSafetyDescriptor::memory,
        ::reovim_kernel_interface::carrier::CarrierSafetyDescriptor::mismatch,
        ::reovim_kernel_interface::carrier::CarrierSafetyDescriptor::new,
        ::reovim_kernel_interface::carrier::CarrierSafetyDescriptor::
            schema_versions,
        ::reovim_kernel_interface::carrier::CarrierSafetyDescriptor::type_id,
        ::reovim_kernel_interface::carrier::CarrierSafetyDescriptor::
            unsafe_boundary,
        ::reovim_kernel_interface::carrier::ConcurrencySafety::new,
        ::reovim_kernel_interface::carrier::ConcurrencySafety::sharing,
        ::reovim_kernel_interface::carrier::ConcurrencySafety::thread_transfer,
    );
}

const fn carrier_items_two() {
    let _ = (
        ::reovim_kernel_interface::carrier::InvocationSafety::invocation,
        ::reovim_kernel_interface::carrier::InvocationSafety::new,
        ::reovim_kernel_interface::carrier::InvocationSafety::unwind,
        ::reovim_kernel_interface::carrier::MemorySafety::borrowing,
        ::reovim_kernel_interface::carrier::MemorySafety::copying,
        ::reovim_kernel_interface::carrier::MemorySafety::dropping,
        ::reovim_kernel_interface::carrier::MemorySafety::lifetime,
        ::reovim_kernel_interface::carrier::MemorySafety::new,
        ::reovim_kernel_interface::carrier::MemorySafety::ownership,
    );
}

const fn contract_items() {
    let _ = (
        ::reovim_kernel_interface::contract::CompatibleVersionRange::contains,
        ::reovim_kernel_interface::contract::CompatibleVersionRange::maximum,
        ::reovim_kernel_interface::contract::CompatibleVersionRange::minimum,
        ::reovim_kernel_interface::contract::CompatibleVersionRange::new,
        ::reovim_kernel_interface::contract::ContractId::new,
        ::reovim_kernel_interface::contract::ContractId::value,
        ::reovim_kernel_interface::contract::ContractVersion::new,
        ::reovim_kernel_interface::contract::ContractVersion::value,
        ::reovim_kernel_interface::contract::VersionRangeError::maximum,
        ::reovim_kernel_interface::contract::VersionRangeError::minimum,
    );
}

const fn identity_items() {
    let _ = (
        ::reovim_kernel_interface::identity::IncarnationId::new,
        ::reovim_kernel_interface::identity::IncarnationId::value,
        ::reovim_kernel_interface::identity::InstanceId::new,
        ::reovim_kernel_interface::identity::InstanceId::value,
    );
}

const fn diagnostic_items_one() {
    let _ = (
        ::reovim_kernel_interface::diagnostic::
            DiagnosticOutputCompletionBounds::activation,
        ::reovim_kernel_interface::diagnostic::
            DiagnosticOutputCompletionBounds::new,
        ::reovim_kernel_interface::diagnostic::
            DiagnosticOutputCompletionBounds::shutdown,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            contract,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            degraded_operation_is_safe,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            lifecycle,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            limits,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            negotiate,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::new,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            operation,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            owner,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            readiness_predicate,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            record_safety,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            record_type,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            shutdown_policy,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            source_stream,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor::
            validate_record,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputLimits::
            byte_capacity,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputLimits::
            completion,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputLimits::new,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputLimits::output,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputLimits::
            record_capacity,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputLimits::
            shutdown_report_record_bytes,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputLimits::
            shutdown_report_records,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputLimits::
            submission_work,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputLimits::
            temporary_bytes,
    );
}

const fn diagnostic_items_two() {
    let _ = (
        ::reovim_kernel_interface::diagnostic::DiagnosticRecordContract::new,
        ::reovim_kernel_interface::diagnostic::DiagnosticRecordContract::
            safety,
        ::reovim_kernel_interface::diagnostic::DiagnosticRecordContract::
            type_id,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputRecord::bind,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputRecord::bytes,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputRecord::output,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputRecord::sequence,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputRecord::size,
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputRecord::stream,
        ::reovim_kernel_interface::diagnostic::DiagnosticRecordRejection::bytes,
        ::reovim_kernel_interface::diagnostic::DiagnosticRecordRejection::cause,
        ::reovim_kernel_interface::diagnostic::DiagnosticRecordRejection::
            into_bytes,
        ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionOutcome::
            accepted,
        ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionOutcome::
            closed,
        ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionOutcome::
            dropped,
        ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionOutcome::
            into_returned_record,
        ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionOutcome::
            returned_record,
        ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionOutcome::
            status,
    );
}

const fn diagnostic_items_three() {
    let _ = (
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationResult::
            cause,
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationResult::
            certainty,
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationResult::
            confirmed_prefix,
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationResult::
            cut,
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationResult::
            new,
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationResult::
            output,
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationResult::
            sequence,
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationResult::
            stream,
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationResult::
            total_bytes,
    );
}

const fn cancellation_items() {
    let _ = (
        ::reovim_kernel_interface::cancellation::CancellationOutcome::cause,
        ::reovim_kernel_interface::cancellation::CancellationOutcome::execution,
        ::reovim_kernel_interface::cancellation::CancellationOutcome::new,
        ::reovim_kernel_interface::cancellation::ExecutionOutcome::is_terminal,
        ::reovim_kernel_interface::cancellation::InstallationOutcome::
            permits_installation,
    );
}

const fn clock_items() {
    let _ = (
        ::reovim_kernel_interface::clock::ClockDescriptor::arithmetic_operation,
        ::reovim_kernel_interface::clock::ClockDescriptor::contract,
        ::reovim_kernel_interface::clock::ClockDescriptor::inspection_operation,
        ::reovim_kernel_interface::clock::ClockDescriptor::instant,
        ::reovim_kernel_interface::clock::ClockDescriptor::instant_safety,
        ::reovim_kernel_interface::clock::ClockDescriptor::lifecycle,
        ::reovim_kernel_interface::clock::ClockDescriptor::maximum_elapsed,
        ::reovim_kernel_interface::clock::ClockDescriptor::negotiate,
        ::reovim_kernel_interface::clock::ClockDescriptor::new,
        ::reovim_kernel_interface::clock::ClockDescriptor::
            observation_operation,
        ::reovim_kernel_interface::clock::ClockDescriptor::owner,
        ::reovim_kernel_interface::clock::ClockDescriptor::readiness_predicate,
        ::reovim_kernel_interface::clock::ClockDescriptor::resolution,
        ::reovim_kernel_interface::clock::ClockDescriptor::validate_stamp,
        ::reovim_kernel_interface::clock::MonotonicInstant::compare,
        ::reovim_kernel_interface::clock::MonotonicInstant::duration_since,
    );
}

const fn event_items_one() {
    let _ = (
        ::reovim_kernel_interface::event::CanonicalEventBytes::bytes,
        ::reovim_kernel_interface::event::CanonicalEventBytes::new,
        ::reovim_kernel_interface::event::CanonicalEventBytes::size,
        ::reovim_kernel_interface::event::CanonicalEventRecordRef::bytes,
        ::reovim_kernel_interface::event::CanonicalEventRecordRef::family,
        ::reovim_kernel_interface::event::CanonicalEventRecordRef::
            schema_version,
        ::reovim_kernel_interface::event::CanonicalEventRecordRef::sequence,
        ::reovim_kernel_interface::event::CanonicalEventRecordRef::size,
        ::reovim_kernel_interface::event::CanonicalEventRecordRef::stream,
        ::reovim_kernel_interface::event::EVENT_RING_MAX_BYTES,
        ::reovim_kernel_interface::event::EventAdmissionOutcome::
            accepted_sequence,
        ::reovim_kernel_interface::event::EventAdmissionOutcome::
            from_clock_error,
        ::reovim_kernel_interface::event::EventAdmissionScope::correlation,
        ::reovim_kernel_interface::event::EventAdmissionScope::lifecycle_owner,
        ::reovim_kernel_interface::event::EventAdmissionScope::new,
        ::reovim_kernel_interface::event::EventAdmissionScope::producer,
        ::reovim_kernel_interface::event::EventAdmissionScope::stream,
        ::reovim_kernel_interface::event::EventCursor::exhausted,
        ::reovim_kernel_interface::event::EventCursor::new,
        ::reovim_kernel_interface::event::EventCursor::position,
        ::reovim_kernel_interface::event::EventCursor::stream_incarnation,
        ::reovim_kernel_interface::event::EventSequence::first,
        ::reovim_kernel_interface::event::EventSequence::is_before,
        ::reovim_kernel_interface::event::EventSequence::new,
        ::reovim_kernel_interface::event::EventSequence::next,
        ::reovim_kernel_interface::event::EventSequence::value,
        ::reovim_kernel_interface::event::EventTimestamp::elapsed_nanoseconds,
        ::reovim_kernel_interface::event::EventTimestamp::instant,
        ::reovim_kernel_interface::event::RemediationRef::catalog,
        ::reovim_kernel_interface::event::RemediationRef::catalog_version,
        ::reovim_kernel_interface::event::RemediationRef::code,
        ::reovim_kernel_interface::event::RetainedSequenceRange::earliest,
        ::reovim_kernel_interface::event::RetainedSequenceRange::empty,
        ::reovim_kernel_interface::event::RetainedSequenceRange::is_empty,
        ::reovim_kernel_interface::event::RetainedSequenceRange::latest,
        ::reovim_kernel_interface::event::RetainedSequenceRange::non_empty,
        ::reovim_kernel_interface::event::STARTUP_CANONICAL_MAX_BYTES,
        ::reovim_kernel_interface::event::SaturatingCounter::from_parts,
        ::reovim_kernel_interface::event::SaturatingCounter::increment,
        ::reovim_kernel_interface::event::SaturatingCounter::saturated,
        ::reovim_kernel_interface::event::SaturatingCounter::value,
        ::reovim_kernel_interface::event::SaturatingCounter::zero,
        ::reovim_kernel_interface::event::SnapshotMetadata::charge_owner,
        ::reovim_kernel_interface::event::SnapshotMetadata::coverage,
        ::reovim_kernel_interface::event::SnapshotMetadata::new,
        ::reovim_kernel_interface::event::SnapshotMetadata::next_cursor,
        ::reovim_kernel_interface::event::SnapshotMetadata::retained,
        ::reovim_kernel_interface::event::SnapshotMetadata::statistics,
        ::reovim_kernel_interface::event::SnapshotMetadata::stream_incarnation,
        ::reovim_kernel_interface::event::SnapshotRequest::admit_record_bytes,
        ::reovim_kernel_interface::event::SnapshotRequest::charge_owner,
        ::reovim_kernel_interface::event::SnapshotRequest::maximum_bytes,
        ::reovim_kernel_interface::event::SnapshotRequest::maximum_records,
        ::reovim_kernel_interface::event::SnapshotRequest::new,
        ::reovim_kernel_interface::event::SnapshotStatistics::copied_bytes,
        ::reovim_kernel_interface::event::SnapshotStatistics::copied_records,
        ::reovim_kernel_interface::event::SnapshotStatistics::evicted_bytes,
    );
}

const fn event_items_two() {
    let _ = (
        ::reovim_kernel_interface::event::SnapshotStatistics::evicted_records,
        ::reovim_kernel_interface::event::SnapshotStatistics::new,
        ::reovim_kernel_interface::event::StartupComponentCounts::active,
        ::reovim_kernel_interface::event::StartupComponentCounts::degraded,
        ::reovim_kernel_interface::event::StartupComponentCounts::new,
        ::reovim_kernel_interface::event::StartupComponentCounts::total,
        ::reovim_kernel_interface::event::StartupComponentCounts::unavailable,
        ::reovim_kernel_interface::event::StartupEventDescriptor::
            admission_operation,
        ::reovim_kernel_interface::event::StartupEventDescriptor::clock,
        ::reovim_kernel_interface::event::StartupEventDescriptor::
            canonical_reference_safety,
        ::reovim_kernel_interface::event::StartupEventDescriptor::contract,
        ::reovim_kernel_interface::event::StartupEventDescriptor::
            family_schema_version,
        ::reovim_kernel_interface::event::StartupEventDescriptor::identities,
        ::reovim_kernel_interface::event::StartupEventDescriptor::limits,
        ::reovim_kernel_interface::event::StartupEventDescriptor::negotiate,
        ::reovim_kernel_interface::event::StartupEventDescriptor::new,
        ::reovim_kernel_interface::event::StartupEventDescriptor::
            observe_timestamp::<
            dyn ::reovim_kernel_interface::clock::MonotonicClock,
        >,
        ::reovim_kernel_interface::event::StartupEventDescriptor::
            remediation_catalog_version,
        ::reovim_kernel_interface::event::StartupEventDescriptor::
            snapshot_operation,
        ::reovim_kernel_interface::event::StartupEventIdentities::contract,
        ::reovim_kernel_interface::event::StartupEventIdentities::
            canonical_reference,
        ::reovim_kernel_interface::event::StartupEventIdentities::family,
        ::reovim_kernel_interface::event::StartupEventIdentities::
            lifecycle_owner,
        ::reovim_kernel_interface::event::StartupEventIdentities::new,
        ::reovim_kernel_interface::event::StartupEventIdentities::
            remediation_catalog,
        ::reovim_kernel_interface::event::StartupEventIdentities::stream,
        ::reovim_kernel_interface::event::StartupEventLimits::canonical_bytes,
        ::reovim_kernel_interface::event::StartupEventLimits::component_counts,
        ::reovim_kernel_interface::event::StartupEventLimits::new,
        ::reovim_kernel_interface::event::StartupEventLimits::ring_bytes,
        ::reovim_kernel_interface::event::StartupEventLimits::snapshot_bytes,
        ::reovim_kernel_interface::event::StartupEventLimits::snapshot_records,
        ::reovim_kernel_interface::event::StartupEventLimits::synchronous_work,
        ::reovim_kernel_interface::event::StartupEventLimits::temporary_bytes,
        ::reovim_kernel_interface::event::StartupEventRecord::canonical,
        ::reovim_kernel_interface::event::StartupEventRecord::canonical_ref,
        ::reovim_kernel_interface::event::StartupEventRecord::family,
        ::reovim_kernel_interface::event::StartupEventRecord::new,
        ::reovim_kernel_interface::event::StartupEventRecord::proposal,
        ::reovim_kernel_interface::event::StartupEventRecord::schema_version,
        ::reovim_kernel_interface::event::StartupEventRecord::scope,
        ::reovim_kernel_interface::event::StartupEventRecord::sequence,
        ::reovim_kernel_interface::event::StartupEventRecord::timestamp,
        ::reovim_kernel_interface::event::StartupProposal::actionability,
        ::reovim_kernel_interface::event::StartupProposal::bootstrap_health,
        ::reovim_kernel_interface::event::StartupProposal::counts,
        ::reovim_kernel_interface::event::StartupProposal::lifecycle_owner,
        ::reovim_kernel_interface::event::StartupProposal::new,
        ::reovim_kernel_interface::event::StartupProposal::readiness,
        ::reovim_kernel_interface::event::StartupProposal::severity,
        ::reovim_kernel_interface::event::StartupProposal::validate_for,
        ::reovim_kernel_interface::event::StartupSnapshot::metadata,
        ::reovim_kernel_interface::event::StartupSnapshot::new,
        ::reovim_kernel_interface::event::StartupSnapshot::records,
    );
}

const fn lifecycle_items() {
    let _ = (
        ::reovim_kernel_interface::lifecycle::Generation::new,
        ::reovim_kernel_interface::lifecycle::Generation::value,
        ::reovim_kernel_interface::lifecycle::LifecycleIdentity::generation,
        ::reovim_kernel_interface::lifecycle::LifecycleIdentity::incarnation,
        ::reovim_kernel_interface::lifecycle::LifecycleIdentity::instance,
        ::reovim_kernel_interface::lifecycle::LifecycleIdentity::new,
        ::reovim_kernel_interface::lifecycle::LifecycleIdentity::
            same_generation,
        ::reovim_kernel_interface::lifecycle::LifecycleIdentity::
            same_incarnation,
    );
}

const fn operation_items() {
    let _ = (
        ::reovim_kernel_interface::operation::ContractSelection::identity,
        ::reovim_kernel_interface::operation::ContractSelection::maximum,
        ::reovim_kernel_interface::operation::ContractSelection::minimum,
        ::reovim_kernel_interface::operation::ContractSelection::new,
        ::reovim_kernel_interface::operation::ContractSelection::selected,
        ::reovim_kernel_interface::operation::ContractSelection::supports,
        ::reovim_kernel_interface::operation::OperationDescriptor::callback,
        ::reovim_kernel_interface::operation::OperationDescriptor::completion,
        ::reovim_kernel_interface::operation::OperationDescriptor::delivery,
        ::reovim_kernel_interface::operation::OperationDescriptor::effect,
        ::reovim_kernel_interface::operation::OperationDescriptor::
            external_observation_inline,
        ::reovim_kernel_interface::operation::OperationDescriptor::invocation,
        ::reovim_kernel_interface::operation::OperationDescriptor::local_inline,
        ::reovim_kernel_interface::operation::OperationDescriptor::new,
        ::reovim_kernel_interface::operation::OperationDescriptor::reentrancy,
    );
}

const fn trait_items() {
    let _: Option<&dyn ::reovim_kernel_interface::clock::MonotonicClock> = None;
    let _: Option<
        &dyn ::reovim_kernel_interface::diagnostic::DiagnosticOutput,
    > = None;
    let _: Option<
        &dyn ::reovim_kernel_interface::event::StartupEventAdmission,
    > = None;
}

const fn trait_method_items() {
    let _ = (
        <dyn ::reovim_kernel_interface::clock::MonotonicClock>::descriptor,
        <dyn ::reovim_kernel_interface::clock::MonotonicClock>::observe,
        <dyn ::reovim_kernel_interface::diagnostic::DiagnosticOutput>::
            descriptor,
        <dyn ::reovim_kernel_interface::diagnostic::DiagnosticOutput>::submit,
        <dyn ::reovim_kernel_interface::event::StartupEventAdmission>::admit,
        <dyn ::reovim_kernel_interface::event::StartupEventAdmission>::
            descriptor,
        <dyn ::reovim_kernel_interface::event::StartupEventAdmission>::scope,
    );
}

const fn variant_items() {
    let _ = (
        bound_arithmetic_rule_variants,
        bound_error_variants,
        bound_unit_variants,
        bound_value_variants,
        bound_validation_order_variants,
        bound_zero_policy_variants,
        cancellation_cancellation_cause_variants,
        cancellation_cancellation_state_variants,
        cancellation_deadline_state_variants,
        cancellation_execution_outcome_variants,
        cancellation_fence_cause_variants,
        cancellation_installation_outcome_variants,
        carrier_borrow_model_variants,
        carrier_descriptor_error_variants,
        carrier_copy_model_variants,
        carrier_drop_model_variants,
        carrier_invocation_model_variants,
        carrier_lifetime_model_variants,
        carrier_memory_coherence_error_variants,
        carrier_ownership_model_variants,
        carrier_sharing_model_variants,
        carrier_thread_transfer_variants,
        carrier_type_mismatch_variants,
        carrier_unsafe_boundary_variants,
        carrier_unwind_model_variants,
        clock_clock_error_variants,
        clock_clock_readiness_predicate_variants,
        clock_instant_ordering_variants,
        diagnostic_closed_cause_variants,
        diagnostic_bound_error_variants,
        diagnostic_destination_cause_variants,
        diagnostic_destination_cut_variants,
        diagnostic_destination_error_variants,
        diagnostic_drop_cause_variants,
        diagnostic_operation_variants,
        diagnostic_output_error_variants,
        diagnostic_readiness_predicate_variants,
        diagnostic_record_error_variants,
        diagnostic_shutdown_policy_variants,
        diagnostic_submission_outcome_variants,
        diagnostic_submission_status_variants,
        event_actionability_variants,
        event_bootstrap_health_variants,
        event_bound_error_variants,
        event_cursor_position_variants,
        event_event_admission_outcome_variants,
        event_event_closed_cause_variants,
        event_event_drop_cause_variants,
        event_event_error_variants,
        event_event_reject_cause_variants,
        event_remediation_code_variants,
        event_severity_variants,
        event_snapshot_coverage_variants,
        effect_effect_certainty_variants,
        lifecycle_lifecycle_state_variants,
        lifecycle_readiness_outcome_variants,
        lifecycle_rollback_outcome_variants,
        lifecycle_shutdown_outcome_variants,
        lifecycle_work_disposition_variants,
        operation_callback_class_variants,
        operation_completion_class_variants,
        operation_contract_selection_error_variants,
        operation_delivery_class_variants,
        operation_effect_class_variants,
        operation_invocation_class_variants,
        operation_operation_descriptor_error_variants,
        operation_reentrancy_class_variants,
    );
}

const fn bound_arithmetic_rule_variants(
    value: ::reovim_kernel_interface::bound::ArithmeticRule,
) {
    match value {
        ::reovim_kernel_interface::bound::ArithmeticRule::Checked => {}
    }
}

const fn bound_error_variants(
    value: ::reovim_kernel_interface::bound::BoundError,
) {
    match value {
        ::reovim_kernel_interface::bound::BoundError::ZeroRejected { .. }
        | ::reovim_kernel_interface::bound::BoundError::ArithmeticOverflow {
            ..
        }
        | ::reovim_kernel_interface::bound::BoundError::UnitMismatch { .. }
        | ::reovim_kernel_interface::bound::BoundError::IncoherentDescriptor {
            ..
        }
        | ::reovim_kernel_interface::bound::BoundError::Exhausted { .. } => {}
    }
}

const fn bound_unit_variants(
    value: ::reovim_kernel_interface::bound::BoundUnit,
) {
    match value {
        ::reovim_kernel_interface::bound::BoundUnit::Count
        | ::reovim_kernel_interface::bound::BoundUnit::Bytes
        | ::reovim_kernel_interface::bound::BoundUnit::DurationNanoseconds => {}
    }
}

const fn bound_value_variants(
    value: ::reovim_kernel_interface::bound::BoundValue,
) {
    match value {
        ::reovim_kernel_interface::bound::BoundValue::Count(_)
        | ::reovim_kernel_interface::bound::BoundValue::Bytes(_)
        | ::reovim_kernel_interface::bound::BoundValue::Duration(_) => {}
    }
}

const fn bound_validation_order_variants(
    value: ::reovim_kernel_interface::bound::ValidationOrder,
) {
    match value {
        ::reovim_kernel_interface::bound::ValidationOrder::
            BeforeTraversalAndAllocation => {}
    }
}

const fn bound_zero_policy_variants(
    value: ::reovim_kernel_interface::bound::ZeroPolicy,
) {
    match value {
        ::reovim_kernel_interface::bound::ZeroPolicy::Allow
        | ::reovim_kernel_interface::bound::ZeroPolicy::Reject => {}
    }
}

const fn carrier_borrow_model_variants(
    value: ::reovim_kernel_interface::carrier::BorrowModel,
) {
    match value {
        ::reovim_kernel_interface::carrier::BorrowModel::Forbidden
        | ::reovim_kernel_interface::carrier::BorrowModel::Shared
        | ::reovim_kernel_interface::carrier::BorrowModel::Exclusive => {}
    }
}

const fn carrier_descriptor_error_variants(
    value: ::reovim_kernel_interface::carrier::CarrierDescriptorError,
) {
    match value {
        ::reovim_kernel_interface::carrier::CarrierDescriptorError::Memory(_)
        | ::reovim_kernel_interface::carrier::CarrierDescriptorError::
            InvocationUnwind { .. } => {}
    }
}

const fn carrier_copy_model_variants(
    value: ::reovim_kernel_interface::carrier::CopyModel,
) {
    match value {
        ::reovim_kernel_interface::carrier::CopyModel::MoveOnly
        | ::reovim_kernel_interface::carrier::CopyModel::Copy => {}
    }
}

const fn carrier_drop_model_variants(
    value: ::reovim_kernel_interface::carrier::DropModel,
) {
    match value {
        ::reovim_kernel_interface::carrier::DropModel::Trivial
        | ::reovim_kernel_interface::carrier::DropModel::ReleasesOwnedState => {
        }
    }
}

const fn carrier_invocation_model_variants(
    value: ::reovim_kernel_interface::carrier::InvocationModel,
) {
    match value {
        ::reovim_kernel_interface::carrier::InvocationModel::DataOnly
        | ::reovim_kernel_interface::carrier::InvocationModel::NoUserCallback
        | ::reovim_kernel_interface::carrier::InvocationModel::UserCallback => {
        }
    }
}

const fn carrier_lifetime_model_variants(
    value: ::reovim_kernel_interface::carrier::LifetimeModel,
) {
    match value {
        ::reovim_kernel_interface::carrier::LifetimeModel::Value
        | ::reovim_kernel_interface::carrier::LifetimeModel::InstanceBound
        | ::reovim_kernel_interface::carrier::LifetimeModel::IncarnationBound =>
            {}
    }
}

const fn carrier_memory_coherence_error_variants(
    value: ::reovim_kernel_interface::carrier::MemoryCoherenceError,
) {
    match value {
        ::reovim_kernel_interface::carrier::MemoryCoherenceError::
            BorrowedWithoutBorrow
        | ::reovim_kernel_interface::carrier::MemoryCoherenceError::
            BorrowedValueLifetime
        | ::reovim_kernel_interface::carrier::MemoryCoherenceError::
            BorrowedReleasesOwnedState
        | ::reovim_kernel_interface::carrier::MemoryCoherenceError::
            CopyDuplicatesReleaseAuthority => {}
    }
}

const fn carrier_ownership_model_variants(
    value: ::reovim_kernel_interface::carrier::OwnershipModel,
) {
    match value {
        ::reovim_kernel_interface::carrier::OwnershipModel::Owned
        | ::reovim_kernel_interface::carrier::OwnershipModel::Borrowed => {}
    }
}

const fn carrier_sharing_model_variants(
    value: ::reovim_kernel_interface::carrier::SharingModel,
) {
    match value {
        ::reovim_kernel_interface::carrier::SharingModel::Exclusive
        | ::reovim_kernel_interface::carrier::SharingModel::Immutable
        | ::reovim_kernel_interface::carrier::SharingModel::Synchronized => {}
    }
}

const fn carrier_thread_transfer_variants(
    value: ::reovim_kernel_interface::carrier::ThreadTransfer,
) {
    match value {
        ::reovim_kernel_interface::carrier::ThreadTransfer::Transferable
        | ::reovim_kernel_interface::carrier::ThreadTransfer::ThreadAffine => {}
    }
}

const fn carrier_type_mismatch_variants(
    value: ::reovim_kernel_interface::carrier::TypeMismatch,
) {
    match value {
        ::reovim_kernel_interface::carrier::TypeMismatch::Reject => {}
    }
}

const fn carrier_unsafe_boundary_variants(
    value: ::reovim_kernel_interface::carrier::UnsafeBoundary,
) {
    match value {
        ::reovim_kernel_interface::carrier::UnsafeBoundary::None
        | ::reovim_kernel_interface::carrier::UnsafeBoundary::Reviewed(_) => {}
    }
}

const fn carrier_unwind_model_variants(
    value: ::reovim_kernel_interface::carrier::UnwindModel,
) {
    match value {
        ::reovim_kernel_interface::carrier::UnwindModel::NotInvocable
        | ::reovim_kernel_interface::carrier::UnwindModel::Contained
        | ::reovim_kernel_interface::carrier::UnwindModel::
            ForbiddenAcrossBoundary => {}
    }
}

const fn diagnostic_bound_error_variants(
    value: ::reovim_kernel_interface::diagnostic::BoundError,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::BoundError::ArithmeticOverflow {
            ..
        }
        | ::reovim_kernel_interface::diagnostic::BoundError::Exhausted {
            ..
        }
        | ::reovim_kernel_interface::diagnostic::BoundError::
            IncoherentDescriptor { .. }
        | ::reovim_kernel_interface::diagnostic::BoundError::UnitMismatch {
            ..
        }
        | ::reovim_kernel_interface::diagnostic::BoundError::ZeroRejected {
            ..
        } => {}
    }
}

const fn diagnostic_closed_cause_variants(
    value: ::reovim_kernel_interface::diagnostic::DiagnosticClosedCause,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::DiagnosticClosedCause::Closing
        | ::reovim_kernel_interface::diagnostic::DiagnosticClosedCause::Closed
        | ::reovim_kernel_interface::diagnostic::DiagnosticClosedCause::Replaced
        | ::reovim_kernel_interface::diagnostic::DiagnosticClosedCause::
            IncompleteShutdown => {}
    }
}

const fn diagnostic_destination_cause_variants(
    value: ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause::
            Success
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause::
            ClosingDiscard
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause::
            ReplacementDiscard
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause::
            Cancellation
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause::
            ShutdownDeadline
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause::
            WriteZero
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause::
            WriteFailure
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause::
            RetryExhaustion
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause::
            WriterContractViolation
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause::
            InternalFailureBeforeCall
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause::
            InternalFailureInCall
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause::
            SupervisorReaping => {}
    }
}

const fn diagnostic_destination_cut_variants(
    value: ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCut,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCut::
            BeforeCall
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCut::
            InCall
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCut::
            Returned => {}
    }
}

const fn diagnostic_destination_error_variants(
    value: ::reovim_kernel_interface::diagnostic::DiagnosticDestinationError,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationError::
            PrefixExceedsRecord { .. }
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationError::
            CauseCutMismatch { .. }
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationError::
            CertaintyMismatch { .. }
        | ::reovim_kernel_interface::diagnostic::DiagnosticDestinationError::
            PrefixMismatch { .. } => {}
    }
}

const fn diagnostic_drop_cause_variants(
    value: ::reovim_kernel_interface::diagnostic::DiagnosticDropCause,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::DiagnosticDropCause::
            InvalidRecord
        | ::reovim_kernel_interface::diagnostic::DiagnosticDropCause::
            BoundExceeded
        | ::reovim_kernel_interface::diagnostic::DiagnosticDropCause::NotReady
        | ::reovim_kernel_interface::diagnostic::DiagnosticDropCause::
            SynchronizationPressure
        | ::reovim_kernel_interface::diagnostic::DiagnosticDropCause::
            RecordCapacity
        | ::reovim_kernel_interface::diagnostic::DiagnosticDropCause::
            ByteCapacity
        | ::reovim_kernel_interface::diagnostic::DiagnosticDropCause::
            ChargeOverflow
        | ::reovim_kernel_interface::diagnostic::DiagnosticDropCause::
            DestinationUnavailable
        | ::reovim_kernel_interface::diagnostic::DiagnosticDropCause::
            HealthUnavailable
        | ::reovim_kernel_interface::diagnostic::DiagnosticDropCause::
            InternalFailure => {}
    }
}

const fn diagnostic_operation_variants(
    value: ::reovim_kernel_interface::diagnostic::DiagnosticOutputOperation,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputOperation::
            DescriptorInspection
        | ::reovim_kernel_interface::diagnostic::DiagnosticOutputOperation::
            RecordPreparation
        | ::reovim_kernel_interface::diagnostic::DiagnosticOutputOperation::
            Submission
        | ::reovim_kernel_interface::diagnostic::DiagnosticOutputOperation::
            Activation
        | ::reovim_kernel_interface::diagnostic::DiagnosticOutputOperation::
            Shutdown => {}
    }
}

const fn diagnostic_output_error_variants(
    value: ::reovim_kernel_interface::diagnostic::DiagnosticOutputError,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputError::Bound(_)
        | ::reovim_kernel_interface::diagnostic::DiagnosticOutputError::
            LimitsLifecycleMismatch
        | ::reovim_kernel_interface::diagnostic::DiagnosticOutputError::
            IncoherentCarrier
        | ::reovim_kernel_interface::diagnostic::DiagnosticOutputError::
            UnsupportedContract
        | ::reovim_kernel_interface::diagnostic::DiagnosticOutputError::
            IncompatibleContract => {}
    }
}

const fn diagnostic_readiness_predicate_variants(
    value: ::reovim_kernel_interface::diagnostic::
        DiagnosticOutputReadinessPredicate,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::
            DiagnosticOutputReadinessPredicate::CurrentGenerationAdmission => {}
    }
}

const fn diagnostic_record_error_variants(
    value: ::reovim_kernel_interface::diagnostic::DiagnosticRecordError,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::DiagnosticRecordError::Bound(_)
        | ::reovim_kernel_interface::diagnostic::DiagnosticRecordError::
            SourceStreamMismatch
        | ::reovim_kernel_interface::diagnostic::DiagnosticRecordError::
            LengthOverflow
        | ::reovim_kernel_interface::diagnostic::DiagnosticRecordError::
            LengthMismatch
        | ::reovim_kernel_interface::diagnostic::DiagnosticRecordError::
            CanonicalMismatch => {}
    }
}

const fn diagnostic_shutdown_policy_variants(
    value: ::reovim_kernel_interface::diagnostic::DiagnosticShutdownPolicy,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::DiagnosticShutdownPolicy::Drain
        | ::reovim_kernel_interface::diagnostic::DiagnosticShutdownPolicy::
            Discard => {}
    }
}

const fn diagnostic_submission_outcome_variants(
    value: &::reovim_kernel_interface::diagnostic::DiagnosticSubmissionOutcome,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionOutcome::
            Accepted
        | ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionOutcome::
            Dropped { .. }
        | ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionOutcome::
            Closed { .. } => {}
    }
}

const fn diagnostic_submission_status_variants(
    value: ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionStatus,
) {
    match value {
        ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionStatus::
            Accepted
        | ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionStatus::
            Dropped
        | ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionStatus::
            Closed => {}
    }
}

const fn cancellation_cancellation_cause_variants(
    value: ::reovim_kernel_interface::cancellation::CancellationCause,
) {
    match value {
        ::reovim_kernel_interface::cancellation::CancellationCause::
            ExplicitRequest
        | ::reovim_kernel_interface::cancellation::CancellationCause::
            Replacement
        | ::reovim_kernel_interface::cancellation::CancellationCause::
            OwnerClosing => {}
    }
}

const fn cancellation_cancellation_state_variants(
    value: ::reovim_kernel_interface::cancellation::CancellationState,
) {
    match value {
        ::reovim_kernel_interface::cancellation::CancellationState::Open
        | ::reovim_kernel_interface::cancellation::CancellationState::
            Requested(_) => {}
    }
}

const fn cancellation_deadline_state_variants(
    value: ::reovim_kernel_interface::cancellation::DeadlineState,
) {
    match value {
        ::reovim_kernel_interface::cancellation::DeadlineState::Pending
        | ::reovim_kernel_interface::cancellation::DeadlineState::Reached => {}
    }
}

const fn cancellation_execution_outcome_variants(
    value: ::reovim_kernel_interface::cancellation::ExecutionOutcome,
) {
    match value {
        ::reovim_kernel_interface::cancellation::ExecutionOutcome::
            NeverStarted
        | ::reovim_kernel_interface::cancellation::ExecutionOutcome::
            OwnedNonterminal
        | ::reovim_kernel_interface::cancellation::ExecutionOutcome::Stopped
        | ::reovim_kernel_interface::cancellation::ExecutionOutcome::
            ContainingProcessReaped => {}
    }
}

const fn cancellation_fence_cause_variants(
    value: ::reovim_kernel_interface::cancellation::FenceCause,
) {
    match value {
        ::reovim_kernel_interface::cancellation::FenceCause::Cancellation
        | ::reovim_kernel_interface::cancellation::FenceCause::Deadline
        | ::reovim_kernel_interface::cancellation::FenceCause::CallerTimeout
        | ::reovim_kernel_interface::cancellation::FenceCause::Replacement
        | ::reovim_kernel_interface::cancellation::FenceCause::Closing
        | ::reovim_kernel_interface::cancellation::FenceCause::Closed => {}
    }
}

const fn cancellation_installation_outcome_variants(
    value: ::reovim_kernel_interface::cancellation::InstallationOutcome,
) {
    match value {
        ::reovim_kernel_interface::cancellation::InstallationOutcome::
            Current
        | ::reovim_kernel_interface::cancellation::InstallationOutcome::
            Fenced(_) => {}
    }
}

const fn clock_clock_error_variants(
    value: ::reovim_kernel_interface::clock::ClockError,
) {
    match value {
        ::reovim_kernel_interface::clock::ClockError::IncompatibleContract
        | ::reovim_kernel_interface::clock::ClockError::UnsupportedContract
        | ::reovim_kernel_interface::clock::ClockError::DescriptorMismatch
        | ::reovim_kernel_interface::clock::ClockError::IncoherentDescriptor {
            resolution_nanoseconds: _,
            maximum_elapsed_nanoseconds: _,
        }
        | ::reovim_kernel_interface::clock::ClockError::IncoherentCarrier
        | ::reovim_kernel_interface::clock::ClockError::NotReady
        | ::reovim_kernel_interface::clock::ClockError::CrossClock
        | ::reovim_kernel_interface::clock::ClockError::StaleGeneration
        | ::reovim_kernel_interface::clock::ClockError::ObservedRegression
        | ::reovim_kernel_interface::clock::ClockError::
            SourceConversionOverflow
        | ::reovim_kernel_interface::clock::ClockError::
            DeterministicAdvancementOverflow
        | ::reovim_kernel_interface::clock::ClockError::DescriptorExhausted {
            elapsed_nanoseconds: _,
            maximum_elapsed_nanoseconds: _,
        }
        | ::reovim_kernel_interface::clock::ClockError::Closing
        | ::reovim_kernel_interface::clock::ClockError::Closed => {}
    }
}

const fn clock_clock_readiness_predicate_variants(
    value: ::reovim_kernel_interface::clock::ClockReadinessPredicate,
) {
    match value {
        ::reovim_kernel_interface::clock::ClockReadinessPredicate::
            CurrentGenerationObservation => {}
    }
}

const fn clock_instant_ordering_variants(
    value: ::reovim_kernel_interface::clock::InstantOrdering,
) {
    match value {
        ::reovim_kernel_interface::clock::InstantOrdering::Before
        | ::reovim_kernel_interface::clock::InstantOrdering::Equal
        | ::reovim_kernel_interface::clock::InstantOrdering::After => {}
    }
}

const fn event_actionability_variants(
    value: ::reovim_kernel_interface::event::Actionability,
) {
    match value {
        ::reovim_kernel_interface::event::Actionability::None
        | ::reovim_kernel_interface::event::Actionability::Operator(_) => {}
    }
}

const fn event_bootstrap_health_variants(
    value: ::reovim_kernel_interface::event::BootstrapHealth,
) {
    match value {
        ::reovim_kernel_interface::event::BootstrapHealth::Clear
        | ::reovim_kernel_interface::event::BootstrapHealth::Imported
        | ::reovim_kernel_interface::event::BootstrapHealth::Pending => {}
    }
}

const fn event_bound_error_variants(
    value: ::reovim_kernel_interface::event::BoundError,
) {
    match value {
        ::reovim_kernel_interface::event::BoundError::
            ZeroRejected { unit : _ }
        | ::reovim_kernel_interface::event::BoundError::
            ArithmeticOverflow { unit : _, left : _, right : _ }
        | ::reovim_kernel_interface::event::BoundError::
            UnitMismatch { expected : _, actual : _ }
        | ::reovim_kernel_interface::event::BoundError::
            IncoherentDescriptor { per_object : _, aggregate : _ }
        | ::reovim_kernel_interface::event::BoundError::
            Exhausted { unit : _, requested : _, maximum : _ } => {}
    }
}

const fn event_cursor_position_variants(
    value: ::reovim_kernel_interface::event::CursorPosition,
) {
    match value {
        ::reovim_kernel_interface::event::CursorPosition::Next(_)
        | ::reovim_kernel_interface::event::CursorPosition::Exhausted => {}
    }
}

const fn event_event_admission_outcome_variants(
    value: ::reovim_kernel_interface::event::EventAdmissionOutcome,
) {
    match value {
        ::reovim_kernel_interface::event::EventAdmissionOutcome::Rejected(
            _,
        )
        | ::reovim_kernel_interface::event::EventAdmissionOutcome::Dropped(_)
        | ::reovim_kernel_interface::event::EventAdmissionOutcome::Closed(_)
        | ::reovim_kernel_interface::event::EventAdmissionOutcome::Accepted(
            _,
        ) => {}
    }
}

const fn event_event_closed_cause_variants(
    value: ::reovim_kernel_interface::event::EventClosedCause,
) {
    match value {
        ::reovim_kernel_interface::event::EventClosedCause::StreamClosing
        | ::reovim_kernel_interface::event::EventClosedCause::StreamClosed
        | ::reovim_kernel_interface::event::EventClosedCause::StreamReplaced
        | ::reovim_kernel_interface::event::EventClosedCause::ClockClosing
        | ::reovim_kernel_interface::event::EventClosedCause::ClockClosed
        | ::reovim_kernel_interface::event::EventClosedCause::ClockReplaced
        | ::reovim_kernel_interface::event::EventClosedCause::
            SequenceExhausted => {}
    }
}

const fn event_event_drop_cause_variants(
    value: ::reovim_kernel_interface::event::EventDropCause,
) {
    match value {
        ::reovim_kernel_interface::event::EventDropCause::RateLimited
        | ::reovim_kernel_interface::event::EventDropCause::
            SynchronizationPressure
        | ::reovim_kernel_interface::event::EventDropCause::AllocationFailure
        | ::reovim_kernel_interface::event::EventDropCause::
            ChargePreparationFailure
        | ::reovim_kernel_interface::event::EventDropCause::
            RenderingFailure => {}
    }
}

const fn event_event_error_variants(
    value: ::reovim_kernel_interface::event::EventError,
) {
    match value {
        ::reovim_kernel_interface::event::EventError::Bound(_)
        | ::reovim_kernel_interface::event::EventError::ChargeOwnerMismatch
        | ::reovim_kernel_interface::event::EventError::
            DescriptorContractMismatch
        | ::reovim_kernel_interface::event::EventError::IncoherentCarrier
        | ::reovim_kernel_interface::event::EventError::UnsupportedContract
        | ::reovim_kernel_interface::event::EventError::IncompatibleContract
        | ::reovim_kernel_interface::event::EventError::ProposalOwnerMismatch
        | ::reovim_kernel_interface::event::EventError::AdmissionScopeMismatch
        | ::reovim_kernel_interface::event::EventError::
            InvalidProposalStructure
        | ::reovim_kernel_interface::event::EventError::
            InvalidProposalClassification
        | ::reovim_kernel_interface::event::EventError::UnassignedSequence
        | ::reovim_kernel_interface::event::EventError::SequenceExhausted
        | ::reovim_kernel_interface::event::EventError::
            InvalidCounterSaturation
        | ::reovim_kernel_interface::event::EventError::InvalidRetainedRange
        | ::reovim_kernel_interface::event::EventError::SnapshotBoundExceeded
        | ::reovim_kernel_interface::event::EventError::InvalidSnapshotMetadata
        | ::reovim_kernel_interface::event::EventError::InvalidStartupRecord
        | ::reovim_kernel_interface::event::EventError::EmptyCanonicalBytes
        | ::reovim_kernel_interface::event::EventError::NonAsciiCanonicalBytes
        | ::reovim_kernel_interface::event::EventError::InvalidCanonicalFraming
        | ::reovim_kernel_interface::event::EventError::CanonicalLengthOverflow
        | ::reovim_kernel_interface::event::EventError::Clock(_) => {}
    }
}

const fn event_event_reject_cause_variants(
    value: ::reovim_kernel_interface::event::EventRejectCause,
) {
    match value {
        ::reovim_kernel_interface::event::EventRejectCause::StreamNotReady
        | ::reovim_kernel_interface::event::EventRejectCause::InvalidAuthority
        | ::reovim_kernel_interface::event::EventRejectCause::InvalidStructure
        | ::reovim_kernel_interface::event::EventRejectCause::BoundExceeded
        | ::reovim_kernel_interface::event::EventRejectCause::
            InvalidActionability
        | ::reovim_kernel_interface::event::EventRejectCause::Clock(_) => {}
    }
}

const fn event_remediation_code_variants(
    value: ::reovim_kernel_interface::event::RemediationCode,
) {
    match value {
        ::reovim_kernel_interface::event::RemediationCode::
            DiagnosticPathDegraded
        | ::reovim_kernel_interface::event::RemediationCode::
            ActivationFailed => {}
    }
}

const fn event_severity_variants(
    value: ::reovim_kernel_interface::event::Severity,
) {
    match value {
        ::reovim_kernel_interface::event::Severity::Info
        | ::reovim_kernel_interface::event::Severity::Warning
        | ::reovim_kernel_interface::event::Severity::Error => {}
    }
}

const fn event_snapshot_coverage_variants(
    value: ::reovim_kernel_interface::event::SnapshotCoverage,
) {
    match value {
        ::reovim_kernel_interface::event::SnapshotCoverage::Complete
        | ::reovim_kernel_interface::event::SnapshotCoverage::Partial {
            first_uncopied: _,
        } => {}
    }
}

const fn effect_effect_certainty_variants(
    value: ::reovim_kernel_interface::effect::EffectCertainty,
) {
    match value {
        ::reovim_kernel_interface::effect::EffectCertainty::Applied
        | ::reovim_kernel_interface::effect::EffectCertainty::
            DefinitelyNotApplied
        | ::reovim_kernel_interface::effect::EffectCertainty::
            OutcomeUnknown => {}
    }
}

const fn lifecycle_lifecycle_state_variants(
    value: ::reovim_kernel_interface::lifecycle::LifecycleState,
) {
    match value {
        ::reovim_kernel_interface::lifecycle::LifecycleState::Constructed
        | ::reovim_kernel_interface::lifecycle::LifecycleState::Starting
        | ::reovim_kernel_interface::lifecycle::LifecycleState::Ready
        | ::reovim_kernel_interface::lifecycle::LifecycleState::RollingBack
        | ::reovim_kernel_interface::lifecycle::LifecycleState::Closing
        | ::reovim_kernel_interface::lifecycle::LifecycleState::Closed
        | ::reovim_kernel_interface::lifecycle::LifecycleState::
            IncompleteShutdown => {}
    }
}

const fn lifecycle_readiness_outcome_variants(
    value: ::reovim_kernel_interface::lifecycle::ReadinessOutcome,
) {
    match value {
        ::reovim_kernel_interface::lifecycle::ReadinessOutcome::Ready
        | ::reovim_kernel_interface::lifecycle::ReadinessOutcome::
            DiagnosticsDegraded
        | ::reovim_kernel_interface::lifecycle::ReadinessOutcome::
            ActivationFailed(_) => {}
    }
}

const fn lifecycle_rollback_outcome_variants(
    value: ::reovim_kernel_interface::lifecycle::RollbackOutcome,
) {
    match value {
        ::reovim_kernel_interface::lifecycle::RollbackOutcome::Reverted
        | ::reovim_kernel_interface::lifecycle::RollbackOutcome::
            IncompleteShutdown => {}
    }
}

const fn lifecycle_shutdown_outcome_variants(
    value: ::reovim_kernel_interface::lifecycle::ShutdownOutcome,
) {
    match value {
        ::reovim_kernel_interface::lifecycle::ShutdownOutcome::Closed
        | ::reovim_kernel_interface::lifecycle::ShutdownOutcome::
            IncompleteShutdown => {}
    }
}

const fn lifecycle_work_disposition_variants(
    value: ::reovim_kernel_interface::lifecycle::WorkDisposition,
) {
    match value {
        ::reovim_kernel_interface::lifecycle::WorkDisposition::Drained
        | ::reovim_kernel_interface::lifecycle::WorkDisposition::Discarded
        | ::reovim_kernel_interface::lifecycle::WorkDisposition::Unresolved => {
        }
    }
}

const fn operation_callback_class_variants(
    value: ::reovim_kernel_interface::operation::CallbackClass,
) {
    match value {
        ::reovim_kernel_interface::operation::CallbackClass::
            NoUserCallback
        | ::reovim_kernel_interface::operation::CallbackClass::
            MayInvokeUserCallback => {}
    }
}

const fn operation_completion_class_variants(
    value: ::reovim_kernel_interface::operation::CompletionClass,
) {
    match value {
        ::reovim_kernel_interface::operation::CompletionClass::
            InlineTerminal
        | ::reovim_kernel_interface::operation::CompletionClass::
            DeferredTerminal => {}
    }
}

const fn operation_contract_selection_error_variants(
    value: ::reovim_kernel_interface::operation::ContractSelectionError,
) {
    match value {
        ::reovim_kernel_interface::operation::ContractSelectionError::
            InvertedRange { minimum : _, maximum : _ }
        | ::reovim_kernel_interface::operation::ContractSelectionError::
            SelectedOutsideRange {
                selected: _,
                minimum: _,
                maximum: _,
            } => {}
    }
}

const fn operation_delivery_class_variants(
    value: ::reovim_kernel_interface::operation::DeliveryClass,
) {
    match value {
        ::reovim_kernel_interface::operation::DeliveryClass::Single
        | ::reovim_kernel_interface::operation::DeliveryClass::Stream => {}
    }
}

const fn operation_effect_class_variants(
    value: ::reovim_kernel_interface::operation::EffectClass,
) {
    match value {
        ::reovim_kernel_interface::operation::EffectClass::LocalOnly
        | ::reovim_kernel_interface::operation::EffectClass::
            ExternalObservation
        | ::reovim_kernel_interface::operation::EffectClass::
            ExternalMutation => {}
    }
}

const fn operation_invocation_class_variants(
    value: ::reovim_kernel_interface::operation::InvocationClass,
) {
    match value {
        ::reovim_kernel_interface::operation::InvocationClass::
            NonBlockingInvocation
        | ::reovim_kernel_interface::operation::InvocationClass::
            CallerMayWait => {}
    }
}

const fn operation_operation_descriptor_error_variants(
    value: ::reovim_kernel_interface::operation::OperationDescriptorError,
) {
    match value {
        ::reovim_kernel_interface::operation::OperationDescriptorError::
            InlineMayNotWait
        | ::reovim_kernel_interface::operation::OperationDescriptorError::
            StreamRequiresDeferred
        | ::reovim_kernel_interface::operation::OperationDescriptorError::
            InlineMayNotInvokeCallback => {}
    }
}

const fn operation_reentrancy_class_variants(
    value: ::reovim_kernel_interface::operation::ReentrancyClass,
) {
    match value {
        ::reovim_kernel_interface::operation::ReentrancyClass::NonReentrant
        | ::reovim_kernel_interface::operation::ReentrancyClass::Reentrant => {}
    }
}
