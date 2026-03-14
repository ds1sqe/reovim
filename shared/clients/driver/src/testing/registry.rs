//! Mock module registry for testing.

use std::collections::HashSet;

use crate::{loader::ClientModuleState, traits::ClientModuleRegistry};

/// Configurable mock for `ClientModuleRegistry`.
///
/// Seed running module kinds for introspection tests.
pub struct MockModuleRegistry {
    running: HashSet<String>,
}

impl MockModuleRegistry {
    /// Create with no running modules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            running: HashSet::new(),
        }
    }

    /// Seed running module kinds (builder pattern).
    #[must_use]
    pub fn with_running(mut self, kinds: &[&str]) -> Self {
        self.running = kinds.iter().map(|s| (*s).to_string()).collect();
        self
    }
}

impl Default for MockModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModuleRegistry for MockModuleRegistry {
    fn is_running(&self, kind: &str) -> bool {
        self.running.contains(kind)
    }

    fn module_state(&self, kind: &str) -> Option<ClientModuleState> {
        if self.running.contains(kind) {
            Some(ClientModuleState::Running)
        } else {
            None
        }
    }

    fn loaded_kinds(&self) -> Vec<String> {
        self.running.iter().cloned().collect()
    }

    fn running_count(&self) -> usize {
        self.running.len()
    }
}
