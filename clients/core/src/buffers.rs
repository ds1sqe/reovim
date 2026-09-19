//! `/self/dev/buffers`, the Client's drafts over the server's Buffers.
//!
//! A draft holds the bytes the screen is laid out on. An edit splices
//! them and joins a pending revision, which is no wire message at all,
//! and a `sync` sends what is pending as one `replace_many`, which is
//! one state on the server and one undo step for the person.
//!
//! Once the server refuses a flush because another Client's run
//! landed in front of this one, the draft is stale: what was typed
//! and what was pending both stay exactly as they were, and every
//! further `sync`, undo, redo, save and edit answers `Stale` too,
//! with no wire message, until `reload` takes the server's head in
//! their place.
//!
//! ```text
//! /self/dev/buffers/         call: open, scratch, close
//! /self/dev/buffers/0        read/write, a Container: the draft's
//!                            bytes; text, history, publish, draft
//! /self/dev/buffers/0/over   read: the server Buffer's rd
//! /self/dev/buffers/0/source read: the file's rd, where there is one
//! ```
//!
//! This is the second Driver of the `dev` Module and holds none of
//! what `Dev` holds: a draft needs `ctx` and nothing else, the server
//! Buffer being an rd the union routes. Every method that needs the
//! server's truth flushes first, here, where a caller cannot forget
//! it, and no borrow of the state outlives a submit: a flush goes down
//! the wire from inside a slot the Host called.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_client_api::{buffers, draft, part};
use reovim_kernel_interface::{
    Calls, Decl, Driver, DriverId, Error, Flags, Handle, Hash, Kind, Rd, Reply,
    Rights, spliced,
};
use reovim_sdk::{cell, encode, ops};
use reovim_text_api::{history, open as text_open, publish, text};

use crate::client::ROOT;

/// This Driver's number in the `dev` Module.
const ME: DriverId = DriverId(1);

/// Where a Buffer of the server's is made.
const TEXT: &str = "/module/text/open";

/// Edits a draft holds before one of them flushes the rest: the cap of
/// `text::Many`, so the edit that would be the 257th goes down with
/// the ones before it and a session longer than that is two states.
const EDITS: usize = 256;

/// The list and a draft alike: what it holds, and what stands below
/// it.
const NODE: Flags = Flags::READ.union(Flags::WRITE).union(Flags::CONTAINER);

/// One draft: the Buffer it is over, and what has not gone down yet.
struct Draft {
    /// The server Buffer, and the handle this Driver holds on it.
    over: Rd,
    h: Handle,
    source: Option<Rd>,
    /// The draft; what the View is laid out on.
    bytes: Vec<u8>,
    /// The server's history as of the last flush. Its head is the
    /// revision the pending edits are taken against.
    tree: history::Tree,
    pending: Vec<text::Edit>,
    /// Set once a flush the server answered `Stale` was not sent
    /// again: cleared only by `reload`, with `pending`.
    stale: bool,
}

impl Draft {
    /// The revision a reader sees: the last flushed one, or the one
    /// the flush will make while edits are pending.
    ///
    /// modules/text numbers a state by how many it holds and not by
    /// its parent, so the one the flush will make is one past the
    /// last, which after an undo is not one past head.
    const fn revision(&self) -> u64 {
        if self.pending.is_empty() {
            self.tree.head
        } else {
            self.tree.nodes.len() as u64 + 1
        }
    }

    /// The tree a reader sees, which is the server's with the node the
    /// flush will make: a prediction, the server numbering revisions
    /// from 1 in the order made, and it is what makes `head !=
    /// published` true while a person is typing.
    fn tree(&self) -> history::Tree {
        let mut tree = self.tree.clone();
        if self.pending.is_empty() {
            return tree;
        }
        tree.nodes.push(history::Node {
            id: self.revision(),
            parent: tree.head,
            label: String::from("replace"),
        });
        tree.head = self.revision();
        tree
    }
}

/// Which rd of this Driver's an rd is.
enum Node {
    /// `/self/dev/buffers` itself.
    List,
    Draft(Draft),
    /// An `over` or a `source` child: a read gives the rd it names.
    Pointer(Rd),
}

