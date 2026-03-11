#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Common Client Model for Reovim
//!
//! This crate provides platform-agnostic abstractions for Reovim clients.
//! It separates **wire format** (data from server) from **rendered state**
//! (client-side interpretation), enabling each platform to adapt server
//! data to its specific rendering needs.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Server (gRPC)                                              │
//! │  • Raw buffer content                                       │
//! │  • Cursor position, mode                                    │
//! │  • Logical layout, overlays                                 │
//! └─────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼ Wire Format (this crate)
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Common Client Model                                        │
//! │  • LogicalLayout, LogicalOverlay (from server)              │
//! │  • WindowTree, RenderedOverlay (client interpretation)      │
//! │  • Panel, Layout traits                                     │
//! └─────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼ Platform implements traits
//! ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
//! │     TUI     │  │     Web     │  │   Android   │
//! │  (Terminal) │  │    (DOM)    │  │  (Compose)  │
//! └─────────────┘  └─────────────┘  └─────────────┘
//! ```
//!
//! # Modules
//!
//! - [`geometry`]: Screen positions, sizes, and rectangles
//! - [`direction`]: Navigation and split directions
//! - [`wire`]: Wire format types from the server

// Foundation types (Phase 10A)
pub mod direction;
pub mod geometry;

// Wire format types (Phase 10B)
pub mod wire;

// Rendered state types (Phase 10C)
pub mod rendered;

// Interaction types (Phase 10D)
pub mod interaction;

// Core traits (Phase 10E)
pub mod traits;

// Sync types (Phase 10F)
pub mod sync;

// Re-export commonly used types
pub use {
    direction::{Direction, SplitDirection},
    geometry::{Rect, ScreenPosition, Size},
};

// Re-export wire format types
pub use wire::{
    ClientPresence, ExtensionCategory, LogicalLayout, LogicalOverlay, OverlayState, SemanticOrigin,
    SyncMode, ViewportState, ViewportUpdate,
};

// Re-export rendered state types
pub use rendered::{OverlayStack, PanelState, RenderedOverlay, Window, WindowTree};

// Re-export interaction types
pub use interaction::{Interaction, InteractionResult};

// Re-export core traits
pub use traits::{DefaultLayoutInterpreter, Focus, FocusManager, Layout, LayoutInterpreter, Panel};

// Re-export sync types
pub use sync::{LayoutSyncMode, OverlaySyncMode, PresenceTracker};

// WASM bindings (feature-gated)
#[cfg(feature = "wasm")]
pub mod wasm;
