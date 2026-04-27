#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Standalone `reovim-server` binary entry point.
//!
//! Parses server CLI arguments and delegates to [`reovim_app_server::run`].

use clap::Parser;

use reovim_app_server::{ServerArgs, run};

#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> std::io::Result<()> {
    let args = ServerArgs::parse();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run(args))
}
