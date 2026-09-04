//! Portable monotonic-clock values and capability contract.

use core::fmt;

pub use ::reovim_foundation::bound::CheckedDuration;
pub use ::reovim_foundation::carrier::CarrierSafetyDescriptor;

use reovim_foundation::bound::ZeroPolicy;
use reovim_foundation::carrier::{
    BorrowModel, ConcurrencySafety, CopyModel, DropModel, InvocationModel,
    InvocationSafety, LifetimeModel, MemorySafety, OwnershipModel,
    SharingModel, ThreadTransfer, TypeMismatch, UnsafeBoundary, UnwindModel,
};
use reovim_foundation::contract::{
    CompatibleVersionRange, ContractId, ContractVersion,
};

use crate::lifecycle::{Generation, LifecycleIdentity};
use crate::operation::{ContractSelection, OperationDescriptor};

/// Immutable contract and instance declaration for one monotonic clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockDescriptor {
    contract: ContractSelection,
    lifecycle: LifecycleIdentity,
    owner: LifecycleIdentity,
    resolution: CheckedDuration,
    maximum_elapsed: CheckedDuration,
    instant_safety: CarrierSafetyDescriptor,
}

impl ClockDescriptor {
    /// Constructs one coherent clock descriptor without observing a clock.
    ///
    /// `instant_type_id` identifies the opaque instant carrier family. It is
    /// supplied by the contract authority rather than generated here.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::IncoherentDescriptor`] when resolution is zero or
    /// exceeds the maximum elapsed duration. Returns
    /// [`ClockError::IncoherentCarrier`] if the fixed instant safety terms
    /// cannot form a coherent Foundation carrier declaration.
    pub const fn new(
        contract: ContractSelection,
        lifecycle: LifecycleIdentity,
        owner: LifecycleIdentity,
        resolution_nanoseconds: u64,
        maximum_elapsed_nanoseconds: u64,
        instant_type_id: u128,
    ) -> Result<Self, ClockError> {
        if !descriptor_durations_are_coherent(
            resolution_nanoseconds,
            maximum_elapsed_nanoseconds,
        ) {
            return Err(ClockError::IncoherentDescriptor {
                resolution_nanoseconds,
                maximum_elapsed_nanoseconds,
            });
        }
        let resolution = match checked_nonzero_duration(
            resolution_nanoseconds,
            resolution_nanoseconds,
            maximum_elapsed_nanoseconds,
        ) {
            Ok(duration) => duration,
            Err(error) => return Err(error),
        };
        let maximum_elapsed = match checked_nonzero_duration(
            maximum_elapsed_nanoseconds,
            resolution_nanoseconds,
            maximum_elapsed_nanoseconds,
        ) {
            Ok(duration) => duration,
            Err(error) => return Err(error),
        };
        let instant_safety = match instant_safety(contract, instant_type_id) {
            Ok(descriptor) => descriptor,
            Err(error) => return Err(error),
        };
        Ok(Self {
            contract,
            lifecycle,
            owner,
            resolution,
            maximum_elapsed,
            instant_safety,
        })
    }

    /// Returns the selected clock contract identity and version range.
    #[must_use]
    pub const fn contract(self) -> ContractSelection {
        self.contract
    }

    /// Returns the capability instance, incarnation, and generation identity.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Returns the Lifecycle Owner identity responsible for this capability.
    #[must_use]
    pub const fn owner(self) -> LifecycleIdentity {
        self.owner
    }

    /// Returns the declared observable clock resolution.
    #[must_use]
    pub const fn resolution(self) -> CheckedDuration {
        self.resolution
    }

    /// Returns the largest elapsed value admitted by this descriptor.
    #[must_use]
    pub const fn maximum_elapsed(self) -> CheckedDuration {
        self.maximum_elapsed
    }

    /// Returns the enforced safety declaration for the instant carrier.
    #[must_use]
    pub const fn instant_safety(self) -> CarrierSafetyDescriptor {
        self.instant_safety
    }

    /// Returns the operation classes for descriptor inspection.
    #[must_use]
    pub const fn inspection_operation(self) -> OperationDescriptor {
        OperationDescriptor::local_inline()
    }

    /// Returns the operation classes for one clock observation.
    #[must_use]
    pub const fn observation_operation(self) -> OperationDescriptor {
        OperationDescriptor::external_observation_inline()
    }

    /// Returns the operation classes for instant comparison and duration.
    #[must_use]
    pub const fn arithmetic_operation(self) -> OperationDescriptor {
        OperationDescriptor::local_inline()
    }

    /// Returns the exact predicate required before publishing readiness.
    #[must_use]
    pub const fn readiness_predicate(self) -> ClockReadinessPredicate {
        ClockReadinessPredicate::CurrentGenerationObservation
    }

    /// Validates one requested contract identity and version.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::UnsupportedContract`] for another contract family
    /// or [`ClockError::IncompatibleContract`] for a version outside this
    /// descriptor's compatible range.
    pub const fn negotiate(
        self,
        requested_contract_id: u128,
        requested_version: u64,
    ) -> Result<(), ClockError> {
        if requested_contract_id != self.contract.identity() {
            return Err(ClockError::UnsupportedContract);
        }
        if requested_version < self.contract.minimum()
            || requested_version > self.contract.maximum()
        {
            return Err(ClockError::IncompatibleContract);
        }
        Ok(())
    }

