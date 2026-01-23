//! Server CLI argument parsing.
//!
//! Contains `SrvArgs` struct for parsing server mode command line arguments.

use clap::Args;

use super::{
    instance,
    module::ModuleConfig,
    server_config::{ServerConfig, TransportMode},
};

/// Server mode CLI arguments.
///
/// These arguments configure how the server listens for connections
/// and how modules are loaded.
///
/// # Instance Naming
///
/// Use `-L` to name this server instance. Named instances are discoverable
/// via the instance registry, allowing clients to connect using `-L name`
/// instead of raw TCP addresses.
///
/// # Transport Precedence
///
/// Transport flags are mutually exclusive. If multiple are specified:
/// 1. `--stdio` wins (single client mode)
/// 2. `-s`/`--socket` next (Unix socket)
/// 3. `-t`/`--tcp` next (specific TCP port)
/// 4. Default: TCP with automatic port fallback (12521-12530)
#[derive(Args, Debug, Clone)]
pub struct SrvArgs {
    /// Instance name for registry discovery.
    ///
    /// Registers this server as a named instance. Clients can then
    /// connect using `reovim -L <name>` instead of specifying the port.
    ///
    /// Default: "default"
    #[arg(
        short = 'L',
        long = "instance",
        value_name = "NAME",
        default_value = "default"
    )]
    pub instance: String,

    /// Start server on specific TCP port.
    #[arg(short, long, value_name = "PORT")]
    pub tcp: Option<u16>,

    /// Start server on Unix socket.
    #[cfg(unix)]
    #[arg(short, long, value_name = "PATH")]
    pub socket: Option<std::path::PathBuf>,

    /// Start server in stdio mode (single client, for embedding).
    #[arg(long)]
    pub stdio: bool,

    /// Additional module search directory (repeatable).
    ///
    /// Modules in these directories are discovered in addition to the
    /// default search paths.
    #[arg(long = "moddir", value_name = "PATH")]
    pub module_dirs: Vec<std::path::PathBuf>,

    /// Load specific module by ID (repeatable).
    ///
    /// Explicitly load these modules on startup. Can be combined with
    /// `--no-defaults` to load only specific modules.
    #[arg(long = "load", value_name = "MODULE_ID")]
    pub load_modules: Vec<String>,

    /// Skip loading default modules.
    ///
    /// When set, only modules specified via `--load` are loaded.
    /// Useful for testing or minimal startup.
    #[arg(long = "no-defaults")]
    pub no_defaults: bool,

    /// Print ready signal to stdout when server is bound.
    ///
    /// When set, server prints `READY <ip>:<port>` to stdout after binding.
    /// Used by integrated mode for process coordination.
    /// Format: `READY 127.0.0.1:12521\n`
    #[arg(long, hide = true)]
    pub ready_signal: bool,
}

impl SrvArgs {
    /// Convert arguments to `ServerConfig`.
    ///
    /// # Panics
    ///
    /// Panics if the instance name is invalid.
    #[must_use]
    pub fn into_config(self) -> ServerConfig {
        // Validate instance name
        if let Err(e) = instance::InstanceRegistry::validate_name(&self.instance) {
            eprintln!("Error: Invalid instance name: {e}");
            std::process::exit(1);
        }

        // Build module config from CLI arguments
        let mut modules = ModuleConfig::new();

        // Add CLI-specified search paths
        for path in self.module_dirs {
            modules = modules.with_search_path(path.to_string_lossy().into_owned());
        }

        // Add CLI-specified modules to autoload
        for module_id in self.load_modules {
            modules = modules.with_autoload(module_id);
        }

        // Set no_defaults flag
        if self.no_defaults {
            modules = modules.with_no_defaults();
        }

        // Build transport config
        let transport = {
            #[cfg(unix)]
            if let Some(path) = self.socket {
                return ServerConfig::unix_socket(path)
                    .with_instance_name(&self.instance)
                    .with_modules(modules)
                    .with_ready_signal(self.ready_signal);
            }

            if self.stdio {
                TransportMode::Stdio
            } else if let Some(port) = self.tcp {
                TransportMode::Tcp { port }
            } else {
                TransportMode::TcpWithFallback
            }
        };

        ServerConfig {
            transport,
            instance_name: self.instance,
            default_session_name: String::from("default"),
            modules,
            default_mode: None,
            ready_signal: self.ready_signal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srv_args_into_config_default() {
        let args = SrvArgs {
            instance: "default".to_string(),
            tcp: None,
            #[cfg(unix)]
            socket: None,
            stdio: false,
            module_dirs: vec![],
            load_modules: vec![],
            no_defaults: false,
            ready_signal: false,
        };

        let config = args.into_config();
        assert!(matches!(config.transport, TransportMode::TcpWithFallback));
        assert_eq!(config.instance_name, "default");
    }

    #[test]
    fn test_srv_args_into_config_tcp() {
        let args = SrvArgs {
            instance: "test".to_string(),
            tcp: Some(9000),
            #[cfg(unix)]
            socket: None,
            stdio: false,
            module_dirs: vec![],
            load_modules: vec![],
            no_defaults: false,
            ready_signal: false,
        };

        let config = args.into_config();
        assert!(matches!(config.transport, TransportMode::Tcp { port: 9000 }));
        assert_eq!(config.instance_name, "test");
    }

    #[test]
    fn test_srv_args_into_config_stdio() {
        let args = SrvArgs {
            instance: "stdio-instance".to_string(),
            tcp: None,
            #[cfg(unix)]
            socket: None,
            stdio: true,
            module_dirs: vec![],
            load_modules: vec![],
            no_defaults: false,
            ready_signal: false,
        };

        let config = args.into_config();
        assert!(matches!(config.transport, TransportMode::Stdio));
    }

    #[test]
    fn test_srv_args_into_config_module_dirs() {
        let args = SrvArgs {
            instance: "default".to_string(),
            tcp: None,
            #[cfg(unix)]
            socket: None,
            stdio: false,
            module_dirs: vec!["/custom/path".into()],
            load_modules: vec!["my-module".to_string()],
            no_defaults: true,
            ready_signal: false,
        };

        let config = args.into_config();
        assert_eq!(config.modules.search_paths, vec!["/custom/path"]);
        assert_eq!(config.modules.autoload, vec!["my-module"]);
        assert!(config.modules.no_defaults);
    }
}
