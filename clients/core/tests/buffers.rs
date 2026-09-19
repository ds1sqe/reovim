//! `/self/dev/buffers` on a Kernel with fs and text: what a draft
//! holds, what an edit of it costs, and what one flush leaves on the
//! server.
//!
//! Nothing in tree opens a draft yet, so each case is the caller a
//! Plugin will be: it holds a handle on what it opened and runs the
//! declared apis on it, the `text` one being the same a server Buffer
//! offers, and reads the Buffer behind the draft through a Client of
//! its own.

use std::cell::Cell;
use std::rc::Rc;

use reovim_client::dev::{self, Table};
use reovim_client::{
    Carrier, Client, Clip, Error, Fault, Handle, Host, HostCalls, Rd, Rights,
};
use reovim_client_api::{buffers, draft};
use reovim_host::clock;
use reovim_kernel_interface::wire::{self, Message};
use reovim_kernel_interface::{Calls, Hash, Op, Reply};
use reovim_kernel_runtime::{Connection, Heap, Kernel, Limits, NoLoader};
use reovim_rig::Rig;
use reovim_sdk::{cell, decode, encode, ops};
use reovim_text_api::{history, publish, text};

/// A draft as its caller holds it: the rd it reads and the handle its
/// calls run on.
#[derive(Clone, Copy)]
struct Draft {
    rd: Rd,
    h: Handle,
}

/// The api and method the link refuses, where it refuses one.
type Refusal = Rc<Cell<Option<(Hash, u16)>>>;

/// One Kernel with text alone, and a link that may refuse one call:
/// what a draft does when the server answers no is what the cases
/// below judge, and a scratch Buffer is all they stand on.
struct Server {
    kernel: Rc<Kernel>,
    refuse: Refusal,
}

impl Server {
    fn new() -> Self {
        let kernel = Kernel::new(clock::now, Heap::default, Limits::default());
        let text = reovim_module_text::boot("");
        kernel.load("text", text.module, text.drivers).unwrap();
        Self { kernel: Rc::new(kernel), refuse: Refusal::default() }
    }

    /// The one call the link refuses from here on, `None` being a link
    /// that carries everything.
    fn refuses(&self, call: Option<(Hash, u16)>) {
        self.refuse.set(call);
    }

    fn client(&self) -> Client {
        let link = Link {
            kernel: self.kernel.clone(),
            connection: Connection::new(),
            refuse: self.refuse.clone(),
        };
        Client::connect(Box::new(link), Vec::new(), clock::now).unwrap()
    }

    /// A Client of its own behind the one door, as the rig makes one.
    fn host(&self) -> Rc<Host> {
        Rc::new(Host::over(self.client(), clock::now))
    }
}

/// The link to that Kernel: every message but the one the case named
/// reaches the server, and that one is `Denied` here.
struct Link {
    kernel: Rc<Kernel>,
    connection: Connection,
    refuse: Refusal,
}

impl Carrier for Link {
    fn exchange(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Fault> {
        if let Some((seq, Message::Request(Op::Call { api, method, .. }))) =
            wire::decode(bytes)
            && self.refuse.get() == Some((api, method))
        {
            let denied = Message::Reply(Reply::err(Error::Denied));
            return Ok(wire::encode(seq, &denied));
        }
        self.connection
            .handle(&self.kernel, bytes, &mut NoLoader)
            .ok_or(Fault::Wire)
    }
}

/// How many handles `/sys/core` says are open on `rd`, read by another
/// Client: what a handle nobody closed shows up in.
fn handles_on(other: &mut Client, rd: Rd) -> usize {
    let core = other.resolve("/sys/core").unwrap();
    let text = String::from_utf8(other.read_all(core).unwrap()).unwrap();
    let on = format!(" rd={} ", rd.0);
    let held = |line: &&str| line.starts_with("handle ") && line.contains(&on);
    text.lines().filter(held).count()
}

/// A Client with `dev` loaded over a rig, and its list of drafts.
struct Drafts {
    host: Rc<Host>,
    list: Rd,
    /// The handle on the list, opened once as a Plugin opens one.
    h: Handle,
}

impl Drafts {
    /// `dev` over `rig` with no Plugin at all and no machine: these
    /// cases judge the list and what stands under it, and nothing ever
    /// holds the region.
    fn over(rig: &Rig) -> Self {
        Self::hosted(rig.host())
    }

