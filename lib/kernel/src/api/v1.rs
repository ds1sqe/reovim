//! Stable Kernel API v1.
//!
//! Linux equivalent: `include/linux/`
//!
//! This module provides the stable public interface for drivers and modules.
//! All types exported here are covered by semver guarantees within the v1.x series.
//!
//! # Stability Guarantee
//!
//! - Breaking changes require a new major version (v2)
//! - New APIs may be added in minor versions
//! - Patch versions contain only bug fixes
//!
//! # Usage
//!
//! ```ignore
//! use reovim_kernel::api::v1::*;
//!
//! // Check API compatibility
//! check_api_version(Version::new(1, 0, 0))?;
//!
//! // Use kernel types
//! let bus = EventBus::new();
//! pr_info!("kernel initialized");
//! ```

// ============================================================================
// Version
// ============================================================================

pub use super::version::{
    API_VERSION, API_VERSION_STR, Version, VersionError, VersionErrorKind, check_api_version,
    is_compatible,
};

// ============================================================================
// Context & Manager
// ============================================================================

pub use super::{
    buffer_manager::{BufferError, BufferManager},
    context::{KernelContext, ModuleContext},
};

// ============================================================================
// Memory Management (mm/)
// ============================================================================

pub use crate::mm::{Buffer, BufferId, Cursor, Edit, Position};

// ============================================================================
// IPC (ipc/)
// ============================================================================

// Event types
pub use crate::ipc::{
    DEFAULT_TIMEOUT, DynEvent, Event, EventBus, EventResult, EventScope, ScopeId, Subscription,
    SubscriptionId,
};

// Channel types
pub use crate::ipc::{
    BoundedReceiver, BoundedSender, OneshotReceiver, OneshotSender, Receiver, RecvError, SendError,
    Sender, TryRecvError, TrySendError, bounded, channel, oneshot,
};

// ============================================================================
// Core Primitives (core/)
// ============================================================================

// Motion
pub use crate::core::{Direction, LinePosition, Motion, MotionEngine, WordBoundary};

// Text Objects
pub use crate::core::{TextObject, TextObjectEngine};

// Registers
pub use crate::core::{RegisterBank, RegisterContent, YankType};

// Marks
pub use crate::core::{Mark, MarkBank, MarkResult, SpecialMark};

// ============================================================================
// Block Operations (block/)
// ============================================================================

pub use crate::block::{
    History, HistoryEntry, Snapshot, Transaction, UndoNode, UndoResult, UndoTree,
};

// ============================================================================
// Scheduler (sched/)
// ============================================================================

pub use crate::sched::{
    BoxedTask,
    // Configuration constants
    DEFAULT_BATCH_SIZE,
    DEFAULT_PRIORITY_QUEUE_CAPACITY,
    DEFAULT_WORK_QUEUE_CAPACITY,
    Executor,
    PRIORITY_QUEUE_DEFAULT_CAPACITY,
    Priority,
    PriorityQueue,
    Runtime,
    RuntimeCommand,
    RuntimeConfig,
    RuntimeState,
    RuntimeStats,
    Task,
    TaskId,
    TaskState,
    WORK_QUEUE_DEFAULT_CAPACITY,
    WORK_QUEUE_MAX_CAPACITY,
    WorkQueue,
};

// ============================================================================
// Logging (printk/)
// ============================================================================

// Types and functions
pub use crate::printk::{
    Level, Logger, NopLogger, ParseLevelError, Record, RecordBuilder, SetLoggerError, flush,
    logger, set_logger,
};

// Internal function needed by macros (hidden from docs)
#[doc(hidden)]
pub use crate::printk::__log;

// Macros (re-export from crate root for unified access)
pub use crate::{pr_debug, pr_err, pr_info, pr_trace, pr_warn};

// ============================================================================
// Module System (api/)
// ============================================================================

pub use super::module::{
    CommandRegistration, EventHandlerRegistration, KeybindingRegistration, Module, ModuleError,
    ModuleId, ModuleInfo, ModuleState, ProbeResult, RegistrationFlags,
};

// ============================================================================
// Syntax Mechanism (api/)
// ============================================================================

pub use super::syntax::SyntaxHighlight;

// ============================================================================
// Sync Primitives (from arch)
// ============================================================================

pub use reovim_arch::sync::{
    ArcSwap, Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_version() {
        assert_eq!(API_VERSION, Version::new(1, 0, 0));
        assert_eq!(API_VERSION_STR, "1.0.0");
    }

    #[test]
    fn test_version_compatibility() {
        // Same version is compatible
        assert!(is_compatible(Version::new(1, 0, 0), Version::new(1, 0, 0)));
        // Older minor is compatible
        assert!(is_compatible(Version::new(1, 0, 0), Version::new(1, 1, 0)));
        // Newer minor is NOT compatible
        assert!(!is_compatible(Version::new(1, 2, 0), Version::new(1, 1, 0)));
        // Different major is NOT compatible
        assert!(!is_compatible(Version::new(2, 0, 0), Version::new(1, 0, 0)));
    }

    #[test]
    fn test_all_types_accessible_via_v1() {
        // Verify key types are accessible via api::v1
        let _: BufferId;
        let _: Position;
        let _: fn() -> EventBus = EventBus::new;
        let _: fn() -> Runtime = Runtime::new;
    }

    #[test]
    fn test_macros_accessible() {
        // Verify printk macros work via v1
        pr_info!("test message");
        pr_debug!("debug: {}", 42);
    }

    #[test]
    fn test_module_types_accessible() {
        // Verify ModuleId and ModuleError are accessible
        let id = ModuleId::new("test-module");
        assert_eq!(id.as_str(), "test-module");

        let _err: ModuleError = ModuleError::NotFound("test".into());
    }

    #[test]
    fn test_syntax_trait_accessible() {
        // Verify SyntaxHighlight trait is accessible
        fn _accepts_highlight<T: SyntaxHighlight>(_: T) {}

        // The trait is available for implementation by drivers
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        enum TestHL {
            Keyword,
        }

        impl SyntaxHighlight for TestHL {
            fn category(&self) -> &'static str {
                "keyword"
            }
        }

        let hl = TestHL::Keyword;
        assert_eq!(hl.category(), "keyword");
    }
}
