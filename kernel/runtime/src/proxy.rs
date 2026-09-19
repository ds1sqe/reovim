//! The Driver that stands for a Client: one per link.
//!
//! A remote walks to `/session/<id>/client/<cid>/self` and everything
//! under it is that Client's own tree, mirrored row by row as somebody
//! asks for it. Metadata is the mirror's and costs no message; every op
//! that acts is forwarded and the Client's answer is the truth.
//!
//! Nothing is sent from here. A slot queues its op and the queue rides
//! the answer to the Client's next `Op::Since`, which connection.rs
//! splices; what comes back is one `Op::Done` per op, in any order.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_kernel_interface::{
    Calls, Driver, DriverId, Error, Flags, Handle, Hash, Info, ModuleId, Op,
    Out, Outcome, Rd, Reply, Rights, Token,
};

use crate::kernel::Kernel;
use crate::log::DEBUG;
use crate::table::ROOT;

/// Ops one link may have outstanding. The bound is the Driver's own
/// (01-core-runtime.md:120-122): past it every slot answers `Busy`, and
/// it is the whole of what holds a Client that answers nothing, there
/// being no clock here to time one out.
const QUEUE: usize = 64;

/// What a caller asked for, which says how the rows answer it.
#[derive(Clone, Copy)]
enum Want {
    /// A `lookup`: the one row it found.
    One,
    /// A listing: every row, in the Client's order.
    All,
}

/// A forwarded `lookup` or `children` being turned into mirror rows,
/// one `Info` at a time.
struct Walk {
    /// The mirrored parent every row is registered under.
    at: Rd,
    want: Want,
    /// The Client's rds still to shape, the first of them asked for
    /// now.
    rest: Vec<Rd>,
    /// The rows made, in the Client's order.
    made: Vec<Rd>,
}

impl Walk {
    /// The caller's answer, once nothing is left to shape.
    fn answer(&self) -> Outcome {
        match self.want {
            Want::One => {
                self.made.first().copied().map(Out::Rd).ok_or(Error::Invalid)
            }
            Want::All => Ok(Out::Rds(self.made.clone())),
        }
    }
}

/// What the proxy does with the Client's answer.
enum Then {
    /// `open`, `read`, `write` and `call`: the Client's outcome is the
    /// caller's, as it stands.
    Answer,
    /// Nobody waits: a `close`, whose handle is gone at the Kernel
    /// whatever the Client says of it.
    Drop,
    /// The one forwarded `Lookup { ROOT, "self" }`: the rd it answers
    /// is what the row already registered at `rd` stands for.
    Root { rd: Rd },
    /// A `lookup` or a `children`: the rds it answers are mirrored
    /// under `at` before the caller sees them.
    Found { at: Rd, want: Want },
    /// An `Info` the proxy asked for itself, to give the rd the walk
    /// stopped at its name and flags.
    Shape(Walk),
}

/// One op the Client has not answered.
struct Job {
    /// The proxy's own token: what the `serve` list names, what the
    /// Client names again in `Op::Done`, and, for an op a caller
    /// waits on, the value the Kernel scoped its own token by. There
    /// is no second map.
    token: Token,
    /// The op to send. The rd of a `Lookup`, `Children` or `Open` is
    /// the mirror's and becomes the Client's own when the job goes
    /// out; an `Info` the proxy asked for itself names a Client rd
    /// already, and every other op names a handle, which is the
    /// Client's own by construction.
    op: Op,
    then: Then,
}

#[derive(Default)]
struct State {
    /// The Client's rd for a mirror row, which is what an op going out
    /// is written in.
    to_client: BTreeMap<Rd, Rd>,
    /// The mirror row for a Client's rd, which is what tells a name
    /// the mirror already holds from one it must still shape.
    to_server: BTreeMap<Rd, Rd>,
    /// Ops made and not sent, in the order they were made.
    queue: VecDeque<Job>,
    /// Ops sent and not answered, by the token the Client names.
    waiting: BTreeMap<Token, Then>,
    next: u64,
}

