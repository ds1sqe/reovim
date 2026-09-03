//! Exact compiler-resolved Kernel Interface public items.

pub const fn witness() -> bool {
    cancellation_items();
    clock_items();
    event_items_one();
    event_items_two();
    lifecycle_items();
    operation_items();
    trait_items();
    trait_method_items();
    variant_items();
    true
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
        &dyn ::reovim_kernel_interface::event::StartupEventAdmission,
    > = None;
}

const fn trait_method_items() {
    let _ = (
        <dyn ::reovim_kernel_interface::clock::MonotonicClock>::descriptor,
        <dyn ::reovim_kernel_interface::clock::MonotonicClock>::observe,
        <dyn ::reovim_kernel_interface::event::StartupEventAdmission>::admit,
        <dyn ::reovim_kernel_interface::event::StartupEventAdmission>::
            descriptor,
        <dyn ::reovim_kernel_interface::event::StartupEventAdmission>::scope,
    );
}

const fn variant_items() {
    let _ = (
        cancellation_cancellation_cause_variants,
        cancellation_cancellation_state_variants,
        cancellation_deadline_state_variants,
        cancellation_execution_outcome_variants,
        cancellation_fence_cause_variants,
        cancellation_installation_outcome_variants,
        clock_clock_error_variants,
        clock_clock_readiness_predicate_variants,
        clock_instant_ordering_variants,
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
