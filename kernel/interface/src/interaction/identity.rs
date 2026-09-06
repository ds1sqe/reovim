//! Opaque identities for one Interaction Session and its deliveries.

/// Stable opaque identity of one Client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientIdentity(u128);

impl ClientIdentity {
    /// Constructs an authority-supplied Client identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Stable opaque identity of one Contract Session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionSessionIdentity(u128);

impl InteractionSessionIdentity {
    /// Constructs an authority-supplied Session identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Stable identity of one Representation attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationAttachmentIdentity(u128);

impl RepresentationAttachmentIdentity {
    /// Constructs an authority-supplied attachment identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Identity of one Interaction request within a Session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionRequestIdentity(u128);

impl InteractionRequestIdentity {
    /// Constructs a caller-supplied request identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Identity of cancellation authority for one Interaction request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionCancellationIdentity(u128);

impl InteractionCancellationIdentity {
    /// Constructs a caller-supplied cancellation identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Root causal identity for one Client interaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionCorrelationIdentity(u128);

impl InteractionCorrelationIdentity {
    /// Constructs a caller-supplied correlation identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Identity of a separately governed Interaction deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionDeadlineIdentity(u128);

impl InteractionDeadlineIdentity {
    /// Constructs an outside-authority deadline identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Identity used to reconcile one uncertain or unavailable Outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionReconcileIdentity(u128);

impl InteractionReconcileIdentity {
    /// Constructs a Runtime-supplied reconciliation identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Identity of one Representation Update stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateStreamIdentity(u128);

impl RepresentationUpdateStreamIdentity {
    /// Constructs a Runtime-supplied stream identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Identity of one Representation Update item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateIdentity(u128);

impl RepresentationUpdateIdentity {
    /// Constructs a Runtime-supplied Update identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Identity of one bounded full-state resynchronization request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationResynchronizationIdentity(u128);

impl RepresentationResynchronizationIdentity {
    /// Constructs a Client-supplied resynchronization identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Monotonic position in one Representation Update stream incarnation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateCursor(u64);

impl RepresentationUpdateCursor {
    /// Cursor before the first delivered Update.
    pub const INITIAL: Self = Self(0);

    /// Constructs a Runtime-supplied stream cursor.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the monotonic cursor scalar.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Returns the next cursor when the sequence has capacity.
    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}
