//! Contract identity and compatible-version primitives.

use core::fmt;

/// Opaque identity of one versioned contract family.
///
/// The value is supplied explicitly. Foundation does not allocate, generate,
/// discover, or assign contract identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContractId(u128);

impl ContractId {
    /// Creates an identity from a contract-authority supplied value.
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

/// Monotonic numeric version inside one contract family.
///
/// This value is independent of the Reovim package version and does not
/// imply Semantic Versioning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContractVersion(u64);

impl ContractVersion {
    /// Creates an explicitly selected contract version.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric version selected by the contract authority.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Failure to construct an inclusive compatible-version range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VersionRangeError {
    minimum: ContractVersion,
    maximum: ContractVersion,
}

impl VersionRangeError {
    const fn inverted(
        minimum: ContractVersion,
        maximum: ContractVersion,
    ) -> Self {
        Self { minimum, maximum }
    }

    /// Returns the rejected lower endpoint.
    #[must_use]
    pub const fn minimum(self) -> ContractVersion {
        self.minimum
    }

    /// Returns the rejected upper endpoint.
    #[must_use]
    pub const fn maximum(self) -> ContractVersion {
        self.maximum
    }
}

impl fmt::Display for VersionRangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("compatible contract range is inverted")
    }
}

impl std::error::Error for VersionRangeError {}

/// Inclusive versions accepted for one separately identified contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompatibleVersionRange {
    minimum: ContractVersion,
    maximum: ContractVersion,
}

impl CompatibleVersionRange {
    /// Constructs an inclusive range after checking endpoint order.
    ///
    /// # Errors
    ///
    /// Returns [`VersionRangeError`] when `minimum` is greater than
    /// `maximum`.
    pub const fn new(
        minimum: ContractVersion,
        maximum: ContractVersion,
    ) -> Result<Self, VersionRangeError> {
        if minimum.value() > maximum.value() {
            return Err(VersionRangeError::inverted(minimum, maximum));
        }
        Ok(Self { minimum, maximum })
    }

    /// Returns the inclusive lower endpoint.
    #[must_use]
    pub const fn minimum(self) -> ContractVersion {
        self.minimum
    }

    /// Returns the inclusive upper endpoint.
    #[must_use]
    pub const fn maximum(self) -> ContractVersion {
        self.maximum
    }

    /// Reports whether `version` lies inside this exact inclusive range.
    #[must_use]
    pub const fn contains(self, version: ContractVersion) -> bool {
        version.value() >= self.minimum.value()
            && version.value() <= self.maximum.value()
    }
}
