#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Content codec driver for reovim.
//!
//! This crate defines the trait interface for content encoding and decoding.
//! It does NOT depend on any specific encoding library (e.g., `encoding_rs`).
//! Those are implementation details of codec modules (`server/modules/codec-*/`).
//!
//! # Design Philosophy
//!
//! This crate follows the Linux kernel "mechanism vs policy" principle:
//!
//! - **Driver provides MECHANISM**: The traits and stores define HOW codecs
//!   are registered, discovered, and invoked.
//! - **Modules provide POLICY**: Codec modules decide WHAT encodings to support
//!   and HOW to detect/decode/encode them.
//!
//! # Architecture
//!
//! ```text
//! ext/server/drivers/codec/               <-- Traits, stores, types
//!        ^
//!        |  implements
//!        |
//! server/modules/codec-utf8/             <-- UTF-8 codec
//! server/modules/codec-hex/              <-- Hex dump codec
//! server/modules/codec-cjk/              <-- CJK encoding codecs
//! server/modules/codec-legacy/           <-- Legacy encoding codecs
//! ```
//!
//! # Components
//!
//! - [`ContentCodec`] — Decode raw bytes to text, encode text back to bytes
//! - [`ContentClassifier`] — Detect content type from raw bytes
//! - [`ContentCodecFactory`] — Create codec instances for content types
//! - [`ContentCodecFactoryStore`] — Service registry for codec factories
//! - [`ContentClassifierStore`] — Service registry for classifiers
//! - [`CodecSessionState`] — Per-buffer codec metadata (session extension)
//! - [`ContentType`] — Content type identifier
//! - [`CodecMetadata`] — Round-trip encoding metadata
//! - [`DecodeResult`] — Result of decoding (content + annotations + metadata)
//! - [`CodecError`] — Error type for codec operations

// ============================================================================
// Modules
// ============================================================================

mod classifier;
mod codec;
mod codec_params;
mod content_type;
mod decoded_edit;
mod domain_codec;
mod error;
mod errors;
mod factory;
mod inode;
mod metadata;
mod stale_check;
mod state;
mod store;

/// Codec driver events (file-type lifecycle, codec selection).
pub mod events;

/// Verification harness for `ContentCodec` implementations.
///
/// Gated behind the `testing` feature flag. Enabled automatically under
/// `#[cfg(test)]` so this crate's own test suite can exercise the
/// harness against its mock codecs.
#[cfg(any(test, feature = "testing"))]
pub mod testing;

// ============================================================================
// Re-exports
// ============================================================================

// Core traits
pub use {
    classifier::ContentClassifier,
    codec::ContentCodec,
    factory::ContentCodecFactory,
    store::{ContentClassifierStore, ContentCodecFactoryStore},
};

// Domain-generic codec traits
pub use domain_codec::{ByteNotifiable, Decode, DecodeOutput, Encode, Index};

// Decoded edits and inode/mount primitives.
pub use {
    decoded_edit::{AnyTreeOp, DecodedEdit, TreeOp, TreePath, text_edit_to_decoded_edit},
    errors::{
        EditError, InodeError, MountCodecError, MountError, SwitchViewError, TranslateEditError,
        UmountCodecError, UmountError,
    },
    inode::{Inode, InodeId, InodeTable, Mount, MountHandle, MountId, MountMode},
};

// Parameters and configuration objects.
pub use codec_params::CodecParams;

// Types
pub use {
    codec::{CodecView, DecodeResult},
    content_type::ContentType,
    error::CodecError,
    metadata::CodecMetadata,
};

// Per-session state
pub use state::{CodecSessionState, MountInfo};

// Domain-text-events re-exports for upper layers (server tests) to avoid
// direct domain-text-events dependencies.
pub use reovim_domain_text_events::{TextBufferModified, TextEdit, TextPosition};

// StaleCheck adapter (#740 Plan 06 Phase 5 sub-commit 5e).
pub use stale_check::{InodeStaleCheck, install as install_stale_check};
