//! Configuration for the which-key module.

/// Configuration for the which-key popup.
#[derive(Debug, Clone)]
pub struct WhichKeyConfig {
    /// Timeout in milliseconds before popup appears after prefix key.
    /// Default: 500ms
    pub timeout_ms: u64,

    /// Maximum width of the popup (0 = auto).
    pub max_width: u16,

    /// Maximum height of the popup (0 = auto).
    pub max_height: u16,

    /// Show command descriptions in the popup.
    pub show_descriptions: bool,

    /// Enable the which-key feature.
    pub enabled: bool,
}

impl Default for WhichKeyConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 500,
            max_width: 0,
            max_height: 0,
            show_descriptions: true,
            enabled: true,
        }
    }
}

impl WhichKeyConfig {
    /// Create a new config with default values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            timeout_ms: 500,
            max_width: 0,
            max_height: 0,
            show_descriptions: true,
            enabled: true,
        }
    }

    /// Set the timeout in milliseconds.
    ///
    /// # Panics
    ///
    /// Panics if timeout is less than 100ms or greater than 5000ms.
    #[must_use]
    pub const fn with_timeout(mut self, timeout_ms: u64) -> Self {
        assert!(timeout_ms >= 100, "Timeout must be at least 100ms");
        assert!(timeout_ms <= 5000, "Timeout must be at most 5000ms");
        self.timeout_ms = timeout_ms;
        self
    }

    /// Validate the timeout is within acceptable bounds.
    #[must_use]
    pub const fn is_timeout_valid(&self) -> bool {
        self.timeout_ms >= 100 && self.timeout_ms <= 5000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WhichKeyConfig::default();
        assert_eq!(config.timeout_ms, 500);
        assert_eq!(config.max_width, 0);
        assert_eq!(config.max_height, 0);
        assert!(config.show_descriptions);
        assert!(config.enabled);
    }

    #[test]
    fn test_config_new() {
        let config = WhichKeyConfig::new();
        assert_eq!(config.timeout_ms, 500);
        assert!(config.enabled);
    }

    #[test]
    fn test_config_with_timeout() {
        let config = WhichKeyConfig::new().with_timeout(750);
        assert_eq!(config.timeout_ms, 750);
    }

    #[test]
    fn test_config_timeout_bounds() {
        let config = WhichKeyConfig::new().with_timeout(100);
        assert!(config.is_timeout_valid());

        let config = WhichKeyConfig::new().with_timeout(5000);
        assert!(config.is_timeout_valid());
    }

    #[test]
    #[should_panic(expected = "Timeout must be at least 100ms")]
    fn test_config_timeout_too_low() {
        let _ = WhichKeyConfig::new().with_timeout(50);
    }

    #[test]
    #[should_panic(expected = "Timeout must be at most 5000ms")]
    fn test_config_timeout_too_high() {
        let _ = WhichKeyConfig::new().with_timeout(10000);
    }
}
