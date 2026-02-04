//! Wire format types from the server.
//!
//! These types represent data as received from the server via gRPC.
//! They are logical/semantic (buffer positions, layout structure) rather
//! than physical (screen coordinates). Clients interpret these types
//! into rendered state appropriate for their platform.

pub mod anchor;
pub mod client;
pub mod layout;
pub mod overlay;
pub mod presence;
pub mod viewport;

pub use {
    anchor::Anchor,
    client::{ClientInfo, ClientMetadata, ClientRelation, ClientViewState},
    layout::LogicalLayout,
    overlay::{LogicalOverlay, OverlayState},
    // DEPRECATED: Use client module types instead
    presence::{ClientPresence, SyncMode},
    viewport::{ViewportState, ViewportUpdate},
};
