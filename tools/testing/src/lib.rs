//! DEV2 E2E exec harness support library (#797 Phase 5).
//!
//! This crate is the host for the `tests/e2e_exec.rs` integration test that
//! implements the walking-skeleton DEV2 smoke:
//!
//! - Builds the `apps/reovim` composition-root binary from the nested apps
//!   workspace.
//! - Runs it with a pipe as stdin (deterministic, non-PTY — DEV5).
//! - Captures stdout (the composed ANSI frame stream).
//! - Asserts the captured frames against committed goldens (DEV3 frame golden,
//!   DEV4 buffer-byte golden).
//! - Runs the same scenario twice and verifies byte-identical captures (DEV5).
//!
//! The library body is intentionally empty; all test logic lives in the
//! `tests/` integration directory.

#![no_std]
