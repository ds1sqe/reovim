//! Typed notification deserialization helpers.
//!
//! Re-exported from `reovim_client_subsys_protocol`. Available when the
//! `serde` feature is enabled. Eliminates manual JSON parsing boilerplate in
//! client modules.
//!
//! # Example
//!
//! ```ignore
//! #[derive(serde::Deserialize)]
//! struct WhichKeyPayload { active: bool, prefix: String }
//!
//! fn on_notification(&mut self, data: &str) {
//!     if let Some(payload) = parse_notification::<WhichKeyPayload>(data) {
//!         self.active = payload.active;
//!     }
//! }
//! ```

pub use reovim_client_subsys_protocol::{DispatchOutcome, NotificationDispatcher};

#[cfg(feature = "serde")]
pub use reovim_client_subsys_protocol::{parse_notification, parse_notification_field};

#[cfg(test)]
#[path = "notification_tests.rs"]
mod tests;