#[derive(Default)]
struct State {
    list: Option<Rd>,
    nodes: BTreeMap<Rd, Node>,
    /// Drafts made so far, which names the next: the count `:buffers`
    /// will want, and not the rd modules/text names a Buffer by.
    made: u64,
}

/// The drafts, and the list that makes them.
pub struct Buffers {
    /// The list's own api, then the four a draft offers.
    decls: [Decl; 5],
    hashes: [Hash; 5],
    state: RefCell<State>,
}

impl Buffers {
    pub(crate) fn new() -> Self {
        let decls = [
            buffers::decl(),
            text::decl(),
            history::decl(),
            publish::decl(),
            draft::decl(),
        ];
        let hashes = decls.each_ref().map(Decl::hash);
        Self { decls, hashes, state: RefCell::new(State::default()) }
    }

    /// `/self/dev/buffers` under `home`, which the Module's `start`
    /// registers it by.
    pub(crate) fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let apis = core::slice::from_ref(&self.decls[0]);
        let rd = self.add(ctx, home, "buffers", NODE, apis, Node::List)?;
        self.state.borrow_mut().list = Some(rd);
        Ok(())
    }

    /// Register `name` under `parent` and record what it is.
    fn add(
        &self,
        ctx: &dyn Calls,
        parent: Rd,
        name: &str,
        flags: Flags,
        apis: &[Decl],
        node: Node,
    ) -> Result<Rd, Error> {
        let rd = ctx.register(ME, parent, Some(name))?;
        ctx.report(rd, flags, apis)?;
        self.state.borrow_mut().nodes.insert(rd, node);
        Ok(rd)
    }

    /// The draft over the server Buffer `over`, made now if this
    /// Client holds none: the Buffer read whole and its history taken,
    /// which is what a flush counts from.
    fn drafted(
        &self,
        ctx: &dyn Calls,
        over: Rd,
        source: Option<Rd>,
    ) -> Result<Rd, Error> {
        if let Some(rd) = self.held(over) {
            return Ok(rd);
        }
        let h = ctx.open(over, Rights::READ.union(Rights::WRITE))?;
        // The handle is the draft's, and there is no draft but the one
        // below: no path out of here leaves it open.
        self.kept(ctx, over, source, h).inspect_err(|_| ops::shut(ctx, &[h]))
    }

    /// The draft the open stands on: the rd it is registered at, the
    /// children below it, and the state that names it.
    fn kept(
        &self,
        mut ctx: &dyn Calls,
        over: Rd,
        source: Option<Rd>,
        h: Handle,
    ) -> Result<Rd, Error> {
        let list = self.state.borrow().list.ok_or(Error::Invalid)?;
        let bytes = ops::whole(ctx, h)?;
        let tree = history::Call(&mut ctx, h).tree()?;
        let name = {
            let mut s = self.state.borrow_mut();
            s.made += 1;
            (s.made - 1).to_string()
        };
        let pending = Vec::new();
        let draft =
            Draft { over, h, source, bytes, tree, pending, stale: false };
        let rd = self.add(
            ctx,
            list,
            &name,
            NODE,
            &self.decls[1..],
            Node::Draft(draft),
        )?;
        self.add(ctx, rd, "over", Flags::READ, &[], Node::Pointer(over))?;
        let Some(source) = source else { return Ok(rd) };
        self.add(ctx, rd, "source", Flags::READ, &[], Node::Pointer(source))?;
        Ok(rd)
    }

    /// The draft and the children below it, out of the table: what
    /// `close` forgets before the tree does.
    fn forgotten(&self, draft: Rd, below: &[Rd]) {
        let mut s = self.state.borrow_mut();
        s.nodes.remove(&draft);
        for rd in below {
            s.nodes.remove(rd);
        }
    }

    /// The draft over `over`, where this Client holds one.
    fn held(&self, over: Rd) -> Option<Rd> {
        let same = |(rd, node): (&Rd, &Node)| match node {
            Node::Draft(draft) => (draft.over == over).then_some(*rd),
            Node::List | Node::Pointer(_) => None,
        };
        self.state.borrow().nodes.iter().find_map(same)
    }

    /// `f` over the draft `rd`: the one place this Driver's state is
    /// reached, and the borrow is gone before the answer is used, a
    /// caller of it being free to submit.
    fn on<T>(
        &self,
        rd: Rd,
        f: impl FnOnce(&mut Draft) -> T,
    ) -> Result<T, Error> {
        let mut s = self.state.borrow_mut();
        Ok(f(drafted(&mut s, rd)?))
    }

    /// The handle this Driver holds on the draft's server Buffer.
    fn handle(&self, rd: Rd) -> Result<Handle, Error> {
        self.on(rd, |draft| draft.h)
    }

    /// `edits` into the draft `rd` as part of the pending revision:
    /// stale first, then the budget, then the bytes, then one local
    /// Modify, which is what makes the screen follow.
    ///
    /// `expected` is the revision the caller saw, 0 for any, and one
    /// that is not what [`Draft::tree`] reports is `Stale`.
    fn edited(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        expected: u64,
        edits: Vec<text::Edit>,
    ) -> Result<(), Error> {
        if self.on(rd, |draft| draft.stale)? {
            return Err(Error::Stale);
        }
        let more = edits.len();
        if self.on(rd, |draft| draft.pending.len() + more > EDITS)? {
            self.flushed(ctx, rd)?;
        }
        let mut s = self.state.borrow_mut();
        let draft = drafted(&mut s, rd)?;
        if expected != 0 && expected != draft.revision() {
            return Err(Error::Stale);
        }
        // The edits are all or nothing, so they land on a copy first,
        // as modules/text lands one `replace_many`.
        let mut next = draft.bytes.clone();
        for edit in &edits {
            next = spliced(&next, edit.at, edit.len, edit.text.as_bytes())?;
        }
        draft.bytes = next;
        draft.pending.extend(edits);
        // The borrow is gone before the Modify: what a watcher does
        // with it re-enters this Host.
        drop(s);
        ctx.changed(rd, Kind::Modify)
    }

    /// The pending revision as one `replace_many` on `over`. A draft
    /// already `stale` answers so before `pending` is even read, so no
    /// message goes; nothing pending otherwise is nothing at all.
    fn flushed(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        if self.on(rd, |draft| draft.stale)? {
            return Err(Error::Stale);
        }
        // Nothing of the borrow outlives the call: it goes down the
        // wire, and what it answers is put back in a borrow of its own.
        let Some((h, expected, edits)) = self.pending(rd)? else {
            return Ok(());
        };
        match sent(ctx, h, expected, &edits) {
            Ok(()) => self.landed(rd),
            // The head moved under the pending edits: they are not
            // sent again. The draft keeps them, pending, and is
            // `stale` from here on, answering every further flush the
            // same way with no wire message, until `reload` takes the
            // server's head in their place.
            Err(Error::Stale) => {
                self.on(rd, |draft| draft.stale = true)?;
                Err(Error::Stale)
            }
            Err(other) => Err(other),
        }
    }

    /// What a flush sends, and nothing where nothing is pending: the
    /// handle, the revision the edits are taken against, and the edits.
    fn pending(
        &self,
        rd: Rd,
    ) -> Result<Option<(Handle, u64, Vec<text::Edit>)>, Error> {
        self.on(rd, |draft| {
            let sends = (draft.h, draft.tree.head, draft.pending.clone());
            (!draft.pending.is_empty()).then_some(sends)
        })
    }

    /// The revision the server just made, which is the one the draft
    /// has been reporting: the prediction, kept.
    fn landed(&self, rd: Rd) -> Result<(), Error> {
        self.on(rd, |draft| {
            draft.tree = draft.tree();
            draft.pending.clear();
        })
    }

    /// The head is what a save just stored, as the server marks it:
    /// what makes `head != published` false until the next edit.
    fn published(&self, rd: Rd) -> Result<(), Error> {
        self.on(rd, |draft| draft.tree.published = draft.tree.head)
    }

    /// The server's truth, whole: the Buffer read and its history
    /// taken, and nothing of the draft's own left over, `stale`
    /// included. The bytes moved under whoever reads them, so the
    /// Modify says so; the borrow is gone before it, what a watcher
    /// does with it re-entering this Host.
    fn reread(
        &self,
        mut ctx: &dyn Calls,
        rd: Rd,
        h: Handle,
    ) -> Result<(), Error> {
        let bytes = ops::whole(ctx, h)?;
        let tree = history::Call(&mut ctx, h).tree()?;
        self.on(rd, |draft| {
            draft.bytes = bytes;
            draft.tree = tree;
            draft.pending.clear();
            draft.stale = false;
        })?;
        ctx.changed(rd, Kind::Modify)
    }

    /// Head one revision back or on: what is pending goes down first,
    /// the move is the server's, and the draft takes the head it left.
    /// The end of the history is the server's `Invalid`, as a caller
    /// of a Buffer already reads it.
    fn walked(
        &self,
        mut ctx: &dyn Calls,
        rd: Rd,
        back: bool,
    ) -> Result<(), Error> {
        self.flushed(ctx, rd)?;
        let h = self.handle(rd)?;
        let mut call = history::Call(&mut ctx, h);
        if back { call.undo() } else { call.redo() }?;
        self.reread(ctx, rd, h)
    }

    /// What a `save_as` may have made of the draft: one over nothing
    /// whose Buffer gained a `source` child goes on as that rd's,
    /// published there and naming it under a `source` child of its
    /// own, which is what keeps the name of what a window shows right.
    /// One that had a source has written a copy and is no more saved
    /// than it was, as the server has it.
    fn adopted(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        target: Rd,
    ) -> Result<(), Error> {
        let over =
            self.on(rd, |draft| draft.source.is_none().then_some(draft.over))?;
        let Some(over) = over else { return Ok(()) };
        if ops::lookup(ctx, over, "source").is_err() {
            return Ok(());
        }
        self.on(rd, |draft| draft.source = Some(target))?;
        self.published(rd)?;
        let pointer = Node::Pointer(target);
        self.add(ctx, rd, "source", Flags::READ, &[], pointer).map(|_| ())
    }

    /// The draft's bytes, or the rd an `over` or a `source` names.
    fn reading(&self, rd: Rd, at: u64, len: usize) -> Result<Vec<u8>, Error> {
        let s = self.state.borrow();
        match s.nodes.get(&rd) {
            Some(Node::Draft(draft)) => Ok(part(&draft.bytes, at, len)),
            Some(Node::Pointer(named)) => Ok(part(&encode(named), at, len)),
            _ => Err(Error::Unsupported),
        }
    }

    /// The draft from `at` becomes `buf` and ends there, as one more
    /// edit of the pending revision.
    fn writing(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        at: u64,
        buf: &[u8],
    ) -> Result<usize, Error> {
        let text = core::str::from_utf8(buf).map_err(|_| Error::Invalid)?;
        let end = self.on(rd, |draft| draft.bytes.len() as u64)?;
        let len = end - at.min(end);
        let edit = text::Edit { at, len, text: text.to_string() };
        self.edited(ctx, rd, 0, alloc::vec![edit])?;
        Ok(buf.len())
    }

    /// The api `api` on `rd`: the list's own, or one of the four a
    /// draft offers.
    fn called(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        api: Hash,
        number: u16,
        input: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let [list, text, history, publish, draft] = self.hashes;
        match api {
            a if a == list => buffers::dispatch(self, ctx, rd, number, input),
            a if a == text => text::dispatch(self, ctx, rd, number, input),
            a if a == history => {
                history::dispatch(self, ctx, rd, number, input)
            }
            a if a == publish => {
                publish::dispatch(self, ctx, rd, number, input)
            }
            a if a == draft => draft::dispatch(self, ctx, rd, number, input),
            _ => Err(Error::Unsupported),
        }
    }
}

