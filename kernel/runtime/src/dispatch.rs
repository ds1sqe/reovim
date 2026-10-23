//! Caller ops: right and declaration checks, then the vtable.

use alloc::string::String;
use alloc::vec::Vec;

use reovim_kernel_interface::{
    Caller, DriverId, Error, Flags, Handle, Hash, ModuleId, Op, Out, Rd, Reply,
    Rights, Token, allows, check_call, check_reply, part,
};

use reovim_kernel_interface::Kind;

use crate::kernel::{Kernel, Slot, State, core, fact, rw};
use crate::log::{self, DEBUG, NOTICE, WARN, level_of};
use crate::table::{Node, Owner, momentary, untraced};

/// An op's answer; `Err` becomes `Ready(Err)` at the caller.
type Answer = Result<Reply<Out>, Error>;

impl Kernel {
    /// Run one op as `caller`, outside the reply window.
    ///
    /// An op a slot submits from inside another runs here too, one
    /// step deeper; past `Limits::reentry` the op is refused so the
    /// chain ends at an error instead of at the stack.
    ///
    /// At `debug` the op is one traced line, its outcome and its own
    /// round trip in it.
    pub(crate) fn run(&self, caller: Caller, op: Op) -> Reply<Out> {
        let Some(_depth) = self.deeper() else {
            return Reply::err(too_deep(self, caller));
        };
        let trace = tracing(self, caller, &op);
        let answer = match op {
            Op::Lookup { rd, name } => lookup(self, caller, rd, &name),
            Op::Open { rd, rights } => open(self, caller, rd, rights),
            Op::Read { h, at, len } => read(self, caller, h, at, len),
            Op::Write { h, at, bytes } => write(self, caller, h, at, &bytes),
            Op::Call { h, api, method, input } => {
                call(self, caller, h, api, method, &input)
            }
            Op::Watch { h, on } => watch(self, caller, h, on),
            Op::Close { h } => close(self, caller, h),
            Op::Ask { token } => {
                let mut s = self.state.borrow_mut();
                s.pending.ask(caller, token).map(Reply::ok)
            }
            Op::Cancel { token } => cancel(self, caller, token),
            Op::Since { cursor } => since(self, caller, cursor),
            Op::Info { rd } => self
                .state
                .borrow()
                .table
                .info(rd)
                .map(|i| Reply::ok(Out::Info(i))),
            Op::Parent { rd } => parent(self, rd),
            Op::Children { rd } => children(self, caller, rd),
            // A caller's answer to what it was asked to serve, and what
            // it announces of the rds it serves, are the link's
            // business and not the tree's; whoever moves the bytes
            // takes both before dispatch ever sees them.
            Op::Done { .. } | Op::Changed { .. } => Err(Error::Unsupported),
        };
        if let Some((words, at, before)) = trace {
            let took = self.now().saturating_sub(at);
            let end = outcome(&answer);
            let heap = grew(before, self.heap().live);
            let line =
                || alloc::format!("{words} {end} took={took} heap={heap}");
            self.say(DEBUG, line);
        }
        answer.unwrap_or_else(Reply::err)
    }

    /// The slot for `h` if `caller` owns it, its rd still exists and it
    /// holds `need`.
    fn slot(
        &self,
        caller: Caller,
        h: Handle,
        need: Rights,
    ) -> Result<Slot, Error> {
        let s = self.state.borrow();
        let slot = s.handles.get(&h).ok_or(Error::Invalid)?;
        if slot.caller != caller {
            return Err(Error::Denied);
        }
        // The `open` that made it has not answered: there is no cookie
        // behind this number yet, so its owner tries again later.
        if slot.opening {
            return Err(Error::Busy);
        }
        if !s.table.exists(slot.rd) {
            return Err(Error::Gone);
        }
        if !slot.rights.contains(need) {
            return Err(Error::Denied);
        }
        Ok(slot.clone())
    }
}

