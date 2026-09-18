//! `cat`, a Driver under `/bin`: its declaration and its
//! run. The slots it is served through are the crate's.
//!
//! One run answers with an output cell holding the bytes of each
//! target in turn. A target that holds no bytes, a Container among
//! them, fails the run and leaves no cell.

use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Error, Rd};
use reovim_sdk::{cell, ops};

use crate::Coreutils;

reovim_sdk::declare! {
    api cat ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { cwd: Rd, targets: [utf8(256); 64] }
}

/// What a run takes, beside the api it belongs to.
pub use cat::Args;
pub(crate) use cat::{decl, dispatch};

impl cat::Api for Coreutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let mut bytes = Vec::new();
        for target in &args.targets {
            let target = cell::resolve(ctx, args.cwd, target)?;
            bytes.extend(ops::bytes(ctx, target)?);
        }
        self.cell(ctx, rd, bytes)
    }
}
