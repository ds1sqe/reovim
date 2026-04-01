//! Search provider driver for reovim.
//!
//! This driver defines the interface for pattern matching in buffers.
//! Following the mechanism/policy separation:
//!
//! - **Mechanism** (this driver): `SearchProvider` trait, types, `SearchKey`, `SearchProviderRegistry`
//! - **Policy** (modules): Implementations like regex-based `SearchEngine`
//!
//! # Architecture
//!
//! ```text
//! server/lib/drivers/search/   → Trait + Types + Key + Registry (MECHANISM)
//! server/modules/search/       → SearchEngine implementation (POLICY)
//! ```

mod key;
mod line_source;
mod provider;
mod registry;
mod types;

pub use {
    key::SearchKey,
    line_source::{BufferLineSource, BufferOpsLineSource, LineSource, VirtualBufferLineSource},
    provider::SearchProvider,
    registry::SearchProviderRegistry,
    types::{Direction, SearchError, SearchMatch},
};
