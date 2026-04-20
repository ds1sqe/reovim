//! Buffer manager driver for reovim.
//!
//! This driver defines the typed key and registry for buffer manager lookup.
//! The `BufferManager` trait itself is defined in the kernel.
//!
//! Following the mechanism/policy separation:
//!
//! - **Mechanism** (kernel): `BufferManager` trait
//! - **Mechanism** (this driver): `BufferManagerKey`, `BufferManagerRegistry`
//! - **Policy** (modules): Implementations like `SimpleBufferManager`
//!
//! # Architecture
//!
//! ```text
//! server/lib/kernel/           → BufferManager trait (MECHANISM)
//! ext/server/drivers/buffer/   → Key + Registry (MECHANISM)
//! server/modules/buffer-simple/→ SimpleBufferManager implementation (POLICY)
//! ```

mod capabilities;
mod key;
mod mock;
mod registry;

pub use {
    capabilities::BufferCapabilities, key::BufferManagerKey, mock::TestBufferManager,
    registry::BufferManagerRegistry,
};

// Provider-text re-exports for server layer access via driver path.
// The server depends on this driver (not on reovim-provider-text directly).
pub use reovim_provider_text::{Buffer, BufferOps, Position, TextBufferRegistry};
