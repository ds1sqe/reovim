//! Caller ops: right and declaration checks, then the vtable.

use alloc::vec::Vec;

use reovim_kernel_interface::{
    Caller, Error, Flags, Handle, Hash, ModuleId, Op, Out, Rd, Reply, Rights,
    value,
};

use crate::kernel::{Kernel, Slot, State};
use crate::table::{Node, Owner};

/// An op's answer; `Err` becomes `Ready(Err)` at the caller.
type Answer = Result<Reply<Out>, Error>;

impl Kernel {
    /// Run one op as `caller`, outside the reply window.
    pub(crate) fn run(&self, caller: Caller, op: Op) -> Reply<Out> {
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
    Ok(Reply::ok(Out::Handle(h)))
}

fn read(k: &Kernel, caller: Caller, h: Handle, at: u64, len: u32) -> Answer {
    let slot = k.slot(caller, h, Rights::READ)?;
    room(k, &slot)?;
    let len = len as usize;
    let reply = match slot.served {
        None => Reply::Ready(kernel_read(k, slot.rd, at, len)),
        Some((module, index)) => {
            k.driver(index).read(&k.calls(module), slot.cookie, at, len)
        }
    };
    if matches!(reply, Reply::Ready(Ok(_))) {
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
        None => Reply::Ready(kernel_write(k, caller, slot.rd, bytes)),
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
    stamp(k, slot.rd, api, method, "dispatch");
    let reply =
        k.driver(index).call(&k.calls(module), slot.cookie, api, method, input);
    stamp(k, slot.rd, api, method, "reply");
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
    let s = k.state.borrow();
    s.callers.get(&caller).ok_or(Error::Invalid)?;
    let (events, cursor) = s.events.since(cursor, &s.watched(caller));
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
const fn room(k: &Kernel, slot: &Slot) -> Result<(), Error> {
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

/// The Kernel-served reads: declaration bytes and the log.
fn kernel_read(
    k: &Kernel,
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
            let start =
                usize::try_from(at).unwrap_or(usize::MAX).min(bytes.len());
            let end = start.saturating_add(len).min(bytes.len());
            Ok(bytes[start..end].to_vec())
        }
        Node::Log => Ok(s.log.read(at, len)),
        Node::Container | Node::Home(_) => Err(Error::Unsupported),
    }
}

/// The one Kernel-served write: a log entry, marked by the caller.
fn kernel_write(
    k: &Kernel,
    caller: Caller,
    rd: Rd,
    bytes: &[u8],
) -> Result<usize, Error> {
    let now = k.now();
    let mut s = k.state.borrow_mut();
    if s.table.get(rd)?.owner != Owner::Kernel(Node::Log) {
        return Err(Error::Unsupported);
    }
    let mark = alloc::string::String::from(s.mark(caller));
    s.log(now, &mark, bytes.to_vec());
    Ok(bytes.len())
}

/// The Kernel measures every `call` with a pair of log entries.
fn stamp(k: &Kernel, rd: Rd, api: Hash, method: u16, phase: &str) {
    let now = k.now();
    let line = alloc::format!(
        "call rd={} api={:016x} method={method} {phase}",
        rd.0,
        api.0
    );
    k.state.borrow_mut().log(now, "kernel", line.into_bytes());
}
