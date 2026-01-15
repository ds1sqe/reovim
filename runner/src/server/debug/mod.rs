//! Debug RPC endpoints for state inspection and diagnostics.
//!
//! This module provides debug functionality for the reovim server:
//!
//! # Phase 1: Foundation
//! - `debug/version`: Server version information
//! - `debug/uptime`: Server uptime
//!
//! # Phase 2: Inspection (TODO)
//! - `debug/kernel_state`: Kernel state summary
//! - `debug/registers`: Register contents
//! - `debug/marks`: Mark contents
//! - `debug/mode_stack`: Mode stack history
//!
//! # Phase 3: Metrics (TODO)
//! - `debug/metrics`: Performance statistics
//! - `debug/handlers`: Per-handler statistics
//!
//! # Phase 4: Log Access (TODO)
//! - `debug/log_level`: Get/set log level
//! - `debug/log_tail`: Recent log entries
//!
//! # Phase 5: Visual Debug (TODO)
//! - `debug/visual_snapshot`: Full state dump for AI/tooling
//!
//! # Architecture
//!
//! ```text
//! runner/src/server/debug/
//! ├── mod.rs                      # Module root, register_handlers()
//! ├── handlers/
//! │   ├── mod.rs                  # Handler exports
//! │   ├── info.rs                 # version, uptime
//! │   ├── inspect.rs              # kernel_state, registers, marks (TODO)
//! │   ├── metrics.rs              # metrics, handlers (TODO)
//! │   └── log.rs                  # log_level, log_tail (TODO)
//! └── infrastructure/
//!     ├── mod.rs                  # Infrastructure exports
//!     ├── uptime.rs               # Server start time
//!     ├── metrics.rs              # Handler metrics (TODO)
//!     └── log_buffer.rs           # Log ring buffer (TODO)
//! ```

pub mod handlers;
pub mod infrastructure;

use {
    crate::server::rpc::RpcDispatcher,
    reovim_protocol::v1::{
        DEBUG_HANDLERS, DEBUG_KERNEL_STATE, DEBUG_LOG_LEVEL, DEBUG_LOG_TAIL, DEBUG_MARKS,
        DEBUG_METRICS, DEBUG_MODE_STACK, DEBUG_REGISTERS, DEBUG_UPTIME, DEBUG_VERSION,
        DEBUG_VISUAL_SNAPSHOT,
    },
    tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt},
};

/// Register all debug handlers with the dispatcher.
///
/// This function is called from `create_default_dispatcher()` to add
/// debug endpoints to the server.
pub fn register_handlers(dispatcher: &mut RpcDispatcher) {
    // Phase 1: Foundation
    dispatcher.register(DEBUG_VERSION, handlers::debug_version);
    dispatcher.register(DEBUG_UPTIME, handlers::debug_uptime);

    // Phase 2: Inspection
    dispatcher.register(DEBUG_KERNEL_STATE, handlers::debug_kernel_state);
    dispatcher.register(DEBUG_REGISTERS, handlers::debug_registers);
    dispatcher.register(DEBUG_MARKS, handlers::debug_marks);
    dispatcher.register(DEBUG_MODE_STACK, handlers::debug_mode_stack);

    // Phase 3: Metrics
    dispatcher.register(DEBUG_METRICS, handlers::debug_metrics);
    dispatcher.register(DEBUG_HANDLERS, handlers::debug_handlers);

    // Phase 4: Log Access
    dispatcher.register(DEBUG_LOG_LEVEL, handlers::debug_log_level);
    dispatcher.register(DEBUG_LOG_TAIL, handlers::debug_log_tail);

    // Phase 5: Visual Debug
    dispatcher.register(DEBUG_VISUAL_SNAPSHOT, handlers::debug_visual_snapshot);
}

/// Initialize debug infrastructure.
///
/// This function should be called once when the server starts.
/// It initializes:
/// - Server start time for uptime tracking
/// - Tracing subscriber with `LogBufferLayer` for log capture
/// - Handler metrics collection
/// - Log ring buffer
pub fn init() {
    // Initialize tracing subscriber with LogBufferLayer.
    // This captures log events to the ring buffer for debug/log_tail.
    // Uses try_init() to avoid panic if subscriber already set.
    let _ = tracing_subscriber::registry()
        .with(infrastructure::LogBufferLayer)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .try_init();

    infrastructure::init_server_start();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_handlers() {
        let mut dispatcher = RpcDispatcher::new();
        register_handlers(&mut dispatcher);

        // Phase 1 handlers should be registered
        assert!(dispatcher.has_method(DEBUG_VERSION));
        assert!(dispatcher.has_method(DEBUG_UPTIME));

        // Phase 2 handlers should be registered
        assert!(dispatcher.has_method(DEBUG_KERNEL_STATE));
        assert!(dispatcher.has_method(DEBUG_REGISTERS));
        assert!(dispatcher.has_method(DEBUG_MARKS));
        assert!(dispatcher.has_method(DEBUG_MODE_STACK));

        // Phase 3 handlers should be registered
        assert!(dispatcher.has_method(DEBUG_METRICS));
        assert!(dispatcher.has_method(DEBUG_HANDLERS));

        // Phase 4 handlers should be registered
        assert!(dispatcher.has_method(DEBUG_LOG_LEVEL));
        assert!(dispatcher.has_method(DEBUG_LOG_TAIL));

        // Phase 5 handlers should be registered
        assert!(dispatcher.has_method(DEBUG_VISUAL_SNAPSHOT));
    }

    #[test]
    fn test_init() {
        // Should not panic when called multiple times
        init();
        init();
    }
}
