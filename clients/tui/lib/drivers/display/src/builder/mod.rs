//! Display builder system for component registration.
//!
//! This module provides a registry and builder pattern for components to
//! register their visual representation (icons, names, styles).
//!
//! # Architecture
//!
//! The display builder follows the mechanism vs policy separation:
//!
//! - **Mechanism** (this module): Defines `DisplayRegistry`, `DisplayInfo`,
//!   and `DisplayInfoBuilder` types that provide the WHAT.
//! - **Policy** (runner/modules): The runner owns the registry and modules
//!   decide HOW to register their display information.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::builder::{DisplayRegistry, ComponentId};
//!
//! // Runner creates and owns the registry
//! let mut registry = DisplayRegistry::new();
//!
//! // Module registers its display info
//! registry.builder(ComponentId::new(1))
//!     .default(" EXPLORER ", "󰙅 ", explorer_style)
//!     .dynamic(|ctx| {
//!         // Compute dynamic display based on state
//!         ctx.downcast_ref::<ExplorerState>()
//!             .map(|s| format!(" EXPLORER ({}) ", s.file_count))
//!     })
//!     .register();
//!
//! // Renderer uses the registry
//! let display = registry.display_string_owned(id, Some(&state));
//! let icon = registry.icon(id);
//! ```

pub mod icons;
mod info_builder;
mod registry;
mod types;

pub use {
    info_builder::DisplayInfoBuilder,
    registry::DisplayRegistry,
    types::{ComponentId, DisplayInfo, DynamicDisplayFn},
};
