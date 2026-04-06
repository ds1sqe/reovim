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
// Context
// ============================================================================

pub use super::context::{KernelContext, ModuleContext};

// ============================================================================
// Memory Management (mm/)
// ============================================================================

// Buffer types
// Note: Buffer moved to reovim-provider-text (#740). Use reovim_provider_text::Buffer.
pub use crate::mm::{BufferId, TabId, WindowId};

// Snapshot moved to reovim-provider-text (#740)
// Note: FileMapping and PieceTree moved to reovim-driver-vfs (#740)

// Line caching
pub use crate::mm::LineCache;

// Background task scheduler
pub use crate::mm::{
    RequestPriority, SaturationRequest, SaturatorConfig, SaturatorHandle, spawn_saturator,
};

// ============================================================================
// IPC (ipc/)
// ============================================================================

// Event types
pub use crate::ipc::{
    CacheKind, CacheUpdated, DEFAULT_TIMEOUT, DispatchResult, DynEvent, Event, EventBus,
    EventResult, EventScope, EventSender, HandlerContext, ScopeId, Subscription, SubscriptionId,
    TargetedEvent,
};

// Event definitions (kernel and driver events)
pub use crate::ipc::events;

// Channel types
pub use crate::ipc::{
    BoundedReceiver, BoundedSender, OneshotReceiver, OneshotSender, Receiver, RecvError, SendError,
    Sender, TryRecvError, TrySendError, bounded, channel, oneshot,
};

// ============================================================================
// Core Primitives (core/)
// ============================================================================

// Note: Mark types (Mark, MarkBank, SpecialMark, MarkResult) moved to
// reovim-driver-session (#740).

// Options
pub use crate::core::{
    ConstraintError, OptionConstraint, OptionError, OptionRegistry, OptionScope, OptionScopeId,
    OptionSpec, OptionValue, SetResult,
};

// Config
pub use crate::core::{Config, ConfigError, ConfigPaths, ConfigValue};

// Mode and Command identity types
// Note: OperatorId moved to modules/vim (Epic #385 - operators are vim-specific policy)
pub use crate::core::{CommandId, CursorStyle, Mode, ModeId, ModeStack};

// ============================================================================
// Block Operations (block/)
// ============================================================================

// BufferManager is MECHANISM (pure storage interface, like Linux page cache)
// It stays in kernel - no I/O policy, just storage lifecycle
pub use super::buffer_manager::{BufferError, BufferManager};

// BufferOps - text buffer operations (extends StorageOps + BufferMeta)
pub use super::buffer_ops::BufferOps;

// BufferCapabilities - capability flags for buffer type queries
pub use super::buffer_caps::BufferCapabilities;

// StorageOps - byte-level storage trait (replaces BufferOps in target architecture)
pub use super::storage_ops::{
    BufferMeta, KernelBuffer, StorageCapabilities, StorageError, StorageOps,
};

// Note: UndoManager, WindowManager, and policy traits (Operator,
// KeymapProvider, CommandHandler) have been moved out of the kernel to follow
// the "mechanism vs policy" principle. See lib/drivers/ for driver-level traits.

// ============================================================================
// Scheduler (sched/)
// ============================================================================

pub use crate::sched::{
    BoxedTask,
    // Configuration constants
    DEFAULT_BATCH_SIZE,
    DEFAULT_MAX_TIMERS,
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
    // Timer types
    TimerConfig,
    TimerHandle,
    TimerId,
    TimerWheel,
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
    ModuleId, ModuleInfo, ModuleProbe, ModuleState, ProbeResult, RegistrationFlags,
};

// ============================================================================
// Service Registry (api/service.rs)
// ============================================================================

pub use super::service::{MultiServiceRegistry, Service, ServiceKey, ServiceRegistry};

// Note: SyntaxHighlight trait has been moved to lib/drivers/syntax/

// ============================================================================
// Sync Primitives (from arch)
// ============================================================================

pub use reovim_arch::sync::{
    ArcSwap, Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

// ============================================================================
// Debug (api/debug.rs)
// ============================================================================

pub use super::debug::{
    // Snapshot types
    KernelStateSnapshot,
    ModeStackSnapshot,
    // Snapshot functions
    snapshot_kernel_state,
    snapshot_mode_stack,
};

// ============================================================================
// Profiling (debug/profiler.rs)
// ============================================================================

#[allow(deprecated)]
pub use crate::debug::{
    // Profiler trait and types
    NopProfiler,
    ProfileGuard,
    ProfileScope,
    Profiler,
    SetProfilerError,
    SpanData,
    SpanId,
    // Global profiler functions
    profiler,
    set_profiler,
};

// Profiling macros (re-export from crate root)
pub use crate::{profile, profile_counter, profile_fn, profile_histogram, profile_scope};

// Metrics registry (for direct histogram/counter access)
pub use crate::debug::{Counter, Histogram, MetricsRegistry, MetricsSnapshot, metrics};

// ============================================================================
// Panic Handling (panic/)
// ============================================================================

pub use crate::panic::{
    // Report
    CrashReport,
    // Handler
    DebugContext,
    // Recovery
    RecoverySnapshot,
    UnsavedBuffer,
    cleanup_old_recovery_files,
    generate_crash_report,
    install_panic_handler,
    is_handler_installed,
    list_recovery_files,
    recovery_dir,
    save_buffer_for_recovery,
    set_debug_context_callback,
    set_recovery_callback,
};

// ============================================================================
// Tests
// ============================================================================
