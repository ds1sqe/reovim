//! Independent capability dimensions for one Resource Access.

use core::fmt;

/// Authority granted over the already selected source Resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceAccessAuthority {
    /// The Access may observe but cannot publish to the source Resource.
    ObserveOnly,
    /// The Access may observe and use its selected publication grammar.
    ObserveAndPublish,
}

/// Stability promised for one Resource Identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceStability {
    /// Source state cannot change during the declared Resource lifetime.
    Stable,
    /// Source state may change independently of this Access.
    ExternallyEvolving,
}

/// Consistency scope of one immutable Snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotConsistency {
    /// One Snapshot coherently observes the complete Resource.
    WholeResource,
    /// Each declared native region is coherent under its own stamp.
    NativeRegion,
}

/// Historical Snapshot availability promised by the adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoricalAvailability {
    /// Only the current authoritative Snapshot may be requested.
    CurrentOnly,
    /// Historical Snapshots may be requested within declared retention.
    BoundedRetention,
}

/// Resource-native mutation grammar selected for source publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceMutationGrammar {
    /// Same-Resource publication is unavailable.
    None,
    /// Publication may replace selected Resource-native state.
    Replacement,
    /// Publication may only add under a family monotonicity relation.
    AddOnly,
    /// Publication may resize under Resource-family rules.
    Resize,
}

/// Independently granted destination-creation capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationCreation {
    /// This Access grants no destination creation authority.
    NotGranted,
    /// A separately authorized destination contract was selected.
    Granted,
}

/// Strength of external Resource change discovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeDiscovery {
    /// The adapter supplies no change discovery.
    None,
    /// A bounded notification invalidates current compatibility evidence.
    Invalidation,
    /// An ordered resumable change feed closes gaps through resynchronization.
    OrderedResumableFeed,
}

/// Typed incoherence between independent capability dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceCapabilityError {
    /// Observe-only source authority cannot select a mutation grammar.
    MutationWithoutPublicationAuthority,
    /// A stable Resource cannot advertise external change discovery.
    ChangeDiscoveryForStableResource,
}

impl fmt::Display for ResourceCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MutationWithoutPublicationAuthority => {
                "mutation requires source publication authority"
            }
            Self::ChangeDiscoveryForStableResource => {
                "stable Resource cannot advertise change discovery"
            }
        })
    }
}

impl core::error::Error for ResourceCapabilityError {}

/// One coherent selection of all independent Resource capability dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceCapabilities {
    source_access: SourceAccessAuthority,
    source_stability: SourceStability,
    snapshot_consistency: SnapshotConsistency,
    history: HistoricalAvailability,
    mutation: ResourceMutationGrammar,
    creation: DestinationCreation,
    change_discovery: ChangeDiscovery,
}

impl ResourceCapabilities {
    /// Constructs a capability set after validating cross-dimension rules.
    ///
    /// # Errors
    ///
    /// Returns a typed error for mutation without publication authority or
    /// change discovery on a Resource declared stable.
    pub const fn new(
        source_access: SourceAccessAuthority,
        source_stability: SourceStability,
        snapshot_consistency: SnapshotConsistency,
        history: HistoricalAvailability,
        mutation: ResourceMutationGrammar,
        creation: DestinationCreation,
        change_discovery: ChangeDiscovery,
    ) -> Result<Self, ResourceCapabilityError> {
        if observe_only_with_mutation(source_access, mutation) {
            return Err(
                ResourceCapabilityError::MutationWithoutPublicationAuthority,
            );
        }
        if stable_with_discovery(source_stability, change_discovery) {
            return Err(
                ResourceCapabilityError::ChangeDiscoveryForStableResource,
            );
        }
        Ok(Self {
            source_access,
            source_stability,
            snapshot_consistency,
            history,
            mutation,
            creation,
            change_discovery,
        })
    }

    /// Returns the source observation/publication authority.
    #[must_use]
    pub const fn source_access(self) -> SourceAccessAuthority {
        self.source_access
    }

    /// Returns the stability promised for one Resource Identity.
    #[must_use]
    pub const fn source_stability(self) -> SourceStability {
        self.source_stability
    }

    /// Returns the Snapshot consistency scope.
    #[must_use]
    pub const fn snapshot_consistency(self) -> SnapshotConsistency {
        self.snapshot_consistency
    }

    /// Returns the historical Snapshot availability.
    #[must_use]
    pub const fn history(self) -> HistoricalAvailability {
        self.history
    }

    /// Returns the selected same-Resource mutation grammar.
    #[must_use]
    pub const fn mutation(self) -> ResourceMutationGrammar {
        self.mutation
    }

    /// Returns the independent destination-creation capability.
    #[must_use]
    pub const fn creation(self) -> DestinationCreation {
        self.creation
    }

    /// Returns the selected external-change discovery strength.
    #[must_use]
    pub const fn change_discovery(self) -> ChangeDiscovery {
        self.change_discovery
    }
}

const fn observe_only_with_mutation(
    access: SourceAccessAuthority,
    mutation: ResourceMutationGrammar,
) -> bool {
    match (access, mutation) {
        (SourceAccessAuthority::ObserveOnly, ResourceMutationGrammar::None)
        | (SourceAccessAuthority::ObserveAndPublish, _) => false,
        (SourceAccessAuthority::ObserveOnly, _) => true,
    }
}

const fn stable_with_discovery(
    stability: SourceStability,
    discovery: ChangeDiscovery,
) -> bool {
    match (stability, discovery) {
        (SourceStability::Stable, ChangeDiscovery::None)
        | (SourceStability::ExternallyEvolving, _) => false,
        (SourceStability::Stable, _) => true,
    }
}
