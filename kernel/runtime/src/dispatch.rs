//! Caller ops: right and declaration checks, then the vtable.

use alloc::string::String;
use alloc::vec::Vec;

use reovim_kernel_interface::{
    Caller, Error, Flags, Handle, Hash, ModuleId, Op, Out, Rd, Reply, Rights,
    value,
};

use reovim_kernel_interface::Kind;

use crate::kernel::{Kernel, Slot, State, core, mem};
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
            Op::Lookup { rd, name } => lookup(self, rd, &name),
            Op::Open { rd, rights } => open(self, caller, rd, rights),
            Op::Read { h, at, len } => read(self, caller, h, at, len),
            Op::Write { h, at, bytes } => write(self, caller, h, at, &bytes),
            Op::Call { h, api, method, input } => {
                call(self, caller, h, api, method, &input)
            }
            Op::Watch { h, on } => watch(self, caller, h, on),
            Op::Close { h } => close(self, caller, h),
            Op::Ask { token } => {
                self.state.borrow_mut().pending.ask(token).map(Reply::ok)
            }
            // No vtable slot carries a cancel; a later `done` is dropped.
            Op::Cancel { token } => self
                .state
                .borrow_mut()
                .pending
                .cancel(token)
                .map(|()| Reply::ok(Out::Unit)),
            Op::Since { cursor } => since(self, caller, cursor),
            Op::Info { rd } => self
                .state
                .borrow()
                .table
                .info(rd)
                .map(|i| Reply::ok(Out::Info(i))),
            Op::Parent { rd } => parent(self, rd),
            Op::Children { rd } => children(self, rd),
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
            ("open", Some(*rd), alloc::format!(" rights={}", asked(*rights)))
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
        Op::Ask { .. } | Op::Cancel { .. } | Op::Since { .. } => return None,
    };
    let rd = rd?;
    if untraced(rd) {
        return None;
    }
    Some(alloc::format!("op={name} caller={} rd={}{rest}", caller.0, rd.0))
}

/// The rights a handle was asked for, as a listing writes a cell's.
const fn asked(rights: Rights) -> &'static str {
    match (rights.contains(Rights::READ), rights.contains(Rights::WRITE)) {
        (true, true) => "rw",
        (true, false) => "r",
        (false, true) => "w",
        (false, false) => "-",
    }
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

fn lookup(k: &Kernel, rd: Rd, name: &str) -> Answer {
    let s = k.state.borrow();
    let owner = s.table.get(rd)?.owner;
    if let Some(child) = s.table.child(rd, name) {
        return Ok(Reply::ok(Out::Rd(child)));
    }
    drop(s);
    let Owner::Driver { module, driver } = owner else {
        return Err(Error::Invalid);
    };
    let (_, d) = k.served(module, driver)?;
    let child = ready_only(d.lookup(&k.calls(module), rd, name))?;
    Ok(Reply::ok(Out::Rd(child)))
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
    let (served, cookie) = match owner {
        Owner::Kernel(_) => (None, 0),
        Owner::Driver { module, driver } => {
            let (index, d) = k.served(module, driver)?;
            let cookie = ready_only(d.open(&k.calls(module), rd, rights))?;
            (Some((module, index)), cookie)
        }
    };
    let mut s = k.state.borrow_mut();
    let h = Handle(s.next_handle);
    s.next_handle += 1;
    let slot = Slot {
        caller,
        rd,
        rights,
        served,
        cookie,
        pending: 0,
        watching: false,
    };
    s.handles.insert(h, slot);
    // A core is one moment: the handle takes the text here and serves
    // it, so reading it in chunks reads one Kernel and not several.
    if owner == Owner::Kernel(Node::Core) {
        let text = core(&s);
        s.cores.insert(h, text);
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
    // A core and the heap line are made at the read out of what the
    // Kernel already holds; reading one leaves nothing behind, atime
    // included.
    if matches!(reply, Reply::Ready(Ok(_))) && !momentary(slot.rd) {
        let now = k.now();
        if let Ok(entry) = k.state.borrow_mut().table.get_mut(slot.rd) {
            entry.atime = now;
        }
    }
    finish(k, h, slot.served, reply, Out::Bytes)
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
        None => Reply::Ready(kernel_write(k, caller, slot.rd, at, bytes)),
        Some((module, index)) => {
            k.driver(index).write(&k.calls(module), slot.cookie, at, bytes)
        }
    };
    let len = |n| Out::Len(u32::try_from(n).unwrap_or(u32::MAX));
    finish(k, h, slot.served, reply, len)
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
    check_call(&k.state.borrow(), &slot, api, method, input)?;
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
    finish(k, h, slot.served, reply, Out::Bytes)
}

