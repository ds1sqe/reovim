//! Prepared-root requests, limits, observations, and typed failures.

use std::{fmt, path::PathBuf, time::Duration};

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FactSet(u16);

impl FactSet {
    const fn empty() -> Self {
        Self(0)
    }

    const fn first(count: u8) -> Self {
        Self((1_u16 << count) - 1)
    }

    const fn record(&mut self, fact: u8, value: bool) {
        let mask = 1_u16 << fact;
        if value {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    const fn contains(self, fact: u8) -> bool {
        self.0 & (1_u16 << fact) != 0
    }

    const fn contains_all(self, count: u8) -> bool {
        self.0 == Self::first(count).0
    }
}

/// Hidden same-binary setup dispatch argument.
pub const SETUP_ARGUMENT: &str = "product-proof-child";

/// Exact environment slot carrying bounded setup JSON without changing argv.
pub const CONFIG_ENVIRONMENT: &str = "REOVIM_PRODUCT_PROOF_CONFIG";

/// Hidden same-binary namespace-PID-1 dispatch argument.
pub const PID1_ARGUMENT: &str = "--reovim-prepared-root-pid1";

/// Bounds applied before a dispatch payload is trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchBounds {
    pub config_bytes: usize,
    pub result_bytes: usize,
    pub json_depth: usize,
    pub json_items: usize,
    pub execution_deadline: Duration,
    pub supervision_deadline: Duration,
}

/// Bounds for every recursively observed exposure source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryLimits {
    pub path_bytes: usize,
    pub entry_count: usize,
    pub total_bytes: u64,
}

/// Exact tmpfs capacity selected for the prepared root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScratchLimits {
    pub size_bytes: u64,
    pub inode_count: u64,
}

/// Exact per-process limits applied by namespace PID 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    pub address_space_bytes: u64,
    pub cpu_seconds: u64,
    pub open_files: u64,
    pub output_file_bytes: u64,
    pub core_dump_bytes: u64,
}

/// Filesystem shape of one read-only prepared-root exposure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExposureKind {
    File,
    Directory,
    CharacterDevice,
}

impl ExposureKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::CharacterDevice => "character_device",
        }
    }
}

/// One exact host source mapped read-only into the prepared root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExposurePlan {
    pub id: String,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub kind: ExposureKind,
    pub executable: bool,
    pub purpose: String,
}

/// Complete caller-owned plan for one prepared-root execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub repository_root: PathBuf,
    pub mount_parent: PathBuf,
    pub checker_executable: PathBuf,
    pub execution_environment: String,
    pub workload_request: Value,
    pub exposures: Vec<ExposurePlan>,
    pub inventory_limits: InventoryLimits,
    pub scratch_limits: ScratchLimits,
    pub resource_limits: ResourceLimits,
    pub dispatch_bounds: DispatchBounds,
    pub workload_result_bytes: usize,
    pub descendant_reap_limit: usize,
}

/// Read-only capability probe result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capability {
    Ready,
    Unavailable { code: &'static str, message: String },
}

/// Exact exposure inventory summary used for pre/post identity joins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryIdentity {
    pub entry_count: usize,
    pub total_bytes: u64,
    pub aggregate_digest: String,
    pub entries: Vec<InventoryEntry>,
}

/// Closed kind of one visible exposure-inventory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InventoryEntryKind {
    File,
    Directory,
    CharacterDevice,
}

impl InventoryEntryKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::CharacterDevice => "character_device",
        }
    }
}

/// One exact relative path and filesystem identity visible through a mount.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InventoryEntry {
    pub relative_path: PathBuf,
    pub kind: InventoryEntryKind,
    pub mode: u32,
    pub size_bytes: u64,
    pub content_digest: Option<String>,
    pub device_identity: Option<u64>,
}

/// Stable evidence for one mounted read-only exposure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExposureObservation {
    pub id: String,
    pub destination: PathBuf,
    pub kind: ExposureKind,
    pub executable: bool,
    pub purpose: String,
    pub inventory: InventoryIdentity,
    pub device_major: Option<u64>,
    pub device_minor: Option<u64>,
    pub mount_flags: MountFlags,
}

/// Closed mount-option evidence without an open string set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MountFlags(FactSet);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MountFact {
    ReadOnly = 0,
    Nosuid = 1,
    Nodev = 2,
    Noexec = 3,
}

impl MountFlags {
    pub(super) const fn empty() -> Self {
        Self(FactSet::empty())
    }

    pub(super) const fn record(&mut self, fact: MountFact, value: bool) {
        self.0.record(fact as u8, value);
    }

    #[must_use]
    pub const fn read_only(self) -> bool {
        self.0.contains(MountFact::ReadOnly as u8)
    }

    #[must_use]
    pub const fn nosuid(self) -> bool {
        self.0.contains(MountFact::Nosuid as u8)
    }

