//! Lookup policy store for `ServiceRegistry`.

use std::sync::{Arc, RwLock};

use {reovim_kernel::api::v1::Service, reovim_subsys_input_contracts::KeyLookupPolicy};

/// Store for a [`KeyLookupPolicy`] registered by a module.
pub struct LookupPolicyStore {
    policy: RwLock<Option<Arc<dyn KeyLookupPolicy>>>,
}

impl LookupPolicyStore {
    /// Create a new empty lookup policy store.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new() -> Self {
        Self {
            policy: RwLock::new(None),
        }
    }

    /// Set the lookup policy.
    pub fn set(&self, policy: Arc<dyn KeyLookupPolicy>) {
        *self
            .policy
            .write()
            .expect("LookupPolicyStore lock poisoned") = Some(policy);
    }

    /// Take the lookup policy, clearing the store.
    pub fn take(&self) -> Option<Arc<dyn KeyLookupPolicy>> {
        self.policy
            .write()
            .expect("LookupPolicyStore lock poisoned")
            .take()
    }

    /// Check if a policy has been registered.
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
