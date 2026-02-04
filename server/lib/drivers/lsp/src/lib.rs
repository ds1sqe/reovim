//! LSP driver for reovim.
//!
//! Provides Language Server Protocol client infrastructure following the
//! kernel driver pattern. This module defines types for LSP communication;
//! concrete implementations will be added in Phase 4.
//!
//! # Architecture
//!
//! Following Linux kernel principles of mechanism vs policy:
//! - **Mechanism**: Request/response types, diagnostic caching, error handling
//! - **Policy**: Concrete LSP client implementations (Phase 4)
//!
//! # Components
//!
//! - [`LspRequest`] - LSP request variants with oneshot response channels
//! - [`LspResponse`] - Unified response type for generic handling
//! - [`DiagnosticCache`] - Lock-free diagnostic storage using `ArcSwap`
//! - [`LspServerConfig`] - Server spawn configuration
//! - [`LspError`] - Comprehensive error types
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
//!
//! # Future Phases
//!
//! - **Phase 4**: `LspClient` struct with async methods
//! - **Phase 5**: Additional request variants (completion, code actions)
//! - **Phase 6**: Multi-server coordination

mod cache;
mod config;
mod error;
mod request;
mod response;
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

// Re-export essential LSP types
pub mod lsp_types {
    //! Re-exports from `lsp-types` crate.
    pub use super::types::*;
}