    #[must_use]
    pub const fn nodev(self) -> bool {
        self.0.contains(MountFact::Nodev as u8)
    }

    #[must_use]
    pub const fn noexec(self) -> bool {
        self.0.contains(MountFact::Noexec as u8)
    }
}

/// Namespace and parent-death relations without ephemeral namespace IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceObservation {
    facts: FactSet,
    pub uid_map: String,
    pub gid_map: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NamespaceFact {
    UserPrivate = 0,
    MountPrivate = 1,
    NetworkPrivate = 2,
    PidPrivate = 3,
    UidMapSingle = 4,
    GidMapSingle = 5,
    SetgroupsDenied = 6,
    PrivatePropagation = 7,
    PidOne = 8,
    ParentDeathSignal = 9,
}

impl NamespaceObservation {
    pub(super) const fn empty(uid_map: String, gid_map: String) -> Self {
        Self { facts: FactSet::empty(), uid_map, gid_map }
    }

    pub(super) const fn record(&mut self, fact: NamespaceFact, value: bool) {
        self.facts.record(fact as u8, value);
    }

    #[must_use]
    pub const fn user_private(&self) -> bool {
        self.has(NamespaceFact::UserPrivate)
    }

    #[must_use]
    pub const fn mount_private(&self) -> bool {
        self.has(NamespaceFact::MountPrivate)
    }

    #[must_use]
    pub const fn network_private(&self) -> bool {
        self.has(NamespaceFact::NetworkPrivate)
    }

    #[must_use]
    pub const fn pid_private(&self) -> bool {
        self.has(NamespaceFact::PidPrivate)
    }

    #[must_use]
    pub const fn uid_map_single(&self) -> bool {
        self.has(NamespaceFact::UidMapSingle)
    }

    #[must_use]
    pub const fn gid_map_single(&self) -> bool {
        self.has(NamespaceFact::GidMapSingle)
    }

    #[must_use]
    pub const fn setgroups_denied(&self) -> bool {
        self.has(NamespaceFact::SetgroupsDenied)
    }

    #[must_use]
    pub const fn private_propagation(&self) -> bool {
        self.has(NamespaceFact::PrivatePropagation)
    }

    #[must_use]
    pub const fn pid_one(&self) -> bool {
        self.has(NamespaceFact::PidOne)
    }

    #[must_use]
    pub const fn parent_death_signal(&self) -> bool {
        self.has(NamespaceFact::ParentDeathSignal)
    }

    #[must_use]
    pub const fn complete(&self) -> bool {
        self.facts.contains_all(10)
    }

    const fn has(&self, fact: NamespaceFact) -> bool {
        self.facts.contains(fact as u8)
    }
}

/// Bounded network state establishing absence of an external carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkObservation {
    pub interfaces: Vec<String>,
    pub ipv4_route_count: usize,
    pub ipv6_route_count: usize,
    pub kernel_route_count: usize,
    pub non_packet_socket_count: usize,
    pub packet_socket_count: usize,
    pub policy_observation_digest: String,
    pub policy_basis: String,
    pub external_carrier_absent: bool,
}

/// Observed tmpfs quota and use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScratchObservation {
    pub size_limit_bytes: u64,
    pub inode_limit: u64,
    pub allocated_bytes: u64,
    pub inode_count: u64,
    pub logical_bytes: u64,
    pub mode_owner_only: bool,
    pub mount_flags: MountFlags,
}

/// Bounded caller-supplied workload result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkloadObservation {
    pub status: String,
    pub bytes: u64,
    pub digest: String,
    pub value: Value,
}

/// Cleanup facts accepted only after all owned effects are reconciled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupObservation(FactSet);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CleanupFact {
    DescendantsReaped = 0,
    ReadersJoined = 1,
    ProcUnmounted = 2,
    ExposuresUnmounted = 3,
    ScratchUnmounted = 4,
    SourceIdentityPreserved = 5,
    ToolIdentityPreserved = 6,
    NamespaceAbsent = 7,
    MountAbsent = 8,
    MountpointIdentityPreserved = 9,
    UnderlyingDirectoryEmpty = 10,
}

impl CleanupObservation {
    pub(super) const fn verified() -> Self {
        Self(FactSet::first(11))
    }

    #[must_use]
    pub const fn descendants_reaped(&self) -> bool {
        self.has(CleanupFact::DescendantsReaped)
    }

    #[must_use]
    pub const fn readers_joined(&self) -> bool {
        self.has(CleanupFact::ReadersJoined)
    }

    #[must_use]
    pub const fn proc_unmounted(&self) -> bool {
        self.has(CleanupFact::ProcUnmounted)
    }

    #[must_use]
    pub const fn exposures_unmounted(&self) -> bool {
        self.has(CleanupFact::ExposuresUnmounted)
    }

    #[must_use]
    pub const fn scratch_unmounted(&self) -> bool {
        self.has(CleanupFact::ScratchUnmounted)
    }

