#![allow(unsafe_code)]
//! The adapter: a C descriptor as `Module` and `Driver`, and the seven
//! calls handed down as an `rv_calls` that lives for one slot call.
//! The only place this workspace calls into C.

use alloc::collections::BTreeSet;
use alloc::ffi::CString;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::cell::RefCell;
use core::ffi::{CStr, c_char, c_void};
use core::ptr;

use reovim_kernel_interface::wire::{self, Message};
use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Hash, Kind, Module,
    Out, Rd, Reply, Rights, Token,
};

use crate::{
    ABI, Ctx, MAGIC, PENDING, RvBytes, RvCalls, RvDriver, RvModule, RvReply,
    error_of,
};

// SAFETY: a descriptor and its tables are immutable data, so a `static`
// one is what a C author writes; what `ctx` points at is theirs to guard,
// and the Kernel runs on one thread.
unsafe impl Sync for RvDriver {}
// SAFETY: as above.
unsafe impl Sync for RvModule {}

/// Most bytes one `read` asks a C Driver for.
const READ_MAX: usize = 1 << 20;

/// A C Module: its descriptor and what keeps it loaded.
pub struct Native {
    table: *const RvModule,
    _keep: Rc<dyn Any>,
}

/// One of its Drivers.
struct NativeDriver {
    table: *const RvDriver,
    /// Own tokens of writes answered Pending; `done` delivers a count.
    writes: RefCell<BTreeSet<u64>>,
    _keep: Rc<dyn Any>,
}

/// What `rv_calls.ctx` points at during one slot call.
struct Handed<'a> {
    calls: &'a dyn Calls,
    writes: Option<&'a RefCell<BTreeSet<u64>>>,
    /// The last `submit` reply, alive until the next.
    reply: RefCell<Vec<u8>>,
}

/// `module` as a Module and its Drivers, once magic and abi check out;
/// `keep` lives as long as they do.
///
/// # Errors
/// `Invalid` for a wrong magic or abi, a null descriptor or a name
/// that is not UTF-8.
///
/// # Safety
/// `module` must point at a descriptor that stays valid, with its
/// tables and name, while `keep` is alive, and every function it names
/// must follow reovim.h.
pub unsafe fn boot(
    module: *const RvModule,
    keep: Rc<dyn Any>,
) -> Result<(String, Booted), Error> {
    // SAFETY: the caller's contract; null is checked here.
    let table = unsafe { module.as_ref() }.ok_or(Error::Invalid)?;
    if table.magic != MAGIC || table.abi != ABI {
        return Err(Error::Invalid);
    }
    // SAFETY: the descriptor's name, as the contract says.
    let name = unsafe { str_of(table.name) }?.ok_or(Error::Invalid)?;
    let drivers = (0..table.driver_count)
        .map(|i| {
            // SAFETY: `drivers` holds `driver_count` tables.
            let table = unsafe { table.drivers.add(i) };
            let writes = RefCell::new(BTreeSet::new());
            let driver = NativeDriver { table, writes, _keep: keep.clone() };
            Rc::new(driver) as Rc<dyn Driver>
        })
        .collect();
    let module: Rc<dyn Module> = Rc::new(Native { table: module, _keep: keep });
    Ok((String::from(name), Booted { module, drivers }))
}

/// A C string as `&str`; null is `None`.
///
/// # Safety
/// `p` is null or NUL-terminated and alive for `'a`.
unsafe fn str_of<'a>(p: *const c_char) -> Result<Option<&'a str>, Error> {
    if p.is_null() {
        return Ok(None);
    }
    // SAFETY: as above.
    let text =
        unsafe { CStr::from_ptr(p) }.to_str().map_err(|_| Error::Invalid)?;
    Ok(Some(text))
}

/// `rv_bytes` as a slice.
///
/// # Safety
/// `b` is none or points at `len` readable bytes alive for `'a`.
const unsafe fn slice_of<'a>(b: RvBytes) -> &'a [u8] {
    if b.ptr.is_null() {
        return &[];
    }
    // SAFETY: as above.
    unsafe { core::slice::from_raw_parts(b.ptr, b.len) }
}

/// The `Handed` behind an `rv_calls.ctx`.
///
/// # Safety
/// `p` came from `table_of` during the slot call still running.
const unsafe fn handed<'a>(p: Ctx) -> &'a Handed<'a> {
    // SAFETY: as above.
    unsafe { &*p.cast::<Handed<'a>>().cast_const() }
}

const fn answer(result: Result<u64, Error>) -> RvReply {
    match result {
        Ok(value) => RvReply::ok(value),
        Err(error) => RvReply::err(error),
    }
}

fn reply<T>(r: RvReply, f: impl FnOnce(u64) -> Result<T, Error>) -> Reply<T> {
    match r.status {
        0 => Reply::Ready(f(r.value)),
        PENDING => Reply::Pending(Token(r.value)),
        status => Reply::err(error_of(status)),
    }
}

