//! `reovim-subsys-domain` — the Domain contract tier (DAG2).
//!
//! This crate holds the contract surface shared between the kernel (which
//! routes through it) and `ext/server/domain/*` Domains (which implement it).
//! Per the layer DAG, the contract tier depends only on Foundation
//! (`reovim-arch`); both `ServerKernel` and `ServerExt` are permitted to
//! depend on `ServerContracts`, but not on each other. Moving these types here
//! is what lets `reovim-domain-text` drop its kernel dependency without
//! breaking the kernel ← contract ← ext layering.
//!
//! ## Contract surface
//!
//! | Module | Types |
//! |---|---|
//! | [`id`] | `DomainId`, `BufferId`, `WindowId` |
//! | [`contract`] | `OnRawInputHandler`, `RenderProjector` |
//! | [`projection`] | `Projection`, `ProjectionSpan`, `ProjectionDecodeError` |
//!
//! The kernel re-exports these at their historical paths
//! (`reovim_kernel::router::*`, `reovim_kernel::projection::*`,
//! `reovim_kernel::session::{BufferId, WindowId}`) so existing consumers compile
//! unchanged.
#![no_std]

pub mod contract;
pub mod id;
pub mod projection;

// Re-export the contract types at the crate root for ergonomic consumption.
pub use {
    contract::{OnRawInputHandler, RenderProjector},
    id::{BufferId, DomainId, WindowId},
    projection::{Projection, ProjectionDecodeError, ProjectionSpan},
};

// ── L12 sibling test files (kernel-selftest bin runs these) ───────────────────
//
// Tests live in sibling `*_tests.rs` files compiled under the `selftest`
// feature; there are no inline `#[cfg(test)] mod tests {}` blocks anywhere in
// this crate (checked by scripts/check-test-layout.sh).

#[cfg(feature = "selftest")]
#[path = "id_tests.rs"]
mod id_tests;

#[cfg(feature = "selftest")]
#[path = "contract_tests.rs"]
mod contract_tests;

#[cfg(feature = "selftest")]
#[path = "projection_tests.rs"]
mod projection_tests;
