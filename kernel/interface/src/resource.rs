//! Domain-neutral Resource state stamps shared by editing contracts.

use crate::lifecycle::LifecycleIdentity;
use crate::operation::ContractSelection;

pub mod access;
pub mod bounds;
pub mod capability;

/// Stable opaque identity of one Resource subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceIdentity(u128);

impl ResourceIdentity {
    /// Constructs an authority-supplied Resource identity.
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

/// Address, Extent, Unit, and Revision schemas for one Resource family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceValueSchemas {
    address: ContractSelection,
    extent: ContractSelection,
    unit: ContractSelection,
    revision: ContractSelection,
}

impl ResourceValueSchemas {
    /// Constructs the four independently selected Resource value schemas.
    #[must_use]
    pub const fn new(
        address: ContractSelection,
        extent: ContractSelection,
        unit: ContractSelection,
        revision: ContractSelection,
    ) -> Self {
        Self { address, extent, unit, revision }
    }

    /// Returns the selected Address schema.
    #[must_use]
    pub const fn address(self) -> ContractSelection {
        self.address
    }

    /// Returns the selected Extent schema.
    #[must_use]
    pub const fn extent(self) -> ContractSelection {
        self.extent
    }

    /// Returns the selected Unit schema.
    #[must_use]
    pub const fn unit(self) -> ContractSelection {
        self.unit
    }

    /// Returns the selected Revision schema.
    #[must_use]
    pub const fn revision(self) -> ContractSelection {
        self.revision
    }
}

/// Exact live Resource Access generation for one Resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceAccessStamp {
    resource: ResourceIdentity,
    lifecycle: LifecycleIdentity,
}

impl ResourceAccessStamp {
    /// Binds a Resource to one Access instance, incarnation, and generation.
    #[must_use]
    pub const fn new(
        resource: ResourceIdentity,
        lifecycle: LifecycleIdentity,
    ) -> Self {
        Self { resource, lifecycle }
    }

    /// Returns the Resource governed by this Access.
    #[must_use]
    pub const fn resource(self) -> ResourceIdentity {
        self.resource
    }

    /// Returns the Access instance, incarnation, and generation.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Reports whether two stamps name the same Access incarnation.
    #[must_use]
    pub const fn same_incarnation(self, other: Self) -> bool {
        self.resource.value() == other.resource.value()
            && self.lifecycle.same_incarnation(other.lifecycle)
    }

    /// Reports whether two stamps name the same complete Access generation.
    #[must_use]
    pub const fn same_generation(self, other: Self) -> bool {
        self.resource.value() == other.resource.value()
            && self.lifecycle.same_generation(other.lifecycle)
    }
}

/// Opaque Resource-native Revision under one selected schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceRevision {
    schema: ContractSelection,
    token: u128,
}

impl ResourceRevision {
    /// Constructs an authority-supplied Revision token.
    #[must_use]
    pub const fn new(schema: ContractSelection, token: u128) -> Self {
        Self { schema, token }
    }

    /// Returns the selected Resource-native Revision schema.
    #[must_use]
    pub const fn schema(self) -> ContractSelection {
        self.schema
    }

    /// Returns the opaque equality token under the selected schema.
    #[must_use]
    pub const fn token(self) -> u128 {
        self.token
    }
}

/// Opaque identity of one immutable Snapshot observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceSnapshotIdentity(u128);

impl ResourceSnapshotIdentity {
    /// Constructs an adapter-supplied Snapshot identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque Snapshot identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Exact Resource, Access, Revision, and Snapshot observation stamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceSnapshotStamp {
    access: ResourceAccessStamp,
    revision: ResourceRevision,
    snapshot: ResourceSnapshotIdentity,
}

impl ResourceSnapshotStamp {
    /// Constructs one immutable observation stamp.
    #[must_use]
    pub const fn new(
        access: ResourceAccessStamp,
        revision: ResourceRevision,
        snapshot: ResourceSnapshotIdentity,
    ) -> Self {
        Self { access, revision, snapshot }
    }

    /// Returns the exact Resource Access generation.
    #[must_use]
    pub const fn access(self) -> ResourceAccessStamp {
        self.access
    }

    /// Returns the Resource-native Revision stamp.
    #[must_use]
    pub const fn revision(self) -> ResourceRevision {
        self.revision
    }

    /// Returns the immutable Snapshot identity.
    #[must_use]
    pub const fn snapshot(self) -> ResourceSnapshotIdentity {
        self.snapshot
    }

    /// Reports whether both values name the same exact Snapshot.
    #[must_use]
    pub const fn same_snapshot(self, other: Self) -> bool {
        self.access.same_generation(other.access)
            && revisions_match(self.revision, other.revision)
            && self.snapshot.value() == other.snapshot.value()
    }

    /// Reports whether both values name the same observed Revision.
    #[must_use]
    pub const fn same_revision(self, other: Self) -> bool {
        self.access.same_generation(other.access)
            && revisions_match(self.revision, other.revision)
    }
}

const fn revisions_match(
    left: ResourceRevision,
    right: ResourceRevision,
) -> bool {
    left.schema.identity() == right.schema.identity()
        && left.schema.selected() == right.schema.selected()
        && left.schema.minimum() == right.schema.minimum()
        && left.schema.maximum() == right.schema.maximum()
        && left.token == right.token
}
