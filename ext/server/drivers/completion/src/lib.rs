#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Completion driver for reovim.
//!
//! This driver defines the interface for pluggable completion sources.
//! Following the mechanism/policy separation:
//!
//! - **Mechanism** (this driver): `CompletionSource` trait, types,
//!   `CompletionSourceRegistry`, `CompletionEngine`
//! - **Policy** (modules): Implementations like `BufferWordsSource`,
//!   `LspCompletionSource`
//!
//! # Architecture
//!
//! ```text
//! ext/server/drivers/completion/   -> Trait + Types + Registry + Engine (MECHANISM)
//! server/modules/completion/       -> Source implementations + orchestration (POLICY)
//! ```

mod context;
mod engine;
mod item;
mod registry;
mod source;

pub use {
    context::CompletionContext,
    engine::{CompletionEngine, EngineItem, TickStatus, push_item, push_items},
    item::{CompletionItem, CompletionKind},
    registry::CompletionSourceRegistry,
    source::CompletionSource,
};