impl State {
    /// The next own token. One counter answers both the `serve` list
    /// and the Kernel's Pending, so a token means one thing.
    const fn token(&mut self) -> Token {
        self.next += 1;
        Token(self.next)
    }

    /// A Client's rd and its mirror row know each other.
    fn bind(&mut self, client: Rd, server: Rd) {
        self.to_client.insert(server, client);
        self.to_server.insert(client, server);
    }

    /// Ops made or sent and not answered: what [`QUEUE`] bounds.
    fn held(&self) -> usize {
        self.queue.len() + self.waiting.len()
    }

    /// The next job to go out, taken from the queue with its op in the
    /// Client's own space; `None` while the one at the front has no
    /// row for its rd yet, which leaves it queued and everything
    /// behind it with it.
    fn next_out(&mut self) -> Option<(Token, Op, Then)> {
        let front = self.queue.front()?;
        let op = outgoing(self, &front.op)?;
        let job = self.queue.pop_front()?;
        Some((job.token, op, job.then))
    }
}

/// The Client on one link, as the Kernel sees it.
#[derive(Default)]
pub struct Proxy {
    state: RefCell<State>,
}

impl Proxy {
    /// The Driver for the Client on this link, attached under its
    /// client rd: one row named `self`, which every name of the
    /// Client's hangs under, and the one forwarded lookup that finds
    /// it. `None` when the Kernel refuses either, which leaves the
    /// link without a `/self` and everything else as it was.
    pub(crate) fn attach(
        kernel: &Kernel,
        client: Rd,
    ) -> Option<(Rc<Self>, ModuleId)> {
        let proxy = Rc::new(Self::default());
        let driver: Rc<dyn Driver> = proxy.clone();
        let name = alloc::format!("client:{}", client.0);
        let id = kernel.attach(&name, alloc::vec![driver]).ok()?;
        let calls = kernel.calls(id);
        let root = calls.register(DriverId(0), client, Some("self")).ok()?;
        calls.report(root, Flags::READ.union(Flags::CONTAINER), &[]).ok()?;
        let mut s = proxy.state.borrow_mut();
        // The two roots are one rd on both sides, and that is the one
        // pair the mirror is born knowing: `/self` itself is found by
        // a lookup, so no constant of the Client's reaches here.
        s.bind(ROOT, ROOT);
        let token = s.token();
        let op = Op::Lookup { rd: ROOT, name: String::from("self") };
        s.queue.push_back(Job { token, op, then: Then::Root { rd: root } });
        drop(s);
        Some((proxy, id))
    }

    /// The ops to serve now, taken out of the queue in the order they
    /// were made, one `debug` line each.
    ///
    /// A job whose rd has no row in the mirror yet — only ever one
    /// made before the Client said where `/self` is — waits, and
    /// everything behind it with it, so the Client is asked in the
    /// order the callers asked.
    pub(crate) fn take_out(&self, kernel: &Kernel) -> Vec<(Token, Op)> {
        let mut out = Vec::new();
        let mut s = self.state.borrow_mut();
        while let Some((token, op, then)) = s.next_out() {
            let (at, word) = (token.0, word(&op));
            kernel.say(DEBUG, || alloc::format!("serve token={at} op={word}"));
            s.waiting.insert(token, then);
            out.push((token, op));
        }
        out
    }

    /// The Client finished an op it was asked to serve: a `lookup` and
    /// a listing are the rows the mirror makes of the rds they answer,
    /// and everything else goes up to the waiting caller as it stands.
    ///
    /// # Errors
    /// `Invalid` for a token this link was not asked about, and
    /// whatever the Kernel says of the `done` it finishes with.
    pub(crate) fn answered(
        &self,
        kernel: &Kernel,
        id: ModuleId,
        token: Token,
        outcome: Outcome,
    ) -> Result<(), Error> {
        let then = self.state.borrow_mut().waiting.remove(&token);
        // The line the level turns off is never formatted, so what it
        // needs of the outcome is taken and the rest left alone.
        let failed = outcome.as_ref().err().copied();
        let at = token.0;
        kernel.say(DEBUG, || {
            failed.map_or_else(
                || alloc::format!("served token={at} ok"),
                |e| alloc::format!("served token={at} err={e:?}"),
            )
        });
        match then.ok_or(Error::Invalid)? {
            Then::Drop => Ok(()),
            Then::Answer => kernel.calls(id).done(token, outcome),
            Then::Root { rd } => self.rooted(rd, &outcome),
            Then::Found { at, want } => {
                let rest = match outcome {
                    Ok(Out::Rd(rd)) => alloc::vec![rd],
                    Ok(Out::Rds(rds)) => rds,
                    // A name that went, or one that never was: what
                    // the Client says is what the caller hears.
                    other => return kernel.calls(id).done(token, other),
                };
                let walk = Walk { at, want, rest, made: Vec::new() };
                self.walk(kernel, id, token, walk)
            }
            Then::Shape(walk) => self.shaped(kernel, id, token, walk, &outcome),
        }
    }

