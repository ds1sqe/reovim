//! `watch`, a Driver under `/bin`: its declaration and its run. The
//! slots it is served through are the crate's.
//!
//! One run answers with an output cell that holds nothing and goes on:
//! it watches its target, and a read at the cell's end answers `seq
//! kind name` for everything that happened to the target since the read
//! before, the name being the child's where an event names one, else
//! the target's own. Nothing the Kernel's ring held before the run is
//! this run's to say.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Error, Event, Handle, Kind, Op, Rd};
use reovim_sdk::cell::{self, Follow};
use reovim_sdk::ops;

use crate::{Sysutils, sgr};

reovim_sdk::declare! {
    api watch ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { cwd: Rd, color: bool, target: utf8(256) }
}

/// What a run takes, beside the api it belongs to.
pub use watch::Args;
pub(crate) use watch::{decl, dispatch};

/// A Create.
const GREEN: &str = "32";
/// A Delete or a Gone.
const RED: &str = "31";
/// An Attrib or a Move.
const YELLOW: &str = "33";
/// An Overflow, which says the ring outran the reader.
const BOLD: &str = "1";

impl watch::Api for Sysutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let target = cell::resolve(ctx, args.cwd, &args.target)?;
        // Named before the watch, so a run that fails leaves nothing
        // open; a name that changes after is a Move the run says.
        let info = ops::info(ctx, target)?;
        let name = info.name.unwrap_or_else(|| format!("rd:{}", target.0));
        let h = ops::watch(ctx, target)?;
        let cursor = ops::since(ctx, 0)?.1;
        let happening =
            Happening { h, target, name, color: args.color, cursor };
        self.following(ctx, rd, Vec::new(), Box::new(happening))
    }
}

/// Between reads: the handle that holds the watch, what is watched and
/// what it is called, whether the lines are painted, and this
/// follower's own place in the Kernel's event ring.
struct Happening {
    h: Handle,
    target: Rd,
    name: String,
    color: bool,
    cursor: u64,
}

impl Follow for Happening {
    fn more(&mut self, ctx: &dyn Calls) -> Result<Option<Vec<u8>>, Error> {
        let (events, cursor) = ops::since(ctx, self.cursor)?;
        self.cursor = cursor;
        let mut text = String::new();
        for event in events.iter().filter(|event| event.rd == self.target) {
            text.push_str(&self.line(ctx, *event));
        }
        Ok(Some(text.into_bytes()))
    }

    fn end(&mut self, ctx: &dyn Calls) {
        let _ = ctx.submit(Op::Close { h: self.h });
    }
}

impl Happening {
    /// One event: `seq kind name`, the name of the child where the
    /// event names one, else of the target itself.
    fn line(&self, ctx: &dyn Calls, event: Event) -> String {
        let named = |child: Rd| {
            let name = ops::info(ctx, child).ok().and_then(|info| info.name);
            name.unwrap_or_else(|| format!("rd:{}", child.0))
        };
        let name = event.child.map_or_else(|| self.name.clone(), named);
        format!("{} {} {name}\n", event.seq, self.kind(event.kind))
    }

    /// The kind of an event, colored as what it did: a Create green, a
    /// removal red, an Attrib or a Move yellow, a Modify as it is.
    fn kind(&self, kind: Kind) -> String {
        let color = self.color;
        match kind {
            Kind::Modify => "Modify".to_string(),
            Kind::Attrib => sgr(color, YELLOW, "Attrib"),
            Kind::Create => sgr(color, GREEN, "Create"),
            Kind::Delete => sgr(color, RED, "Delete"),
            Kind::Move => sgr(color, YELLOW, "Move"),
            Kind::Gone => sgr(color, RED, "Gone"),
            Kind::Overflow => sgr(color, BOLD, "Overflow"),
        }
    }
}
