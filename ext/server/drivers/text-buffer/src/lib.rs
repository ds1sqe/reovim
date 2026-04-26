//! Buffer driver for reovim — runtime-loaded cdylib that satisfies the
//! [`reovim_subsys_buffer::BufferDriver`] contract over the rope-backed
//! `provider-text` storage.
//!
//! # Layers
//!
//! - **Mechanism** (kernel): `BufferManager` trait
//! - **Mechanism** (this driver, legacy): `BufferManagerKey`,
//!   `BufferManagerRegistry` — kept on the rlib surface so existing
//!   server consumers (Phase 5/6 will retire them) still build.
//! - **Mechanism** (this driver, new): [`TextBufferDriverImpl`] +
//!   [`TextBufferImpl`] — the cdylib surface backing the buffer subsys.
//! - **Policy** (modules): higher-level buffer managers in
//!   `ext/server/modules/`.
//!
//! # cdylib export
//!
//! `declare_buffer_driver!` (from `reovim-driver-macros`) emits the
//! `REOVIM_BUFFER_DRIVER_VTABLE` symbol that the runtime loader
//! discovers. The rlib surface keeps the legacy `pub use`s so
//! intra-workspace consumers can keep linking until #774 retires the
//! old paths.

mod buffer_impl;
mod capabilities;
mod driver;
mod key;
mod mock;
mod probe;
mod registry;

#[cfg(test)]
mod buffer_impl_tests;
#[cfg(test)]
mod driver_tests;

pub use {
    buffer_impl::TextBufferImpl, capabilities::BufferCapabilities, driver::TextBufferDriverImpl,
    key::BufferManagerKey, mock::TestBufferManager, registry::BufferManagerRegistry,
};

// Provider-text re-exports for server layer access via driver path.
// The server depends on this driver (not on reovim-provider-text directly).
pub use reovim_provider_text::{Buffer, BufferOps, Position, TextBufferRegistry};

// Cdylib export — `REOVIM_BUFFER_DRIVER_VTABLE` symbol.
reovim_driver_macros::declare_buffer_driver!(TextBufferDriverImpl);
