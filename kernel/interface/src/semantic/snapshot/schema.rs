/// Exact selected semantic snapshot type/version, not a negotiation range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationSnapshotSchema {
    type_identity: u128,
    schema_version: u64,
}

impl RepresentationSnapshotSchema {
    /// Names the exact encoding selected by the snapshot descriptor.
    #[must_use]
    pub const fn new(type_identity: u128, schema_version: u64) -> Self {
        Self { type_identity, schema_version }
    }

    /// Returns the semantic encoding family's opaque identity.
    #[must_use]
    pub const fn type_identity(self) -> u128 {
        self.type_identity
    }

    /// Returns the selected version, not the set of compatible versions.
    #[must_use]
    pub const fn schema_version(self) -> u64 {
        self.schema_version
    }
}
