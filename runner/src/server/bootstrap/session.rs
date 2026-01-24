//! Session creation and initialization.
//!
//! Contains the session factory function that wires up all registries
//! and creates a fully-configured session.

use std::sync::Arc;

use {
    reovim_driver_input::{ModeProviderKey, ModeProviderRegistry},
    reovim_driver_vfs::{VfsProviderRegistry, VfsScheme},
};

use crate::server::session::{Session, SessionId};

/// Create a session with all default registries wired up.
///
/// This is the entry point for creating sessions that have working keybindings.
/// Used by both `ensure_default_session()` and `handle_client()`.
pub fn create_session_with_defaults(id: SessionId) -> Arc<Session> {
    let (
        mode_registry,
        command_registry,
        keymap_registry,
        module_registry,
        resolver_registry,
        compositor,
        _mode_providers, // TODO(Epic #417 Part 3): Remove when build_default_registries() is simplified
        services,
    ) = super::build_default_registries();

    // Query mode provider from ServiceRegistry using typed key (Epic #417)
    // Modules registered their providers during init(), now we query them
    let initial_mode = services
        .get::<ModeProviderRegistry>()
        .and_then(|registry| registry.get(&ModeProviderKey::Entry))
        .map(|provider| {
            tracing::info!(mode = %provider.entry_mode(), "got entry mode from ServiceRegistry");
            provider.entry_mode().clone()
        })
        .expect("VimModule must register entry mode provider during init()");

    // Get VFS from ServiceRegistry using typed key (Epic #417)
    let vfs = services
        .get::<VfsProviderRegistry>()
        .and_then(|registry| registry.get(&VfsScheme::File))
        .expect("VFS provider must be registered by vfs-local module");

    // Create kernel context (Epic #417 Part 2: queries BufferManager from ServiceRegistry)
    // Pass services Arc so kernel has access to ClipboardProvider and other services
    let kernel = super::real_kernel_context(Arc::clone(&services));

    // Handle empty session: create buffer if handlers indicate so
    let empty_session_registry = super::build_empty_session_registry(&services);
    super::handle_empty_session(&kernel, &empty_session_registry);

    Session::with_registries(
        id,
        kernel,
        initial_mode,
        vfs,
        mode_registry,
        command_registry,
        keymap_registry,
        module_registry,
        resolver_registry,
        compositor,
    )
}
