//! Codec-side adapter for the session driver's `StaleCheck` hook.
//!
//! `#740` Plan 06 Phase 5 sub-commit 5e.
//!
//! `reovim-driver-session` defines the [`StaleCheck`](reovim_driver_text_session::StaleCheck)
//! trait and calls it from
//! [`SessionRuntime::buffer_content`](reovim_driver_text_session::SessionRuntime).
//! This module provides the codec-side implementation so the session
//! crate never has to import `InodeTable` / `CodecSessionState` — the
//! session → codec dependency edge stays absent.
//!
//! # Current scope
//!
//! The adapter body is deliberately a low-cost probe: it emits a
//! `tracing::trace!` event per call so the stale-check traffic can be
//! observed via the server log tap, and it keeps a counter for
//! test-side introspection. It does **not** yet mutate
//! [`CodecSessionState`] because `BufferApi::buffer_content` passes
//! `&self` to the hook, so the adapter cannot reach the
//! [`ExtensionMap`](reovim_driver_text_session::ExtensionMap) the codec
//! state lives in without a larger lock migration.
//!
//! The full re-decode-on-read path — "when a peer mount is stale,
//! re-decode from `inode.bytes` before the client reads" — is tracked
//! as a follow-up once [`CodecSessionState`] migrates out of the
//! per-call `ExtensionMap` borrow.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use {reovim_driver_text_session::StaleCheck, reovim_kernel::api::v1::BufferId, tracing::trace};

/// Codec-side [`StaleCheck`] adapter installed at bootstrap.
///
/// Holds a shared call counter so tests can assert the hook was
/// invoked without reaching into the session crate.
#[derive(Debug, Default)]
pub struct InodeStaleCheck {
    calls: AtomicUsize,
}

impl InodeStaleCheck {
    /// Construct a fresh stale-check adapter with a zeroed counter.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
        }
    }

    /// Total number of [`refresh_if_stale`](StaleCheck::refresh_if_stale)
    /// calls observed by this adapter instance.
    #[must_use]
    pub fn call_count(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }
}

impl StaleCheck for InodeStaleCheck {
    fn refresh_if_stale(&self, buffer: BufferId) {
        let count = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
        trace!(buffer_id = buffer.as_usize(), total_calls = count, "codec-stale-check-invoked");
    }
}

/// Construct an `Arc<dyn StaleCheck>` backed by [`InodeStaleCheck`].
///
/// Small convenience wrapper for bootstrap code so the call site does
/// not need an explicit `as Arc<dyn StaleCheck>` cast.
#[must_use]
pub fn install() -> Arc<dyn StaleCheck> {
    Arc::new(InodeStaleCheck::new())
}

#[cfg(test)]
#[path = "stale_check_tests.rs"]
mod tests;
