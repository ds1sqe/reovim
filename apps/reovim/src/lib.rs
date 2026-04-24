#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! `reovim` launcher library.
//!
//! Dispatches the top-level CLI to the subprocess passthrough: spawns
//! the appropriate sibling `reovim-*` bin via
//! `std::process::Command`. An in-process composition mode is a
//! planned future addition tracked under issue #769; this library
//! carries the feature-gated optional dependencies for that mode
//! ahead of the code that uses them.

pub mod subprocess;

pub use subprocess::{Cli, Cmd};

/// Top-level entry point.
///
/// Parses-already `Cli` and dispatches through the subprocess
/// passthrough.
///
/// # Errors
///
/// Returns any `std::io::Error` raised by the subprocess dispatcher.
pub fn run(cli: Cli) -> std::io::Result<()> {
    subprocess::run(cli)
}
