#![no_std]
#![forbid(unsafe_code)]
//! Markdown over a Buffer: what a `.md` draft's constructs look like,
//! served to whoever draws it; a Module a Client loads.
//!
//! It holds no region and shows nothing of its own. Its home,
//! `/self/markdown`, is a Container that answers for the name of a
//! draft under `/self/dev/buffers` whose `source` is a Markdown file:
//! the child is made when it is first asked for, with a `looks` Driver
//! under it, and any other name is `Invalid`. Nothing stands there
//! ahead of the ask, so the drawer's own walk is what makes a child
//! come, and a draft that goes takes its child with it at the next
//! ask.
//!
//! `runs` reads the draft whole and answers the looks of the lines the
//! range meets: the construct's own meaning over its text and `MARKER`
//! over the markers, which the drawer conceals. It names meanings and
//! never colours, and it edits nothing: `:w` writes the bytes it read.

extern crate alloc;

mod parse;

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_client_api::looks;
use reovim_kernel_interface::{
    Booted, Calls, Driver, DriverId, Error, Flags, Hash, Module, Rd, Reply,
    Rights,
};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, decode, ops};

/// The one Driver of this Module, which serves every node it has.
const ME: DriverId = DriverId(0);

/// Where the drafts a Client holds stand.
const DRAFTS: &str = "/self/dev/buffers";

/// What a draft's source is named for this Plugin to decorate it.
const SUFFIXES: [&str; 2] = [".md", ".markdown"];

/// The Plugin, ready for a Client to load.
#[must_use]
pub fn boot() -> Booted {
    let markdown = Rc::new(Loaded::default());
    Booted { module: markdown.clone(), drivers: alloc::vec![markdown] }
}

/// One draft this Plugin decorates.
struct Served {
    /// `/self/markdown/<n>`, which goes when the draft does.
    node: Rd,
    /// `/self/markdown/<n>/looks`, whose `runs` the drawer reads.
    looks: Rd,
    /// `/self/dev/buffers/<n>`, whose bytes the parse reads.
    draft: Rd,
}

/// What the Plugin keeps: the drafts it has been asked for, and
/// nothing else. No borrow of it outlives an op.
#[derive(Default)]
struct Loaded {
    served: RefCell<Vec<Served>>,
}

impl Module for Loaded {
    /// Nothing: the home stands empty until somebody asks it for a
    /// draft, and the tree says which drafts there are.
    fn start(&self, _: &dyn Calls, _: Rd) -> Result<(), Error> {
        Ok(())
    }
}

impl Driver for Loaded {
    /// The home, asked for a name: the drafts that went are let go
    /// first, then the one asked for is made where it is ours.
    fn lookup(&self, ctx: &dyn Calls, rd: Rd, name: &str) -> Reply<Rd> {
        self.reap(ctx);
        self.came(ctx, rd, name).into()
    }

    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        api: Hash,
        method: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        if api != looks::hash() {
            return Reply::err(Error::Unsupported);
        }
        looks::dispatch(self, ctx, Rd(h), method, input).into()
    }
}

impl looks::Api for Loaded {
    /// The looks of the lines `args` meets. The draft is read whole: a
    /// draft is small, and a read of the range widened to whole lines
    /// is what a case that shows the whole too slow would ask for.
    fn runs(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: looks::Range,
    ) -> Result<Vec<looks::Look>, Error> {
        let bytes = ops::bytes(ctx, self.draft(rd)?)?;
        let text = core::str::from_utf8(&bytes).map_err(|_| Error::Invalid)?;
        Ok(parse::looks(text, &args))
    }
}

impl Loaded {
    /// The child `name` stands for, registered now with its `looks`
    /// Driver under it: `Invalid` for a name that is no draft of this
    /// Client's, and for a draft whose source is no Markdown file.
    fn came(&self, ctx: &dyn Calls, home: Rd, name: &str) -> Result<Rd, Error> {
        let draft = ops::lookup(ctx, cell::resolve(ctx, ROOT, DRAFTS)?, name)?;
        if !ours(ctx, draft) {
            return Err(Error::Invalid);
        }
        let node = ctx.register(ME, home, Some(name))?;
        ctx.report(node, Flags::READ.union(Flags::CONTAINER), &[])?;
        let looks = ctx.register(ME, node, Some("looks"))?;
        ctx.report(looks, Flags::READ, &[looks::decl()])?;
        self.served.borrow_mut().push(Served { node, looks, draft });
        Ok(node)
    }

    /// The drafts that are no longer there, let go with their children:
    /// what the tree has retired this Plugin holds no node for.
    fn reap(&self, ctx: &dyn Calls) {
        let gone: Vec<Rd> = self
            .served
            .borrow()
            .iter()
            .filter(|it| ops::info(ctx, it.draft).is_err())
            .map(|it| it.node)
            .collect();
        self.served.borrow_mut().retain(|it| !gone.contains(&it.node));
        for node in gone {
            let _ = ctx.unregister(node);
        }
    }

    /// The draft the `looks` node `rd` stands over.
    fn draft(&self, rd: Rd) -> Result<Rd, Error> {
        let served = self.served.borrow();
        let found = served.iter().find(|it| it.looks == rd);
        found.map(|it| it.draft).ok_or(Error::Invalid)
    }
}

/// Whether the draft `draft` is one of this Plugin's: its `source` is
/// a file whose name ends as Markdown's does. A draft over nothing has
/// no source and is nobody's to decorate.
fn ours(ctx: &dyn Calls, draft: Rd) -> bool {
    let Ok(child) = ops::lookup(ctx, draft, "source") else { return false };
    let Ok(held) = ops::bytes(ctx, child) else { return false };
    let Ok(source) = decode::<Rd>(&held) else { return false };
    let Ok(info) = ops::info(ctx, source) else { return false };
    let name = info.name.unwrap_or_default();
    SUFFIXES.iter().any(|end| name.ends_with(end))
}
