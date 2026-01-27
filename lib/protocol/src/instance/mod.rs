//! Instance management for reovim servers.
//!
//! This module provides:
//! - [`InstanceInfo`] - Information about a running instance
//! - [`TransportInfo`] - Transport address for connecting to an instance
//! - [`InstanceRegistry`] - File-based registry for instance discovery
//!
//! # Usage
//!
//! ```ignore
//! use reovim_protocol::instance::{InstanceInfo, InstanceRegistry, TransportInfo};
//!
//! // Create registry
//! let registry = InstanceRegistry::new();
//!
//! // Register an instance
//! let info = InstanceInfo::new(
//!     "my-project".to_string(),
//!     std::process::id(),
//!     TransportInfo::tcp("127.0.0.1", 12521),
//! );
//! registry.register(&info)?;
//!
//! // List instances
//! for instance in registry.list()? {
//!     println!("{}: {}", instance.name, instance.transport);
//! }
//!
//! // Get specific instance
//! if let Some(info) = registry.get("my-project")? {
//!     println!("Found: {}", info.transport);
//! }
//! ```

mod info;
mod registry;

pub use {
    info::{InstanceInfo, TransportInfo},
    registry::InstanceRegistry,
};
