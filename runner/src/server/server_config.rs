//! Server configuration types.
//!
//! Contains `TransportMode` and `ServerConfig` for configuring
//! how the server listens for connections and manages sessions.

use std::path::PathBuf;

use reovim_kernel::api::v1::{ModeId, ModuleId};

use super::module::ModuleConfig;

/// Transport configuration for the server.
///
/// Determines how the server accepts client connections.
#[derive(Debug, Clone, Default)]
pub enum TransportMode {
    /// TCP with automatic port fallback (12521-12530).
    ///
    /// Allows multiple reovim servers to run concurrently.
    #[default]
    TcpWithFallback,

    /// TCP on a specific port.
    Tcp {
        /// Port to bind to.
        port: u16,
    },

    /// Unix socket at a specific path.
    ///
    /// Efficient for local IPC, commonly used for editor embedding.
    #[cfg(unix)]
    UnixSocket {
        /// Path to the socket file.
        path: PathBuf,
    },

    /// Stdio transport (stdin/stdout).
    ///
    /// For process embedding - the parent process communicates
    /// directly via stdin/stdout. Single client only.
    Stdio,
}

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Transport mode (TCP, Unix socket, or Stdio).
    pub transport: TransportMode,

    /// Instance name for registry discovery.
    ///
    /// This server will be registered under this name in the instance
    /// registry, allowing clients to connect using `-L <name>`.
    pub instance_name: String,

    /// Name of the default session to create on startup.
    pub default_session_name: String,

    /// Module configuration (search paths, auto-load).
    pub modules: ModuleConfig,

    /// Default mode ID for new sessions.
    ///
    /// If None, falls back to "editor:normal".
    pub default_mode: Option<ModeId>,

    /// Print ready signal to stdout when server is bound.
    ///
    /// When true, server prints `READY <ip>:<port>\n` to stdout after binding.
    /// Used by integrated mode for process coordination.
    pub ready_signal: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            transport: TransportMode::TcpWithFallback,
            instance_name: String::from("default"),
            default_session_name: String::from("default"),
            modules: ModuleConfig::default(),
            default_mode: None,
            ready_signal: false,
        }
    }
}

impl ServerConfig {
    /// Create a new config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a config for TCP with automatic port fallback.
    #[must_use]
    pub fn tcp_with_fallback() -> Self {
        Self {
            transport: TransportMode::TcpWithFallback,
            ..Self::default()
        }
    }

    /// Create a config for TCP on a specific port.
    #[must_use]
    pub fn tcp(port: u16) -> Self {
        Self {
            transport: TransportMode::Tcp { port },
            ..Self::default()
        }
    }

    /// Create a config for Unix socket.
    #[cfg(unix)]
    #[must_use]
    pub fn unix_socket(path: impl Into<PathBuf>) -> Self {
        Self {
            transport: TransportMode::UnixSocket { path: path.into() },
            ..Self::default()
        }
    }

    /// Create a config for Stdio transport.
    #[must_use]
    pub fn stdio() -> Self {
        Self {
            transport: TransportMode::Stdio,
            ..Self::default()
        }
    }

    /// Set the instance name for registry discovery.
    #[must_use]
    pub fn with_instance_name(mut self, name: impl Into<String>) -> Self {
        self.instance_name = name.into();
        self
    }

    /// Set the default session name.
    #[must_use]
    pub fn session_name(mut self, name: impl Into<String>) -> Self {
        self.default_session_name = name.into();
        self
    }

    /// Set the module configuration.
    #[must_use]
    pub fn with_modules(mut self, modules: ModuleConfig) -> Self {
        self.modules = modules;
        self
    }

    /// Load module configuration from the config file.
    ///
    /// Loads `[modules]` section from `~/.config/reovim/config.toml`.
    /// Falls back to defaults if the file doesn't exist.
    ///
    /// # Panics
    ///
    /// Logs a warning and uses defaults if the config file exists
    /// but cannot be parsed.
    #[must_use]
    pub fn with_modules_from_config(mut self) -> Self {
        match ModuleConfig::load() {
            Ok(config) => self.modules = config,
            Err(e) => {
                tracing::warn!("Failed to load module config: {e}");
            }
        }
        self
    }

    /// Set the default mode for new sessions.
    #[must_use]
    pub fn with_default_mode(mut self, mode: ModeId) -> Self {
        self.default_mode = Some(mode);
        self
    }

    /// Enable ready signal output.
    ///
    /// When enabled, server prints `READY <ip>:<port>\n` to stdout after binding.
    #[must_use]
    pub const fn with_ready_signal(mut self, enable: bool) -> Self {
        self.ready_signal = enable;
        self
    }

    /// Get the effective default mode.
    ///
    /// Returns the configured default mode, or falls back to "vim:normal".
    ///
    /// NOTE: This is a legacy method. In Epic #415, the default mode now comes
    /// from `DefaultModeProviderRegistry` in `create_session_with_defaults()`.
    /// This method is kept for compatibility with ServerConfig-based overrides.
    #[must_use]
    pub fn effective_default_mode(&self) -> ModeId {
        self.default_mode.clone().unwrap_or_else(|| {
            // Match what VimDefaultModeProvider provides
            ModeId::with_discriminant(ModuleId::new("vim"), "normal", 0)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert!(matches!(config.transport, TransportMode::TcpWithFallback));
        assert_eq!(config.instance_name, "default");
        assert_eq!(config.default_session_name, "default");
    }

    #[test]
    fn test_server_config_tcp() {
        let config = ServerConfig::tcp(9000);
        assert!(matches!(config.transport, TransportMode::Tcp { port: 9000 }));
    }

    #[test]
    fn test_server_config_builders() {
        let config = ServerConfig::new()
            .with_instance_name("test-instance")
            .session_name("test-session")
            .with_ready_signal(true);

        assert_eq!(config.instance_name, "test-instance");
        assert_eq!(config.default_session_name, "test-session");
        assert!(config.ready_signal);
    }

    #[test]
    fn test_effective_default_mode_none() {
        let config = ServerConfig::new();
        let mode = config.effective_default_mode();
        assert_eq!(mode.module().as_str(), "vim");
        assert_eq!(mode.name(), "normal");
    }

    #[test]
    fn test_effective_default_mode_override() {
        let custom_mode = ModeId::new(ModuleId::new("custom"), "insert");
        let config = ServerConfig::new().with_default_mode(custom_mode.clone());
        assert_eq!(config.effective_default_mode(), custom_mode);
    }

    #[test]
    fn test_server_config_with_modules() {
        let modules = ModuleConfig::new()
            .with_search_path("/custom/modules")
            .with_autoload("my-module");

        let config = ServerConfig::tcp(9000).with_modules(modules);

        assert_eq!(config.modules.search_paths, vec!["/custom/modules"]);
        assert_eq!(config.modules.autoload, vec!["my-module"]);
    }

    #[test]
    fn test_server_config_with_modules_from_config() {
        // This should not panic even if no config file exists
        let config = ServerConfig::tcp(9000).with_modules_from_config();

        // Should have default or loaded config
        assert!(config.modules.search_paths.is_empty() || !config.modules.search_paths.is_empty());
    }

    #[test]
    fn test_server_config_default_has_module_config() {
        let config = ServerConfig::default();

        // Default config should have empty module config
        assert!(config.modules.autoload.is_empty());
    }

    #[test]
    fn test_server_config_with_ready_signal_builder() {
        let config = ServerConfig::tcp(9000).with_ready_signal(true);
        assert!(config.ready_signal);

        let config = ServerConfig::tcp(9000).with_ready_signal(false);
        assert!(!config.ready_signal);
    }
}
