#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! TUI cell-grid capability for `FrameTarget` slots.
//!
//! This crate provides the `CellCapability` data type used by TUI render
//! handlers and surface encoders. It lives on the ext side of the
//! core/ext boundary so that core (`clients/lib/subsys/codec/`) remains
//! shape-blind: core knows only `FrameTarget`, not whether a target
//! carries a cell grid, a pixel buffer, a DOM tree, or a mesh scene.
//!
//! Provided types:
//!
//! - [`Cell`]: a single displayable cell (character + style).
//! - [`CellCapability`]: a bounded 2D grid of cells.
//! - [`CellStyle`]: foreground/background color + attribute flags.
//! - [`CellAttrs`]: bitflags for bold/italic/underline/reverse/dim.
//! - [`CellColor`]: rgb/ansi-256/named/default color variants.
//!
//! Nothing in flight 17-α consumes these types; Plan 18 adds a render
//! handler that populates `CellCapability` slots on a `FrameTarget` for
//! the TUI render thread to consume. The dead-code allowance is locked
//! in Plan 17 §1 (issue #150 precedent).

pub mod capability;
pub mod style;

#[cfg(test)]
mod capability_tests;
#[cfg(test)]
mod style_tests;

pub use {
    capability::{Cell, CellCapability, WriteCellError},
    style::{CellAttrs, CellColor, CellStyle},
};
