#![allow(unsafe_code)]
//! A descriptor built in Rust, as a C author would build one, driven
//! through a real Kernel: every slot and every call at least once.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use reovim_kernel_abi::{
    ABI, Ctx, MAGIC, RvBytes, RvCalls, RvDriver, RvModule, RvReply, boot,
};
use reovim_kernel_interface::wire::{self, Message};
use reovim_kernel_interface::{
    Caller, Decl, Error, Handle, Kind, Method, Op, Out, Rd, Reply, Rights, Ty,
};
use reovim_kernel_runtime::{Heap, Kernel, Limits, ROOT};

/// The counter, its rd and the token of a call answered pending.
static COUNT: AtomicU64 = AtomicU64::new(0);
static RD: AtomicU64 = AtomicU64::new(0);
static LATER: AtomicU64 = AtomicU64::new(0);
static STOPPED: AtomicU64 = AtomicU64::new(0);
/// What `call` answers through `out`.
static mut OUT: [u8; 32] = [0; 32];

fn decl() -> Decl {
    let m = |number, name: &str, output| Method {
        number,
        name: name.to_string(),
        input: Ty::Unit,
        output,
        right: Rights::READ,
    };
    Decl {
        api: "counter".to_string(),
        ver: 1,
        methods: vec![
            m(0, "later", Ty::Bytes(32)),
            m(1, "info", Ty::Bytes(32)),
            m(2, "rename", Ty::Unit),
            m(3, "bye", Ty::Unit),
        ],
    }
}

const unsafe fn k<'a>(k: *const RvCalls) -> &'a RvCalls {
    unsafe { &*k }
}

unsafe extern "C" fn start(
    _: Ctx,
    calls: *const RvCalls,
    home: u64,
) -> RvReply {
    let k = unsafe { k(calls) };
    let rd =
        unsafe { (k.register_.unwrap())(k.ctx, 0, home, c"count".as_ptr()) };
    if rd.status != 0 {
        return rd;
    }
    RD.store(rd.value, Ordering::Relaxed);
    let bytes = decl().canonical();
    let api = RvBytes::of(&bytes);
    unsafe { (k.report.unwrap())(k.ctx, rd.value, 3, &raw const api, 1) }
}

unsafe extern "C" fn stop(_: Ctx) {
    STOPPED.store(1, Ordering::Relaxed);
}

const unsafe extern "C" fn open(
    _: Ctx,
    _: *const RvCalls,
    rd: u64,
    _: u32,
) -> RvReply {
    RvReply::ok(rd)
}

unsafe extern "C" fn read(
    _: Ctx,
    _: *const RvCalls,
    _: u64,
    at: u64,
    buf: *mut u8,
    len: usize,
) -> RvReply {
    if at > 0 {
        return RvReply::ok(0);
    }
    let text = COUNT.load(Ordering::Relaxed).to_string();
    let n = text.len().min(len);
    unsafe { buf.copy_from_nonoverlapping(text.as_ptr(), n) };
    RvReply::ok(n as u64)
}

unsafe extern "C" fn write(
    _: Ctx,
    calls: *const RvCalls,
    _: u64,
    _: u64,
    buf: RvBytes,
) -> RvReply {
    let k = unsafe { k(calls) };
    let text = unsafe { std::slice::from_raw_parts(buf.ptr, buf.len) };
    let Ok(n) = std::str::from_utf8(text).unwrap_or("x").parse::<u64>() else {
        return RvReply::err(Error::Invalid);
    };
    COUNT.store(n, Ordering::Relaxed);
    let rd = RD.load(Ordering::Relaxed);
    unsafe { (k.changed.unwrap())(k.ctx, rd, 0) };
    let later = LATER.swap(0, Ordering::Relaxed);
    if later != 0 {
        unsafe { (k.done.unwrap())(k.ctx, later, 0, RvBytes::of(b"later")) };
    }
    RvReply::ok(buf.len as u64)
}

/// `submit` an `Info` of our rd and answer `ok` when it decodes.
unsafe fn info(k: &RvCalls) -> &'static [u8] {
    let op = Op::Info { rd: Rd(RD.load(Ordering::Relaxed)) };
    let request = wire::encode(0, &Message::Request(op));
    let mut reply = RvBytes::NONE;
    let r = unsafe {
        (k.submit.unwrap())(k.ctx, RvBytes::of(&request), &raw mut reply)
    };
    let bytes = unsafe { std::slice::from_raw_parts(reply.ptr, reply.len) };
    match (r.status, wire::decode(bytes)) {
        (0, Some((0, Message::Reply(Reply::Ready(Ok(Out::Info(_))))))) => b"ok",
        _ => b"bad",
    }
}

unsafe extern "C" fn call(
    _: Ctx,
    calls: *const RvCalls,
    _: u64,
    _: u64,
    method: u16,
    _: RvBytes,
    out: *mut RvBytes,
) -> RvReply {
    let k = unsafe { k(calls) };
    let rd = RD.load(Ordering::Relaxed);
    let answer: &[u8] = match method {
        0 => {
            LATER.store(7, Ordering::Relaxed);
            return RvReply::pending(7);
        }
        1 => unsafe { info(k) },
        2 => {
            return unsafe {
                (k.name.unwrap())(k.ctx, rd, c"counter".as_ptr())
            };
        }
        _ => return unsafe { (k.unregister.unwrap())(k.ctx, rd) },
    };
    unsafe {
        OUT[..answer.len()].copy_from_slice(answer);
        *out =
            RvBytes { ptr: (&raw const OUT).cast::<u8>(), len: answer.len() };
    }
    RvReply::ok(0)
}