/// The op begun, in the trace's words, at the instant it began and
/// with the live heap it began on, or `None` when nothing is traced.
/// The heap is read only here, as the clock is: what the level turns
/// off costs nothing.
fn tracing(k: &Kernel, caller: Caller, op: &Op) -> Option<(String, u64, u64)> {
    if !k.logs(DEBUG) {
        return None;
    }
    let words = words(k, caller, op)?;
    Some((words, k.now(), k.heap().live))
}

/// What an op cost the heap: the live bytes after it less the live
/// bytes before, always signed, so `heap=+0` says it cost nothing.
fn grew(before: u64, after: u64) -> String {
    if after >= before {
        alloc::format!("+{}", after - before)
    } else {
        alloc::format!("-{}", before - after)
    }
}

/// The op in the trace's words, or `None` for one that is never
/// traced: an op on one of the rds `untraced` names, which a follower
/// reading the log would otherwise feed or which the Kernel makes out
/// of what a trace would have just written, and one that names no
/// Resource, which is a caller asking after what it asked for already.
fn words(k: &Kernel, caller: Caller, op: &Op) -> Option<String> {
    let of = |h: &Handle| k.state.borrow().handles.get(h).map(|slot| slot.rd);
    let empty = String::new;
    let (name, rd, rest) = match op {
        Op::Lookup { rd, name } => {
            ("lookup", Some(*rd), alloc::format!(" name={name}"))
        }
        Op::Open { rd, rights } => {
            let asked = rw(
                rights.contains(Rights::READ),
                rights.contains(Rights::WRITE),
            );
            ("open", Some(*rd), alloc::format!(" rights={asked}"))
        }
        Op::Read { h, at, len } => {
            ("read", of(h), alloc::format!(" h={} at={at} len={len}", h.0))
        }
        Op::Write { h, at, bytes } => (
            "write",
            of(h),
            alloc::format!(" h={} at={at} len={}", h.0, bytes.len()),
        ),
        Op::Call { h, api, method, input } => (
            "call",
            of(h),
            alloc::format!(
                " h={} api={:016x} method={method} len={}",
                h.0,
                api.0,
                input.len()
            ),
        ),
        Op::Watch { h, on } => {
            ("watch", of(h), alloc::format!(" h={} on={on}", h.0))
        }
        Op::Close { h } => ("close", of(h), alloc::format!(" h={}", h.0)),
        Op::Info { rd } => ("info", Some(*rd), empty()),
        Op::Parent { rd } => ("parent", Some(*rd), empty()),
        Op::Children { rd } => ("children", Some(*rd), empty()),
        // `Changed` names an rd, but one the caller serves and nobody
        // walked to here, so it is no Resource of this trace's.
        Op::Ask { .. }
        | Op::Cancel { .. }
        | Op::Since { .. }
        | Op::Done { .. }
        | Op::Changed { .. } => return None,
    };
    let rd = rd?;
    if untraced(rd) {
        return None;
    }
    Some(alloc::format!("op={name} caller={} rd={}{rest}", caller.0, rd.0))
}

/// What the op answered: the one number its `Out` carries, or the name
/// of the error that refused it.
fn outcome(answer: &Answer) -> String {
    let out = match answer {
        Err(error) | Ok(Reply::Ready(Err(error))) => {
            return alloc::format!("err={error:?}");
        }
        Ok(Reply::Pending(token)) => {
            return alloc::format!("pending={}", token.0);
        }
        Ok(Reply::Ready(Ok(out))) => out,
    };
    match out {
        Out::Rd(rd) => alloc::format!("ok={}", rd.0),
        Out::Handle(h) => alloc::format!("ok={}", h.0),
        Out::Len(len) => alloc::format!("ok={len}"),
        Out::Bytes(bytes) => alloc::format!("ok={}", bytes.len()),
        Out::Rds(rds) => alloc::format!("ok={}", rds.len()),
        Out::Unit | Out::Info(_) | Out::Events { .. } => String::from("ok"),
    }
}