fn watch(k: &Kernel, caller: Caller, h: Handle, on: bool) -> Answer {
    let slot = k.slot(caller, h, Rights::READ)?;
    if slot.watching != on {
        set_watch(&mut k.state.borrow_mut(), caller, h, slot.rd, on);
    }
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
        s.cores.remove(&h);
        if slot.watching {
            unwatch(&mut s, caller, slot.rd);
        }
        slot
    };
    if let Some((module, index)) = slot.served {
        // The handle is gone whatever the Driver says.
        let _ = k.driver(index).close(&k.calls(module), slot.cookie);
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
    Ok(Reply::ok(Out::Events { events, cursor }))
}

fn parent(k: &Kernel, rd: Rd) -> Answer {
    let parent = k.state.borrow().table.get(rd)?.parent;
    Ok(Reply::ok(Out::Rd(parent.ok_or(Error::Invalid)?)))
}

fn children(k: &Kernel, rd: Rd) -> Answer {
    let first_time = {
        let mut s = k.state.borrow_mut();
        let entry = s.table.get_mut(rd)?;
        let first = !entry.listed;
        entry.listed = true;
        first.then_some(entry.owner)
    };
    if let Some(Owner::Driver { module, driver }) = first_time {
        // The cache is what `Children` answers from; the reply itself
        // carries nothing.
        let (_, d) = k.served(module, driver)?;
        let _ = d.lookup(&k.calls(module), rd, "");
    }
    let children = k.state.borrow().table.get(rd)?.children.clone();
    Ok(Reply::ok(Out::Rds(children)))
}

/// The rd's flags cap the asked rights; asking nothing is `Denied` too.
fn allows(flags: Flags, rights: Rights) -> bool {
    !rights.is_empty()
        && Flags::from_bits(rights.bits())
            .is_some_and(|asked| flags.contains(asked))
}

/// `Busy` when the handle is at its Pending cap.
fn room(k: &Kernel, slot: &Slot) -> Result<(), Error> {
    if slot.pending >= k.limits().pending_per_handle {
        Err(Error::Busy)
    } else {
        Ok(())
    }
}

fn check_call(
    s: &State,
    slot: &Slot,
    api: Hash,
    method: u16,
    input: &[u8],
) -> Result<(), Error> {
    let entry = s.table.get(slot.rd)?;
    if !entry.apis.contains(&api) {
        return Err(Error::Unsupported);
    }
    let decl = s.decls.get(&api).ok_or(Error::Unsupported)?;
    let method = decl.method(method).ok_or(Error::Unsupported)?;
    if !slot.rights.contains(method.right) {
        return Err(Error::Denied);
    }
    if !value::fits(input, &method.input) {
        return Err(Error::Invalid);
    }
    Ok(())
}

/// Turn a slot's answer into the caller's: a Pending token is scoped by
/// the Module and counted on the handle.
fn finish<T>(
    k: &Kernel,
    h: Handle,
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
    let token = s.pending.open(module, own, h)?;
    if let Some(slot) = s.handles.get_mut(&h) {
        slot.pending += 1;
    }
    Ok(Reply::Pending(token))
}

/// Only `read`, `write` and `call` may go Pending.
fn ready_only<T>(reply: Reply<T>) -> Result<T, Error> {
    match reply {
        Reply::Ready(result) => result,
        Reply::Pending(_) => Err(Error::Unsupported),
    }
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

/// The Kernel-served reads: declaration bytes, the log, a parameter,
/// the heap and the core the handle took when it opened.
fn kernel_read(
    k: &Kernel,
    h: Handle,
    rd: Rd,
    at: u64,
    len: usize,
) -> Result<Vec<u8>, Error> {
    let s = k.state.borrow();
    let Owner::Kernel(node) = s.table.get(rd)?.owner else {
        return Err(Error::Invalid);
    };
    match node {
        Node::Decl(hash) => {
            let bytes = s.decls.get(&hash).ok_or(Error::Gone)?.canonical();
            Ok(part(&bytes, at, len))
        }
        Node::Log => Ok(s.log.read(at, len)),
        Node::Level => Ok(part(log::name(s.log.level()).as_bytes(), at, len)),
        Node::Limit(param) => {
            let value = alloc::format!("{}", s.limits.get(param));
            Ok(part(value.as_bytes(), at, len))
        }
        Node::Mem => Ok(part(mem(s.heap()).as_bytes(), at, len)),
        Node::Core => Ok(part(s.cores.get(&h).ok_or(Error::Gone)?, at, len)),
        Node::Cell => {
            let bytes = s.cells.get(&rd).ok_or(Error::Gone)?;
            Ok(part(bytes, at, len))
        }
        Node::Container | Node::Home(_) => Err(Error::Unsupported),
    }
}

/// Up to `len` bytes from `at`, or as many as there are.
fn part(bytes: &[u8], at: u64, len: usize) -> Vec<u8> {
    let start = usize::try_from(at).unwrap_or(usize::MAX).min(bytes.len());
    let end = start.saturating_add(len).min(bytes.len());
    bytes[start..end].to_vec()
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
