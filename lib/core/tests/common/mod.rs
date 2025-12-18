//! Common test utilities for integration tests

pub use reovim_core::runtime::{TestRuntime, TestRuntimeBuilder};
pub use reovim_core::testing::keys::keys_from_str;

/// Create a test runtime builder with standard 80x24 size.
pub fn standard_runtime() -> TestRuntimeBuilder {
    TestRuntime::builder().with_size(80, 24)
}

/// Create a test runtime builder with content.
pub fn runtime_with_content(content: &str) -> TestRuntimeBuilder {
    standard_runtime().with_content(content)
}
