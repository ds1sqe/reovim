#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Clipboard provider driver for reovim.
//!
//! This driver defines the interface for OS clipboard access.
//! Following the mechanism/policy separation:
//!
//! - **Mechanism** (this driver): `ClipboardProvider` trait, `ClipboardKey`, registry
//! - **Policy** (modules): Implementations with actual OS clipboard integration
//!
//! # Architecture (#515 Phase 4)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Per-client RegisterBank                                     │
//! │  - Unnamed register ("")                                     │
//! │  - Named registers (a-z)                                     │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Per-client HistoryRing                                      │
//! │  - Numbered registers (0-9, yank history)                    │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Clipboard Driver (this crate)                               │
//! │  - System clipboard (+)                                      │
//! │  - Selection clipboard (*)                                   │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_clipboard::{ClipboardKey, ClipboardProviderRegistry};
//!
//! // In module init():
//! let clipboard = Arc::new(MyClipboardService::new());
//! let registry = ctx.services.get_or_create::<ClipboardProviderRegistry>();
//! registry.register(ClipboardKey::Default, clipboard);
//!
//! // In operator execute():
//! if let Some(registry) = ctx.services.get::<ClipboardProviderRegistry>() {
//!     if let Some(provider) = registry.get(&ClipboardKey::Default) {
//!         if register == Some('+') {
//!             let _ = provider.copy_to_clipboard(&content.text);
//!         }
//!     }
//! }
//! ```

mod error;
mod key;
mod provider;
mod registry;

pub use {
    error::ClipboardError, key::ClipboardKey, provider::ClipboardProvider,
    registry::ClipboardProviderRegistry,
};
