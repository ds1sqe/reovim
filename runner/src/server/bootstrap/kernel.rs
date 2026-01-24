//! Kernel context creation.
//!
//! Provides factory functions for creating a fully functional `KernelContext`
//! with all subsystems initialized.

use std::sync::Arc;

use {
    reovim_arch::sync::RwLock,
    reovim_driver_buffer::{BufferManagerKey, BufferManagerRegistry},
    reovim_kernel::api::v1::{
        EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, OptionScope, OptionSpec,
        OptionValue, RegisterBank, ServiceRegistry, TextObjectEngine,
    },
};

/// Create a real `KernelContext` with working buffer management.
///
/// Unlike `KernelContext::default()` which uses stubs, this creates
/// a fully functional kernel context suitable for actual editing.
///
/// # Arguments
///
/// * `services` - `ServiceRegistry` to query `BufferManager` from and store in context
///
/// # Panics
///
/// Panics if no `BufferManager` is registered in `ServiceRegistry`.
pub fn real_kernel_context(services: Arc<ServiceRegistry>) -> KernelContext {
    let option_registry = Arc::new(OptionRegistry::new());
    register_default_options(&option_registry);

    // Query BufferManager from ServiceRegistry (Epic #417 Part 2)
    // Module registered it during init(), runner doesn't know concrete type
    let buffer_manager = services
        .get::<BufferManagerRegistry>()
        .and_then(|registry| registry.get(&BufferManagerKey::Simple))
        .expect("BufferManager must be registered by buffer-simple module");

    KernelContext::new(
        Arc::new(EventBus::new()),
        buffer_manager,
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(RwLock::new(RegisterBank::new())),
        Arc::new(RwLock::new(MarkBank::new())),
        option_registry,
        services,
    )
}

/// Register default editor options.
///
/// These options are registered at startup and available to all modules.
/// Following mechanism vs policy: the registry (mechanism) is in kernel,
/// the option definitions (policy) are here in the runner.
fn register_default_options(registry: &OptionRegistry) {
    // Indentation options
    let _ = registry.register(
        OptionSpec::new(
            "autoindent",
            "Copy indent from current line when starting new line",
            OptionValue::bool(true),
        )
        .with_short("ai")
        .with_scope(OptionScope::Buffer),
    );

    let _ = registry.register(
        OptionSpec::new(
            "smartindent",
            "Smart autoindenting for C-like languages",
            OptionValue::bool(false),
        )
        .with_short("si")
        .with_scope(OptionScope::Buffer),
    );

    // Tab options (for display line calculations)
    let _ = registry.register(
        OptionSpec::new("tabstop", "Number of spaces that a tab counts for", OptionValue::int(8))
            .with_short("ts")
            .with_scope(OptionScope::Buffer),
    );
}