fn lookup(k: &Kernel, caller: Caller, rd: Rd, name: &str) -> Answer {
    let s = k.state.borrow();
    let entry = s.table.get(rd)?;
    let (owner, flags) = (entry.owner, entry.flags);
    if let Some(child) = s.table.child(rd, name) {
        return Ok(Reply::ok(Out::Rd(child)));
    }
    drop(s);
    let Some((module, driver)) = serving(owner) else {
        return Err(Error::Invalid);
    };
    // A Module that registered no Driver at all answers `Invalid`,
    // which is `driver_index`'s own word for a `DriverId` it lacks.
    let (index, d) = k.served(module, driver)?;
    let reply = named(flags, d.lookup(&k.calls(module), rd, name));
    finish(k, caller, None, Some((module, index)), reply, Out::Rd)
}

/// Which Driver answers for a name `rd` holds no child of: the one the
/// row was registered with, and a Module's first for its own home, so a
/// Module may make a child when it is asked for one rather than
/// register a row for every rd it might ever serve. The Client's own
/// tree says the same of `/self/<name>` (`clients/core`, `serving`).
const fn serving(owner: Owner) -> Option<(ModuleId, DriverId)> {
    match owner {
        Owner::Driver { module, driver } => Some((module, driver)),
        Owner::Kernel(Node::Home(module)) => Some((module, DriverId(0))),
        Owner::Kernel(_) => None,
    }
}

/// The answer to a lookup a Container's Driver would not make. A row
/// flagged `CONTAINER` holds only the names it registered, exactly
/// what [`children`] answers from, so a name missing from the table is
/// not there: `Invalid`, which a Kernel Container's missing name has
/// always answered, and a home is inserted with `CONTAINER` among its
/// `Flags` (`OPEN`, in `Kernel::enter`), so it reads no differently
/// than before. The same rule now holds over a Driver-owned Container,
/// as `clients/core`'s `named` already reads one. A row that is no
/// Container holds no names at all: `Unsupported` stands for it, and a
/// Driver with a `lookup` of its own still answers for itself with
/// whatever else it says.
const fn named(flags: Flags, reply: Reply<Rd>) -> Reply<Rd> {
    match reply {
        Reply::Ready(Err(Error::Unsupported))
            if flags.contains(Flags::CONTAINER) =>
        {
            Reply::Ready(Err(Error::Invalid))
        }
        other => other,
    }
}

fn open(k: &Kernel, caller: Caller, rd: Rd, rights: Rights) -> Answer {
    let owner = {
        let s = k.state.borrow();
        let entry = s.table.get(rd)?;
        if !allows(entry.flags, rights) {
            return Err(Error::Denied);
        }
        entry.owner
    };
    let (served, reply) = match owner {
        Owner::Kernel(_) => (None, Reply::ok(0)),
        Owner::Driver { module, driver } => {
            let (index, d) = k.served(module, driver)?;
            (Some((module, index)), d.open(&k.calls(module), rd, rights))
        }
    };
    // A Driver that must ask before it answers leaves the handle
    // without its cookie: the slot is made here all the same, so the
    // `done` has somewhere to put the cookie, and no op may use the
    // number until it lands.
    let (cookie, own) = match reply {
        Reply::Ready(result) => (result?, None),
        Reply::Pending(token) => (0, Some(token.0)),
    };
    let mut s = k.state.borrow_mut();
    let h = Handle(s.next_handle);
    // The token before the slot: a Driver that names an outstanding
    // one leaves no half-open handle behind.
    let token = match own {
        None => None,
        Some(own) => {
            let (module, _) = served.ok_or(Error::Unsupported)?;
            Some(s.pending.opening(module, own, caller, h)?)
        }
    };
    s.next_handle += 1;
    let opening = token.is_some();
    let slot = Slot {
        caller,
        rd,
        rights,
        served,
        cookie,
        pending: usize::from(opening),
        watching: false,
        opening,
    };
    s.handles.insert(h, slot);
    if let Some(token) = token {
        return Ok(Reply::Pending(token));
    }
    Ok(Reply::ok(Out::Handle(h)))
}

