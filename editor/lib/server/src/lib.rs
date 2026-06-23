//! `reovim-server-rt` — the framed-protocol server runtime (#797 Phase 3).
//!
//! This crate owns the UDS listener, the per-connection framed-protocol
//! carrier loop, the SP9.1 handshake (`Hello`/`HelloAck`), the attach flow
//! (`Attach`/`AttachAck`), input dispatch (`SendInput` → editor core), and the
//! notify push (`AttachEvent::Projection`). The carrier lives here, NOT in
//! `uapi/protocol` (L11 — `uapi/protocol` stays sans-IO pure).
//!
//! ## Layer
//!
//! `ServerRuntime` — may depend on `EditorCore` + `ServerContracts` +
//! `Foundation` (category DAG, 1.2 §2).  Crates above this tier
//! (`apps/*`) provide the listener path and call [`start_listener`].
//!
//! ## Boot-stage 6 wiring (gap-2 resolution)
//!
//! Boot stage 6 in `editor/lib/core/src/boot.rs` runs as a structural stub
//! — it emits `boot.stage.{start,ok}` and performs no policy work during the
//! editor-core `EditorInit::boot` call. **The composition root fills the stage-6 body
//! by calling [`start_listener`] after `EditorInit::boot` returns**, keeping the
//! `EditorCore → ServerRuntime` dependency direction illegal (the editor-core
//! crate has no dep on this crate; only the composition root imports both and
//! wires them). This is the same pattern as the text-Domain registration:
//! `register_domain` is called by the composition root after boot, not inside
//! `EditorInit::boot`. The comment in `boot.rs` stage 6 already says
//! "framed-protocol runtime / in-memory adapter started — stub", acknowledging
//! that the policy body arrives later. No spec edit is needed (gap-2
//! confirmed: the stub rule permits a later phase to fill the body).
//!
//! ## Module tree
//!
//! | Module | Purpose |
//! |---|---|
//! | [`listener`] | UDS accept loop + thread-per-connection spawn |
//! | [`conn`] | Per-connection framed read/write state |
//! | [`carrier`] | Carrier loop: read frame → dispatch → write response |
//! | [`notify`] | Projection notify push |
//! | [`error`] | Typed error kinds (no `&'static str` errors on public APIs) |
#![no_std]

pub mod carrier;
pub mod conn;
pub mod error;
pub mod listener;
pub mod notify;

pub use listener::start_listener;

// ── L12 sibling test files ────────────────────────────────────────────────────
//
// Tests live in sibling `*_tests.rs` files compiled under `selftest`; there
// are no inline `#[cfg(test)] mod tests {}` blocks (L12.1).

#[cfg(feature = "selftest")]
#[path = "error_tests.rs"]
mod error_tests;

#[cfg(feature = "selftest")]
#[path = "listener_tests.rs"]
mod listener_tests;

#[cfg(feature = "selftest")]
#[path = "conn_tests.rs"]
mod conn_tests;

#[cfg(feature = "selftest")]
#[path = "carrier_tests.rs"]
mod carrier_tests;

#[cfg(feature = "selftest")]
#[path = "notify_tests.rs"]
mod notify_tests;
