//! Undo provider driver for reovim.
//!
//! This driver defines the interface for per-buffer undo/redo operations.
//! Following the mechanism/policy separation:
//!
//! - **Mechanism** (this driver): `UndoProvider` trait, `UndoKey`, `UndoProviderRegistry`
//! - **Policy** (modules): Implementations like `UndoRegistry` with persistence
//!
//! # Architecture
//!
//! ```text
//! server/lib/drivers/undo/     → Trait + Key + Registry + Error (MECHANISM)
//! server/modules/undo/         → UndoRegistry implementation (POLICY)
//! ```

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod error;
mod key;
mod provider;
mod registry;

pub use {
    error::UndoPersistError, key::UndoKey, provider::UndoProvider, registry::UndoProviderRegistry,
};
