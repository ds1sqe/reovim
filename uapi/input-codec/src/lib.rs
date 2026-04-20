#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Input codec crate — opaque input mechanism.
//!
//! This is the closed UAPI mechanism layer for input events.  It owns:
//! - The opaque `InputEvent` envelope and frozen 8-byte header format.
//! - Header accessor functions (`input_kind`, `input_flags`, `input_context`).
//! - The `Codec` trait that platform codec modules implement.
//! - `InputSequence` — an ordered series of payloads for keybinding lookup.
//!
//! Platform-typed vocabulary (`KeyCode`, `Modifiers`, `KeyEvent`, `MouseEvent`,
//! etc.) lives in `ext/input-codec/<platform>/` crates, NOT here.  This crate
//! is intentionally closed: it never grows per-platform content.
//!
//! # Kind Registry
//!
//! | Range | Owner | Purpose |
//! |-------|-------|---------|
//! | `0x0001-0x00FF` | In-repo well-known codecs | Key, pointer, scroll |
//! | `0x0100-0xFFFF` | External crates | Anyone can claim a range |

pub mod codec;
pub mod input_event;
pub mod input_sequence;

pub use {
    codec::Codec,
    input_event::{
        INPUT_HEADER_SIZE, InputEvent, InputFlags, InputPayloadError, input_context, input_flags,
        input_kind,
    },
    input_sequence::InputSequence,
};

#[cfg(test)]
mod codec_tests;
#[cfg(test)]
mod input_event_tests;
#[cfg(test)]
mod input_sequence_tests;
