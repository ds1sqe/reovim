//! Clipboard provider driver for reovim.
//!
//! This driver defines the interface for system clipboard access and yank history.
//! Following the mechanism/policy separation:
//!
//! - **Mechanism** (this driver): `ClipboardProvider` trait, `ClipboardKey`, registry
//! - **Policy** (modules): Implementations with actual OS clipboard integration
//!
//! # Architecture
//!
//! This driver complements the kernel's `RegisterBank`:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Kernel RegisterBank                                        │
//! │  - Unnamed register ("")                                    │
//! │  - Named registers (a-z)                                    │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Clipboard Driver (this crate)                              │
//! │  - System clipboard (+)                                     │
//! │  - Selection clipboard (*)                                  │
//! │  - History registers (0-9)                                  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage Flow
//!
//! ```text
//! Yank/Delete Operation:
//! 1. Check register name
//! 2. If +/* → ClipboardProvider.copy_to_clipboard/selection()
//! 3. If a-z/unnamed → RegisterBank.set_by_name()
//! 4. Always → ClipboardProvider.push_history() (for 0-9 access)
//!
//! Paste Operation:
//! 1. Check register name
//! 2. If +/* → ClipboardProvider.paste_from_clipboard/selection()
//! 3. If 0-9 → ClipboardProvider.history_entry()
//! 4. If a-z/unnamed → RegisterBank.get_by_name()
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
//!         provider.push_history(content.clone());
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
