//! The Domain dispatch contracts — `OnRawInputHandler` and `RenderProjector`
//! (§4.1 §5, walking-skeleton subset, #797).
//!
//! These traits are the contract surface the kernel routes through and that
//! `ext/server/domain/*` crates implement. They live in the contract tier
//! (DAG2): the kernel's `DomainRouter` stores `&'static dyn` rows of these
//! traits, and ext Domains implement them, so neither tier owns the trait
//! definition — the contract tier does.

use reovim_arch::ds::Bytes;

use crate::{
    id::{BufferId, WindowId},
    projection::Projection,
};

/// The `OnRawInput` handler contract (§4.1 §5, walking-skeleton subset).
///
/// Called by `Session::dispatch_input` with the lock NOT held (CC14).
/// Returns the new buffer bytes and cursor position after applying the input.
///
/// The implementation MAY allocate (the returned `Bytes` is heap-owned via
/// `arch::ds::Bytes`). The kernel passes ownership of the snapshot buffer
/// in; the handler owns it, modifies it, and returns it.
///
/// ```rust,no_run
/// // no_run: trait; see TextHandler in reovim-domain-text for a concrete impl.
/// ```
pub trait OnRawInputHandler: Send + Sync {
    /// Applies the raw input bytes to the buffer snapshot, returning the
    /// updated buffer and cursor position.
    ///
    /// `buffer`: owned buffer snapshot (rule of three: one handler, so
    /// transferring ownership avoids an unnecessary clone inside the handler).
    /// `cursor`: current cursor byte offset.
    /// `input`: the raw input bytes from the `SendInput` message.
    fn on_raw_input(&self, buffer: Bytes, cursor: usize, input: &[u8]) -> (Bytes, usize);
}

/// The `Render` projector contract (§4.1 §5, walking-skeleton subset).
///
/// Called by `Session::dispatch_input` with the lock NOT held (CC14).
///
/// ```rust,no_run
/// // no_run: trait; see TextProjector in reovim-domain-text for a concrete impl.
/// ```
pub trait RenderProjector: Send + Sync {
    /// Renders the buffer snapshot into a `Projection`.
    ///
    /// `buffer`: buffer snapshot at projection time.
    /// `cursor`: cursor byte offset.
    /// `buffer_id`/`window_id`: routing identifiers copied from session state.
    ///
    /// # Errors
    ///
    /// Returns `Err(&'static str)` on allocation failure.
    fn render(
        &self,
        buffer: Bytes,
        cursor: usize,
        buffer_id: BufferId,
        window_id: WindowId,
    ) -> Result<Projection, &'static str>;
}

// L12 layout: tests in sibling contract_tests.rs, declared in lib.rs.
