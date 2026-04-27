#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Text-domain carve-out for the content codec.
//!
//! This crate is the text-specific tier of the Plan 14 Phase C three-tier
//! pattern. The domain-neutral mechanism (traits, `DecodedEdit`, inode
//! primitives) lives in `uapi/content-codec/`. The text-specific pieces
//! moved out of the old combined `ext/server/drivers/codec/` live here:
//!
//! - [`TextEdit`] — concrete domain-edit payload carried by
//!   [`reovim_content_codec::DecodedEdit::Domain`] for the text domain.
//! - [`text_edit_to_decoded_edit`] — conversion helper from
//!   `reovim_domain_text_events::TextBufferModified` to a `DecodedEdit`.
//! - [`CodecSessionState`] — per-buffer codec metadata and mount table
//!   registered as a text-session `SessionExtension`.
//! - [`MountInfo`] — wire-friendly descriptor for mounts.
//! - [`InodeStaleCheck`] / [`install_stale_check`] — adapter wiring the
//!   text-session `StaleCheck` hook back into the codec inode table.
//!
//! The crate also installs itself as a module (via `declare_module!`)
//! so the runtime can load/unload it symmetrically. No content codec is
//! registered here today — text format codecs (utf8, cjk, legacy, csv,
//! hex, etc.) live in sibling `ext/content-codec/<format>/` crates.
//! This module is the text-domain runtime glue.

mod stale_check;
mod state;
mod text_edit;

pub use {
    stale_check::{InodeStaleCheck, install as install_stale_check},
    state::{CodecSessionState, MountInfo},
    text_edit::{TextEdit, text_edit_to_decoded_edit},
};

// Convenience re-exports so downstream consumers can import domain-text
// event types through this crate rather than taking a direct dep on
// `reovim-domain-text-events`. Mirrors what the pre-Phase-C driver
// offered.
pub use reovim_domain_text_events::{
    TextBufferModified, TextEdit as DomainTextEventEdit, TextPosition,
};

// ============================================================================
// Module entry point — `declare_module!`
// ============================================================================

mod module;
pub use module::TextContentCodecModule;

// `declare_module!` emits the FFI entry-point used when the module is
// dynamically loaded. Gated behind the `dynamic` feature so static
// (rlib) consumers do not produce duplicate `REOVIM_MODULE_API_VERSION`
// symbols when linked alongside other `declare_module!` crates.
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(TextContentCodecModule);
