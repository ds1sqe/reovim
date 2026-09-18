//! `log`, a Driver under `/bin`: its declaration and its run. The
//! slots it is served through are the crate's.
//!
//! One run answers with an output cell holding one line per entry of
//! `/sys/log` from `from` on, `seq time mark level text`, as the Kernel
//! wrote it. `from` 0 is from the first entry the ring still holds.

use alloc::string::String;

use reovim_kernel_interface::{Calls, Error, Rd};

use crate::Sysutils;
use crate::entries;

reovim_sdk::declare! {
    api log ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { from: u64 }
}

/// What a run takes, beside the api it belongs to.
pub use log::Args;
pub(crate) use log::{decl, dispatch};

impl log::Api for Sysutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let mut text = String::new();
        for entry in entries::from(ctx, args.from)? {
            text.push_str(&entry.line());
        }
        self.cell(ctx, rd, text.into_bytes())
    }
}