    /// The same over a Host of any rig's making.
    fn hosted(host: Rc<Host>) -> Self {
        let table: Table = Box::new(|_| None);
        let dev =
            dev::boot(&host, table, Clip::none(), "", None, None).unwrap();
        let list = cell::resolve(&host.own(), dev, "buffers").unwrap();
        let h = host.own().open(list, Rights::WRITE).unwrap();
        Self { host, list, h }
    }

    /// The one door, taken for the statement that makes an op.
    fn calls(&self) -> HostCalls<'_> {
        self.host.own()
    }

    /// `f` over that door, which is what a declared api's caller side
    /// runs on: every call below is one of these.
    fn on<T>(&self, f: impl FnOnce(&mut &dyn Calls) -> T) -> T {
        let own = self.calls();
        let mut ctx: &dyn Calls = &own;
        f(&mut ctx)
    }

    /// A draft over `source`, held as a Plugin holds one.
    fn open(&self, source: Rd) -> Draft {
        let rd = self.on(|ctx| buffers::Call(ctx, self.h).open(source));
        self.held(rd.unwrap())
    }

    /// A draft over nothing.
    fn scratch(&self) -> Draft {
        self.held(self.scratching().unwrap())
    }

    /// The same as the caller sees it, a refused one included.
    fn scratching(&self) -> Result<Rd, Error> {
        self.on(|ctx| buffers::Call(ctx, self.h).scratch())
    }

    /// That draft out of the tree, and what the close answered.
    fn close(&self, draft: Draft) -> Result<(), Error> {
        self.on(|ctx| buffers::Call(ctx, self.h).close(draft.rd))
    }

    /// The rd with a handle on it, as every case below holds one.
    fn held(&self, rd: Rd) -> Draft {
        let rights = Rights::READ.union(Rights::WRITE);
        Draft { rd, h: self.calls().open(rd, rights).unwrap() }
    }

    /// One edit into the draft, which is what an operator makes.
    fn edit(
        &self,
        draft: Draft,
        at: u64,
        len: u64,
        text: &str,
    ) -> Result<(), Error> {
        let edit = text::Edit { at, len, text: String::from(text) };
        self.on(|ctx| text::Call(ctx, draft.h).replace(edit))
    }

    /// The flush, and whether the draft's bytes moved under it.
    fn sync(&self, draft: Draft) -> bool {
        self.synced(draft).unwrap()
    }

    /// The same as the caller sees it, a refused one included.
    fn synced(&self, draft: Draft) -> Result<bool, Error> {
        self.on(|ctx| draft::Call(ctx, draft.h).sync())
    }

    /// What the draft answers `history.tree` with.
    fn tree(&self, draft: Draft) -> history::Tree {
        self.on(|ctx| history::Call(ctx, draft.h).tree()).unwrap()
    }

    /// Head one revision back, or on.
    fn walk(&self, draft: Draft, back: bool) -> Result<(), Error> {
        let own = self.calls();
        let mut ctx: &dyn Calls = &own;
        let mut call = history::Call(&mut ctx, draft.h);
        if back { call.undo() } else { call.redo() }
    }

    /// The head written back to the source, or to `target`.
    fn save(&self, draft: Draft, target: Option<Rd>) {
        self.on(|ctx| {
            let mut call = publish::Call(ctx, draft.h);
            match target {
                Some(target) => call.save_as(target),
                None => call.save(),
            }
        })
        .unwrap();
    }

    /// What the cell `rd` holds.
    fn bytes(&self, rd: Rd) -> Vec<u8> {
        ops::bytes(&self.calls(), rd).unwrap()
    }

    /// The rd an `over` or a `source` child names.
    fn points(&self, draft: Draft, name: &str) -> Rd {
        let child = cell::resolve(&self.calls(), draft.rd, name).unwrap();
        decode(&self.bytes(child)).unwrap()
    }

    /// The names under `rd`, as `ls` shows them.
    fn names(&self, rd: Rd) -> Vec<String> {
        let own = self.calls();
        let ctx: &dyn Calls = &own;
        let mut names: Vec<String> = ops::children(ctx, rd)
            .unwrap()
            .into_iter()
            .filter_map(|child| ops::info(ctx, child).ok()?.name)
            .collect();
        names.sort();
        names
    }
}

/// The history of the server Buffer `over`, read by another Client.
fn history_of(other: &mut Client, over: Rd) -> history::Tree {
    let out = other.call_by_name(over, "tree", encode(&())).unwrap();
    decode(&out).unwrap()
}

