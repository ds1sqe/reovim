#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! LSP driver for reovim.
//!
//! Provides Language Server Protocol client infrastructure following the
//! kernel driver pattern.
//!
//! # Architecture
//!
//! Following Linux kernel principles of mechanism vs policy:
//! - **Mechanism**: JSON-RPC transport, client lifecycle, provider trait
//! - **Policy**: Concrete LSP module implementations (in `reovim-module-lsp`)
//!
//! # Components
//!
//! - [`LspRequest`] - LSP request variants with oneshot response channels
//! - [`LspResponse`] - Unified response type for generic handling
//! - [`DiagnosticCache`] - Lock-free diagnostic storage using `ArcSwap`
//! - [`LspServerConfig`] - Server spawn configuration
//! - [`LspError`] - Comprehensive error types
//! - [`client::Client`] - LSP client with JSON-RPC communication
//! - [`LspProvider`] - Service trait for language server access
//! - [`LspProviderRegistry`] - Keyed registry for LSP providers
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_lsp::{LspServerConfig, LspRequest, DiagnosticCache};
//! use std::path::Path;
//!
//! // Create server configuration
//! let config = LspServerConfig::rust_analyzer(Path::new("/my/project"));
//!
//! // Diagnostic cache for lock-free reads
//! let cache = DiagnosticCache::new();
//! ```

mod cache;
pub mod client;
mod config;
mod error;
pub mod jsonrpc;
mod key;
mod lifecycle;
mod provider;
mod registry;
mod request;
mod response;
pub mod transport;
mod types;

// Error types
pub use error::{LspError, ModuleError};

// Configuration
pub use config::{LspServerConfig, uri_from_path};

// Request/Response types
pub use {
    request::{LspRequest, NavigationResult},
    response::LspResponse,
};

// Diagnostic cache
pub use cache::{BufferDiagnostics, DiagnosticCache};

// Provider trait and registry (Epic #520)
pub use {key::LspKey, provider::LspProvider, registry::LspProviderRegistry};

// Lifecycle traits (#542 - cross-module decoupling)
pub use lifecycle::{LspLifecycle, LspLifecycleRegistry};

// Re-export essential LSP types
pub mod lsp_types {
    //! Re-exports from `lsp-types` crate.
    pub use super::types::*;
}
