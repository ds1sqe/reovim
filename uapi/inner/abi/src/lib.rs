//! `reovim-uapi-abi` — the frozen `#[repr(C)]` ABI type catalog.
//!
//! This crate is the single layout authority for every type that crosses the
//! cdylib ABI boundary (6.3 §1).  Other chapters reference by type name;
//! they do not redefine.  `uapi/abi/src/*.rs` mirrors the catalog exactly;
//! mismatch fails CI (golden offset/size tests, Phase 3).
//!
//! **Layer:** uapi tier — `#![no_std]`, `core`-only.  No `alloc`, no
//! `arch/` dependency, no third-party crates (L9/DAG5 + L10/DAG6).
//!
//! **ABI-freeze discipline (AB15):** every layout in this crate that ships
//! stable is frozen forever.  The catalog is a transcription of the spec,
//! not a design exercise.
//!
//! ## Module structure (mirrors spec chapter §§2–9, §12)
//!
//! | Module | Spec section |
//! |---|---|
//! | [`ids`] | 6.3 §2.1 — versions + identifiers |
//! | [`slices`] | 6.3 §2.2 — byte-slice types |
//! | [`error`] | 6.3 §2.3–2.4, 6.2 §3 — `ErrorCode`, `LogLevel` |
//! | [`vtable`] | 6.3 §3, 6.2 §2 — vtable header + `ManifestKind` |
//! | [`config`] | 6.3 §4, 6.4 — config slice ABI |
//! | [`coordination`] | 6.3 §5 — coordination carriers |
//! | [`service`] | 6.3 §6 — service descriptor |
//! | [`input`] | 6.3 §7 — `RawInput` + per-kind payloads |
//! | [`stream`] | 6.3 §8 — stream substrate |
//! | [`tree`] | 6.3 §9 — domain tree |
//! | [`frame`] | 6.3 §12 — wire frame header |
#![no_std]

pub mod config;
pub mod coordination;
pub mod error;
pub mod frame;
pub mod ids;
pub mod input;
pub mod service;
pub mod slices;
pub mod stream;
pub mod tree;
pub mod vtable;

// Re-export frequently-used top-level types for convenience.
pub use {
    error::{ErrorCode, LogLevel},
    frame::FrameHeader,
    ids::{AbiVersion, Version},
    vtable::{ManifestKind, VtableHeader},
};
