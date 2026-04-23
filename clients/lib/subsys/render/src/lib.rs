#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Client render subsystem.
//!
//! Provides the `RenderTarget` trait, the `ClientRender` driver trait,
//! and the C-stable ABI sibling types used by runtime-loaded render
//! drivers.

pub mod abi;
pub mod client_render;
pub mod target;

pub use client_render::{ClientRender, ClientRenderDriverProbe, ClientRenderError};
pub use target::{RenderError, RenderTarget};