    /// The Client's `/self`: the row the mirror was born with stands
    /// for it from here on, and every job held for want of it goes out
    /// with the next `Since`. Nobody waits on this one.
    fn rooted(&self, rd: Rd, outcome: &Outcome) -> Result<(), Error> {
        let Ok(Out::Rd(root)) = outcome else {
            return Err(Error::Invalid);
        };
        self.state.borrow_mut().bind(*root, rd);
        Ok(())
    }

    /// Shape the next rd of the walk, or answer the caller with the
    /// rows. An rd the mirror already holds costs no message.
    fn walk(
        &self,
        kernel: &Kernel,
        id: ModuleId,
        token: Token,
        mut walk: Walk,
    ) -> Result<(), Error> {
        while let Some(server) = self.known(&walk) {
            walk.rest.remove(0);
            walk.made.push(server);
        }
        let Some(&rd) = walk.rest.first() else {
            return kernel.calls(id).done(token, walk.answer());
        };
        // Its name and flags are the Client's; the row is made when
        // they land.
        self.hold(token, Op::Info { rd }, Then::Shape(walk));
        Ok(())
    }

    /// The mirror row for the rd the walk stands at, if it has one.
    fn known(&self, walk: &Walk) -> Option<Rd> {
        let rd = walk.rest.first()?;
        self.state.borrow().to_server.get(rd).copied()
    }

    /// The name and flags of the rd the walk stopped at: one mirror
    /// row, and on with the walk.
    fn shaped(
        &self,
        kernel: &Kernel,
        id: ModuleId,
        token: Token,
        walk: Walk,
        outcome: &Outcome,
    ) -> Result<(), Error> {
        let calls = kernel.calls(id);
        let (Ok(Out::Info(info)), Some(rd)) =
            (outcome, walk.rest.first().copied())
        else {
            // The name went between the listing and the asking.
            return calls.done(token, Err(errored(outcome)));
        };
        if self.state.borrow().to_server.contains_key(&rd) {
            // Another walk made the row while this one asked for it.
            return self.walk(kernel, id, token, walk);
        }
        match row(&calls, walk.at, info) {
            Ok(server) => self.state.borrow_mut().bind(rd, server),
            // The mirror cannot hold it; the caller hears why rather
            // than waiting for ever.
            Err(error) => return calls.done(token, Err(error)),
        }
        self.walk(kernel, id, token, walk)
    }

    /// One more op on a token a caller is already waiting on: it goes
    /// behind whatever else is queued and is counted with the rest.
    fn hold(&self, token: Token, op: Op, then: Then) {
        let mut s = self.state.borrow_mut();
        s.queue.push_back(Job { token, op, then });
    }

    /// One op for the Client and the token the caller waits on, or
    /// `Busy` past [`QUEUE`].
    fn queue<T>(&self, op: Op, then: Then) -> Reply<T> {
        let mut s = self.state.borrow_mut();
        if s.held() >= QUEUE {
            return Reply::err(Error::Busy);
        }
        let token = s.token();
        s.queue.push_back(Job { token, op, then });
        Reply::Pending(token)
    }
}