fn read(k: &Kernel, caller: Caller, h: Handle, at: u64, len: u32) -> Answer {
    let slot = k.slot(caller, h, Rights::READ)?;
    room(k, &slot)?;
    let len = len as usize;
    let reply = match slot.served {
        None => Reply::Ready(kernel_read(k, h, slot.rd, at, len)),
        Some((module, index)) => {
            k.driver(index).read(&k.calls(module), slot.cookie, at, len)
        }
    };
    // A fact, a core and a heard time are made at the read out of what
    // the Kernel already holds; reading one leaves nothing behind,
    // atime included.
    if matches!(reply, Reply::Ready(Ok(_))) {
        let mut s = k.state.borrow_mut();
        if let Ok(entry) = s.table.get_mut(slot.rd)
            && !matches!(entry.owner, Owner::Kernel(node) if momentary(node))
        {
            entry.atime = k.now();
        }
    }
    finish(k, caller, Some(h), slot.served, reply, Out::Bytes)
}

fn write(
    k: &Kernel,
    caller: Caller,
    h: Handle,
    at: u64,
    bytes: &[u8],
) -> Answer {
    let slot = k.slot(caller, h, Rights::WRITE)?;
    room(k, &slot)?;
    let reply = match slot.served {
        None => {
            let written = kernel_write(k, caller, slot.rd, at, bytes);
            // The cell is new, so the text this handle read before the
            // write is old: it goes, and the handle's next read makes
            // the value afresh.
            k.state.borrow_mut().snaps.remove(&h);
            Reply::Ready(written)
        }
        Some((module, index)) => {
            k.driver(index).write(&k.calls(module), slot.cookie, at, bytes)
        }
    };
    let len = |n| Out::Len(u32::try_from(n).unwrap_or(u32::MAX));
    finish(k, caller, Some(h), slot.served, reply, len)
}

fn call(
    k: &Kernel,
    caller: Caller,
    h: Handle,
    api: Hash,
    method: u16,
    input: &[u8],
) -> Answer {
    let slot = k.slot(caller, h, Rights::empty())?;
    {
        let s = k.state.borrow();
        let offered = &s.table.get(slot.rd)?.apis;
        check_call(slot.rights, offered, &s.decls, api, method, input)?;
    }
    room(k, &slot)?;
    let (module, index) = slot.served.ok_or(Error::Unsupported)?;
    let measure = k.logs(DEBUG).then(|| (k.now(), k.heap().live));
    stamp(k, slot.rd, api, method, "dispatch", None);
    let reply =
        k.driver(index).call(&k.calls(module), slot.cookie, api, method, input);
    let spent = measure.map(|(at, before)| {
        (k.now().saturating_sub(at), grew(before, k.heap().live))
    });
    stamp(k, slot.rd, api, method, "reply", spent);
    if let Reply::Ready(Ok(bytes)) = &reply
        && check_reply(&k.state.borrow().decls, api, method, bytes).is_err()
    {
        k.say(DEBUG, || {
            alloc::format!(
                "call api={:016x} method={method} invalid reply len={}",
                api.0,
                bytes.len()
            )
        });
        return Err(Error::Invalid);
    }
    let reply = finish(k, caller, Some(h), slot.served, reply, Out::Bytes)?;
    if let Reply::Pending(token) = reply {
        k.state.borrow_mut().pending.calling(token, api, method)?;
    }
    Ok(reply)
}

fn watch(k: &Kernel, caller: Caller, h: Handle, on: bool) -> Answer {
    let slot = k.slot(caller, h, Rights::READ)?;
    // Nothing changed, so nothing is said: a Driver that counts the
    // hints, as the Driver standing for a Client does, hears one per
    // handle that watches and not one per op a caller sends.
    if slot.watching == on {
        return Ok(Reply::ok(Out::Unit));
    }
    set_watch(&mut k.state.borrow_mut(), caller, h, slot.rd, on);
    if let Some((module, index)) = slot.served {
        // A hint only; the Driver's answer does not change the watch.
        let _ = k.driver(index).watch(&k.calls(module), slot.rd, on);
    }
    Ok(Reply::ok(Out::Unit))
}

