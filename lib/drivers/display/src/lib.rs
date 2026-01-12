//! Display driver for reovim.
//!
//! Linux equivalent: `drivers/video/`, `drivers/gpu/`
//!
//! # Architecture
//!
//! This crate defines trait interfaces for display operations.
//! Types are re-exported from `reovim-core` during the staged migration.
//!
//! ```text
//! lib/drivers/display/      <-- Traits (this crate)
//!        ^
//!        |  re-exports types from (Phase 3.3)
//!        |
//! lib/core/                 <-- Cell, Style, FrameBuffer, ZGroup, ZOrder
//! ```
//!
//! # Migration Path
//!
//! - **Phase 3.3 (current)**: Re-export types from `reovim-core`
//! - **Phase 5-6 (future)**: Types move to this crate, `lib/core` imports from here
//!
//! # Components
//!
//! - [`DisplayDriver`] - Terminal lifecycle and rendering
//! - [`WindowManager`] - Window splits, tabs, layout
//! - [`Compositor`] - Z-order layer management
//! - [`DisplayCapabilities`] - Terminal feature detection
//! - [`RenderCommand`] - Render operation commands

// ============================================================================
// Modules
// ============================================================================

mod capabilities;
mod command;
mod compositor;
mod error;
mod traits;
mod window;

// ============================================================================
// Re-exports
// ============================================================================

// Error types
pub use error::DisplayError;

// Capability detection
pub use capabilities::DisplayCapabilities;

// Render commands
pub use command::RenderCommand;

// Window types
pub use window::{NavigateDirection, Rect, SplitDirection, TerminalSize, WindowId};

// Traits
pub use traits::{DisplayDriver, WindowManager};

// Compositor (trait + re-exports from core)
pub use compositor::{Composable, ComposableId, Compositor, ZGroup, ZOrder};

// ============================================================================
// TODO(Phase 5-6): Type Migration
// ============================================================================
//
// After type migration, these will be local definitions:
//   - Cell, FrameBuffer → from crate::cell, crate::buffer
//   - Style, Attributes, Color, ColorMode → from crate::style, crate::color
//   - ZGroup, ZOrder, Composable, ComposableId → from crate::compositor (already exported above)
//
// For now, re-export from lib/core to maintain type compatibility.
// When migration happens:
//   1. Move type definitions from lib/core to this crate
//   2. Update lib/core to import FROM this driver
//   3. Remove reovim-core dependency from this crate's Cargo.toml
// ============================================================================

pub use reovim_core::{
    frame::{Cell, FrameBuffer},
    highlight::{Attributes, Color, ColorMode, Style},
};
