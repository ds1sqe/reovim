//! Registry for configurable components that participate in profile save/load
//!
//! This registry coordinates serialization and deserialization across all
//! registered configurable components (core runtime + plugins).

use {
    super::Configurable,
    std::{
        collections::HashMap,
        sync::{Arc, RwLock},
    },
};

/// Thread-safe registry for configurable components
///
/// Components register themselves via the `RegisterConfigurable` event.
/// The registry coordinates saving all components to a profile and
/// loading them back.
pub struct ProfileRegistry {
    /// Map of section name -> configurable component
    components: RwLock<HashMap<&'static str, Arc<RwLock<dyn Configurable + Send + Sync>>>>,
}

impl ProfileRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            components: RwLock::new(HashMap::new()),
        }
    }

    /// Register a configurable component
    ///
    /// The component's `config_section()` is used as the key.
    /// If a component with the same section already exists, it will be overwritten.
    ///
    /// # Panics
    ///
    /// Panics if the component's `RwLock` is poisoned or if the registry's
    /// internal lock cannot be acquired.
    pub fn register(&self, component: Arc<RwLock<dyn Configurable + Send + Sync>>) {
        let section = {
            let comp = component.read().unwrap();
            comp.config_section()
        };

        if let Ok(mut components) = self.components.write() {
            if components.insert(section, component).is_some() {
                tracing::warn!(section = %section, "Configurable component already registered, overwriting");
            } else {
                tracing::debug!(section = %section, "Configurable component registered");
            }
        }
    }

    /// Save all registered components to a configuration map
    ///
    /// Returns a map of section names to their serialized configuration data.
    /// This can be merged into a profile's TOML structure.
    #[must_use]
    pub fn save_all(&self) -> HashMap<String, HashMap<String, toml::Value>> {
        let mut config = HashMap::new();

        if let Ok(components) = self.components.read() {
            for (section, component) in components.iter() {
                if let Ok(comp) = component.read() {
                    let data = comp.to_config();
                    if !data.is_empty() {
                        let data_len = data.len();
                        config.insert((*section).to_string(), data);
                        tracing::debug!(section = %section, keys = data_len, "Component config saved");
                    }
                }
            }
        }

        config
    }

    /// Load configuration data into all registered components
    ///
    /// Takes a map of section names to configuration data and applies them
    /// to the corresponding registered components.
    ///
    /// Missing sections are skipped (components keep their current state).
    /// Unknown sections are logged and ignored.
    pub fn load_all(&self, config: &HashMap<String, HashMap<String, toml::Value>>) {
        if let Ok(components) = self.components.read() {
            for (section, data) in config {
                if let Some(component) = components.get(section.as_str()) {
                    if let Ok(mut comp) = component.write() {
                        comp.from_config(data);
                        tracing::debug!(section = %section, keys = data.len(), "Component config loaded");
                    }
                } else {
                    tracing::debug!(section = %section, "Unknown config section, skipping");
                }
            }
        }
    }

    /// Get the number of registered components
    #[must_use]
    pub fn component_count(&self) -> usize {
        self.components.read().map(|c| c.len()).unwrap_or(0)
    }

    /// List all registered section names
    #[must_use]
    pub fn list_sections(&self) -> Vec<String> {
        self.components
            .read()
            .map(|c| c.keys().map(|k| (*k).to_string()).collect())
            .unwrap_or_default()
    }

    /// Check if a section is registered
    #[must_use]
    pub fn has_section(&self, section: &str) -> bool {
        self.components
            .read()
            .map(|c| c.contains_key(section))
            .unwrap_or(false)
    }
}

impl Default for ProfileRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ProfileRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileRegistry")
            .field("component_count", &self.component_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestComponent {
        enabled: bool,
        value: i64,
    }

    impl Configurable for TestComponent {
        fn config_section(&self) -> &'static str {
            "test"
        }

        fn to_config(&self) -> HashMap<String, toml::Value> {
            let mut data = HashMap::new();
            data.insert("enabled".to_string(), toml::Value::Boolean(self.enabled));
            data.insert("value".to_string(), toml::Value::Integer(self.value));
            data
        }

        fn from_config(&mut self, data: &HashMap<String, toml::Value>) {
            if let Some(enabled) = data.get("enabled").and_then(toml::Value::as_bool) {
                self.enabled = enabled;
            }
            if let Some(value) = data.get("value").and_then(toml::Value::as_integer) {
                self.value = value;
            }
        }
    }

    #[test]
    fn test_registry_register() {
        let registry = ProfileRegistry::new();
        let component: Arc<RwLock<dyn Configurable + Send + Sync>> =
            Arc::new(RwLock::new(TestComponent {
                enabled: true,
                value: 42,
            }));

        registry.register(component);
        assert_eq!(registry.component_count(), 1);
        assert!(registry.has_section("test"));
    }

    #[test]
    fn test_registry_save_all() {
        let registry = ProfileRegistry::new();
        let component: Arc<RwLock<dyn Configurable + Send + Sync>> =
            Arc::new(RwLock::new(TestComponent {
                enabled: true,
                value: 42,
            }));

        registry.register(component);

        let config = registry.save_all();
        assert!(config.contains_key("test"));

        let test_config = &config["test"];
        assert_eq!(test_config.get("enabled").and_then(toml::Value::as_bool), Some(true));
        assert_eq!(test_config.get("value").and_then(toml::Value::as_integer), Some(42));
    }

    #[test]
    fn test_registry_load_all() {
        let registry = ProfileRegistry::new();
        let component = Arc::new(RwLock::new(TestComponent {
            enabled: false,
            value: 0,
        }));

        let component_dyn: Arc<RwLock<dyn Configurable + Send + Sync>> = component.clone();
        registry.register(component_dyn);

        let mut config = HashMap::new();
        let mut test_data = HashMap::new();
        test_data.insert("enabled".to_string(), toml::Value::Boolean(true));
        test_data.insert("value".to_string(), toml::Value::Integer(99));
        config.insert("test".to_string(), test_data);

        registry.load_all(&config);

        {
            let comp = component.read().unwrap();
            assert!(comp.enabled);
            assert_eq!(comp.value, 99);
            drop(comp);
        }
    }

    #[test]
    fn test_registry_list_sections() {
        let registry = ProfileRegistry::new();
        let component1: Arc<RwLock<dyn Configurable + Send + Sync>> =
            Arc::new(RwLock::new(TestComponent {
                enabled: true,
                value: 1,
            }));

        registry.register(component1);

        let sections = registry.list_sections();
        assert_eq!(sections.len(), 1);
        assert!(sections.contains(&"test".to_string()));
    }
}