impl buffers::Api for Buffers {
    fn open(&self, ctx: &dyn Calls, _: Rd, source: Rd) -> Result<Rd, Error> {
        let over = buffer(ctx, Some(source))?;
        self.drafted(ctx, over, Some(source))
    }

    fn scratch(&self, ctx: &dyn Calls, _: Rd) -> Result<Rd, Error> {
        let over = buffer(ctx, None)?;
        self.drafted(ctx, over, None)
    }

    /// The end of the draft and not of what was typed into it: what is
    /// pending goes down as one revision first, and then the rd, the
    /// children below it and the handle all go. The Buffer stays,
    /// modules/text having no way to free one. A `stale` draft closes
    /// the same way and answers `Stale`, as it answers any refused
    /// flush: what was typed did not go down.
    ///
    /// Every step runs whatever the one before answered, so a draft
    /// that is closing is closed whatever fails; the first error is
    /// what the caller is told.
    fn close(&self, ctx: &dyn Calls, _: Rd, draft: Rd) -> Result<(), Error> {
        let h = self.handle(draft)?;
        let sent = self.flushed(ctx, draft);
        let below = ops::children(ctx, draft);
        self.forgotten(draft, below.as_deref().unwrap_or(&[]));
        let gone = ctx.unregister(draft);
        let closed = ops::close(ctx, h);
        sent.and(below).and(gone).and(closed)
    }
}

