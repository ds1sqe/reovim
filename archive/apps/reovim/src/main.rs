#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! `reovim` launcher bin entry point.
//!
//! Parses the top-level CLI and delegates to
//! [`reovim_app_launcher::run`].

use clap::Parser;

use reovim_app_launcher::{Cli, run};

#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    run(cli)
}
