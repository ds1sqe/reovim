//! TUI platform runtime — thin UDS client + raw terminal ANSI paint (#797 Phase 4).
//!
//! This is the deliberately THIN single-crate realization documented in 8.2 §2:
//! the UDS carrier, the framed handshake, and raw terminal/ANSI paint live
//! together until the second platform or driver consumer arrives. The 8.1
//! `clients/lib/subsys/*` split grows out of this crate, not beside it (rule of
//! three — the seam shapes are observed when the second consumer arrives).
//!
//! ## Layer rules (DAG2)
//!
//! `ClientExt` may depend on `ClientContracts` + `Foundation`. There are no
//! `ClientContracts` crates yet (`clients/lib/subsys/` is still empty in v0.16),
//! so this crate depends on `Foundation` only: pure DS containers plus
//! `uapi/*` vocabulary. The optional `arch` edge is selftest-only.
//!
//! ## What this crate owns
//!
//! - [`carrier`]: UDS connect, framed handshake (`Hello`→`HelloAck`→`Attach`→
//!   `AttachAck`), notify read loop.
//! - [`input`]: raw-byte stdin read + `RawInput` classification.
//! - [`frame`]: pure ANSI frame composer — `Projection` bytes → ANSI escape
//!   sequence bytes.
//! - [`paint`]: raw terminal paint loop — enter raw mode via the up-face
//!   `uapi/terminal` control table supplied by the composition root, register
//!   the pre-exit terminal-restore callback via the up-face `uapi/panic`
//!   control table, read stdin + send `SendInput`, receive
//!   `AttachEvent::Projection` + paint.
//! - [`run`]: `pub fn run(args: RunArgs) -> Result<(), RunError>` — the platform
//!   contract entry (8.2 §1).
//!
//! ## Pre-exit terminal-restore (gap-7, 8.2 §2)
//!
//! Before entering raw mode the platform runtime registers the supplied
//! terminal-restore function through the panic/log control table. If the process
//! panics, the handler calls the hook before rendering its output, so the
//! terminal is back in cooked mode when the panic line reaches the controlling
//! terminal.
#![no_std]

pub mod carrier;
pub mod frame;
pub mod input;
pub mod paint;

pub use paint::{RunArgs, RunError, run};

// ── L12 sibling test files ────────────────────────────────────────────────────
//
// Tests live in sibling `*_tests.rs` files under `selftest` (L12.1).

#[cfg(feature = "selftest")]
#[path = "carrier_tests.rs"]
mod carrier_tests;

#[cfg(feature = "selftest")]
#[path = "frame_tests.rs"]
mod frame_tests;

#[cfg(feature = "selftest")]
#[path = "paint_tests.rs"]
mod paint_tests;
