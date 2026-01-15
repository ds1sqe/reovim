//! Stable kernel API.
//!
//! Linux equivalent: `include/linux/`
//!
//! This module defines the stable interface that drivers and modules depend on.
//! All public APIs are accessed exclusively via the `v1` module.
//!
//! # Module Structure
//!
//! - `v1`: Stable API v1 - the primary public interface
//! - `unstable`: Experimental APIs (feature-gated)
//! - `internal`: Internal kernel APIs (doc hidden)
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
// Internal modules (not directly exposed)
// ============================================================================

// Buffer manager trait (mechanism) - used by KernelContext
mod buffer_manager;
mod context;
mod debug;
// Note: pub(crate) to allow core/mode.rs to access ModuleId
pub(crate) mod module;
mod version;

// Note: The following modules have been removed as part of the kernel-driver
// architecture cleanup (Issue #213):
// - traits.rs - Operator, KeymapProvider, CommandHandler moved to drivers
// - syntax.rs - SyntaxHighlight moved to lib/drivers/syntax/
// - undo_manager.rs - UndoManager trait moved to runner
// - window_manager.rs - WindowId moved to mm/, WindowManager to runner

// ============================================================================
// Public modules
// ============================================================================

/// Stable API v1.
///
/// This is the primary public interface. All stable kernel APIs are accessed
/// through this module.
pub mod v1;

/// Experimental APIs (feature-gated).
///
/// APIs in this module may change without notice. Enable the `unstable-api`
/// feature to use them.
#[cfg(feature = "unstable-api")]
pub mod unstable;

/// Internal kernel APIs.
///
/// These APIs are for kernel-internal use only and are hidden from documentation.
#[doc(hidden)]
pub mod internal;

// ============================================================================
// Re-exports
// ============================================================================

/// Re-export v1 at api level for convenience.
///
/// This allows both `use reovim_kernel::api::v1::*` and
/// `use reovim_kernel::api::*` to work.
pub use v1::*;
