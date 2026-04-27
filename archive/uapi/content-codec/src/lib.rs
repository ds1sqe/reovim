#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Content codec uapi — domain-neutral envelope, traits, and registry
//! primitives for content encoding/decoding.
//!
//! This crate is the **closed mechanism tier** of the three-tier content
//! codec pattern:
//!
//! ```text
//! uapi/content-codec/             ← CLOSED mechanism (this crate)
//!   envelope + traits + DecodedEdit + inode/mount primitives
//!
//! server/lib/subsys/content-codec/ ← CONTRACT + REGISTRY (Plan 14 Phase C.2)
//!   re-exports uapi + ContentCodecRegistry trait + default impl
//!
//! ext/content-codec/<variant>/    ← PER-VARIANT IMPL (Plan 14 Phase C.3–C.5)
//!   concrete codec modules (text, xxd, utf8, cjk, csv, hex, pdf, rlib,
//!   tar-gz, binary-struct, legacy)
//! ```
//!
//! # Components
//!
//! - [`ContentCodec`] — decode raw bytes, translate decoded edits to byte edits.
//! - [`ContentClassifier`] — detect content type from raw bytes.
//! - [`ContentCodecFactory`] — create codec instances for a content type.
//! - [`ContentCodecFactoryStore`] / [`ContentClassifierStore`] — service registries.
//! - [`ContentType`] / [`CodecMetadata`] — content identity + round-trip metadata.
//! - [`DecodedEdit`] — domain-neutral edit enum with type-erased
//!   [`DecodedEdit::Domain`] variant for per-domain edit payloads.
//! - [`DomainEdit`] / [`TreeOp`] — opaque wrappers for domain and tree ops.
//! - [`Inode`] / [`InodeTable`] / [`Mount`] — canonical byte storage + mount
//!   primitives used by session-state crates to orchestrate codec views.
//! - [`Decode`] / [`Encode`] / [`Index`] — domain-generic codec traits
//!   (`Decode<D>`, `Encode<D>`, `Index<D>`) parameterised by [`reovim_domain::Domain`].
//! - [`Annotation`] / [`AnnotationKind`] / [`AnnotationTarget`] /
//!   [`AnnotationPayload`] — codec-produced annotation data types used by
//!   [`DecodeResult`]. Re-exported by `reovim-driver-annotation` for the
//!   gutter annotation pipeline.

// ============================================================================
// Modules
// ============================================================================

mod annotation;
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

pub use {
    annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
    classifier::ContentClassifier,
    codec::{CodecView, ContentCodec, DecodeResult},
    codec_params::CodecParams,
    content_type::ContentType,
    decoded_edit::{AnyDomainEdit, AnyTreeOp, DecodedEdit, DomainEdit, TreeOp, TreePath},
    domain_codec::{ByteNotifiable, Decode, DecodeOutput, Encode, Index},
    error::CodecError,
    errors::{
        EditError, InodeError, MountCodecError, MountError, SwitchViewError, TranslateEditError,
        UmountCodecError, UmountError,
    },
    factory::ContentCodecFactory,
    inode::{Inode, InodeId, InodeTable, Mount, MountHandle, MountId, MountMode},
    metadata::CodecMetadata,
    store::{ContentClassifierStore, ContentCodecFactoryStore},
};
