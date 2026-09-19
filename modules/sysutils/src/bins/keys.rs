//! `keys`, a Driver under `/bin`: its declaration and its run. The
//! slots it is served through are the crate's.
//!
//! One run parses the notation and writes every code it names into a
//! keyboard cell in one op, so a line typed into another Client costs
//! that Client one tick. Its output cell holds nothing, a run that says
//! nothing answering nothing.

use alloc::vec::Vec;

use reovim_client_api::notation;
use reovim_kernel_interface::{Calls, Error, Rd};
use reovim_sdk::{cell, ops};

use crate::Sysutils;

reovim_sdk::declare! {
    api keys ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { cwd: Rd, target: utf8(256), notation: utf8(1024) }
}

/// What a run takes, beside the api it belongs to.
pub use keys::Args;
pub(crate) use keys::{decl, dispatch};

impl keys::Api for Sysutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let target = cell::resolve(ctx, args.cwd, &args.target)?;
        let named = notation::keys(&args.notation).ok_or(Error::Invalid)?;
        let mut codes = Vec::with_capacity(named.len() * 8);
        for key in named {
            codes.extend(key.code().to_le_bytes());
        }
        ops::put(ctx, target, codes)?;
        self.cell(ctx, rd, Vec::new())
    }
}
