#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Standalone `reovim-web` lib — a thin wrapper that delegates to
//! [`reovim_client_ext_platform_web`] for the actual Web platform
//! runtime (Flight 76 spike).
//!
//! The bin in `src/main.rs` parses [`WebArgs`] and calls [`run`].

pub use reovim_client_ext_platform_web::{RunError, WebArgs};

/// Run the Web platform runtime with the given args.
///
/// # Errors
///
/// Returns [`RunError::Io`] if the platform runtime fails to bind or
/// serve.
pub fn run(args: WebArgs) -> Result<(), RunError> {
    reovim_client_ext_platform_web::run(args)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