    #[must_use]
    pub const fn source_identity_preserved(&self) -> bool {
        self.has(CleanupFact::SourceIdentityPreserved)
    }

    #[must_use]
    pub const fn tool_identity_preserved(&self) -> bool {
        self.has(CleanupFact::ToolIdentityPreserved)
    }

    #[must_use]
    pub const fn namespace_absent(&self) -> bool {
        self.has(CleanupFact::NamespaceAbsent)
    }

    #[must_use]
    pub const fn mount_absent(&self) -> bool {
        self.has(CleanupFact::MountAbsent)
    }

    #[must_use]
    pub const fn mountpoint_identity_preserved(&self) -> bool {
        self.has(CleanupFact::MountpointIdentityPreserved)
    }

    #[must_use]
    pub const fn underlying_directory_empty(&self) -> bool {
        self.has(CleanupFact::UnderlyingDirectoryEmpty)
    }

    const fn has(&self, fact: CleanupFact) -> bool {
        self.0.contains(fact as u8)
    }
}

/// Complete stable prepared-root result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub execution_environment: String,
    pub exposures: Vec<ExposureObservation>,
    pub namespaces: NamespaceObservation,
    pub network_before: NetworkObservation,
    pub network_after: NetworkObservation,
    pub scratch: ScratchObservation,
    pub workload: WorkloadObservation,
    pub cleanup: CleanupObservation,
}

/// Fixed in-chroot paths exposed to the workload callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkloadContext {
    pub scratch_root: PathBuf,
    pub repository_root: Option<PathBuf>,
    pub workload_request: Value,
}

/// Typed setup, execution, observation, or cleanup failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: &'static str,
    pub subject: Option<String>,
    pub message: String,
}

impl Error {
    pub(super) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, subject: None, message: message.into() }
    }

    pub(super) fn at(
        code: &'static str,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self { code, subject: Some(subject.into()), message: message.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(subject) = &self.subject {
            return write!(
                formatter,
                "{}: {}: {}",
                self.code, subject, self.message
            );
        }
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Config {
    pub mountpoint: PathBuf,
    pub repository_root: PathBuf,
    pub execution_environment: String,
    pub workload_request: Value,
    pub checker_executable: PathBuf,
    pub exposures: Vec<ConfiguredExposure>,
    pub inventory_limits: InventoryLimits,
    pub scratch_limits: ScratchLimits,
    pub resource_limits: ResourceLimits,
    pub dispatch_bounds: DispatchBounds,
    pub workload_result_bytes: usize,
    pub descendant_reap_limit: usize,
    pub parent_namespaces: NamespaceLinks,
    pub outside_uid: u32,
    pub outside_gid: u32,
}

/// Parent-supervised execution plus stable prepared-root evidence.
#[derive(Debug)]
pub struct Execution {
    pub evidence: Evidence,
    pub process: crate::process::ProcessResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConfiguredExposure {
    pub plan: ExposurePlan,
    pub expected: InventoryIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NamespaceLinks {
    pub user: String,
    pub mount: String,
    pub network: String,
    pub pid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Pid1Result {
    pub namespaces: NamespaceObservation,
    pub network_before: NetworkObservation,
    pub network_after: NetworkObservation,
    pub scratch: ScratchObservation,
    pub workload: WorkloadObservation,
    pub descendants_reaped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SetupResult {
    pub exposures: Vec<ExposureObservation>,
    pub pid1: Pid1Result,
    pub cleanup: SetupCleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SetupCleanup(FactSet);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SetupFact {
    ReadersJoined = 0,
    ProcUnmounted = 1,
    ExposuresUnmounted = 2,
    ScratchUnmounted = 3,
    SourceIdentityPreserved = 4,
    ToolIdentityPreserved = 5,
}

impl SetupCleanup {
    pub(super) const fn empty() -> Self {
        Self(FactSet::empty())
    }

    pub(super) const fn record(&mut self, fact: SetupFact, value: bool) {
        self.0.record(fact as u8, value);
    }

    pub(super) const fn readers_joined(self) -> bool {
        self.has(SetupFact::ReadersJoined)
    }

    pub(super) const fn proc_unmounted(self) -> bool {
        self.has(SetupFact::ProcUnmounted)
    }

    pub(super) const fn exposures_unmounted(self) -> bool {
        self.has(SetupFact::ExposuresUnmounted)
    }

    pub(super) const fn scratch_unmounted(self) -> bool {
        self.has(SetupFact::ScratchUnmounted)
    }

    pub(super) const fn source_identity_preserved(self) -> bool {
        self.has(SetupFact::SourceIdentityPreserved)
    }

    pub(super) const fn tool_identity_preserved(self) -> bool {
        self.has(SetupFact::ToolIdentityPreserved)
    }

    const fn has(self, fact: SetupFact) -> bool {
        self.0.contains(fact as u8)
    }
}
