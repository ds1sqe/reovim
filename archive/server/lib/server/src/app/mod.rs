//! Application state for the server.
//!
//! Separates runtime concerns from kernel context. The kernel provides
//! core services (buffers, events, options); the server tracks runtime
//! state like active buffer and mode stack.

use {
    reovim_kernel::api::v1::{KernelContext, ServiceRegistry},
    reovim_subsys_session::ExtensionMap,
    std::sync::Arc,
};

/// Application state combining kernel context with runtime state.
///
/// # Design Philosophy
///
/// `KernelContext` provides kernel services (buffers, event bus, etc.)
/// but intentionally does NOT track runtime state like "which buffer is active"
/// because that's a runtime/window manager concern.
///
/// `AppState` wraps `KernelContext` and adds runtime-specific state that
/// the server and commands need to operate.
///
#[derive(Debug)]
pub struct AppState {
    /// Kernel context providing access to all kernel services.
    pub kernel: KernelContext,

    /// Service registry for cross-module service discovery.
    pub services: Arc<ServiceRegistry>,

    /// Whether the application is running.
    pub running: bool,

    /// Per-session module extensions (Epic #385).
    pub extensions: ExtensionMap,
}

impl AppState {
    /// Create a new application state.
    #[must_use]
    pub fn new(kernel: KernelContext) -> Self {
        let services = Arc::clone(&kernel.services);
        Self {
            kernel,
            services,
            running: true,
            extensions: ExtensionMap::new(),
        }
    }

    /// Request the application to quit.
    #[allow(clippy::missing_const_for_fn)]
    pub fn request_quit(&mut self) {
        self.running = false;
    }

    /// Request clients to detach (server continues running).
    #[allow(clippy::missing_const_for_fn)]
    pub fn request_detach(&mut self) {
        tracing::info!("Detach requested - clients will be notified");
    }

    /// Check if the application should continue running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
