//! LSP provider trait.
//!
//! Defines the service interface for LSP providers. Modules implement
//! this trait to provide language server functionality.

use crate::{DiagnosticCache, request::LspRequest};

/// LSP provider interface for language server communication.
///
/// # Design Philosophy
///
/// - **Non-blocking**: `send_request()` uses `try_send()` for backpressure
/// - **Lock-free reads**: `diagnostics()` returns a reference to `DiagnosticCache`
///   which uses `ArcSwap` for lock-free reads
/// - **Lifecycle awareness**: `is_active()` reports whether the server is running
///
/// # Example
///
/// ```ignore
/// use reovim_driver_text_lsp::{LspProvider, LspRequest};
///
/// let provider: &dyn LspProvider = /* from ServiceRegistry */;
/// if provider.is_active() {
///     provider.send_request(LspRequest::DidOpen { ... });
///     let diags = provider.diagnostics().get(&uri);
/// }
/// ```
pub trait LspProvider: Send + Sync {
    /// Send an LSP request to the background task.
    ///
    /// Returns `true` if the request was accepted, `false` if the channel
    /// is full (backpressure) or closed (server stopped).
    fn send_request(&self, request: LspRequest) -> bool;

    /// Get the shared diagnostic cache for lock-free reads.
    fn diagnostics(&self) -> &DiagnosticCache;

    /// Check if the language server is running and initialized.
    fn is_active(&self) -> bool;

    /// Server capabilities from the initialize response (#521, #530).
    ///
    /// Returns `Arc<ServerCapabilities>` for cheap sharing. Consumers MUST
    /// check capabilities before sending requests (e.g.,
    /// `caps.completion_provider.is_some()` before completion).
    /// Returns `None` if the server hasn't completed initialization.
    ///
    /// Uses `Arc` instead of `&T` to support lock-free dynamic capability
    /// registration via `ArcSwap` (#533).
    fn capabilities(&self) -> Option<std::sync::Arc<lsp_types::ServerCapabilities>>;

    /// Project root path this server covers.
    fn root_path(&self) -> &std::path::Path;

    /// Language ID this server handles (e.g., `"rust"`, `"python"`).
    fn language_id(&self) -> &str;

    /// Server info (name, version) from the initialize response.
    ///
    /// Returns `None` if the server didn't provide this information.
    fn server_info(&self) -> Option<&lsp_types::ServerInfo>;
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