/// A Driver may open and close another rd itself: our own, here.
unsafe extern "C" fn watch(
    _: Ctx,
    calls: *const RvCalls,
    _: u64,
    _: u32,
) -> RvReply {
    let k = unsafe { k(calls) };
    let h = unsafe { (k.open.unwrap())(k.ctx, RD.load(Ordering::Relaxed), 1) };
    if h.status != 0 {
        return h;
    }
    let close = Op::Close { h: Handle(h.value) };
    let op = wire::encode(0, &Message::Request(close));
    let mut reply = RvBytes::NONE;
    unsafe { (k.submit.unwrap())(k.ctx, RvBytes::of(&op), &raw mut reply) }
}

const unsafe extern "C" fn close(_: Ctx, _: *const RvCalls, _: u64) -> RvReply {
    RvReply::ok(0)
}

static DRIVER: RvDriver = RvDriver {
    ctx: std::ptr::null_mut(),
    lookup: None,
    open: Some(open),
    read: Some(read),
    write: Some(write),
    call: Some(call),
    watch: Some(watch),
    close: Some(close),
};

static MODULE: RvModule = RvModule {
    magic: MAGIC,
    abi: ABI,
    name: c"native".as_ptr(),
    ctx: std::ptr::null_mut(),
    start: Some(start),
    stop: Some(stop),
    drivers: &raw const DRIVER,
    driver_count: 1,
};

struct Rig {
    kernel: Kernel,
    caller: Caller,
    seq: Cell<u64>,
}

impl Rig {
    fn submit(&self, op: Op) -> Reply<Out> {
        self.seq.set(self.seq.get() + 1);
        self.kernel.submit(self.caller, self.seq.get(), op)
    }

    fn ok(&self, op: Op) -> Out {
        match self.submit(op) {
            Reply::Ready(Ok(out)) => out,
            other => panic!("{other:?}"),
        }
    }

    fn resolve(&self, path: &str) -> Result<Rd, Error> {
        path.split('/').filter(|n| !n.is_empty()).try_fold(ROOT, |rd, n| {
            match self.submit(Op::Lookup { rd, name: n.to_string() }) {
                Reply::Ready(Ok(Out::Rd(rd))) => Ok(rd),
                Reply::Ready(Err(e)) => Err(e),
                other => panic!("{other:?}"),
            }
        })
    }

    fn bytes(reply: Reply<Out>) -> Vec<u8> {
        match reply {
            Reply::Ready(Ok(Out::Bytes(bytes))) => bytes,
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn a_descriptor_serves_through_the_kernel() {
    let bad = RvModule { abi: ABI + 1, ..MODULE };
    let refused = unsafe { boot(&raw const bad, Rc::new(())) };
    assert!(matches!(refused, Err(Error::Invalid)));

    let (name, booted) =
        unsafe { boot(&raw const MODULE, Rc::new(())) }.unwrap();
    assert_eq!(name, "native");
    let kernel = Kernel::new(|| 1, Heap::default, Limits::default());
    kernel.load(&name, booted.module, booted.drivers).unwrap();
    let caller = kernel.caller_new();
    let rig = Rig { kernel, caller, seq: Cell::new(0) };

    let count = rig.resolve("/module/native/count").unwrap();
    let h = match rig
        .ok(Op::Open { rd: count, rights: Rights::READ | Rights::WRITE })
    {
        Out::Handle(h) => h,
        other => panic!("{other:?}"),
    };
    rig.ok(Op::Watch { h, on: true });
    let cursor = match rig.ok(Op::Since { cursor: 0 }) {
        Out::Events { cursor, .. } => cursor,
        other => panic!("{other:?}"),
    };
    assert_eq!(Rig::bytes(rig.submit(Op::Read { h, at: 0, len: 16 })), b"0");
    assert_eq!(
        rig.submit(Op::Write { h, at: 0, bytes: b"5".to_vec() }),
        Reply::ok(Out::Len(1))
    );
    assert_eq!(Rig::bytes(rig.submit(Op::Read { h, at: 0, len: 16 })), b"5");
    let Out::Events { events, .. } = rig.ok(Op::Since { cursor }) else {
        panic!("since")
    };
    assert_eq!(events[0].kind, Kind::Modify);

    let api = decl().hash();
    let call = |h, method| Op::Call { h, api, method, input: Vec::new() };
    let Reply::Pending(token) = rig.submit(call(h, 0)) else {
        panic!("pending")
    };
    assert_eq!(rig.submit(Op::Ask { token }), Reply::err(Error::Busy));
    rig.ok(Op::Write { h, at: 0, bytes: b"6".to_vec() });
    assert_eq!(Rig::bytes(rig.submit(Op::Ask { token })), b"later");
    assert_eq!(Rig::bytes(rig.submit(call(h, 1))), b"ok");
    assert_eq!(Rig::bytes(rig.submit(call(h, 2))), b"");
    assert!(rig.resolve("/module/native/counter").is_ok());
    assert_eq!(
        rig.submit(Op::Write { h, at: 0, bytes: b"x".to_vec() }),
        Reply::err(Error::Invalid)
    );
    rig.ok(Op::Close { h });

    let h = match rig.ok(Op::Open { rd: count, rights: Rights::READ }) {
        Out::Handle(h) => h,
        other => panic!("{other:?}"),
    };
    assert_eq!(Rig::bytes(rig.submit(call(h, 3))), b"");
    assert_eq!(
        rig.submit(Op::Read { h, at: 0, len: 16 }),
        Reply::err(Error::Gone)
    );
    assert_eq!(rig.resolve("/module/native/counter"), Err(Error::Invalid));
    assert_eq!(rig.kernel.unload("native"), Err(Error::Busy));
    rig.ok(Op::Close { h });
    assert_eq!(rig.kernel.unload("native"), Ok(()));
    assert_eq!(STOPPED.load(Ordering::Relaxed), 1);
}
