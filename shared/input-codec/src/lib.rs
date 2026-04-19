#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Input codec crate — typed key/mouse vocab plus well-known payload codecs.
//!
//! This crate lives outside subsys (in `shared/`) because it owns the typed
//! key/mouse vocabulary, arch conversions, kind values, and body layouts.
//! `reovim-subsys-input` remains the home of the opaque `InputEvent` envelope
//! and frozen 8-byte header only.
//!
//! # Kind Registry
//!
//! | Range | Owner | Purpose |
//! |-------|-------|---------|
//! | `0x0001-0x00FF` | This crate (in-repo) | Well-known: key, pointer, scroll |
//! | `0x0100-0xFFFF` | External crates | Anyone can claim a range |

mod contracts;
mod convert;
pub mod key;
pub mod key_types;
pub mod mouse_types;
pub mod pointer;
pub mod scroll;

pub use {
    contracts::{key_event_to_contract_token, key_sequence_to_key_events},
    key_types::{KeyCode, KeyEvent, KeyEventKind, KeymapResult, Modifiers},
    mouse_types::{MouseButton, MouseEvent, MouseEventKind},
};

#[cfg(test)]
mod convert_tests;
#[cfg(test)]
mod key_tests;
#[cfg(test)]
mod mouse_tests;
#[cfg(test)]
mod pointer_tests;
#[cfg(test)]
mod scroll_tests;
