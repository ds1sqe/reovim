//! Projection store — server-side cache for domain projections.
//!
//! Manages versioned projection state per (ClientId, ProjectionTag).
//! Persistent projections are deduplicated by payload equality.
//! Transient projections are forwarded immediately and never cached.

use std::collections::HashMap;

use reovim_subsys_coordination::{
    DomainId, Projection, ProjectionDelivery, ProjectionTag,
};
use reovim_subsys_session::ClientId;

/// A stored projection entry with version tracking.
#[derive(Debug, Clone)]
struct StoredProjection {
    projection: Projection,
    version: u64,
}

/// Server-side projection store.
///
/// Caches persistent projections per (ClientId, ProjectionTag).
/// Assigns monotonic versions per slot. Deduplicates by payload equality.
#[derive(Debug, Default)]
pub struct ProjectionStore {
    /// Stored projections keyed by (client_id, tag).
    entries: HashMap<(usize, ProjectionTag), StoredProjection>,
    /// Next version to assign (monotonically increasing).
    next_version: u64,
}

/// Result of updating the store with new projections.
#[derive(Debug)]
pub struct StoreUpdateResult {
    /// Persistent projections that changed (new version assigned).
    pub changed: Vec<VersionedProjection>,
    /// Transient projections to forward immediately.
    pub transient: Vec<Projection>,
}

/// A projection with its assigned version.
#[derive(Debug, Clone)]
pub struct VersionedProjection {
    pub projection: Projection,
    pub version: u64,
}

impl ProjectionStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed the store with initial projections for a new client.
    ///
    /// Called BEFORE sending JoinResponse (D15 ordering).
    /// Transient projections are debug_assert'd and filtered (D21).
    pub fn seed(&mut self, client_id: ClientId, projections: Vec<Projection>) {
        for proj in projections {
            if proj.delivery == ProjectionDelivery::Transient {
                debug_assert!(
                    false,
                    "initial_projections must return only Persistent projections"
                );
                continue;
            }
            self.insert(client_id, proj);
        }
    }

    /// Update the store with projections from `collect_projections`.
    ///
    /// Returns which projections changed (for notification) and which
    /// are transient (for immediate forwarding).
    pub fn update(
        &mut self,
        client_id: ClientId,
        projections: Vec<Projection>,
    ) -> StoreUpdateResult {
        let mut changed = Vec::new();
        let mut transient = Vec::new();

        for proj in projections {
            match proj.delivery {
                ProjectionDelivery::Transient => {
                    // Forward immediately, never cache
                    transient.push(proj);
                }
                ProjectionDelivery::Persistent => {
                    let key = (client_id.as_usize(), proj.tag.clone());

                    // Deduplicate by payload equality
                    if let Some(existing) = self.entries.get(&key) {
                        if existing.projection.payload == proj.payload {
                            continue; // No change, skip notification
                        }
                    }

                    let version = self.next_version;
                    self.next_version += 1;

                    changed.push(VersionedProjection {
                        projection: proj.clone(),
                        version,
                    });

                    self.entries.insert(
                        key,
                        StoredProjection {
                            projection: proj,
                            version,
                        },
                    );
                }
            }
        }

        StoreUpdateResult { changed, transient }
    }

    /// Get all projections for a client (for state queries).
    pub fn get_all(&self, client_id: ClientId) -> Vec<VersionedProjection> {
        self.entries
            .iter()
            .filter(|((cid, _), _)| *cid == client_id.as_usize())
            .map(|(_, stored)| VersionedProjection {
                projection: stored.projection.clone(),
                version: stored.version,
            })
            .collect()
    }

    /// Remove all projections for a client (on client disconnect).
    pub fn remove_client(&mut self, client_id: ClientId) {
        self.entries
            .retain(|(cid, _), _| *cid != client_id.as_usize());
    }

    /// Remove projections for a specific domain (on domain detach).
    pub fn remove_domain(&mut self, domain_id: DomainId) {
        self.entries
            .retain(|_, stored| stored.projection.domain_id != domain_id);
    }

    fn insert(&mut self, client_id: ClientId, proj: Projection) {
        let key = (client_id.as_usize(), proj.tag.clone());
        let version = self.next_version;
        self.next_version += 1;

        self.entries.insert(
            key,
            StoredProjection {
                projection: proj,
                version,
            },
        );
    }
}
