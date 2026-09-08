//! Bounded kernel.editing/v2 diagnostics; never editing acceptance authority.

use core::fmt;

use crate::buffer::{BufferStateStamp, plan::BufferCorrelationIdentity};
use crate::clock::{ClockDescriptor, ClockError, MonotonicInstant};
use crate::contract::ContractId;
use crate::interaction::identity::InteractionRequestIdentity;
use crate::interaction::outcome::InteractionOutcomeCause;
use crate::interaction::session::InteractionSessionHandle;
use crate::interaction::update::RepresentationUpdateIdentityStamp;
use crate::lifecycle::LifecycleIdentity;

use super::{CanonicalEventRecordRef, CheckedByteCount, EventSequence};

mod encoding;

/// Stable identity of the editing diagnostic schema, separate from startup.
pub const EDITING_FAMILY: u128 = 0x7265_6f76_696d_2e65_6469_7469_6e67;
/// Maximum canonical storage for one record, including its terminal LF.
pub const EDITING_CANONICAL_MAX_BYTES: usize = 2048;

/// Observable progress at an actual public request/owner boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditingStage {
    /// Initial state was published after successful open.
    Opened,
    /// Request ownership and terminal capacity were reserved.
    Admitted,
    /// Native preparation finished outside Buffer authority.
    Prepared,
    /// Core atomically installed state and History.
    Committed,
    /// A semantic snapshot/update became available to an observer.
    UpdateReady,
    /// Snapshot production/retention failed; an accepted edit stays accepted.
    UpdateFailed,
    /// The observer received the exact qualified semantic update.
    Received,
    /// The observer installed that semantic state, not a presentation frame.
    Installed,
    /// Work terminated without an accepted edit.
    Rejected,
    /// Admission was fenced while owned work drains.
    Closing,
    /// Owned execution and release obligations are terminal.
    Closed,
}

impl EditingStage {
    /// Returns the schema's exact ASCII stage token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Opened => "opened",
            Self::Admitted => "admitted",
            Self::Prepared => "prepared",
            Self::Committed => "committed",
            Self::UpdateReady => "update-ready",
            Self::UpdateFailed => "update-failed",
            Self::Received => "received",
            Self::Installed => "installed",
            Self::Rejected => "rejected",
            Self::Closing => "closing",
            Self::Closed => "closed",
        }
    }
}

/// Safe identities only. Missing evidence is explicit, never guessed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditingContext {
    /// Originating Session; never replaced by the observing Session.
    pub session: Option<InteractionSessionHandle>,
    /// Independently qualified observer for delivery/client stages.
    pub observer: Option<InteractionSessionHandle>,
    /// Exact produced or received update, distinct from its origin request.
    pub update: Option<RepresentationUpdateIdentityStamp>,
    /// Request identity within that exact Session.
    pub request: Option<InteractionRequestIdentity>,
    /// Buffer-plan correlation, not an authority grant.
    pub correlation: Option<BufferCorrelationIdentity>,
    /// Exact observed state; contains no native data or selection.
    pub state: Option<BufferStateStamp>,
    /// Known causal parent within the same diagnostic stream.
    pub parent: Option<EventSequence>,
    /// A separately known incoming cause, for explicit joins rather than trees.
    pub additional_parent: Option<EventSequence>,
    /// Typed terminal rejection reason, never an arbitrary error string.
    pub cause: Option<InteractionOutcomeCause>,
}

/// A rejected diagnostic does not reject the associated editing request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditingRecordError {
    /// The injected clock could not qualify the supplied instant.
    Clock(ClockError),
    /// A causal parent does not precede this record in its stream.
    InvalidParent,
    /// Canonical rendering exceeds the declared wire/storage bound.
    EncodingBound,
}

impl fmt::Display for EditingRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "editing diagnostic: {self:?}")
    }
}

impl core::error::Error for EditingRecordError {}

/// Complete typed record with its one canonical rendering in inline storage.
///
/// The stream owner assigns sequence/producer and controls admission. This
/// constructor proves schema coherence, not admission or an edit's outcome.
#[derive(Clone)]
pub struct EditingRecord {
    stream: LifecycleIdentity,
    producer: LifecycleIdentity,
    sequence: EventSequence,
    clock: LifecycleIdentity,
    nanoseconds: u64,
    stage: EditingStage,
    context: EditingContext,
    canonical: encoding::Bytes,
}

impl EditingRecord {
    /// Renders a qualified record without allocation or callbacks.
    ///
    /// # Errors
    /// Rejects another clock generation, invalid parent or excessive encoding.
    pub fn new(
        stream: LifecycleIdentity,
        producer: LifecycleIdentity,
        sequence: EventSequence,
        clock: ClockDescriptor,
        instant: MonotonicInstant,
        stage: EditingStage,
        context: EditingContext,
    ) -> Result<Self, EditingRecordError> {
        clock.validate_stamp(instant).map_err(EditingRecordError::Clock)?;
        if context.parent.is_some_and(|parent| !parent.is_before(sequence))
            || context
                .additional_parent
                .is_some_and(|parent| !parent.is_before(sequence))
        {
            return Err(EditingRecordError::InvalidParent);
        }
        let mut record = Self {
            stream,
            producer,
            sequence,
            clock: clock.lifecycle(),
            nanoseconds: instant.elapsed_nanoseconds(),
            stage,
            context,
            canonical: encoding::Bytes::new(),
        };
        record.canonical = encoding::render(&record)?;
        Ok(record)
    }

    /// Returns the exact typed stage.
    #[must_use]
    pub const fn stage(&self) -> EditingStage {
        self.stage
    }

    /// Returns safe request/state evidence, not native content.
    #[must_use]
    pub const fn context(&self) -> &EditingContext {
        &self.context
    }

    /// Returns the stream-assigned sequence.
    #[must_use]
    pub const fn sequence(&self) -> EventSequence {
        self.sequence
    }

    /// Returns the exact stream incarnation qualifying causal references.
    #[must_use]
    pub const fn stream(&self) -> LifecycleIdentity {
        self.stream
    }

    /// Returns the exact source clock incarnation for stage-time comparison.
    #[must_use]
    pub const fn clock(&self) -> LifecycleIdentity {
        self.clock
    }

    /// Returns observed elapsed ticks; subtract only within this clock.
    #[must_use]
    pub const fn nanoseconds(&self) -> u64 {
        self.nanoseconds
    }

    /// Borrows the byte-identical record consumed by every diagnostic sink.
    ///
    /// # Panics
    /// Panics only if private canonical storage violates its nonempty bound.
    #[must_use]
    pub fn canonical(&self) -> CanonicalEventRecordRef<'_> {
        CanonicalEventRecordRef {
            stream: self.stream,
            family: ContractId::new(EDITING_FAMILY),
            schema_version: 2,
            sequence: self.sequence,
            bytes: self.canonical.as_slice(),
            size: CheckedByteCount::new(
                self.canonical.len() as u64,
                crate::bound::ZeroPolicy::Reject,
            )
            .expect("bounded nonempty canonical record"),
        }
    }
}

impl fmt::Debug for EditingRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EditingRecord")
            .field("sequence", &self.sequence)
            .field("stage", &self.stage)
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}
