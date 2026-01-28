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
//! lib/drivers/search/   → Trait + Types + Key + Registry (MECHANISM)
//! modules/search/       → SearchEngine implementation (POLICY)
//! ```

mod key;
mod provider;
mod registry;
mod types;

pub use {
    key::SearchKey,
    provider::SearchProvider,
    registry::SearchProviderRegistry,
    types::{Direction, SearchError, SearchMatch},
};