fn close(k: &Kernel, caller: Caller, h: Handle) -> Answer {
    let slot = {
        let mut s = k.state.borrow_mut();
        if s.handles.get(&h).ok_or(Error::Invalid)?.caller != caller {
            return Err(Error::Denied);
        }
        let slot = s.handles.remove(&h).ok_or(Error::Invalid)?;
        s.snaps.remove(&h);
        if slot.watching {
            unwatch(&mut s, caller, slot.rd);
        }
        slot
    };
    if let Some((module, index)) = slot.served {
        let calls = k.calls(module);
        // A close implies unwatch, which is `Op::Close`'s own doc: the
        // Driver is told of this one as it is told of every other.
        if slot.watching {
            let _ = k.driver(index).watch(&calls, slot.rd, false);
        }
        // The handle is gone whatever the Driver says.
        let _ = k.driver(index).close(&calls, slot.cookie);
    }
    Ok(Reply::ok(Out::Unit))
}

fn since(k: &Kernel, caller: Caller, cursor: u64) -> Answer {
    let mut s = k.state.borrow_mut();
    s.callers.get(&caller).ok_or(Error::Invalid)?;
    let (events, cursor) = s.events.since(cursor, &s.watched(caller));
    if events.iter().any(|e| e.kind == Kind::Overflow) {
        let caller = caller.0;
        s.say(WARN, || {
            alloc::format!("overflow caller={caller} cursor={cursor}")
        });
    }
    // Empty here always: what a caller is asked to serve is the link's
    // and is spliced in by whoever moves the bytes.
    let serve = Vec::new();
    Ok(Reply::ok(Out::Events { events, cursor, serve }))
}

fn parent(k: &Kernel, rd: Rd) -> Answer {
    let parent = k.state.borrow().table.get(rd)?.parent;
    Ok(Reply::ok(Out::Rd(parent.ok_or(Error::Invalid)?)))
}

fn children(k: &Kernel, caller: Caller, rd: Rd) -> Answer {
    let unlisted = {
        let s = k.state.borrow();
        let entry = s.table.get(rd)?;
        (!entry.listed).then_some(entry.owner)
    };
    if let Some(Owner::Driver { module, driver }) = unlisted {
        // A Ready listing registers as it goes, so the cache is what
        // `Children` answers from and the reply itself carries
        // nothing. A Driver that must ask first answers `Pending`, and
        // then its own `done` is the listing: this is the one place
        // the answer does not come from the Kernel's table, the table
        // not being full yet when the op returns.
        let (_, d) = k.served(module, driver)?;
        // Marked while the Driver runs, so a `Children` it submits
        // from inside its own listing answers the cache as it stands
        // and the Driver is not asked again.
        mark(k, rd, true);
        if let Reply::Pending(own) = d.lookup(&k.calls(module), rd, "") {
            // It has listed nothing yet: the mark goes back until the
            // rows land, so a listing that is cancelled or refused
            // leaves the next `Children` to make it rather than every
            // later one answering from a cache nobody filled.
            mark(k, rd, false);
            let mut s = k.state.borrow_mut();
            let token = s.pending.listing(module, own.0, caller, rd)?;
            return Ok(Reply::Pending(token));
        }
    }
    let children = k.state.borrow().table.get(rd)?.children.clone();
    Ok(Reply::ok(Out::Rds(children)))
}

/// The cache of `rd`'s children full or not. An rd that went while its
/// Driver ran has no cache left to mark. The proxy marks a mirrored
/// row not full when its Client says a name came under it.
pub fn mark(k: &Kernel, rd: Rd, full: bool) {
    if let Ok(entry) = k.state.borrow_mut().table.get_mut(rd) {
        entry.listed = full;
    }
}

/// `Busy` when the handle is at its Pending cap.
fn room(k: &Kernel, slot: &Slot) -> Result<(), Error> {
    if slot.pending >= k.limits().pending_per_handle {
        Err(Error::Busy)
    } else {
        Ok(())
    }
}

