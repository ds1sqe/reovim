//! Testing utilities for integration tests
//!
//! This module provides helpers for writing integration tests,
//! including key event creation, string-to-key conversion,
//! and server-based end-to-end test harness.

pub mod assertions;
pub mod client;
pub mod keys;
pub mod server;
pub mod visual;

pub use {
    assertions::{ServerTest, ServerTestResult},
    client::{ClientError, ModeInfo, TelescopeInfo, TestClient, WhichKeyBindingInfo, WhichKeyInfo},
    keys::{char_key, ctrl, key, key_mod, keys_from_str},
    server::ServerTestHarness,
    visual::VisualAssertions,
};
