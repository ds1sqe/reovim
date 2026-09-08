use reovim_kernel_interface::bound::{CheckedByteCount, CheckedCount};
use reovim_kernel_interface::buffer::BufferIncarnationStamp;
use reovim_kernel_interface::lifecycle::LifecycleIdentity;
use reovim_kernel_interface::resource::ResourceSnapshotStamp;
use reovim_kernel_interface::resource::state::{
    ResourceStateLimits, ResourceStateModelBinding,
};

/// Retention reservations for one Buffer; exhaustion never evicts History.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferLimits {
    /// Retained nodes including the initial state, at least one.
    pub history_nodes: CheckedCount,
    /// Aggregate conservative native-state charge of all retained nodes.
    /// Node metadata is separately bounded by `history_nodes`.
    pub resident_bytes: CheckedByteCount,
}

/// Assembly-selected identity and contracts, not Client-supplied authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferConfig {
    /// Exact coherent initial Resource observation reserved by Runtime.
    pub source: ResourceSnapshotStamp,
    /// New Buffer lifetime, published by Runtime only after initialization.
    pub incarnation: BufferIncarnationStamp,
    /// New History lifetime; Core supplies its initial and subsequent nodes.
    pub history: LifecycleIdentity,
    /// Selected native provider and schemas; replacement fences this Buffer.
    pub model: ResourceStateModelBinding,
    /// Authority of the native plans admitted to this Buffer.
    pub plan_owner: LifecycleIdentity,
    /// Fixed reservations acquired before the Buffer becomes authoritative.
    pub limits: BufferLimits,
    /// Per-candidate native preparation and installation bounds.
    pub preparation: ResourceStateLimits,
}