    /// Creates an opaque instant inside this descriptor's elapsed bound.
    ///
    /// Adapters use this operation after converting their private clock value.
    /// No other public instant constructor exists.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::DescriptorExhausted`] when `elapsed_nanoseconds`
    /// exceeds the declared maximum.
    pub const fn instant(
        self,
        elapsed_nanoseconds: u64,
    ) -> Result<MonotonicInstant, ClockError> {
        if elapsed_nanoseconds > self.maximum_elapsed.nanoseconds() {
            return Err(ClockError::DescriptorExhausted {
                elapsed_nanoseconds,
                maximum_elapsed_nanoseconds: self.maximum_elapsed.nanoseconds(),
            });
        }
        Ok(MonotonicInstant { lifecycle: self.lifecycle, elapsed_nanoseconds })
    }

    /// Validates that an instant carries this exact clock stamp.
    ///
    /// Lifecycle phase validation remains the capability owner's
    /// responsibility. Historical stamps can remain valid after close, so
    /// this operation does not claim readiness or current lifecycle phase.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::CrossClock`] for another instance or incarnation,
    /// or [`ClockError::StaleGeneration`] for another generation.
    pub const fn validate_stamp(
        self,
        instant: MonotonicInstant,
    ) -> Result<(), ClockError> {
        validate_lifecycle(self.lifecycle, instant.lifecycle)
    }
}

/// One opaque elapsed observation from an exact clock generation.
#[derive(Clone, Copy)]
pub struct MonotonicInstant {
    lifecycle: LifecycleIdentity,
    elapsed_nanoseconds: u64,
}

/// Inspectable predicate required for one clock readiness publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockReadinessPredicate {
    /// Negotiation succeeded and one current in-bound observation completed.
    CurrentGenerationObservation,
}

impl MonotonicInstant {
    /// Returns elapsed nanoseconds for sibling contract stamping only.
    #[must_use]
    pub(crate) const fn elapsed_nanoseconds(self) -> u64 {
        self.elapsed_nanoseconds
    }

    /// Compares two observations from the same clock generation.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::CrossClock`] for another instance or incarnation,
    /// or [`ClockError::StaleGeneration`] for another generation.
    pub const fn compare(
        self,
        other: Self,
    ) -> Result<InstantOrdering, ClockError> {
        match validate_lifecycle(self.lifecycle, other.lifecycle) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        if self.elapsed_nanoseconds < other.elapsed_nanoseconds {
            return Ok(InstantOrdering::Before);
        }
        if self.elapsed_nanoseconds > other.elapsed_nanoseconds {
            return Ok(InstantOrdering::After);
        }
        Ok(InstantOrdering::Equal)
    }

    /// Derives the checked duration from `earlier` to this instant.
    ///
    /// Equal instants produce a zero duration. Historical instants remain
    /// comparable after close because this operation consults no live state.
    ///
    /// # Errors
    ///
    /// Returns a cross-clock or stale-generation error before arithmetic, and
    /// [`ClockError::ObservedRegression`] when this instant precedes `earlier`.
    pub const fn duration_since(
        self,
        earlier: Self,
    ) -> Result<CheckedDuration, ClockError> {
        match validate_lifecycle(self.lifecycle, earlier.lifecycle) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        let Some(elapsed) =
            self.elapsed_nanoseconds.checked_sub(earlier.elapsed_nanoseconds)
        else {
            return Err(ClockError::ObservedRegression);
        };
        checked_duration(elapsed)
    }
}

/// Total ordering of two instants from one clock generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstantOrdering {
    /// The left instant precedes the right instant.
    Before,
    /// Both instants carry the same elapsed observation.
    Equal,
    /// The left instant follows the right instant.
    After,
}

/// Typed construction, lifecycle, and instant-operation failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockError {
    /// Contract negotiation selected an incompatible version.
    IncompatibleContract,
    /// The requested clock contract or behavior is unsupported.
    UnsupportedContract,
    /// The selected family/version matches but descriptor terms differ.
    DescriptorMismatch,
    /// Resolution and maximum elapsed duration cannot form one descriptor.
    IncoherentDescriptor {
        /// Rejected resolution in nanoseconds.
        resolution_nanoseconds: u64,
        /// Rejected maximum elapsed duration in nanoseconds.
        maximum_elapsed_nanoseconds: u64,
    },
    /// The fixed instant carrier declaration was internally incoherent.
    IncoherentCarrier,
    /// The capability has not published readiness.
    NotReady,
    /// An instant belongs to another clock instance or incarnation.
    CrossClock,
    /// An instant belongs to another capability generation.
    StaleGeneration,
    /// A later-minus-earlier request observed the reverse order.
    ObservedRegression,
    /// A hosted clock value cannot fit the portable elapsed carrier.
    SourceConversionOverflow,
    /// Deterministic test advancement exceeded the portable elapsed carrier.
    DeterministicAdvancementOverflow,
    /// An elapsed value exceeds the descriptor's maximum.
    DescriptorExhausted {
        /// Rejected elapsed value in nanoseconds.
        elapsed_nanoseconds: u64,
        /// Applicable maximum in nanoseconds.
        maximum_elapsed_nanoseconds: u64,
    },
    /// Capability shutdown has begun and rejects observation.
    Closing,
    /// The capability is closed and rejects observation.
    Closed,
}

