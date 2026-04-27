//! Library surface of the `cargo reovim-dev` subcommand.
//!
//! The bin target wires `clap` and dispatches to this crate. Pulling
//! the work into a lib target lets integration tests under `tests/`
//! exercise `stage_all` and `scan_staging` directly without shelling
//! out to the built bin.

#![forbid(missing_docs)]

pub mod launch;
pub mod stage;