#[test]
fn a_draft_is_the_clients_own_copy_and_an_edit_of_it_is_no_message() {
    let rig = Rig::new("buffers-draft");
    rig.file("note", b"one\ntwo\n");
    let it = Drafts::over(&rig);
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();

    // The draft is named by a count of its own, and names both the
    // Buffer it is over and the file that Buffer is over.
    let draft = it.open(note);
    assert_eq!(it.names(it.list), ["0"]);
    let at = ops::where_is(&it.calls(), draft.rd).unwrap();
    assert_eq!(at, "/self/dev/buffers/0");
    assert_eq!(it.names(draft.rd), ["over", "source"]);
    assert_eq!(it.points(draft, "source"), note);
    let over = it.points(draft, "over");

    // Three edits, and not one of them reaches the server.
    let sent = rig.sent();
    it.edit(draft, 3, 0, " and a half").unwrap();
    it.edit(draft, 0, 3, "ONE").unwrap();
    it.edit(draft, 0, 0, "first: ").unwrap();
    assert_eq!(rig.sent(), sent, "an edit of a draft is no wire message");
    assert_eq!(it.bytes(draft.rd), b"first: ONE and a half\ntwo\n");
    assert_eq!(other.read_all(over).unwrap(), b"one\ntwo\n", "as it was");

    // A write of the cell is the same edit by the other road: the
    // draft from there becomes those bytes and ends there.
    ops::put(&it.calls(), draft.rd, b"all of it\n".to_vec()).unwrap();
    assert_eq!(it.bytes(draft.rd), b"all of it\n");

    // A second open of the same source is that draft, so a region
    // shown twice costs one Buffer; a closed one takes what it held
    // with it and the count does not go back.
    assert_eq!(it.open(note).rd, draft.rd);
    assert_eq!(it.names(it.list), ["0"]);
    it.close(draft).unwrap();
    assert_eq!(it.names(it.list), [] as [String; 0]);
    it.open(note);
    assert_eq!(it.names(it.list), ["1"]);
}

#[test]
fn one_sync_is_one_call_and_one_revision_holding_every_edit() {
    let rig = Rig::new("buffers-sync");
    rig.file("note", b"one\ntwo\n");
    let it = Drafts::over(&rig);
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();
    let draft = it.open(note);
    let over = it.points(draft, "over");

    for at in [0, 1, 2] {
        it.edit(draft, at, 0, "x").unwrap();
    }
    let sent = rig.sent();
    assert!(!it.sync(draft), "nothing moved under the caller");
    assert_eq!(rig.sent() - sent, 1, "one call, whatever the session typed");
    assert_eq!(other.read_all(over).unwrap(), b"xxxone\ntwo\n");

    // One state for the three, which is one undo step for the person.
    let server = history_of(&mut other, over);
    assert_eq!(server.nodes.len(), 2, "the open, and the three edits");
    assert_eq!(server.head, 2);

    // And nothing pending is nothing at all.
    let sent = rig.sent();
    assert!(!it.sync(draft));
    assert_eq!(rig.sent(), sent, "no message for a flush with nothing in it");
}

#[test]
fn a_sync_that_found_the_head_moved_lands_behind_what_moved_it() {
    let rig = Rig::new("buffers-stale");
    rig.file("note", b"one\ntwo\n");
    let it = Drafts::over(&rig);
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();
    let draft = it.open(note);
    let over = it.points(draft, "over");

    // A second Client's run between the edit and the flush: the first
    // attempt is Stale, and the edits go again on top of what landed.
    it.edit(draft, 0, 0, "first\n").unwrap();
    let append = text::Edit { at: 8, len: 0, text: String::from("three\n") };
    other.call_by_name(over, "replace", encode(&append)).unwrap();
    assert!(it.sync(draft), "the draft's bytes moved under the caller");

    // Both runs landed, in the server's order, and the draft holds
    // exactly what the server holds.
    let held = b"first\none\ntwo\nthree\n";
    assert_eq!(other.read_all(over).unwrap(), held);
    assert_eq!(it.bytes(draft.rd), held);
    let server = history_of(&mut other, over);
    assert_eq!(server.nodes.len(), 3, "the open, the other run, and ours");
    assert_eq!(it.tree(draft).head, server.head, "the head it took");
}

#[test]
fn edits_the_server_no_longer_has_room_for_end_at_its_head() {
    let rig = Rig::new("buffers-refused");
    rig.file("note", b"one\ntwo\n");
    let it = Drafts::over(&rig);
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();
    let draft = it.open(note);
    let over = it.points(draft, "over");

    // The pending edit is taken against bytes the server has left, so
    // the second attempt is refused and makes no state.
    it.edit(draft, 4, 4, "TWO\n").unwrap();
    other.write_all(over, b"x\n".to_vec()).unwrap();
    assert!(it.sync(draft), "and it says so");

    assert_eq!(it.bytes(draft.rd), b"x\n", "the server head, whole");
    assert_eq!(other.read_all(over).unwrap(), b"x\n");
    let server = history_of(&mut other, over);
    assert_eq!(server.nodes.len(), 2, "nothing was merged silently");
    assert_eq!(it.tree(draft).head, server.head);
}