impl Driver for Proxy {
    fn lookup(&self, _: &dyn Calls, rd: Rd, name: &str) -> Reply<Rd> {
        // An empty name is the Kernel asking for the whole listing,
        // which is `Op::Children` at the Client.
        if name.is_empty() {
            let then = Then::Found { at: rd, want: Want::All };
            return self.queue(Op::Children { rd }, then);
        }
        let op = Op::Lookup { rd, name: String::from(name) };
        self.queue(op, Then::Found { at: rd, want: Want::One })
    }

    fn open(&self, _: &dyn Calls, rd: Rd, rights: Rights) -> Reply<u64> {
        // The cookie the Kernel writes into the slot is the Client's
        // own handle, so every op after this one names it already and
        // the proxy keeps one table and not two.
        self.queue(Op::Open { rd, rights }, Then::Answer)
    }

    fn read(
        &self,
        _: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        let len = u32::try_from(len).unwrap_or(u32::MAX);
        self.queue(Op::Read { h: Handle(h), at, len }, Then::Answer)
    }

    fn write(
        &self,
        _: &dyn Calls,
        h: u64,
        at: u64,
        buf: &[u8],
    ) -> Reply<usize> {
        let bytes = buf.to_vec();
        self.queue(Op::Write { h: Handle(h), at, bytes }, Then::Answer)
    }

    fn call(
        &self,
        _: &dyn Calls,
        h: u64,
        api: Hash,
        method: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        let input = input.to_vec();
        self.queue(Op::Call { h: Handle(h), api, method, input }, Then::Answer)
    }

    fn watch(&self, _: &dyn Calls, _: Rd, _: bool) -> Reply<()> {
        // A hint, and the Kernel's watch does not depend on what it
        // answers, so a remote never waits for one to cross. The slot
        // names an rd where the wire names a handle; what a forwarded
        // watch is, and what it announces, is 05.2's.
        Reply::ok(())
    }

    fn close(&self, _: &dyn Calls, h: u64) -> Reply<()> {
        // The handle is gone at the Kernel whatever the Client says,
        // so the remote is answered here and the Client hears of it
        // with the next `Since`.
        let _: Reply<()> = self.queue(Op::Close { h: Handle(h) }, Then::Drop);
        Reply::ok(())
    }
}

/// One mirror row for a Client's rd: registered under `at` with the
/// Client's name and reported with its flags. The apis are empty, so a
/// remote `call` on a mirrored rd is `Unsupported` at the Kernel and
/// never reaches here; that is 05.5's.
fn row(calls: &dyn Calls, at: Rd, info: &Info) -> Result<Rd, Error> {
    let rd = calls.register(DriverId(0), at, info.name.as_deref())?;
    calls.report(rd, info.flags, &[])?;
    Ok(rd)
}

/// The op as it goes out, with the mirror's rd turned into the
/// Client's own; `None` while there is no row for it yet.
fn outgoing(s: &State, op: &Op) -> Option<Op> {
    let at = |rd: &Rd| s.to_client.get(rd).copied();
    Some(match op {
        Op::Lookup { rd, name } => {
            Op::Lookup { rd: at(rd)?, name: name.clone() }
        }
        Op::Children { rd } => Op::Children { rd: at(rd)? },
        Op::Open { rd, rights } => Op::Open { rd: at(rd)?, rights: *rights },
        // An `Info` names a Client rd the Client itself gave, and the
        // rest name a handle, which is the Client's own: the cookie
        // the Kernel holds for a mirrored rd is what its `open`
        // answered.
        other => other.clone(),
    })
}

/// An answer of the wrong shape: the error it carried, or `Invalid`
/// for an `Out` that is not what the op answers.
fn errored(outcome: &Outcome) -> Error {
    outcome.as_ref().err().copied().unwrap_or(Error::Invalid)
}

/// The op in the trace's words, as dispatch says them.
const fn word(op: &Op) -> &'static str {
    match op {
        Op::Lookup { .. } => "lookup",
        Op::Children { .. } => "children",
        Op::Info { .. } => "info",
        Op::Open { .. } => "open",
        Op::Read { .. } => "read",
        Op::Write { .. } => "write",
        Op::Call { .. } => "call",
        Op::Close { .. } => "close",
        // Nothing else is ever queued.
        _ => "op",
    }
}
