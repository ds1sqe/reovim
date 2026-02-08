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
//! server/lib/drivers/buffer/   → Key + Registry (MECHANISM)
//! server/modules/buffer-simple/→ SimpleBufferManager implementation (POLICY)
//! ```

mod key;
mod mock;
mod registry;

pub use {key::BufferManagerKey, mock::TestBufferManager, registry::BufferManagerRegistry};
