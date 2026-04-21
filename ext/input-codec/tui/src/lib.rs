#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! TUI platform input codec for reovim.
//!
//! This crate provides the typed keyboard, mouse, and scroll vocabulary for
//! terminal (TUI) platforms, together with `Codec` implementations for each.
//! It is a module-tier crate: on load it registers `TuiKeyCodec`,
//! `TuiMouseCodec`, and `TuiScrollCodec` with the server's
//! `InputCodecRegistry`; on unload it removes them.
//!
//! # Hierarchy
//!
//! ```text
//! uapi/input-codec/          — closed mechanism (InputEvent, Codec trait)
//!   server/lib/subsys/input/ — InputCodecRegistry trait + DefaultInputCodecRegistry
//!     ext/input-codec/tui/   — THIS CRATE (TUI vocab + codec impls + module entry)
//! ```
//!
//! # Kind assignments
//!
//! | Kind   | Codec           | Type         |
//! |--------|-----------------|--------------|
//! | 0x0001 | `TuiKeyCodec`   | `KeyEvent`   |
//! | 0x0002 | `TuiMouseCodec` | `MouseEvent` |
//! | 0x0003 | `TuiScrollCodec`| `ScrollEvent`|

pub mod codecs;
pub mod key_types;
pub mod module;
pub mod mouse_types;
pub mod scroll;

pub use {
    codecs::{
        KIND_KEY, KIND_MOUSE, KIND_SCROLL, TuiKeyCodec, TuiMouseCodec, TuiScrollCodec,
        build_full_payload, decode_key_event, decode_mouse_event, decode_scroll_event,
        encode_key_event, encode_mouse_event, encode_scroll_event,
    },
    key_types::{KeyCode, KeyEvent, KeyEventKind, KeymapResult, Modifiers},
    module::{TuiInputCodecModule, unregister_codecs},
    mouse_types::{MouseButton, MouseEvent, MouseEventKind},
    scroll::ScrollEvent,
};

#[cfg(test)]
mod codecs_tests;
#[cfg(test)]
mod key_types_tests;
#[cfg(test)]
mod module_tests;
#[cfg(test)]
mod mouse_types_tests;
