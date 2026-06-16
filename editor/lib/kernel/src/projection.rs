//! `Projection` and `ProjectionSpan` — re-exported from the Domain contract
//! tier (§5.3 walking-skeleton subset, #797).
//!
//! These types live in `reovim-subsys-domain` (the Domain contract tier, DAG2):
//! the kernel's `dispatch_input` returns a `Projection`, and ext Domains produce
//! one, so the type belongs to the contract that sits between them rather than
//! to the kernel itself. This module re-exports them so callers that name
//! `reovim_kernel::projection::{Projection, ProjectionSpan, ProjectionDecodeError}`
//! continue to compile.

pub use reovim_subsys_domain::projection::{Projection, ProjectionDecodeError, ProjectionSpan};

// The encode/decode tests and the type doc-tests live with the types in
// `reovim-subsys-domain`. No sibling test file is declared for this re-export
// module.