impl fmt::Display for ClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::IncompatibleContract => "clock contract is incompatible",
            Self::UnsupportedContract => "clock contract is unsupported",
            Self::DescriptorMismatch => "clock descriptor terms differ",
            Self::IncoherentDescriptor { .. } => {
                "clock descriptor durations are incoherent"
            }
            Self::IncoherentCarrier => "clock instant carrier is incoherent",
            Self::NotReady => "clock is not ready",
            Self::CrossClock => "instants belong to different clocks",
            Self::StaleGeneration => "clock generation is stale",
            Self::ObservedRegression => "clock observation regressed",
            Self::SourceConversionOverflow => {
                "host clock value exceeds the elapsed carrier"
            }
            Self::DeterministicAdvancementOverflow => {
                "deterministic clock advancement overflowed"
            }
            Self::DescriptorExhausted { .. } => {
                "elapsed value exceeds the clock descriptor"
            }
            Self::Closing => "clock is closing",
            Self::Closed => "clock is closed",
        })
    }
}

impl core::error::Error for ClockError {}

/// Public semantic port implemented by one injected monotonic-clock adapter.
pub trait MonotonicClock {
    /// Returns the immutable descriptor negotiated for this capability.
    fn descriptor(&self) -> ClockDescriptor;

    /// Observes one current opaque instant without waiting or retaining work.
    ///
    /// # Errors
    ///
    /// Returns the exact typed contract, lifecycle, conversion, or bound
    /// failure for this observation attempt.
    fn observe(&self) -> Result<MonotonicInstant, ClockError>;
}

const fn descriptor_durations_are_coherent(
    resolution_nanoseconds: u64,
    maximum_elapsed_nanoseconds: u64,
) -> bool {
    resolution_nanoseconds > 0
        && resolution_nanoseconds <= maximum_elapsed_nanoseconds
}

const fn checked_nonzero_duration(
    nanoseconds: u64,
    resolution_nanoseconds: u64,
    maximum_elapsed_nanoseconds: u64,
) -> Result<CheckedDuration, ClockError> {
    match CheckedDuration::from_nanoseconds(nanoseconds, ZeroPolicy::Reject) {
        Ok(duration) => Ok(duration),
        Err(_) => Err(ClockError::IncoherentDescriptor {
            resolution_nanoseconds,
            maximum_elapsed_nanoseconds,
        }),
    }
}

const fn instant_safety(
    contract: ContractSelection,
    instant_type_id: u128,
) -> Result<CarrierSafetyDescriptor, ClockError> {
    let Ok(versions) = CompatibleVersionRange::new(
        ContractVersion::new(contract.minimum()),
        ContractVersion::new(contract.maximum()),
    ) else {
        return Err(ClockError::IncoherentCarrier);
    };
    let memory = MemorySafety::new(
        OwnershipModel::Owned,
        BorrowModel::Forbidden,
        CopyModel::Copy,
        DropModel::Trivial,
        LifetimeModel::Value,
    );
    let concurrency = ConcurrencySafety::new(
        ThreadTransfer::Transferable,
        SharingModel::Immutable,
    );
    let invocation = InvocationSafety::new(
        InvocationModel::DataOnly,
        UnwindModel::NotInvocable,
    );
    match CarrierSafetyDescriptor::new(
        ContractId::new(instant_type_id),
        versions,
        memory,
        concurrency,
        invocation,
        TypeMismatch::Reject,
        UnsafeBoundary::None,
    ) {
        Ok(descriptor) => Ok(descriptor),
        Err(_) => Err(ClockError::IncoherentCarrier),
    }
}

const fn validate_lifecycle(
    expected: LifecycleIdentity,
    actual: LifecycleIdentity,
) -> Result<(), ClockError> {
    if expected.instance() != actual.instance()
        || expected.incarnation() != actual.incarnation()
    {
        return Err(ClockError::CrossClock);
    }
    validate_generation(expected.generation(), actual.generation())
}

const fn validate_generation(
    expected: Generation,
    actual: Generation,
) -> Result<(), ClockError> {
    if expected.value() == actual.value() {
        return Ok(());
    }
    Err(ClockError::StaleGeneration)
}

const fn checked_duration(
    nanoseconds: u64,
) -> Result<CheckedDuration, ClockError> {
    match CheckedDuration::from_nanoseconds(nanoseconds, ZeroPolicy::Allow) {
        Ok(duration) => Ok(duration),
        Err(_) => Err(ClockError::IncoherentDescriptor {
            resolution_nanoseconds: nanoseconds,
            maximum_elapsed_nanoseconds: nanoseconds,
        }),
    }
}
