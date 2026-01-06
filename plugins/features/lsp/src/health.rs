//! LSP health check for the health-check plugin.
//!
//! Provides status information about the LSP server connection and diagnostics.

use std::{fmt, sync::Arc};

use reovim_plugin_health_check::{HealthCheck, HealthCheckResult};

use crate::SharedLspManager;

/// Health check for LSP server status.
///
/// Reports:
/// - Server running status
/// - Number of open documents
/// - Diagnostic cache summary
pub struct LspHealthCheck {
    manager: Arc<SharedLspManager>,
}

impl fmt::Debug for LspHealthCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LspHealthCheck").finish_non_exhaustive()
    }
}

impl LspHealthCheck {
    /// Create a new LSP health check.
    #[must_use]
    pub const fn new(manager: Arc<SharedLspManager>) -> Self {
        Self { manager }
    }
}

impl HealthCheck for LspHealthCheck {
    fn name(&self) -> &'static str {
        "Language Server"
    }

    fn category(&self) -> &'static str {
        "Plugin"
    }

    fn run(&self) -> HealthCheckResult {
        self.manager.with(|m| {
            if !m.running {
                return HealthCheckResult::warning("Server not running").with_details(
                    "LSP server is not active. Open a supported file (.rs) to start.",
                );
            }

            // Gather statistics
            let doc_count = m.documents.buffer_ids().len();
            let pending_syncs = m.documents.has_pending_syncs();

            // Diagnostic cache stats
            let (cache_entries, total_diagnostics) = m.cache.as_ref().map_or((0, 0), |c| {
                let all = c.get_all();
                let total: usize = all.values().map(|bd| bd.diagnostics.len()).sum();
                (all.len(), total)
            });

            // Build details string
            let mut details = Vec::new();
            details.push("Server: rust-analyzer (running)".to_string());
            details.push(format!("Open documents: {doc_count}"));
            details.push(format!("Documents with diagnostics: {cache_entries}"));
            details.push(format!("Total diagnostics: {total_diagnostics}"));
            if pending_syncs {
                details.push("Pending syncs: yes (debouncing)".to_string());
            }

            HealthCheckResult::ok(format!(
                "Running - {doc_count} docs, {total_diagnostics} diagnostics"
            ))
            .with_details(details.join("\n"))
        })
    }
}
