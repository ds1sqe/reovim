//! Text Domain — `OnRawInput` handler + `Render` projector (§4.1 subset, #797).
//!
//! This is the **walking-skeleton** realization of the text Domain
//! (`ext/server/domain/text/`, spec category). It is a static in-repo crate
//! that the composition root registers directly — statically linked until
//! the cdylib loader lands (#778 Phase 5). The kernel routes opaque through `DomainRouter`;
//! the kernel crate has zero dependency on this crate (core/ext boundary).
//! This crate implements the contract traits from `reovim-subsys-domain` (the
//! Domain contract tier, DAG2) and does not depend on the kernel.
//!
//! ## What is implemented
//!
//! - [`TextHandler`]: `OnRawInput` handler — applies the minimal edit set to
//!   the buffer bytes: printable-char insert at cursor, backspace delete,
//!   cursor left, cursor right.
//! - [`TextProjector`]: `Render` projector — emits a [`Projection`] with one
//!   `ProjectionSpan` over the full buffer byte range plus a cursor marker
//!   (§5.3 walking-skeleton subset).
//!
//! ## What is NOT implemented (deferred to #778 Phase 4)
//!
//! - DT10 priority bands, DT17 tie-break.
//! - Multi-attachment, focus transitions (DT14..DT16).
//! - `DomainManifest`, `DomainScope`.
//! - View-slot lifecycle (DT3), per-client state.
//!
//! ## Dependency direction
//!
//! `reovim-domain-text` → `reovim-subsys-domain` (implements the
//! `OnRawInputHandler` / `RenderProjector` contracts, produces `Projection`).
//! It has no dependency on the kernel; the kernel depends on the same contract
//! tier and routes opaque through it (core/ext boundary, `CLAUDE.md §Design
//! Rules`).
#![no_std]

pub mod handler;
pub mod projector;

pub use {handler::TextHandler, projector::TextProjector};

// ── L12 sibling test files ────────────────────────────────────────────────────
#[cfg(feature = "selftest")]
#[path = "handler_tests.rs"]
mod handler_tests;

#[cfg(feature = "selftest")]
#[path = "projector_tests.rs"]
mod projector_tests;
