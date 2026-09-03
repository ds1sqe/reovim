use reovim_kernel_interface::clock::{
    CheckedDuration, ClockDescriptor, ClockError, ClockReadinessPredicate,
    InstantOrdering, MonotonicInstant,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::operation::{
    ContractSelection, EffectClass, InvocationClass,
};

use super::{clone_value, error_traits, traits};

struct FixedClock {
    descriptor: ClockDescriptor,
    instant: MonotonicInstant,
}

impl FixedClock {
    const fn new(
        descriptor: ClockDescriptor,
        instant: MonotonicInstant,
    ) -> Self {
        Self { descriptor, instant }
    }
}

impl ::reovim_kernel_interface::clock::MonotonicClock for FixedClock {
    fn descriptor(&self) -> ClockDescriptor {
        self.descriptor
    }

    fn observe(&self) -> Result<MonotonicInstant, ClockError> {
        Ok(self.instant)
    }
}

const fn make_descriptor() -> Option<ClockDescriptor> {
    let Ok(contract) = ContractSelection::new(101, 2, 1, 3) else {
        return None;
    };
    let lifecycle = LifecycleIdentity::new(201, 202, Generation::new(3));
    let owner = LifecycleIdentity::new(301, 302, Generation::new(4));
    match ClockDescriptor::new(contract, lifecycle, owner, 1, 1_000, 401) {
        Ok(descriptor) => Some(descriptor),
        Err(_) => None,
    }
}

pub const fn const_cases() -> bool {
    descriptor_const_cases()
        && instant_const_cases()
        && error_const_cases()
        && derive_const_cases()
}

const fn descriptor_const_cases() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let contract = descriptor.contract();
    let lifecycle = descriptor.lifecycle();
    let owner = descriptor.owner();
    let safety = descriptor.instant_safety();

    contract.identity() == 101
        && lifecycle.instance() == 201
        && lifecycle.incarnation() == 202
        && lifecycle.generation().value() == 3
        && owner.instance() == 301
        && descriptor.resolution().nanoseconds() == 1
        && descriptor.maximum_elapsed().nanoseconds() == 1_000
        && safety.type_id().value() == 401
        && effect_code(descriptor.inspection_operation().effect()) == 0
        && effect_code(descriptor.observation_operation().effect()) == 1
        && invocation_code(descriptor.observation_operation().invocation()) == 0
        && effect_code(descriptor.arithmetic_operation().effect()) == 0
        && readiness_code(descriptor.readiness_predicate()) == 0
        && descriptor.negotiate(101, 1).is_ok()
        && descriptor.negotiate(101, 3).is_ok()
        && unit_clock_error(
            descriptor.negotiate(999, 2),
            ClockError::UnsupportedContract,
        )
        && unit_clock_error(
            descriptor.negotiate(101, 4),
            ClockError::IncompatibleContract,
        )
}

const fn effect_code(value: EffectClass) -> u8 {
    match value {
        EffectClass::LocalOnly => 0,
        EffectClass::ExternalObservation => 1,
        EffectClass::ExternalMutation => 2,
    }
}

const fn invocation_code(value: InvocationClass) -> u8 {
    match value {
        InvocationClass::NonBlockingInvocation => 0,
        InvocationClass::CallerMayWait => 1,
    }
}

const fn readiness_code(value: ClockReadinessPredicate) -> u8 {
    match value {
        ClockReadinessPredicate::CurrentGenerationObservation => 0,
    }
}

const fn instant_const_cases() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Ok(first) = descriptor.instant(10) else {
        return false;
    };
    let Ok(equal) = descriptor.instant(10) else {
        return false;
    };
    let Ok(later) = descriptor.instant(25) else {
        return false;
    };

    descriptor.validate_stamp(first).is_ok()
        && ordering(first.compare(later), InstantOrdering::Before)
        && ordering(first.compare(equal), InstantOrdering::Equal)
        && ordering(later.compare(first), InstantOrdering::After)
        && duration_is(later.duration_since(first), 15)
        && duration_clock_error(
            first.duration_since(later),
            ClockError::ObservedRegression,
        )
        && foreign_instant_cases(descriptor, first)
}

const fn foreign_instant_cases(
    descriptor: ClockDescriptor,
    instant: MonotonicInstant,
) -> bool {
    let contract = descriptor.contract();
    let owner = descriptor.owner();
    let foreign_lifecycle =
        LifecycleIdentity::new(211, 202, descriptor.lifecycle().generation());
    let stale_lifecycle = LifecycleIdentity::new(201, 202, Generation::new(99));
    let Ok(foreign) =
        ClockDescriptor::new(contract, foreign_lifecycle, owner, 1, 1_000, 401)
    else {
        return false;
    };
    let Ok(stale) =
        ClockDescriptor::new(contract, stale_lifecycle, owner, 1, 1_000, 401)
    else {
        return false;
    };
    let Ok(foreign_instant) = foreign.instant(10) else {
        return false;
    };
    let Ok(stale_instant) = stale.instant(10) else {
        return false;
    };

    unit_clock_error(
        descriptor.validate_stamp(foreign_instant),
        ClockError::CrossClock,
    ) && unit_clock_error(
        descriptor.validate_stamp(stale_instant),
        ClockError::StaleGeneration,
    ) && ordering_clock_error(
        instant.compare(foreign_instant),
        ClockError::CrossClock,
    ) && duration_clock_error(
        instant.duration_since(stale_instant),
        ClockError::StaleGeneration,
    )
}

