//! Lookup policy store for `ServiceRegistry`.
//!
//! This module provides a store for the [`KeyLookupPolicy`] that modules can
//! register during `init()`. Bootstrap extracts this after all modules
//! initialize to configure the `KeymapRegistry`.
//!
//! # Architecture
//!
//! Modules (e.g., `VimModule`) register their lookup policy during `init()`.
//! Bootstrap reads this from `ServiceRegistry` when wiring the keymap.
//! If no policy is registered, `EagerLookupPolicy` is used as the default.
//!
//! This decouples bootstrap from any specific module import (#620).

use std::sync::{Arc, RwLock};

use reovim_kernel::api::v1::Service;

use super::lookup::KeyLookupPolicy;

/// Store for a [`KeyLookupPolicy`] registered by a module.
///
/// Modules register their lookup policy during `init()` by calling `set()`.
/// After all modules are initialized, bootstrap extracts the policy via `take()`.
///
/// # Thread Safety
///
/// Uses `RwLock` for interior mutability.
pub struct LookupPolicyStore {
    policy: RwLock<Option<Arc<dyn KeyLookupPolicy>>>,
}

impl LookupPolicyStore {
    /// Create a new empty lookup policy store.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // RwLock::new() is not const
    pub fn new() -> Self {
        Self {
            policy: RwLock::new(None),
        }
    }

    /// Set the lookup policy.
    ///
    /// Called by modules during `init()`. If a policy was already set, it
    /// is replaced (last writer wins).
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn set(&self, policy: Arc<dyn KeyLookupPolicy>) {
        *self
            .policy
            .write()
            .expect("LookupPolicyStore lock poisoned") = Some(policy);
    }

    /// Take the lookup policy, clearing the store.
    ///
    /// Called by bootstrap after all modules are initialized.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn take(&self) -> Option<Arc<dyn KeyLookupPolicy>> {
        self.policy
            .write()
            .expect("LookupPolicyStore lock poisoned")
            .take()
    }

    /// Check if a policy has been registered.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn has_policy(&self) -> bool {
        self.policy
            .read()
            .expect("LookupPolicyStore lock poisoned")
            .is_some()
    }
}

impl Default for LookupPolicyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for LookupPolicyStore {}

impl std::fmt::Debug for LookupPolicyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LookupPolicyStore")
            .field("has_policy", &self.has_policy())
            .finish()
    }
}

#[cfg(test)]
#[path = "lookup_policy_store_tests.rs"]
mod tests;