#[test]
fn the_edit_past_the_budget_flushes_the_ones_before_it() {
    let rig = Rig::new("buffers-budget");
    rig.file("note", b"\n");
    let it = Drafts::over(&rig);
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();
    let draft = it.open(note);
    let over = it.points(draft, "over");

    // The cap of `text::Many` is what one call carries, so the 257th
    // edit sends the ones before it and starts the next revision.
    let sent = rig.sent();
    for _ in 0..256 {
        it.edit(draft, 0, 0, "x").unwrap();
    }
    assert_eq!(rig.sent(), sent, "no message for any of the 256");
    it.edit(draft, 0, 0, "y").unwrap();
    assert_eq!(rig.sent() - sent, 1, "the 257th flushed the rest");
    assert_eq!(other.read_all(over).unwrap().len(), 257, "the 256 and the \n");

    // A session longer than the budget is two states, and nothing else
    // flushes without being asked.
    assert!(!it.sync(draft));
    assert_eq!(history_of(&mut other, over).nodes.len(), 3);
}

#[test]
fn a_draft_with_edits_pending_reports_the_node_the_flush_will_make() {
    let rig = Rig::new("buffers-tree");
    rig.file("note", b"one\n");
    let it = Drafts::over(&rig);
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();
    let draft = it.open(note);
    let over = it.points(draft, "over");

    let opened = it.tree(draft);
    assert_eq!((opened.head, opened.published), (1, 1), "as it was opened");
    let sent = rig.sent();
    it.edit(draft, 0, 0, "x").unwrap();
    let pending = it.tree(draft);
    assert_eq!(rig.sent(), sent, "the tree is answered here");
    assert_ne!(pending.head, pending.published, "which draws the [+]");
    let made =
        history::Node { id: 2, parent: 1, label: String::from("replace") };
    assert_eq!(pending.nodes, [opened.nodes[0].clone(), made]);

    // The flush makes that node, numbered as the draft said it would.
    assert!(!it.sync(draft));
    assert_eq!(it.tree(draft), pending);
    assert_eq!(history_of(&mut other, over).nodes, pending.nodes);

    // What walks a history by revision is an undo tree over the server
    // Buffer, which a caller reaches through `over`.
    let goto = it.on(|ctx| history::Call(ctx, draft.h).goto(1));
    assert_eq!(goto, Err(Error::Unsupported));
}

#[test]
fn undo_and_redo_walk_the_servers_history_and_the_draft_follows() {
    let rig = Rig::new("buffers-walk");
    rig.file("note", b"one\ntwo\n");
    let it = Drafts::over(&rig);
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();
    let draft = it.open(note);

    // The pending edit goes down first, so there is a state to undo.
    it.edit(draft, 0, 0, "first\n").unwrap();
    it.walk(draft, true).unwrap();
    assert_eq!(it.bytes(draft.rd), b"one\ntwo\n");
    assert_eq!(it.tree(draft).head, 1);
    it.walk(draft, false).unwrap();
    assert_eq!(it.bytes(draft.rd), b"first\none\ntwo\n");
    assert_eq!(it.tree(draft).head, 2);

    // The end of the history is the server's answer, as a caller of a
    // Buffer already reads it.
    it.walk(draft, true).unwrap();
    assert_eq!(it.walk(draft, true), Err(Error::Invalid));

    // A state is numbered by how many the server holds, so the node an
    // edit after an undo predicts is one past the last and not one
    // past head: the revision a status row shows is read off it.
    it.edit(draft, 0, 0, "x").unwrap();
    assert_eq!(it.tree(draft).head, 3, "the branch the flush will make");
    assert!(!it.sync(draft));
    let over = it.points(draft, "over");
    assert_eq!(history_of(&mut other, over).head, 3, "as the server made it");
}

