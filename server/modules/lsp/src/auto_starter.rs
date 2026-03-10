//! LSP auto-starter implementation.
//!
//! Implements `LspLifecycle` to auto-start LSP servers. This code was
//! extracted from `completion/src/commands.rs::try_auto_start_lsp()` to
//! decouple the completion module from direct module-lsp imports.

use std::sync::Arc;

use {
    reovim_driver_lsp::{
        LspKey, LspLifecycle, LspProvider, LspProviderRegistry, LspRequest, LspServerConfig,
        uri_from_path,
    },
    reovim_driver_session::{PendingLevel, PendingNotificationQueue},
    reovim_kernel::api::v1::ServiceRegistry,
    tracing::{info, warn},
};

use crate::diagnostic_bridge::DiagnosticPathIndex;

use crate::LspSaturator;

/// Auto-starts LSP servers using [`LspSaturator`].
///
/// Spawns the server asynchronously via the tokio runtime and registers
/// it in `LspProviderRegistry`. Progress notifications are pushed to
/// `PendingNotificationQueue` if available.
pub struct LspAutoStarter;

#[cfg_attr(coverage_nightly, coverage(off))]
impl LspLifecycle for LspAutoStarter {
    fn auto_start(
        &self,
        services: &Arc<ServiceRegistry>,
        config: LspServerConfig,
        language_id: String,
        file_path: String,
        buffer_content: String,
        buffer_id: u64,
    ) {
        // Populate DiagnosticPathIndex so URI→BufferId resolution works
        // even before a BufferSaved event fires.
        let uri = uri_from_path(std::path::Path::new(&file_path));
        if let Some(path_index) = services.get::<DiagnosticPathIndex>() {
            path_index.insert(uri.as_str().to_string(), buffer_id);
        }

        let services_clone = Arc::clone(services);
        let notify_queue = services.get::<PendingNotificationQueue>();

        if let Some(q) = &notify_queue {
            q.push(PendingLevel::Info, format!("Starting {language_id} language server..."));
        }

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                info!(lang = %language_id, "Auto-starting LSP server");
                match LspSaturator::start(config, language_id.clone()).await {
                    Ok(lsp_handle) => {
                        // Send DidOpen so the server knows about the file.
                        let uri = uri_from_path(std::path::Path::new(&file_path));
                        lsp_handle.send_request(LspRequest::DidOpen {
                            uri,
                            language_id: language_id.clone(),
                            version: 1,
                            content: buffer_content,
                        });

                        // Register in LspProviderRegistry.
                        let registry = services_clone.get_or_create::<LspProviderRegistry>();
                        registry.register(LspKey::Language(language_id), Arc::new(lsp_handle));
                        info!("LSP server registered and ready");
                        if let Some(q) = &notify_queue {
                            q.push(PendingLevel::Success, "Language server ready");
                        }
                    }
                    Err(e) => {
                        warn!("Failed to start LSP server: {e}");
                        if let Some(q) = &notify_queue {
                            q.push(
                                PendingLevel::Warning,
                                format!("Failed to start language server: {e}"),
                            );
                        }
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_starter_implements_lifecycle() {
        let starter = LspAutoStarter;
        let _: &dyn LspLifecycle = &starter;
    }
}
