#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Standalone `reovim-tui` binary entry point.
//!
//! Parses TUI CLI arguments and delegates to [`reovim_app_tui::run`].

use clap::Parser;

use reovim_app_tui::{TuiArgs, run};

/// Top-level CLI for the standalone `reovim-tui` binary.
///
/// Wraps [`TuiArgs`] as the sole argument group; clap derives the
/// parser from the platform-owned struct so an in-process launcher
/// and the standalone bin share one schema.
#[derive(Parser, Debug)]
#[command(name = "reovim-tui")]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(flatten)]
    tui: TuiArgs,
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run(cli.tui))
}
