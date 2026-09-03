use core::fmt;

use crate::clock::{
    ClockDescriptor, ClockError, MonotonicClock, MonotonicInstant,
};

use super::{
    EventClosedCause, EventError, EventRejectCause, EventTimestamp,
    StartupEventDescriptor,
};

impl crate::event::EventAdmissionScope {
    /// Constructs the identity stamp for one accepted dispatch context.
    ///
    /// The value records a scope; possession alone does not grant admission.
    /// The [`super::StartupEventAdmission`] implementation remains the
    /// capability that validates and exercises authority before proposal work.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::AdmissionScopeMismatch`] when the lifecycle owner
    /// differs from the descriptor's exact generation.
    pub const fn new(
        descriptor: &StartupEventDescriptor,
        lifecycle_owner: crate::lifecycle::LifecycleIdentity,
        producer: u128,
        correlation: u128,
    ) -> Result<Self, EventError> {
        let identities = descriptor.identities();
        if !identities.lifecycle_owner().same_generation(lifecycle_owner) {
            return Err(EventError::AdmissionScopeMismatch);
        }
        Ok(Self {
            stream: identities.stream(),
            lifecycle_owner,
            producer: reovim_foundation::identity::InstanceId::new(producer),
            correlation: reovim_foundation::identity::InstanceId::new(
                correlation,
            ),
        })
    }

    /// Returns the exact Event Stream stamp for this dispatch context.
    #[must_use]
    pub const fn stream(self) -> crate::lifecycle::LifecycleIdentity {
        self.stream
    }

    /// Returns the lifecycle owner stamp accepted for this context.
    #[must_use]
    pub const fn lifecycle_owner(self) -> crate::lifecycle::LifecycleIdentity {
        self.lifecycle_owner
    }

    /// Returns the editor-derived producer identity scalar.
    #[must_use]
    pub const fn producer(self) -> u128 {
        self.producer.value()
    }

    /// Returns the editor-derived correlation identity scalar.
    #[must_use]
    pub const fn correlation(self) -> u128 {
        self.correlation.value()
    }

    pub(super) const fn matches_descriptor(
        self,
        descriptor: &StartupEventDescriptor,
    ) -> bool {
        let identities = descriptor.identities();
        self.stream.same_generation(identities.stream())
            && self
                .lifecycle_owner
                .same_generation(identities.lifecycle_owner())
    }
}

impl crate::event::EventTimestamp {
    /// Stamps one current instant under the descriptor's exact clock domain.
    ///
    /// This exposes whole elapsed nanoseconds only as Event timestamp data;
    /// it does not expose the clock's private origin or wall-time meaning.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::Clock`] for another clock or stale generation.
    pub(crate) const fn new(
        descriptor: &StartupEventDescriptor,
        instant: MonotonicInstant,
    ) -> Result<Self, EventError> {
        match descriptor.clock().validate_stamp(instant) {
            Ok(()) => Ok(Self {
                instant,
                elapsed_nanoseconds: instant.elapsed_nanoseconds(),
            }),
            Err(error) => Err(EventError::Clock(error)),
        }
    }

    /// Returns the opaque clock instant retained by this timestamp.
    #[must_use]
    pub const fn instant(self) -> MonotonicInstant {
        self.instant
    }

    /// Returns whole nanoseconds from the bound clock's private origin.
    #[must_use]
    pub const fn elapsed_nanoseconds(self) -> u64 {
        self.elapsed_nanoseconds
    }
}

impl crate::event::StartupEventDescriptor {
    /// Observes the descriptor-bound clock and stamps an Event timestamp.
    ///
    /// This is the only public startup timestamp construction path. Creating
    /// a timestamp does not admit a record or assign a sequence.
    ///
    /// # Errors
    ///
    /// Returns a typed descriptor, lifecycle, observation, or stamp failure.
    pub fn observe_timestamp<C>(
        &self,
        clock: &C,
    ) -> Result<EventTimestamp, EventError>
    where
        C: MonotonicClock + ?Sized,
    {
        let actual = clock.descriptor();
        validate_clock_descriptor(self.clock(), actual)?;
        let instant = clock.observe().map_err(EventError::Clock)?;
        EventTimestamp::new(self, instant)
    }
}

