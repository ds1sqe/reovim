#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Input codec crate — typed key/mouse vocab plus well-known payload codecs.
//!
//! This crate lives outside subsys (in `shared/`) because it owns the typed
//! key/mouse vocabulary, arch conversions, kind values, and body layouts.
//! `reovim-subsys-input` remains the home of the opaque `InputEvent` envelope
//! and frozen 8-byte header only.
//!
//! Mission #753 Plan 10 Phase 1 keeps temporary compatibility overlap with
//! `reovim-subsys-input`'s legacy typed definitions. This crate provides the
//! explicit local↔legacy adapters needed for that overlap while the remaining
//! consumers are repointed in later phases.
//!
//! # Kind Registry
//!
//! | Range | Owner | Purpose |
//! |-------|-------|---------|
//! | `0x0001-0x00FF` | This crate (in-repo) | Well-known: key, pointer, scroll |
//! | `0x0100-0xFFFF` | External crates | Anyone can claim a range |

mod convert;
pub mod key;
pub mod key_types;
mod legacy;
pub mod mouse_types;
pub mod pointer;
pub mod scroll;

pub use {
    key_types::{KeyCode, KeyEvent, KeyEventKind, KeymapResult, Modifiers},
    mouse_types::{MouseButton, MouseEvent, MouseEventKind},
};

#[cfg(test)]
mod convert_tests;
#[cfg(test)]
mod key_tests;
#[cfg(test)]
mod legacy_tests;
#[cfg(test)]
mod mouse_tests;
#[cfg(test)]
mod pointer_tests;
#[cfg(test)]
mod scroll_tests;
