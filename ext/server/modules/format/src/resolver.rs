//! Formatter resolution logic.
//!
//! Resolution order:
//! 1. External formatter (from `FormatterRegistry`, if configured for filetype)
//! 2. LSP `textDocument/formatting` (if server supports it)
//! 3. No-op (no formatter available)

use std::{path::Path, sync::Arc};

use {reovim_driver_formatter::FormatterRegistry, reovim_kernel::api::v1::ServiceRegistry};

use crate::lsp_formatter;

/// Resolve and format buffer content.
///
/// Tries external formatter first, then LSP, then returns `None`.
/// Failures from external formatters fall through to LSP; LSP failures
/// fall through to `None`. Errors are logged but never block the caller.
pub fn resolve_and_format(
    content: &str,
    path: &str,
    filetype: &str,
    services: &Arc<ServiceRegistry>,
) -> Option<String> {
    // 1. Try external formatter
    if let Some(formatted) = try_external(content, path, filetype, services) {
        return Some(formatted);
    }

    // 2. Try LSP formatting
    try_lsp(content, path, services)
}

/// Attempt formatting with an external formatter from the registry.
fn try_external(
    content: &str,
    path: &str,
    filetype: &str,
    services: &Arc<ServiceRegistry>,
) -> Option<String> {
    let registry = services.get::<FormatterRegistry>()?;
    let provider = registry.get(filetype)?;

    match provider.format(content, Path::new(path)) {
        Ok(formatted) => Some(formatted),
        Err(e) => {
            tracing::warn!(formatter = provider.name(), error = %e, "External formatter failed");
            None
        }
    }
}

/// Attempt formatting via LSP.
#[cfg_attr(coverage_nightly, coverage(off))]
fn try_lsp(content: &str, path: &str, services: &Arc<ServiceRegistry>) -> Option<String> {
    lsp_formatter::format_with_lsp(content, path, services)
}

/// Detect filetype from a file path extension.
///
/// Returns the language identifier used as the `FormatterRegistry` key.
#[must_use]
pub fn detect_filetype(path: &str) -> String {
    lsp_formatter::language_from_path(path).to_string()
}

#[cfg(test)]
#[path = "resolver_tests.rs"]
mod tests;
