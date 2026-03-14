//! Mock server handle for testing.

use std::{collections::HashMap, sync::Mutex};

use crate::{OptionMetadata, OptionValue, ServerHandle};

/// Configurable mock for `ServerHandle`.
///
/// Seeds option responses and records executed commands for assertion.
pub struct MockServerHandle {
    options: HashMap<String, OptionValue>,
    commands: Mutex<Vec<String>>,
}

impl MockServerHandle {
    /// Create with no seeded options and empty command log.
    #[must_use]
    pub fn new() -> Self {
        Self {
            options: HashMap::new(),
            commands: Mutex::new(Vec::new()),
        }
    }

    /// Seed option values that `get_options` will return (builder pattern).
    #[must_use]
    pub fn with_options(mut self, options: HashMap<String, OptionValue>) -> Self {
        self.options = options;
        self
    }

    /// Get the list of commands executed via `execute_command`.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn executed_commands(&self) -> Vec<String> {
        self.commands.lock().expect("mutex poisoned").clone()
    }
}

impl Default for MockServerHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerHandle for MockServerHandle {
    fn get_options(&self, names: &[&str]) -> Vec<(String, OptionValue)> {
        names
            .iter()
            .filter_map(|name| {
                self.options
                    .get(*name)
                    .map(|v| ((*name).to_string(), v.clone()))
            })
            .collect()
    }

    fn execute_command(&self, command: &str) {
        self.commands
            .lock()
            .expect("mutex poisoned")
            .push(command.to_string());
    }

    fn list_commands(&self) -> Vec<String> {
        Vec::new()
    }

    fn get_option_metadata(&self, _name: &str) -> Option<OptionMetadata> {
        None
    }
}