impl text::Api for Buffers {
    fn replace(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        edit: text::Edit,
    ) -> Result<(), Error> {
        self.edited(ctx, rd, 0, alloc::vec![edit])
    }

    fn replace_many(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        many: text::Many,
    ) -> Result<(), Error> {
        self.edited(ctx, rd, many.expected, many.edits)
    }
}

impl history::Api for Buffers {
    fn tree(&self, _: &dyn Calls, rd: Rd) -> Result<history::Tree, Error> {
        self.on(rd, |draft| draft.tree())
    }

    /// `Unsupported`: what walks a history by revision is an undo tree
    /// over the server Buffer, which a caller reaches through `over`.
    fn goto(&self, _: &dyn Calls, _: Rd, _: u64) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn undo(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        self.walked(ctx, rd, true)
    }

    fn redo(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        self.walked(ctx, rd, false)
    }
}

impl publish::Api for Buffers {
    fn save(&self, mut ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        self.flushed(ctx, rd)?;
        let h = self.handle(rd)?;
        publish::Call(&mut ctx, h).save()?;
        self.published(rd)
    }

    fn save_as(
        &self,
        mut ctx: &dyn Calls,
        rd: Rd,
        target: Rd,
    ) -> Result<(), Error> {
        self.flushed(ctx, rd)?;
        let h = self.handle(rd)?;
        publish::Call(&mut ctx, h).save_as(target)?;
        self.adopted(ctx, rd, target)
    }
}

