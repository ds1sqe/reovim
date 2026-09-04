//! Opaque identities for semantic policy and requests.

/// Stable opaque identity of one semantic Domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainIdentity(u128);

impl DomainIdentity {
    /// Constructs an authority-supplied Domain identity.
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

/// Stable opaque identity of one Representation policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationIdentity(u128);

impl RepresentationIdentity {
    /// Constructs an authority-supplied Representation identity.
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

/// Stable opaque identity of one maintained Projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionIdentity(u128);

impl ProjectionIdentity {
    /// Constructs an authority-supplied Projection identity.
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

/// Exact predecessor generation of one maintained Projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionGeneration(u64);

impl ProjectionGeneration {
    /// Initial generation before a Projection result is installed.
    pub const INITIAL: Self = Self(0);

    /// Constructs an authority-supplied Projection generation.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the ordered generation scalar.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Stable opaque identity of one semantic coordinate space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoordinateSpaceIdentity(u128);

impl CoordinateSpaceIdentity {
    /// Constructs an authority-supplied coordinate-space identity.
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

/// Identity of one semantic request within its caller authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticRequestIdentity(u128);

impl SemanticRequestIdentity {
    /// Constructs an authority-supplied semantic request identity.
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

/// Unique identity of one immutable semantic Intent value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticIntentIdentity(u128);

impl SemanticIntentIdentity {
    /// Constructs an authority-supplied semantic Intent identity.
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