impl crate::event::EventAdmissionOutcome {
    /// Maps a clock observation failure to its exact admission category.
    #[must_use]
    pub const fn from_clock_error(error: ClockError) -> Self {
        match error {
            ClockError::Closing => Self::Closed(EventClosedCause::ClockClosing),
            ClockError::Closed => Self::Closed(EventClosedCause::ClockClosed),
            ClockError::StaleGeneration => {
                Self::Closed(EventClosedCause::ClockReplaced)
            }
            _ => Self::Rejected(EventRejectCause::Clock(error)),
        }
    }

    /// Returns the committed sequence only for an accepted record.
    #[must_use]
    pub const fn accepted_sequence(self) -> Option<super::EventSequence> {
        match self {
            Self::Accepted(sequence) => Some(sequence),
            Self::Rejected(_) | Self::Dropped(_) | Self::Closed(_) => None,
        }
    }
}

impl fmt::Display for crate::event::EventError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bound(_) => "Event value exceeds a declared bound",
            Self::ChargeOwnerMismatch => "Event bound charge owner differs",
            Self::DescriptorContractMismatch => {
                "Event descriptor contract identities differ"
            }
            Self::IncoherentCarrier => {
                "Event canonical-reference carrier is incoherent"
            }
            Self::UnsupportedContract => "Event contract is unsupported",
            Self::IncompatibleContract => "Event contract is incompatible",
            Self::ProposalOwnerMismatch => "proposal owner stamp differs",
            Self::AdmissionScopeMismatch => {
                "Event admission scope differs from its descriptor"
            }
            Self::InvalidProposalStructure => {
                "proposal readiness and component states contradict"
            }
            Self::InvalidProposalClassification => {
                "proposal classification differs from readiness"
            }
            Self::UnassignedSequence => "Event sequence is unassigned",
            Self::SequenceExhausted => "Event sequence is exhausted",
            Self::InvalidCounterSaturation => {
                "Event counter saturation is incoherent"
            }
            Self::InvalidRetainedRange => "retained sequence range is inverted",
            Self::SnapshotBoundExceeded => "snapshot bound is exceeded",
            Self::InvalidSnapshotMetadata => "snapshot metadata is incoherent",
            Self::InvalidStartupRecord => {
                "startup record differs from its typed fields"
            }
            Self::EmptyCanonicalBytes => "canonical Event bytes are empty",
            Self::NonAsciiCanonicalBytes => {
                "canonical Event bytes are not ASCII"
            }
            Self::InvalidCanonicalFraming => {
                "canonical Event framing is invalid"
            }
            Self::CanonicalLengthOverflow => {
                "canonical Event length exceeds the size carrier"
            }
            Self::Clock(_) => "Event timestamp clock identity is invalid",
        })
    }
}

impl std::error::Error for crate::event::EventError {}

fn validate_clock_descriptor(
    expected: ClockDescriptor,
    actual: ClockDescriptor,
) -> Result<(), EventError> {
    if expected.contract().identity() != actual.contract().identity() {
        return Err(EventError::Clock(ClockError::UnsupportedContract));
    }
    if !expected.contract().supports(actual.contract().selected()) {
        return Err(EventError::Clock(ClockError::IncompatibleContract));
    }
    let expected_stamp = expected.lifecycle();
    let actual_stamp = actual.lifecycle();
    if !expected_stamp.same_incarnation(actual_stamp) {
        return Err(EventError::Clock(ClockError::CrossClock));
    }
    if expected_stamp.generation() != actual_stamp.generation() {
        return Err(EventError::Clock(ClockError::StaleGeneration));
    }
    if expected != actual {
        return Err(EventError::Clock(ClockError::DescriptorMismatch));
    }
    Ok(())
}
