//! Synchronization types for multi-client scenarios.
//!
//! These types manage how state is synchronized between multiple
//! clients attached to the same session.

pub mod layout;
pub mod overlay;
pub mod presence;

pub use {layout::LayoutSyncMode, overlay::OverlaySyncMode, presence::PresenceTracker};
