#![no_std]
#![forbid(unsafe_code)]
//! The search Driver: a view over any rd it can read.
//!
//! `open` starts a view that fills with what a pattern matches, whole
//! lines at a time, from the start of the line holding `from` and
//! around once back to it. It speaks to the Kernel only: the source
//! is opened READ once at `open` and read a window at a time, never
//! watched; nothing of it is indexed, since it may change under any
//! read.

extern crate alloc;

mod fill;

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Handle, Hash, Kind,
    Module, Rd, Reply, Rights, part,
};
use reovim_pattern::Pattern;
use reovim_sdk::progress::{self, Progress};
use reovim_sdk::{Encode as _, ops};
pub use reovim_search_api::search;

use fill::Fill;

/// The api rd.
const OPEN: Flags = Flags::READ.union(Flags::WRITE);
/// A view: no bytes of its own, a Container over `matches` and
/// `progress`.
const VIEW: Flags = Flags::READ.union(Flags::CONTAINER);

enum Node {
    Open,
    /// A view: its two children.
    View {
        matches: Rd,
        progress: Rd,
    },
    /// `<n>/matches`: the entries found so far, encoded; the fill
    /// that grows them, taken out while a slice runs and put back
    /// unless it is now whole; the source handle, kept here too so
    /// [`Search::close_view`] has it even once the fill is gone; the
    /// view's `progress` rd; an error a slice hit, answered again on
    /// every later read past what is held.
    Matches {
        fill: Option<Fill>,
        handle: Handle,
        entries: Vec<u8>,
        progress: Rd,
        failed: Option<Error>,
    },
    /// `<n>/progress`: the two numbers `progress` declares.
    Progress(Progress),
}

#[derive(Default)]
struct State {
    home: Option<Rd>,
    nodes: BTreeMap<Rd, Node>,
    /// Views made so far, for their names: a count from 0 that never
    /// goes back.
    views: u64,
}

/// The Driver and its Module in one; pass it twice at `load`.
pub struct Search {
    decl: Decl,
    hash: Hash,
    state: RefCell<State>,
    /// The work one slice may spend: the same budget for the first
    /// slice `open` runs and for every one a read of `matches` cranks.
    budget: usize,
}

impl Default for Search {
    /// `2^14`: the largest of five measured budgets whose slowest
    /// slice stays under 8 ms for all four patterns the minor names,
    /// on a release build over a generated 64 MiB; the table is in
    /// the commit's report, not kept here.
    fn default() -> Self {
        Self::with_budget(1 << 14)
    }
}

impl Search {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A Driver whose slices spend at most `budget`.
    #[must_use]
    pub fn with_budget(budget: usize) -> Self {
        let decl = search::decl();
        let hash = decl.hash();
        Self { decl, hash, state: RefCell::new(State::default()), budget }
    }

    /// `open`: `view` already registered; its two children, its first
    /// slice, and its three nodes. Any failure here is the caller's
    /// to undo, by unregistering `view`.
    fn filled(
        &self,
        ctx: &dyn Calls,
        view: Rd,
        mut fill: Fill,
    ) -> Result<(), Error> {
        ctx.report(view, VIEW, &[])?;
        let matches = ctx.register(DriverId(0), view, Some("matches"))?;
        ctx.report(matches, Flags::READ, &[])?;
        let progress = ctx.register(DriverId(0), view, Some("progress"))?;
        ctx.report(progress, Flags::READ, &[progress::decl()])?;

        let handle = fill.handle();
        let (found, outcome) = fill::slice(&mut fill, ctx, self.budget);
        outcome?;
        let (done, total) = fill.progress();
        let whole = fill.whole();
        let mut state = self.state.borrow_mut();
        let node = Node::Matches {
            fill: (!whole).then_some(fill),
            handle,
            entries: encoded(&found),
            progress,
            failed: None,
        };
        state.nodes.insert(matches, node);
        state.nodes.insert(progress, Node::Progress(Progress { done, total }));
        state.nodes.insert(view, Node::View { matches, progress });
        drop(state);
        // Theoretical only past this point: the nodes stand either
        // way, so a failure here would double the close this view's
        // own close already does and orphan it besides.
        let _ = ctx.changed(progress, Kind::Modify);
        Ok(())
    }

    /// `open`: the source opened by the caller, the pattern already
    /// parsed. Nothing is left registered, and the source handle is
    /// closed, unless a view comes of it.
    fn opened(
        &self,
        ctx: &dyn Calls,
        source: Rd,
        handle: Handle,
        from: u64,
        pattern: Pattern,
    ) -> Result<Rd, Error> {
        let total = fill::measured(ctx, handle)?;
        let start = fill::line_start(ctx, handle, from, total)?;
        let home = self.state.borrow().home.ok_or(Error::Invalid)?;
        let name = {
            let mut state = self.state.borrow_mut();
            let n = state.views;
            state.views += 1;
            n.to_string()
        };
        let view = ctx.register(DriverId(0), home, Some(&name))?;
        let fill = Fill::new(pattern, source, handle, start, total);
        match self.filled(ctx, view, fill) {
            Ok(()) => Ok(view),
            Err(error) => {
                let _ = ctx.unregister(view);
                Err(error)
            }
        }
    }

