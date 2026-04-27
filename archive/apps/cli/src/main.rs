#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Standalone `reovim-cli` binary entry point.
//!
//! Parses CLI arguments and delegates to [`reovim_app_cli::run`].

use clap::Parser;

use reovim_app_cli::{CliArgs, run};

#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> std::io::Result<()> {
    let args = CliArgs::parse();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run(args))
}
