//! `reovim-kernel` — the kernel process core.
//!
//! This crate is the kernel tier of the v0.16 sovereign rebuild (#778).
//! It sits directly above `arch/` and `uapi/abi` in the layer DAG (L9/DAG5):
//! no third-party crates, no `alloc`, every heap container an `lib_ds::*`
//! type (L10/DAG6).
//!
//! ## Design: two types, one handoff (2.1, LF13)
//!
//! Boot and steady state are different types, not different flags:
//!
//! - [`Init`] exists only during boot. It exclusively owns the kernel state
//!   under construction (`&mut self`, no locks), runs the boot stages (2.2 §1),
//!   and is *consumed* at the handoff.
//! - [`Kernel`] is the steady-state root — inert shared state behind the
//!   hostapi. It can only be constructed by [`Init::boot`] (LF13).
//!
//! ## Boot-stage contract (2.2 §1, structural-stub rule)
//!
//! A boot stage's realized invariant is its `boot.stage.{start,ok}` event
//! contract and its ordering position — not the completeness of its policy
//! work. Stages whose policy work is deferred to a later master-ladder phase
//! emit their events at the correct position and perform no policy work (a
//! no-op stage is `boot.stage.*`-conformant provided its ordering holds).
//! Stage numbers are therefore stable, which the LOG/OBS goldens depend on.
//!
//! ## Module tree
//!
//! | Module | Purpose |
//! |---|---|
//! | [`init`] | `Init` typestate, `LauncherArgs`, `BootError` |
//! | [`kernel`] | `Kernel` steady-state root (boot-core subset of 2.1 §3) |
//! | [`clock`] | `BootClock` — monotonic re-base + CLOCK\_REALTIME anchor |
//! | [`event_bus`] | `DS12EventBus`, `DS12Event`, OBS1 boot families |
//! | [`boot`] | Boot-stage driver, stages 1..7, OBS1 event emission |
//! | [`log`] | LOG1 one mechanism: LOG2 renderer, LOG6 ring, LOG7 sink |
//! | [`router`] | `DomainRouter` SUBSET — intern + one handler + one projector row (contract types re-exported from `reovim-subsys-domain`) |
//! | [`session`] | `Session` + `SessionState` — one session, single-entry focus chain |
//! | [`projection`] | re-export of `Projection`/`ProjectionSpan` from `reovim-subsys-domain` (DAG2 contract tier) |
#![no_std]

pub mod boot;
pub mod clock;
pub mod event_bus;
pub mod init;
pub mod kernel;
pub mod log;
pub mod projection;
pub mod router;
pub mod session;
pub mod state;

// Re-export the primary entry-point types at the crate root for callers.
pub use {
    clock::BootClock,
    init::{BootError, Init, LauncherArgs},
    kernel::Kernel,
};

// ── L12 sibling test files (kernel-selftest bin in tests/fixtures/ runs these) ─
//
// Tests live in sibling `*_tests.rs` files compiled under the `selftest`
// feature; there are no inline `#[cfg(test)] mod tests {}` blocks anywhere in
// this crate (checked by scripts/check-test-layout.sh).

#[cfg(feature = "selftest")]
#[path = "init_tests.rs"]
mod init_tests;

#[cfg(feature = "selftest")]
#[path = "clock_tests.rs"]
mod clock_tests;

#[cfg(feature = "selftest")]
#[path = "kernel_tests.rs"]
mod kernel_tests;

#[cfg(feature = "selftest")]
#[path = "event_bus_tests.rs"]
mod event_bus_tests;

#[cfg(feature = "selftest")]
#[path = "boot_tests.rs"]
mod boot_tests;

#[cfg(feature = "selftest")]
#[path = "router_tests.rs"]
mod router_tests;

#[cfg(feature = "selftest")]
#[path = "session_tests.rs"]
mod session_tests;

#[cfg(feature = "selftest")]
#[path = "state_tests.rs"]
mod state_tests;

// `projection.rs` is a re-export of `reovim-subsys-domain` types; its tests and
// doc-tests live with the types in that crate. No sibling test module here.

// The log sub-module test files are declared inside log/mod.rs (the sibling
// `*_tests.rs` pattern used within a sub-module). No additional declarations
// needed here.
