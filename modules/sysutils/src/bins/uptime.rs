//! `uptime`, a Driver under `/bin`: its declaration and its run. The
//! slots it is served through are the crate's.
//!
//! One run answers with an output cell holding one line: how long the
//! Kernel has been up, which is the span `/sys/info/uptime` counts,
//! said in days, hours, minutes and seconds.

use alloc::format;
use alloc::string::String;

use reovim_kernel_interface::{Calls, Error, Rd};

use crate::{Sysutils, fact};

reovim_sdk::declare! {
    api uptime ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args {}
}

/// What a run takes, beside the api it belongs to: a record with no
/// field, which is how a run that takes nothing is declared.
pub use uptime::Args;
pub(crate) use uptime::{decl, dispatch};

/// The fact this run is a wording of.
const UPTIME: &str = "/sys/info/uptime";

impl uptime::Api for Sysutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, _: Args) -> Result<Rd, Error> {
        self.cell(ctx, rd, spelled(fact(ctx, UPTIME)?).into_bytes())
    }
}

/// `up [Nd] Nh Nm Ns` of a span of nanoseconds: the days only once
/// there are any, and the other three whatever they stand at, so a
/// Kernel made a moment ago reads `up 0h 0m 0s` and no line is bare.
fn spelled(nanos: u64) -> String {
    let seconds = nanos / 1_000_000_000;
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    let day = if days == 0 { String::new() } else { format!("{days}d ") };
    let (hour, minute) = (rest / 3600, rest % 3600 / 60);
    format!("up {day}{hour}h {minute}m {}s\n", rest % 60)
}

#[cfg(test)]
mod tests {
    use super::spelled;

    #[test]
    fn a_span_says_its_days_only_once_there_are_any() {
        // A Kernel made a moment ago, and the second that has not come
        // yet, which is still the second before it.
        assert_eq!(spelled(0), "up 0h 0m 0s\n");
        assert_eq!(spelled(7_999_999_999), "up 0h 0m 7s\n");
        // 3h 20m 4s, and the same span four days later.
        let up = 12_004_000_000_000;
        assert_eq!(spelled(up), "up 3h 20m 4s\n");
        assert_eq!(spelled(up + 4 * 86_400_000_000_000), "up 4d 3h 20m 4s\n");
    }
}
