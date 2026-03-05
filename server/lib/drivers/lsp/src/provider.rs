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
/// use reovim_driver_lsp::{LspProvider, LspRequest};
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
    /// Consumers MUST check capabilities before sending requests
    /// (e.g., `caps.completion_provider.is_some()` before completion).
    /// Returns `None` if the server hasn't completed initialization.
    fn capabilities(&self) -> Option<&lsp_types::ServerCapabilities>;

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
mod tests {
    use super::*;

    /// Mock provider for testing trait object safety.
    struct MockLspProvider;

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(clippy::unnecessary_literal_bound)]
    impl LspProvider for MockLspProvider {
        fn send_request(&self, _request: LspRequest) -> bool {
            false
        }

        fn diagnostics(&self) -> &DiagnosticCache {
            // Use a leaked static for test purposes
            static CACHE: std::sync::OnceLock<DiagnosticCache> = std::sync::OnceLock::new();
            CACHE.get_or_init(DiagnosticCache::new)
        }

        fn is_active(&self) -> bool {
            false
        }

        fn capabilities(&self) -> Option<&lsp_types::ServerCapabilities> {
            None
        }

        fn root_path(&self) -> &std::path::Path {
            std::path::Path::new("/mock")
        }

        fn language_id(&self) -> &str {
            "mock"
        }

        fn server_info(&self) -> Option<&lsp_types::ServerInfo> {
            None
        }
    }

    #[test]
    fn test_lsp_provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockLspProvider>();
    }

    #[test]
    fn test_lsp_provider_trait_object_safe() {
        fn assert_object_safe(_: &dyn LspProvider) {}
        let provider = MockLspProvider;
        assert_object_safe(&provider);
    }

    #[test]
    fn test_mock_provider_send_request() {
        let provider = MockLspProvider;
        assert!(!provider.send_request(LspRequest::Shutdown));
    }

    #[test]
    fn test_mock_provider_diagnostics() {
        let provider = MockLspProvider;
        let cache = provider.diagnostics();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_mock_provider_is_active() {
        let provider = MockLspProvider;
        assert!(!provider.is_active());
    }
}
