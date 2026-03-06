#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Range-finder TUI extension — jump labels and fold indicators.
//!
//! Provides two `TuiExtension` implementations:
//! - [`RangeFinderJumpExtension`] — renders jump labels at buffer positions
//! - [`RangeFinderFoldExtension`] — renders fold indicators at collapsed lines
//!
//! Both parse JSON from server-side bridges (`JumpBridge`, `FoldBridge`)
//! and render overlays using `ViewportContext` for buffer-to-screen mapping.

mod fold;
mod jump;

pub use {fold::RangeFinderFoldExtension, jump::RangeFinderJumpExtension};