unsafe extern "C" fn register(
    p: Ctx,
    driver: u32,
    parent: u64,
    name: *const c_char,
) -> RvReply {
    // SAFETY: `p` and `name` are what reovim.h says for this call.
    let (h, name) = unsafe { (handed(p), str_of(name)) };
    let name = match name {
        Ok(name) => name,
        Err(error) => return RvReply::err(error),
    };
    answer(h.calls.register(DriverId(driver), Rd(parent), name).map(|rd| rd.0))
}

unsafe extern "C" fn name(p: Ctx, rd: u64, name: *const c_char) -> RvReply {
    // SAFETY: as `register`.
    let (h, name) = unsafe { (handed(p), str_of(name)) };
    let name = match name {
        Ok(name) => name,
        Err(error) => return RvReply::err(error),
    };
    answer(h.calls.name(Rd(rd), name).map(|()| 0))
}

unsafe extern "C" fn report(
    p: Ctx,
    rd: u64,
    flags: u32,
    apis: *const RvBytes,
    count: usize,
) -> RvReply {
    // SAFETY: `apis` holds `count` entries, each as `slice_of` needs.
    let (h, apis) = unsafe {
        let apis = if count == 0 {
            &[]
        } else {
            core::slice::from_raw_parts(apis, count)
        };
        (handed(p), apis)
    };
    let decls: Result<Vec<Decl>, Error> = apis
        .iter()
        // SAFETY: as above.
        .map(|b| Decl::parse(unsafe { slice_of(*b) }))
        .collect();
    let flags = Flags::from_bits(flags).ok_or(Error::Invalid);
    let result = decls.and_then(|decls| h.calls.report(Rd(rd), flags?, &decls));
    answer(result.map(|()| 0))
}

unsafe extern "C" fn unregister(p: Ctx, rd: u64) -> RvReply {
    // SAFETY: as `register`.
    let h = unsafe { handed(p) };
    answer(h.calls.unregister(Rd(rd)).map(|()| 0))
}

unsafe extern "C" fn changed(p: Ctx, rd: u64, kind: u32) -> RvReply {
    // SAFETY: as `register`.
    let h = unsafe { handed(p) };
    let kind = match kind {
        0 => Kind::Modify,
        1 => Kind::Attrib,
        _ => return RvReply::err(Error::Invalid),
    };
    answer(h.calls.changed(Rd(rd), kind).map(|()| 0))
}

unsafe extern "C" fn done(
    p: Ctx,
    token: u64,
    status: u32,
    out: RvBytes,
) -> RvReply {
    // SAFETY: as `register`; `out` as `slice_of` needs.
    let (h, out) = unsafe { (handed(p), slice_of(out)) };
    let write = h.writes.is_some_and(|w| w.borrow_mut().remove(&token));
    let outcome = match (status, write) {
        (0, true) => Ok(Out::Len(u32::try_from(out.len()).unwrap_or(u32::MAX))),
        (0, false) => Ok(Out::Bytes(out.to_vec())),
        (status, _) => Err(error_of(status)),
    };
    answer(h.calls.done(Token(token), outcome).map(|()| 0))
}

unsafe extern "C" fn open(p: Ctx, rd: u64, rights: u32) -> RvReply {
    // SAFETY: as `register`.
    let h = unsafe { handed(p) };
    let rights = Rights::from_bits(rights).ok_or(Error::Invalid);
    answer(rights.and_then(|r| h.calls.open(Rd(rd), r)).map(|h| h.0))
}

unsafe extern "C" fn submit(p: Ctx, op: RvBytes, out: *mut RvBytes) -> RvReply {
    // SAFETY: as `register`; `op` as `slice_of` needs.
    let (h, op) = unsafe { (handed(p), slice_of(op)) };
    let Some((_, Message::Request(op))) = wire::decode(op) else {
        return RvReply::err(Error::Invalid);
    };
    let bytes = wire::encode(0, &Message::Reply(h.calls.submit(op)));
    let mut reply = h.reply.borrow_mut();
    *reply = bytes;
    if !out.is_null() {
        // SAFETY: `out` is the caller's writable `rv_bytes`.
        unsafe { *out = RvBytes::of(&reply) };
    }
    RvReply::ok(0)
}

/// The `rv_calls` over `h`, valid while `h` is.
fn table_of(h: &Handed<'_>) -> RvCalls {
    RvCalls {
        ctx: ptr::from_ref(h).cast_mut().cast::<c_void>(),
        register_: Some(register),
        name: Some(name),
        report: Some(report),
        unregister: Some(unregister),
        changed: Some(changed),
        done: Some(done),
        open: Some(open),
        submit: Some(submit),
    }
}