impl draft::Api for Buffers {
    fn sync(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        self.flushed(ctx, rd)
    }

    /// The other way out of `Stale`: the server's head, whole,
    /// whatever was pending dropped and `stale` cleared with it.
    fn reload(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        let h = self.handle(rd)?;
        self.reread(ctx, rd, h)
    }
}

impl Driver for Buffers {
    /// The rd itself, as modules/text's Buffer does: nothing a draft
    /// holds is one handle's.
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        _: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        self.reading(Rd(h), at, len).into()
    }

    fn write(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        buf: &[u8],
    ) -> Reply<usize> {
        self.writing(ctx, Rd(h), at, buf).into()
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        api: Hash,
        method: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        self.called(ctx, Rd(h), api, method, input).into()
    }
}

/// The draft `rd` is, or `Unsupported` for any other rd of this
/// Driver's.
fn drafted(s: &mut State, rd: Rd) -> Result<&mut Draft, Error> {
    match s.nodes.get_mut(&rd) {
        Some(Node::Draft(draft)) => Ok(draft),
        _ => Err(Error::Unsupported),
    }
}

/// A Buffer of the server's over `source`, or over nothing: the rd a
/// draft stands on.
fn buffer(mut ctx: &dyn Calls, source: Option<Rd>) -> Result<Rd, Error> {
    let opener = cell::resolve(ctx, ROOT, TEXT)?;
    let h = ctx.open(opener, Rights::WRITE)?;
    let mut call = text_open::Call(&mut ctx, h);
    let buffer = match source {
        Some(source) => call.open(source),
        None => call.new(),
    };
    ops::close(ctx, h)?;
    buffer
}

/// `edits` as one state on the server, taken against `expected`: one
/// wire message, whatever a session typed.
fn sent(
    mut ctx: &dyn Calls,
    h: Handle,
    expected: u64,
    edits: &[text::Edit],
) -> Result<(), Error> {
    let many = text::Many { expected, edits: edits.to_vec() };
    text::Call(&mut ctx, h).replace_many(many)
}
