//! `date`, a Driver under `/bin`: its declaration and its run. The
//! slots it is served through are the crate's.
//!
//! One run answers with an output cell holding one line: the moment
//! `/sys/info/time` counts, as a calendar reads it in UTC. There is no
//! timezone, the host offering none, and no clock of the reader's own.

use alloc::format;

use reovim_kernel_interface::{Calls, Error, Rd};
use reovim_sdk::date::civil;

use crate::{Sysutils, fact};

reovim_sdk::declare! {
    api date ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args {}
}

/// What a run takes, beside the api it belongs to: a record with no
/// field, which is how a run that takes nothing is declared.
pub use date::Args;
pub(crate) use date::{decl, dispatch};

/// The fact this run is a wording of.
const TIME: &str = "/sys/info/time";

impl date::Api for Sysutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, _: Args) -> Result<Rd, Error> {
        let at = civil(fact(ctx, TIME)?);
        let (year, month, day) = (at.year, at.month, at.day);
        let line = format!(
            "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}\n",
            at.hour, at.minute, at.second
        );
        self.cell(ctx, rd, line.into_bytes())
    }
}
