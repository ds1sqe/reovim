//! Stale-content check hook for decoded buffer views.
//!
//! `#740` Plan 06 Phase 5 sub-commit 5e introduces this hook as the
//! narrow seam the codec driver uses to re-decode a mount whose
//! decoded view is known to be stale relative to the underlying
//! canonical bytes.
//!
//! The session driver owns the trait because [`SessionRuntime`](crate::SessionRuntime)
//! needs to call it from [`BufferApi::buffer_content`](crate::api::BufferApi::buffer_content)
//! before returning text to callers. The codec driver installs an impl
//! via [`SessionShared::install_stale_check`](crate::types::SessionShared::install_stale_check).
//! This keeps the session → codec dependency edge absent: session
//! names only `dyn StaleCheck`, codec provides the implementation.
//!
//! # Why a hook and not a direct call?
//!
//! - `reovim-driver-session` must not depend on `reovim-driver-codec`
//!   (the reverse edge already exists — codec depends on session for
//!   `SessionExtension`).
//! - Tests and headless paths that have no codec driver loaded can
//!   still read buffer content without conditionally importing codec
//!   types.
//! - Future caches / incremental re-decode implementations can swap
//!   the hook without touching the session driver.
//!
//! # Current behaviour
//!
//! The hook is called with `&self` so implementations must use
//! interior mutability if they need to mutate state. The codec
//! driver's adapter emits a `tracing::trace!` event on every call and
//! is a no-op otherwise — full re-decode-on-read requires a larger
//! refactor of how `CodecSessionState` is stored (it currently lives
//! inside `ExtensionMap` and is only reachable through
//! `&mut SessionRuntime`, so a `&self` hook cannot mutate it without
//! a lock migration). That larger refactor is deferred to a follow-up
//! issue.
//!
//! Meanwhile the hook provides:
//!   - a structured event stream for debugging and telemetry
//!   - the compile-time crate-boundary guarantee
//!   - the call site at `SessionRuntime::buffer_content` so future
//!     Phase 7+ work can replace the body without another migration

use reovim_kernel::api::v1::BufferId;

/// Contract for refreshing decoded buffer content before a read.
///
/// Implementations MUST be cheap to call on the hot path — every
/// `BufferApi::buffer_content` invocation pays the cost, including
/// LSP completion, gRPC `get_raw_content`, and FFI
/// `reovim_buffer_content` callers.
///
/// Implementations MUST be idempotent: a mount whose decoded view is
/// already current (`content_valid == true`) should observe no change
/// from a call.
///
/// Implementations MAY use interior mutability. The hook is called via
/// `&self` so the session driver does not have to surface a
/// `&mut SessionRuntime` just to consult the hook.
pub trait StaleCheck: Send + Sync {
    /// Ensure the decoded text buffer behind `buffer` reflects the
    /// current canonical inode bytes.
    ///
    /// No-op if the mount's `content_valid == true`.
    ///
    /// # Parameters
    ///
    /// - `buffer` — the buffer identifier whose decoded view the caller
    ///   is about to read.
    fn refresh_if_stale(&self, buffer: BufferId);
}

#[cfg(test)]
#[path = "stale_check_tests.rs"]
mod tests;
