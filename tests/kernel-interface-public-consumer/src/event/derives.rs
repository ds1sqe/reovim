use core::fmt::Debug;

pub const fn const_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::event::StartupEventIdentities,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::event::StartupEventLimits,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::event::StartupEventDescriptor,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::event::BootstrapHealth,
    >() && crate::derive_witness::<::reovim_kernel_interface::event::Severity>()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::RemediationCode,
        >()
        && remediation_ref_derive()
        && actionability_derive()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::StartupComponentCounts,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::StartupProposal,
        >()
        && event_sequence_derive()
        && crate::clone_copy_witness::<
            ::reovim_kernel_interface::event::EventTimestamp,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::EventAdmissionScope,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::SaturatingCounter,
        >()
        && event_cursor_derive()
        && cursor_position_derive()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::RetainedSequenceRange,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::SnapshotRequest,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::SnapshotStatistics,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::SnapshotCoverage,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::SnapshotMetadata,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::CanonicalEventBytes<'static>,
        >()
        && crate::clone_copy_witness::<
            ::reovim_kernel_interface::event::StartupEventRecord<'static>,
        >()
        && crate::clone_copy_witness::<
            ::reovim_kernel_interface::event::StartupSnapshot<'static>,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::EventRejectCause,
        >()
        && event_drop_cause_derive()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::EventClosedCause,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::EventAdmissionOutcome,
        >()
        && event_error_derive()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::BoundDescriptor,
        >()
        && bound_error_derive()
        && checked_count_derive()
        && crate::derive_witness::<
            ::reovim_kernel_interface::event::CheckedByteCount,
        >()
}

const fn event_cursor_derive() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::event::EventCursor>()
}

const fn event_error_derive() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::event::EventError>()
}

const fn bound_error_derive() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::event::BoundError>()
}

const fn remediation_ref_derive() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::event::RemediationRef>()
}

const fn actionability_derive() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::event::Actionability>()
}

const fn event_sequence_derive() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::event::EventSequence>()
}

const fn cursor_position_derive() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::event::CursorPosition>()
}

const fn event_drop_cause_derive() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::event::EventDropCause>()
}

const fn checked_count_derive() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::event::CheckedCount>()
}

pub fn exercise() -> bool {
    let Some(descriptor) = super::make_descriptor() else {
        return false;
    };
    clone_debug_eq_value(descriptor.limits())
        && clone_debug_eq_value(&descriptor)
}

fn clone_debug_eq_value<T>(value: &T) -> bool
where
    T: Clone + Debug + Eq,
{
    super::super::require_debug(value);
    value == &value.clone()
}
