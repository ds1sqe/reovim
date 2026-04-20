//! Re-export notification queue types from driver-session.
//!
//! The canonical `PendingNotificationQueue` lives in `reovim-driver-session`
//! so that multiple modules can push notifications without depending on each other.
//! This re-export preserves backward compatibility.

pub use reovim_driver_text_session::{
    PendingEntry, PendingLevel, PendingNotification, PendingNotificationQueue, PendingOp,
};
