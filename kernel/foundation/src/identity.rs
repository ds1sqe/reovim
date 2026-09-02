//! Explicit instance and incarnation identity primitives.

/// Opaque identity of one constructed contract or mechanism instance.
///
/// The value is supplied explicitly. Foundation has no ambient generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstanceId(u128);

impl InstanceId {
    /// Creates an identity from an Assembly- or authority-supplied value.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the stable scalar carried by this identity.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Opaque identity of one lifetime of an instance.
///
/// Replacement or reopen receives another explicitly supplied value even if
/// the associated instance identity is unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IncarnationId(u128);

impl IncarnationId {
    /// Creates an incarnation from an owner-supplied value.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the stable scalar carried by this identity.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}