const fn error_const_cases() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let invalid = ClockDescriptor::new(
        descriptor.contract(),
        descriptor.lifecycle(),
        descriptor.owner(),
        0,
        1_000,
        401,
    );
    let exhausted = descriptor.instant(1_001);

    incoherent_descriptor_code(invalid) == 1
        && descriptor_exhausted_code(exhausted) == 1
}

const fn incoherent_descriptor_code(
    result: Result<ClockDescriptor, ClockError>,
) -> u8 {
    match result {
        Err(ClockError::IncoherentDescriptor {
            resolution_nanoseconds: 0,
            maximum_elapsed_nanoseconds: 1_000,
        }) => 1,
        _ => 0,
    }
}

const fn descriptor_exhausted_code(
    result: Result<MonotonicInstant, ClockError>,
) -> u8 {
    match result {
        Err(ClockError::DescriptorExhausted {
            elapsed_nanoseconds: 1_001,
            maximum_elapsed_nanoseconds: 1_000,
        }) => 1,
        _ => 0,
    }
}

const fn ordering(
    result: Result<InstantOrdering, ClockError>,
    expected: InstantOrdering,
) -> bool {
    match result {
        Ok(actual) => actual as u8 == expected as u8,
        Err(_) => false,
    }
}

const fn duration_is(
    result: Result<CheckedDuration, ClockError>,
    expected: u64,
) -> bool {
    match result {
        Ok(duration) => duration.nanoseconds() == expected,
        Err(_) => false,
    }
}

const fn unit_clock_error(
    result: Result<(), ClockError>,
    expected: ClockError,
) -> bool {
    match result {
        Err(actual) => clock_error_code(actual) == clock_error_code(expected),
        Ok(()) => false,
    }
}

const fn duration_clock_error(
    result: Result<CheckedDuration, ClockError>,
    expected: ClockError,
) -> bool {
    match result {
        Err(actual) => clock_error_code(actual) == clock_error_code(expected),
        Ok(_) => false,
    }
}

const fn ordering_clock_error(
    result: Result<InstantOrdering, ClockError>,
    expected: ClockError,
) -> bool {
    match result {
        Err(actual) => clock_error_code(actual) == clock_error_code(expected),
        Ok(_) => false,
    }
}

const fn clock_error_code(error: ClockError) -> u8 {
    match error {
        ClockError::IncompatibleContract => 1,
        ClockError::UnsupportedContract => 2,
        ClockError::DescriptorMismatch => 3,
        ClockError::IncoherentDescriptor { .. } => 4,
        ClockError::IncoherentCarrier => 5,
        ClockError::NotReady => 6,
        ClockError::CrossClock => 7,
        ClockError::StaleGeneration => 8,
        ClockError::ObservedRegression => 9,
        ClockError::SourceConversionOverflow => 10,
        ClockError::DeterministicAdvancementOverflow => 11,
        ClockError::DescriptorExhausted { .. } => 12,
        ClockError::Closing => 13,
        ClockError::Closed => 14,
    }
}

const fn derive_const_cases() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::clock::CheckedDuration>()
        && crate::derive_witness::<
            ::reovim_kernel_interface::clock::CarrierSafetyDescriptor,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::clock::ClockDescriptor,
        >()
        && crate::clone_copy_witness::<
            ::reovim_kernel_interface::clock::MonotonicInstant,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::clock::ClockReadinessPredicate,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::clock::InstantOrdering,
        >()
        && clock_error_derive()
}

const fn clock_error_derive() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::clock::ClockError>()
}

pub fn exercise() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Ok(instant) = descriptor.instant(20) else {
        return false;
    };
    let clock = FixedClock::new(descriptor, instant);
    let Ok(observed) =
        ::reovim_kernel_interface::clock::MonotonicClock::observe(&clock)
    else {
        return false;
    };
    let error = ClockError::NotReady;

    ::reovim_kernel_interface::clock::MonotonicClock::descriptor(&clock)
        == descriptor
        && ordering(observed.compare(instant), InstantOrdering::Equal)
        && traits(descriptor)
        && clone_copy_value(instant)
        && clock_error_variants()
        && error_traits(&error)
}

fn clone_copy_value(value: MonotonicInstant) -> bool {
    let cloned = clone_value(&value);
    let (left, right) = super::copy_pair(cloned);
    matches_same_instant(left, right)
}

const fn matches_same_instant(
    left: MonotonicInstant,
    right: MonotonicInstant,
) -> bool {
    ordering(left.compare(right), InstantOrdering::Equal)
}

fn clock_error_variants() -> bool {
    let values = [
        ClockError::IncompatibleContract,
        ClockError::UnsupportedContract,
        ClockError::DescriptorMismatch,
        ClockError::IncoherentDescriptor {
            resolution_nanoseconds: 2,
            maximum_elapsed_nanoseconds: 1,
        },
        ClockError::IncoherentCarrier,
        ClockError::NotReady,
        ClockError::CrossClock,
        ClockError::StaleGeneration,
        ClockError::ObservedRegression,
        ClockError::SourceConversionOverflow,
        ClockError::DeterministicAdvancementOverflow,
        ClockError::DescriptorExhausted {
            elapsed_nanoseconds: 2,
            maximum_elapsed_nanoseconds: 1,
        },
        ClockError::Closing,
        ClockError::Closed,
    ];
    values.into_iter().all(traits)
}
