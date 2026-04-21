//! Process-global surface-descriptor dispatch registry for the TUI.
//!
//! The TUI populates this registry at startup (via
//! [`register_builtin_surface_handlers`]) with the built-in handler
//! set, then [`notification_handler`](crate::notification_handler) looks
//! up handlers by `kind` on every incoming `SurfaceChanged`
//! notification.
//!
//! Module-level state (rather than per-`TuiCoreState` state) is
//! deliberate: the handler table is stable for the lifetime of the
//! process, all clients in the process share the same decoder vocabulary,
//! and registration is a one-shot initialisation — threading an
//! `Arc<Registry>` through `TuiCoreState`, `NotificationContext`, and
//! every construction call-site would force a large ripple for no
//! functional gain. The `OnceLock` guarantees initialisation happens
//! exactly once and is visible across all TUI threads.
//!
//! Adding a new surface-descriptor kind is a pure extension: create a
//! new ext crate, add its handler to the registration list, no edits
//! to `notification_handler.rs`.

use {
    reovim_client_subsys_codec::{
        DefaultSurfaceDescriptorHandlerRegistry, SurfaceDescriptorHandlerRegistry,
    },
    reovim_tui_mod_surface_descriptor_cell_grid::CellGridSurfaceHandler,
    std::sync::{Arc, OnceLock},
};

static REGISTRY: OnceLock<Arc<DefaultSurfaceDescriptorHandlerRegistry>> = OnceLock::new();

/// Return the process-global surface-descriptor registry.
///
/// Returns `None` if [`register_builtin_surface_handlers`] has not yet
/// been called. In normal TUI operation the registry is initialised in
/// the startup path before the notification loop begins.
#[must_use]
pub fn global_registry() -> Option<Arc<DefaultSurfaceDescriptorHandlerRegistry>> {
    REGISTRY.get().cloned()
}

/// Initialise the process-global registry with the built-in handler
/// set (currently only `CellGridSurfaceHandler` / kind = `0x0001`).
///
/// Idempotent: safe to call from any startup path. Subsequent calls
/// are no-ops and do not replace previously-registered handlers.
///
/// Returns the registry `Arc` (freshly initialised or already present).
#[allow(clippy::must_use_candidate)]
pub fn register_builtin_surface_handlers() -> Arc<DefaultSurfaceDescriptorHandlerRegistry> {
    REGISTRY
        .get_or_init(|| {
            let registry = Arc::new(DefaultSurfaceDescriptorHandlerRegistry::new());
            registry.register(Arc::new(CellGridSurfaceHandler::new()));
            registry
        })
        .clone()
}

#[cfg(test)]
#[path = "surface_decoding_tests.rs"]
mod tests;
