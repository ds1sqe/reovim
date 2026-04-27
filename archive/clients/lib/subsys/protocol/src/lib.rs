#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Client protocol subsystem.
//!
//! Protocol-neutral notification-stream contracts and parse helpers.
//! Concrete dispatch implementations live in downstream crates.

pub mod dispatch;

pub use dispatch::{DispatchOutcome, NotificationDispatcher};

#[cfg(feature = "serde")]
pub mod parse;

#[cfg(feature = "serde")]
pub use parse::{parse_notification, parse_notification_field};

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