/// Turn a slot's answer into the caller's: a Pending token is scoped by
/// the Module, owed to `caller` and to no other, and counted on the
/// handle the op names, if it names one.
fn finish<T>(
    k: &Kernel,
    caller: Caller,
    h: Option<Handle>,
    served: Option<(ModuleId, usize)>,
    reply: Reply<T>,
    out: impl FnOnce(T) -> Out,
) -> Answer {
    let own = match reply {
        Reply::Ready(result) => return Ok(Reply::Ready(result.map(out))),
        Reply::Pending(token) => token.0,
    };
    let (module, _) = served.ok_or(Error::Unsupported)?;
    let mut s = k.state.borrow_mut();
    let token = s.pending.open(module, own, caller, h)?;
    if let Some(slot) = h.and_then(|h| s.handles.get_mut(&h)) {
        slot.pending += 1;
    }
    Ok(Reply::Pending(token))
}

/// No vtable slot carries a cancel; a later `done` is dropped. The
/// slot an `open` that was still waiting made goes with the token,
/// its cookie being one nothing will ever deliver. A token another
/// caller holds is `Denied` and nothing moves.
fn cancel(k: &Kernel, caller: Caller, token: Token) -> Answer {
    let mut s = k.state.borrow_mut();
    if let Some(h) = s.pending.cancel(caller, token)? {
        s.handles.remove(&h);
    }
    Ok(Reply::ok(Out::Unit))
}

fn set_watch(s: &mut State, caller: Caller, h: Handle, rd: Rd, on: bool) {
    if let Some(slot) = s.handles.get_mut(&h) {
        slot.watching = on;
    }
    if on {
        if let Some(state) = s.callers.get_mut(&caller) {
            *state.watched.entry(rd).or_insert(0) += 1;
        }
    } else {
        unwatch(s, caller, rd);
    }
}

fn unwatch(s: &mut State, caller: Caller, rd: Rd) {
    let Some(state) = s.callers.get_mut(&caller) else { return };
    let Some(count) = state.watched.get_mut(&rd) else { return };
    *count -= 1;
    if *count == 0 {
        state.watched.remove(&rd);
    }
}

/// The Kernel-served reads: declaration bytes, the log, the bytes of a
/// cell, and the cells the Kernel makes out of what it holds, each
/// served from the handle that reads it.
fn kernel_read(
    k: &Kernel,
    h: Handle,
    rd: Rd,
    at: u64,
    len: usize,
) -> Result<Vec<u8>, Error> {
    let mut s = k.state.borrow_mut();
    let Owner::Kernel(node) = s.table.get(rd)?.owner else {
        return Err(Error::Invalid);
    };
    match node {
        Node::Decl(hash) => {
            let bytes = s.decls.get(&hash).ok_or(Error::Gone)?.canonical();
            Ok(part(&bytes, at, len))
        }
        Node::Log => Ok(s.log.read(at, len)),
        Node::Cell => {
            let bytes = s.cells.get(&rd).ok_or(Error::Gone)?;
            Ok(part(bytes, at, len))
        }
        Node::Container | Node::Home(_) => Err(Error::Unsupported),
        made => Ok(part(held(&mut s, h, made)?, at, len)),
    }
}

/// The text `h` serves for a cell the Kernel makes rather than stores:
/// a parameter, a fact, a caller's heard time, a core. It is made at
/// this handle's first read and every read of the handle is a part of
/// that one text, so a reader walking the cell in chunks is answered
/// one value and not one per chunk. A write through the handle takes it
/// back; a reader that wants a newer value opens again.
fn held(s: &mut State, h: Handle, node: Node) -> Result<&[u8], Error> {
    if !s.snaps.contains_key(&h) {
        let text = match node {
            Node::Level => Vec::from(log::name(s.log.level()).as_bytes()),
            Node::Limit(param) => {
                alloc::format!("{}", s.limits.get(param)).into_bytes()
            }
            Node::Info(which) => fact(s, which).into_bytes(),
            Node::Core => core(s),
            Node::Heard(caller) => {
                let heard = s.callers.get(&caller).ok_or(Error::Gone)?.heard;
                alloc::format!("{heard}").into_bytes()
            }
            _ => return Err(Error::Unsupported),
        };
        s.snaps.insert(h, text);
    }
    s.snaps.get(&h).map(Vec::as_slice).ok_or(Error::Gone)
}