impl Module for Native {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        // SAFETY: the descriptor lives while `_keep` does.
        let table = unsafe { &*self.table };
        let Some(start) = table.start else { return Err(Error::Unsupported) };
        let h = Handed {
            calls: ctx,
            writes: None,
            reply: RefCell::new(Vec::new()),
        };
        let k = table_of(&h);
        // SAFETY: `k` lives for this call, as reovim.h says.
        let r = unsafe { start(table.ctx, &raw const k, home.0) };
        reply(r, |_| Ok(())).into_result()
    }

    fn stop(&self) {
        // SAFETY: as `start`.
        let table = unsafe { &*self.table };
        if let Some(stop) = table.stop {
            // SAFETY: as `start`.
            unsafe { stop(table.ctx) };
        }
    }
}

trait IntoResult<T> {
    fn into_result(self) -> Result<T, Error>;
}

impl<T> IntoResult<T> for Reply<T> {
    fn into_result(self) -> Result<T, Error> {
        match self {
            Self::Ready(result) => result,
            Self::Pending(_) => Err(Error::Invalid),
        }
    }
}

impl NativeDriver {
    /// One slot call with the calls handed down; `None` for an empty
    /// slot.
    fn with(
        &self,
        ctx: &dyn Calls,
        slot: impl FnOnce(&RvDriver, *const RvCalls) -> Option<RvReply>,
    ) -> Option<RvReply> {
        // SAFETY: the table lives while `_keep` does.
        let table = unsafe { &*self.table };
        let writes = Some(&self.writes);
        let h = Handed { calls: ctx, writes, reply: RefCell::new(Vec::new()) };
        let k = table_of(&h);
        slot(table, &raw const k)
    }
}

/// What a slot answered; an empty slot is `Unsupported`.
fn slot<T>(
    r: Option<RvReply>,
    f: impl FnOnce(u64) -> Result<T, Error>,
) -> Reply<T> {
    r.map_or(Reply::err(Error::Unsupported), |r| reply(r, f))
}

fn count(n: u64) -> Result<usize, Error> {
    usize::try_from(n).map_err(|_| Error::Invalid)
}

impl Driver for NativeDriver {
    fn lookup(&self, ctx: &dyn Calls, rd: Rd, name: &str) -> Reply<Rd> {
        let Ok(name) = CString::new(name) else {
            return Reply::err(Error::Invalid);
        };
        // SAFETY: the slot's contract; `name` lives for the call.
        let r = self.with(ctx, |t, k| {
            t.lookup.map(|f| unsafe { f(t.ctx, k, rd.0, name.as_ptr()) })
        });
        slot(r, |v| Ok(Rd(v)))
    }

    fn open(&self, ctx: &dyn Calls, rd: Rd, rights: Rights) -> Reply<u64> {
        // SAFETY: the slot's contract.
        let r = self.with(ctx, |t, k| {
            t.open.map(|f| unsafe { f(t.ctx, k, rd.0, rights.bits()) })
        });
        slot(r, Ok)
    }

    fn read(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        let mut buf = alloc::vec![0u8; len.min(READ_MAX)];
        let (ptr, len) = (buf.as_mut_ptr(), buf.len());
        // SAFETY: `buf` is writable for `len` bytes during the call.
        let r = self.with(ctx, |t, k| {
            t.read.map(|f| unsafe { f(t.ctx, k, h, at, ptr, len) })
        });
        match slot(r, count) {
            Reply::Ready(Ok(n)) if n <= len => {
                buf.truncate(n);
                Reply::ok(buf)
            }
            Reply::Ready(_) => Reply::err(Error::Invalid),
            Reply::Pending(token) => Reply::Pending(token),
        }
    }

    fn write(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        buf: &[u8],
    ) -> Reply<usize> {
        // SAFETY: the slot's contract; `buf` lives for the call.
        let r = self.with(ctx, |t, k| {
            t.write.map(|f| unsafe { f(t.ctx, k, h, at, RvBytes::of(buf)) })
        });
        let got = slot(r, count);
        if let Reply::Pending(token) = got {
            self.writes.borrow_mut().insert(token.0);
        }
        got
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        api: Hash,
        method: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        let mut out = RvBytes::NONE;
        let (input, o) = (RvBytes::of(input), &raw mut out);
        // SAFETY: the slot's contract; `out` is ours to be written.
        let r = self.with(ctx, |t, k| {
            t.call.map(|f| unsafe { f(t.ctx, k, h, api.0, method, input, o) })
        });
        // SAFETY: `out` is Driver-owned and alive until the next slot call.
        slot(r, |_| Ok(unsafe { slice_of(out) }.to_vec()))
    }

    fn watch(&self, ctx: &dyn Calls, rd: Rd, on: bool) -> Reply<()> {
        // SAFETY: the slot's contract.
        let r = self.with(ctx, |t, k| {
            t.watch.map(|f| unsafe { f(t.ctx, k, rd.0, u32::from(on)) })
        });
        slot(r, |_| Ok(()))
    }

    fn close(&self, ctx: &dyn Calls, h: u64) -> Reply<()> {
        // SAFETY: the slot's contract.
        let r =
            self.with(ctx, |t, k| t.close.map(|f| unsafe { f(t.ctx, k, h) }));
        slot(r, |_| Ok(()))
    }
}
