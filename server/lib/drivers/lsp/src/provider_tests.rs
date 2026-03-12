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

    fn capabilities(&self) -> Option<std::sync::Arc<lsp_types::ServerCapabilities>> {
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