/// The Kernel-served writes: a log entry, marked by the caller, a
/// parameter (the level by its name or its number, a limit in decimal),
/// or a cell's bytes from `at` on. A parameter is applied as it is
/// written and says so at `notice`, so the log holds what changed the
/// Kernel it is kept by.
fn kernel_write(
    k: &Kernel,
    caller: Caller,
    rd: Rd,
    at: u64,
    bytes: &[u8],
) -> Result<usize, Error> {
    let now = k.now();
    let mut s = k.state.borrow_mut();
    match s.table.get(rd)?.owner {
        Owner::Kernel(Node::Log) => {
            let mark = String::from(s.mark(caller));
            let (level, text) = level_of(bytes);
            s.log(&mark, level, text.to_vec());
        }
        Owner::Kernel(Node::Level) => {
            let text = word(at, bytes)?;
            let level = log::level_named(text).ok_or(Error::Invalid)?;
            let was = log::name(s.log.level());
            s.log.set_level(level);
            s.table.get_mut(rd)?.mtime = now;
            s.emit(rd, Kind::Modify, None);
            let (set, caller) = (log::name(level), caller.0);
            s.say(NOTICE, || {
                alloc::format!("loglevel was={was} now={set} caller={caller}")
            });
        }
        Owner::Kernel(Node::Limit(param)) => {
            let value: u64 =
                word(at, bytes)?.parse().or(Err(Error::Invalid))?;
            let was = s.limits.get(param);
            s.limits.set(param, value)?;
            // A smaller ring trims on its next append, which the
            // `Modify` below and the `notice` after it are; the other
            // three are read where they are used, so they hold already.
            s.table.get_mut(rd)?.mtime = now;
            s.emit(rd, Kind::Modify, None);
            let (name, caller) = (param.name(), caller.0);
            s.say(NOTICE, || {
                alloc::format!(
                    "limit name={name} was={was} now={value} caller={caller}"
                )
            });
        }
        Owner::Kernel(Node::Cell) => {
            let cell = s.cells.get_mut(&rd).ok_or(Error::Gone)?;
            let at = usize::try_from(at).map_err(|_| Error::Invalid)?;
            if at > cell.len() {
                return Err(Error::Invalid);
            }
            cell.truncate(at);
            cell.extend_from_slice(bytes);
            s.table.get_mut(rd)?.mtime = now;
            s.emit(rd, Kind::Modify, None);
        }
        _ => return Err(Error::Unsupported),
    }
    Ok(bytes.len())
}

/// The one word a parameter is written as: the whole cell, so a write
/// past its front is `Invalid`, and the newline a hand leaves trimmed.
fn word(at: u64, bytes: &[u8]) -> Result<&str, Error> {
    if at != 0 {
        return Err(Error::Invalid);
    }
    let text = core::str::from_utf8(bytes).map_err(|_| Error::Invalid)?;
    Ok(text.trim())
}

/// The op refused for standing too deep: one `warn` line naming the
/// caller whose chain it is and the depth it was refused at.
fn too_deep(k: &Kernel, caller: Caller) -> Error {
    k.say(WARN, || {
        alloc::format!("reentry caller={} depth={}", caller.0, k.depth())
    });
    Error::Busy
}

/// The Kernel measures every `call` with a pair of log entries: one at
/// dispatch, one at the reply, which carries the round trip between
/// them in the clock's unit and what it cost the heap.
fn stamp(
    k: &Kernel,
    rd: Rd,
    api: Hash,
    method: u16,
    phase: &str,
    spent: Option<(u64, String)>,
) {
    k.say(DEBUG, || {
        let spent = spent.map_or_else(String::new, |(took, heap)| {
            alloc::format!(" took={took} heap={heap}")
        });
        alloc::format!(
            "call rd={} api={:016x} method={method} {phase}{spent}",
            rd.0,
            api.0
        )
    });
}