    /// The fill to crank at `at`, taken out of `rd`'s node: `Err` when
    /// a past slice failed, answered again; `Ok(None)` when `at` is
    /// already held, the fill is already whole, or `rd` is not a
    /// `matches` rd — either way the caller reads what is held.
    fn past_end(&self, rd: Rd, at: usize) -> Result<Option<Fill>, Error> {
        let mut state = self.state.borrow_mut();
        let Some(Node::Matches { fill, entries, failed, .. }) =
            state.nodes.get_mut(&rd)
        else {
            return Ok(None);
        };
        if at < entries.len() {
            return Ok(None);
        }
        if let Some(error) = *failed {
            return Err(error);
        }
        Ok(fill.take())
    }

    /// `fill` and `found` (empty on `error`) back into `rd`'s node:
    /// `entries` grown, `failed` set, `progress` set from
    /// `fill.progress()`, `fill` kept unless it is now whole. The
    /// `progress` rd, for the caller's own `changed`.
    fn went_on(
        &self,
        rd: Rd,
        fill: Fill,
        found: &[search::Match],
        error: Option<Error>,
    ) -> Rd {
        let (done, total) = fill.progress();
        let whole = fill.whole();
        let mut state = self.state.borrow_mut();
        let Some(Node::Matches {
            fill: slot, entries, progress, failed, ..
        }) = state.nodes.get_mut(&rd)
        else {
            return rd;
        };
        entries.extend(encoded(found));
        *failed = error;
        *slot = (!whole).then_some(fill);
        let progress_rd = *progress;
        if let Some(Node::Progress(record)) = state.nodes.get_mut(&progress_rd)
        {
            *record = Progress { done, total };
        }
        progress_rd
    }

    /// What `rd` holds at `at`, whichever node it is; `Unsupported`
    /// for anything else.
    fn held(&self, rd: Rd, at: u64, len: usize) -> Result<Vec<u8>, Error> {
        let state = self.state.borrow();
        match state.nodes.get(&rd) {
            Some(Node::Matches { entries, .. }) => Ok(part(entries, at, len)),
            Some(Node::Progress(record)) => {
                Ok(part(&reovim_sdk::encode(record), at, len))
            }
            _ => Err(Error::Unsupported),
        }
    }

    /// What is already held, in what it holds; past it, `failed`
    /// answered again, one slice's new entries, `Busy` at nothing new
    /// yet, or the empty read once whole.
    fn read(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        at: u64,
        len: usize,
    ) -> Result<Vec<u8>, Error> {
        let held = usize::try_from(at).unwrap_or(usize::MAX);
        let Some(mut fill) = self.past_end(rd, held)? else {
            return self.held(rd, at, len);
        };
        let (found, outcome) = fill::slice(&mut fill, ctx, self.budget);
        let whole = fill.whole();
        let error = outcome.err();
        let progress_rd = self.went_on(rd, fill, &found, error);
        ctx.changed(progress_rd, Kind::Modify)?;
        if let Some(error) = error {
            return Err(error);
        }
        if !found.is_empty() {
            return self.held(rd, at, len);
        }
        if whole { Ok(Vec::new()) } else { Err(Error::Busy) }
    }

    /// The view `rd` ends: the source handle closed, its two children
    /// and itself unregistered. Closing a handle on `matches` or
    /// `progress` instead, or on `open`, does nothing.
    fn close_view(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        let mut state = self.state.borrow_mut();
        let Some(Node::View { matches, progress }) = state.nodes.get(&rd)
        else {
            return Ok(());
        };
        let (matches, progress) = (*matches, *progress);
        state.nodes.remove(&rd);
        let handle = match state.nodes.remove(&matches) {
            Some(Node::Matches { handle, .. }) => Some(handle),
            _ => None,
        };
        state.nodes.remove(&progress);
        drop(state);
        if let Some(handle) = handle {
            let _ = ops::close(ctx, handle);
        }
        ctx.unregister(rd)
    }
}

/// The bytes of every entry in `matches`, in order, with no framing
/// between them: [`search::ENTRY`] each.
fn encoded(matches: &[search::Match]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for one in matches {
        one.encode(&mut bytes);
    }
    bytes
}

impl search::Api for Search {
    fn open(
        &self,
        ctx: &dyn Calls,
        _: Rd,
        args: search::Open,
    ) -> Result<Rd, Error> {
        let pattern =
            Pattern::parse(&args.pattern, args.ignorecase, args.smartcase)
                .ok_or(Error::Invalid)?;
        let handle = ctx.open(args.source, Rights::READ)?;
        match self.opened(ctx, args.source, handle, args.from, pattern) {
            Ok(view) => Ok(view),
            Err(error) => {
                let _ = ops::close(ctx, handle);
                Err(error)
            }
        }
    }
}

impl Module for Search {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        self.state.borrow_mut().home = Some(home);
        let open = ctx.register(DriverId(0), home, Some("open"))?;
        ctx.report(open, OPEN, core::slice::from_ref(&self.decl))?;
        self.state.borrow_mut().nodes.insert(open, Node::Open);
        Ok(())
    }
}

impl Driver for Search {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        self.read(ctx, Rd(h), at, len).into()
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        api: Hash,
        method: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        if api != self.hash {
            return Reply::err(Error::Unsupported);
        }
        search::dispatch(self, ctx, Rd(h), method, input).into()
    }

    fn close(&self, ctx: &dyn Calls, h: u64) -> Reply<()> {
        self.close_view(ctx, Rd(h)).into()
    }
}

/// The search Module, ready for `load`; `root` is unused.
#[must_use]
pub fn boot(_root: &str) -> Booted {
    let search = Rc::new(Search::new());
    Booted { module: search.clone(), drivers: alloc::vec![search] }
}
