//! Domain-neutral Buffer identity and state stamps.

use crate::lifecycle::LifecycleIdentity;
use crate::resource::ResourceSnapshotStamp;

pub mod bounds;
pub mod carrier;
pub mod history;
pub mod plan;

pub use history::{BufferHistoryEdge, BufferHistoryEdgeError};

/// Stable opaque identity of one Buffer working context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferIdentity(u128);

impl BufferIdentity {
    /// Constructs an authority-supplied Buffer identity.
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

/// Monotonic local version of one Buffer incarnation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferVersion(u64);

impl BufferVersion {
    /// Initial version before any local Transaction is accepted.
    pub const INITIAL: Self = Self(0);

    /// Constructs an authority-supplied local Buffer version.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the local version scalar.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Exact opened lifetime and generation of one Buffer identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferIncarnationStamp {
    buffer: BufferIdentity,
    lifecycle: LifecycleIdentity,
}

impl BufferIncarnationStamp {
    /// Binds a Buffer identity to one lifecycle incarnation and generation.
    #[must_use]
    pub const fn new(
        buffer: BufferIdentity,
        lifecycle: LifecycleIdentity,
    ) -> Self {
        Self { buffer, lifecycle }
    }

    /// Returns the stable Buffer identity.
    #[must_use]
    pub const fn buffer(self) -> BufferIdentity {
        self.buffer
    }

    /// Returns the Buffer lifecycle identity.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Reports whether two stamps name one Buffer incarnation.
    #[must_use]
    pub const fn same_incarnation(self, other: Self) -> bool {
        self.buffer.value() == other.buffer.value()
            && self.lifecycle.same_incarnation(other.lifecycle)
    }

    /// Reports whether two stamps name one complete Buffer generation.
    #[must_use]
    pub const fn same_generation(self, other: Self) -> bool {
        self.buffer.value() == other.buffer.value()
            && self.lifecycle.same_generation(other.lifecycle)
    }
}

/// Requested movement through the Buffer's authoritative History.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferHistoryDirection {
    /// Restore the current node's parent.
    Undo,
    /// Restore a selected child of the current node.
    Redo,
}

/// Opaque retained-state identity within one History incarnation.
///
/// Identity implies neither ordering, depth nor adjacency. Core supplies the
/// root identity too; no distinguished scalar means root or empty History.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferHistoryNodeIdentity(u128);

impl BufferHistoryNodeIdentity {
    /// Constructs an authority-supplied node identity, not a tree index.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque identity scalar, never an ordinal position.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Exact History incarnation, generation, and selected node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferHistoryStamp {
    lifecycle: LifecycleIdentity,
    node: BufferHistoryNodeIdentity,
}

impl BufferHistoryStamp {
    /// Constructs one observed History state.
    #[must_use]
    pub const fn new(
        lifecycle: LifecycleIdentity,
        node: BufferHistoryNodeIdentity,
    ) -> Self {
        Self { lifecycle, node }
    }

    /// Returns the History lifecycle identity.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Returns the selected History node, independent of Buffer version.
    #[must_use]
    pub const fn node(self) -> BufferHistoryNodeIdentity {
        self.node
    }

    /// Reports whether two values name the same complete History state.
    #[must_use]
    pub const fn same_state(self, other: Self) -> bool {
        self.lifecycle.same_generation(other.lifecycle)
            && self.node.value() == other.node.value()
    }
}

/// Exact Resource, Buffer, local-version, and History state observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferStateStamp {
    source: ResourceSnapshotStamp,
    buffer: BufferIncarnationStamp,
    version: BufferVersion,
    history: BufferHistoryStamp,
}

impl BufferStateStamp {
    /// Constructs one coherent caller-observed target stamp.
    #[must_use]
    pub const fn new(
        source: ResourceSnapshotStamp,
        buffer: BufferIncarnationStamp,
        version: BufferVersion,
        history: BufferHistoryStamp,
    ) -> Self {
        Self { source, buffer, version, history }
    }

    /// Returns the exact base Resource Snapshot.
    #[must_use]
    pub const fn source(self) -> ResourceSnapshotStamp {
        self.source
    }

    /// Returns the exact Buffer incarnation and generation.
    #[must_use]
    pub const fn buffer(self) -> BufferIncarnationStamp {
        self.buffer
    }

    /// Returns the observed local Buffer version.
    #[must_use]
    pub const fn version(self) -> BufferVersion {
        self.version
    }

    /// Returns the observed History state.
    #[must_use]
    pub const fn history(self) -> BufferHistoryStamp {
        self.history
    }

    /// Reports whether both values name the same exact Buffer target state.
    #[must_use]
    pub const fn same_state(self, other: Self) -> bool {
        self.source.same_snapshot(other.source)
            && self.buffer.same_generation(other.buffer)
            && self.version.value() == other.version.value()
            && self.history.same_state(other.history)
    }
}
