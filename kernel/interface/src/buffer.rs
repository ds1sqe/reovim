//! Domain-neutral Buffer identity and state stamps.

use crate::lifecycle::LifecycleIdentity;
use crate::resource::ResourceSnapshotStamp;

pub mod bounds;
pub mod carrier;
pub mod plan;

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

/// Ordered boundary between applied and redo-eligible History entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferHistoryPosition(u64);

impl BufferHistoryPosition {
    /// Initial position of an empty History.
    pub const INITIAL: Self = Self(0);

    /// Constructs an authority-supplied History position.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the opaque ordered position scalar.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Exact History incarnation, generation, and current position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferHistoryStamp {
    lifecycle: LifecycleIdentity,
    position: BufferHistoryPosition,
}

impl BufferHistoryStamp {
    /// Constructs one observed History state.
    #[must_use]
    pub const fn new(
        lifecycle: LifecycleIdentity,
        position: BufferHistoryPosition,
    ) -> Self {
        Self { lifecycle, position }
    }

    /// Returns the History lifecycle identity.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Returns the current History position.
    #[must_use]
    pub const fn position(self) -> BufferHistoryPosition {
        self.position
    }

    /// Reports whether two values name the same complete History state.
    #[must_use]
    pub const fn same_state(self, other: Self) -> bool {
        self.lifecycle.same_generation(other.lifecycle)
            && self.position.value() == other.position.value()
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
