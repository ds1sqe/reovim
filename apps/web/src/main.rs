#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Standalone `reovim-web` binary entry point.

use clap::Parser;

use reovim_app_web::{RunError, WebArgs, run};

#[derive(Parser, Debug)]
#[command(name = "reovim-web")]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(flatten)]
    web: WebArgs,
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> Result<(), RunError> {
    let cli = Cli::parse();
    run(cli.web)
}
