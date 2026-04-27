#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Buffer subsys.
//!
//! Closed-tier trait surfaces for byte-container drivers.
//!
//! # Architecture
//!
//! Subsys-buffer defines the buffer-driver lifecycle contract and its
//! supporting types ([`Buffer`], [`BufferDriver`], [`BufferError`],
//! [`CodecAttachmentId`]). Concrete drivers live under
//! `ext/server/drivers/text-buffer/` and implement both traits; the
//! server crate consumes them through `Arc<dyn BufferDriver>` and
//! `Arc<dyn Buffer>`. Cdylib drivers export the canonical
//! `REOVIM_BUFFER_DRIVER_VTABLE` symbol via the
//! `declare_buffer_driver!` macro under `uapi/driver-macros/`.
//!
//! ```text
//! server/lib/subsys/buffer/        <-- Contracts (this crate)
//!        ^
//!        |  implemented by
//!        |
//! ext/server/drivers/text-buffer/  <-- cdylib driver (SP03)
//! ```
//!
//! # Components
//!
//! - [`Buffer`] — byte-container with edit fan-out and codec attachments
//! - [`BufferDriver`] — factory: produces `Arc<dyn Buffer>` for a given
//!   file path / initial bytes; carries lifecycle (open / close / list)
//! - [`BufferSubscribable`] — async edit-subscription (separated from
//!   `Buffer` to avoid coupling the core `Buffer` trait object to
//!   tokio; wire when async consumers are added)
//! - [`CodecAttachmentId`] — `NonZeroU32` newtype identifying a codec slot
//! - [`BufferError`] — structured error enum
//! - [`abi`] — `#[repr(C)]` FFI types for cdylib loading

pub mod abi;
pub mod buffer;
pub mod driver;
pub mod error;
pub mod subscribe;

pub use {
    buffer::{Buffer, CodecAttachmentId},
    driver::BufferDriver,
    error::BufferError,
    subscribe::BufferSubscribable,
};
