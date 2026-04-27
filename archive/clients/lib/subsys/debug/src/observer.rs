//! `DebugObserver` sub-handle trait.
//!
//! An observer is produced by `ClientDebugSurface::observe` and
//! represents a fallible stream of opaque byte frames. The host's
//! loader exposes a safe wrapper whose lifetime borrows the driver
//! instance, so the Rust type system forbids two observers on the same
//! driver at once. This mirrors the ABI's sub-vtable pattern:
//! `observe_start` returns a `(*const DebugObserverVTable, *mut c_void)`
//! pair, and the safe wrapper carries that pair with the borrow
//! lifetime anchored to the driver.

use crate::client_debug::DebugError;

/// A debug-frame observer sub-handle.
///
/// Frame bodies are opaque bytes; neither the CLI nor the server
/// inspects them. The driver's schema name (published in
/// [`crate::client_debug::DebugProbe::observe_schemas`]) tells
/// downstream consumers how to decode.
pub trait DebugObserver {
    /// Pump the next frame.
    ///
    /// - `Ok(Some(bytes))`: frame produced. Host owns the `Vec` until
    ///   it is dropped.
    /// - `Ok(None)`: end of stream. Further calls may produce `None`
    ///   again or an error, depending on the driver.
    /// - `Err(DebugError)`: driver-reported error. The stream is not
    ///   automatically closed; callers should drop the observer to
    ///   release resources.
    ///
    /// # Errors
    ///
    /// Returns `DebugError` if the driver cannot produce the next
    /// frame.
    fn next_frame(&mut self) -> Result<Option<Vec<u8>>, DebugError>;
}

#[cfg(test)]
#[path = "observer_tests.rs"]
mod tests;