#[test]
fn a_draft_the_server_would_not_answer_for_leaves_no_handle_open() {
    let server = Server::new();
    let it = Drafts::hosted(server.host());
    let mut other = server.client();

    // The Buffer is made and opened, and the history the flush counts
    // from is refused: the draft cannot be made of what came back.
    server.refuses(Some((history::hash(), 0)));
    assert_eq!(it.scratching(), Err(Error::Denied));
    assert_eq!(it.names(it.list), [] as [String; 0]);

    // The Buffer stands, and nothing holds it: a draft that was not
    // made keeps no handle on one.
    let buffer = other.resolve("/module/text/new1").unwrap();
    assert_eq!(handles_on(&mut other, buffer), 0, "the handle it opened");

    // The next draft is made, holds it, and hands it back at the close.
    server.refuses(None);
    let draft = it.scratch();
    let over = it.points(draft, "over");
    assert_eq!(handles_on(&mut other, over), 1);
    it.close(draft).unwrap();
    assert_eq!(handles_on(&mut other, over), 0);
}

#[test]
fn a_close_sends_what_is_pending_and_frees_the_draft_either_way() {
    let server = Server::new();
    let it = Drafts::hosted(server.host());
    let mut other = server.client();

    // A close is the end of the draft and not of what was typed into
    // it: what is pending goes down before the Buffer is let go.
    let draft = it.scratch();
    let over = it.points(draft, "over");
    it.edit(draft, 0, 0, "typed\n").unwrap();
    assert_eq!(it.close(draft), Ok(()));
    assert_eq!(other.read_all(over).unwrap(), b"typed\n", "not dropped");

    // A flush the server refuses is what the caller is told, and every
    // step of the close ran anyway: the draft is out of the tree and
    // the handle it held is closed.
    let next = it.scratch();
    let over = it.points(next, "over");
    it.edit(next, 0, 0, "lost\n").unwrap();
    server.refuses(Some((text::hash(), 1)));
    assert_eq!(it.close(next), Err(Error::Denied));
    server.refuses(None);
    assert_eq!(it.names(it.list), [] as [String; 0], "out of the tree");
    assert_eq!(handles_on(&mut other, over), 0, "and not held");
    assert_eq!(other.read_all(over).unwrap(), b"", "the flush was refused");
}

#[test]
fn a_sync_the_server_refuses_keeps_the_draft_and_the_edits_pending() {
    let server = Server::new();
    let it = Drafts::hosted(server.host());
    let mut other = server.client();
    let draft = it.scratch();
    let over = it.points(draft, "over");

    // A refusal that is no `Stale` moves nothing: the draft holds what
    // was typed, and the revision it reports is still the one the
    // flush will make.
    it.edit(draft, 0, 0, "one\n").unwrap();
    let pending = it.tree(draft);
    server.refuses(Some((text::hash(), 1)));
    assert_eq!(it.synced(draft), Err(Error::Denied));
    assert_eq!(it.bytes(draft.rd), b"one\n", "as the person typed it");
    assert_eq!(it.tree(draft), pending, "and still unpublished");
    assert_eq!(other.read_all(over).unwrap(), b"", "nothing landed");

    // The next key works, on top of what is still pending, and the
    // flush that is not refused sends every one of them as one state.
    it.edit(draft, 0, 0, "two\n").unwrap();
    assert_eq!(it.bytes(draft.rd), b"two\none\n");
    server.refuses(None);
    assert!(!it.sync(draft));
    assert_eq!(other.read_all(over).unwrap(), b"two\none\n");
    assert_eq!(history_of(&mut other, over).nodes.len(), 2, "one state");
}

#[test]
fn save_writes_the_file_and_save_as_gives_a_draft_over_nothing_one() {
    let rig = Rig::new("buffers-publish");
    rig.file("note", b"one\n");
    rig.file("other", b"");
    let it = Drafts::over(&rig);
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();
    let target = other.resolve("/module/fs/root/other").unwrap();

    let draft = it.open(note);
    it.edit(draft, 0, 0, "first\n").unwrap();
    it.save(draft, None);
    assert_eq!(rig.disk("note"), b"first\none\n", "what is pending too");
    let tree = it.tree(draft);
    assert_eq!(tree.head, tree.published, "nothing left to write");

    // A draft over nothing has no source until a `save_as` gives the
    // Buffer under it one, and then it is that file's.
    let scratch = it.scratch();
    assert_eq!(it.names(scratch.rd), ["over"]);
    it.edit(scratch, 0, 0, "made\n").unwrap();
    it.save(scratch, Some(target));
    assert_eq!(rig.disk("other"), b"made\n");
    assert_eq!(it.names(scratch.rd), ["over", "source"]);
    assert_eq!(it.points(scratch, "source"), target);
    let tree = it.tree(scratch);
    assert_eq!(tree.head, tree.published, "it goes on as that file's");
}
