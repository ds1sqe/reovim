//! `/sys` as sysfs: the five parameters under `/sys/limit`, each read
//! and written in decimal and applied where it is used; the five facts
//! under `/sys/info`, each made at the read and written by nobody;
//! `/sys/core`, everything the Kernel holds in one read that changes
//! nothing.

mod common;

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use common::{ok, tick, written};
use reovim_kernel_interface::{
    Calls, Decl, Driver, DriverId, Error, Flags, Method, Module, Op, Out, Rd,
    Reply, Rights, Ty,
};
use reovim_kernel_runtime::{
    CORE, Heap, Kernel, LIMITS, LOG, LOGLEVEL, Limits, MEM, ROOT, SYS,
};

/// What the stand-in host has handed out. The Kernel crate links no
/// allocator, so `/sys/info/mem` is tested against the one seam it
/// has: the function the host passes.
static LIVE: AtomicU64 = AtomicU64::new(64);

fn heap() -> Heap {
    let live = LIVE.load(Ordering::Relaxed);
    Heap { live, peak: live * 2, allocs: 7 }
}

/// A clock a case moves by hand, so a fact grows a digit where the case
/// says it does and not where the test happened to run.
static AT: AtomicU64 = AtomicU64::new(0);

fn at() -> u64 {
    AT.load(Ordering::Relaxed)
}

/// `op` by a caller of its own, which is how a case reads one cell.
fn alone(kernel: &Kernel, op: Op) -> Out {
    ok(kernel, kernel.caller_new(), 1, op)
}

/// The whole cell at `/sys/<path>`, as text.
fn cat(kernel: &Kernel, path: &str) -> String {
    let rd = resolve(kernel, path);
    let caller = kernel.caller_new();
    let Out::Handle(h) = ok(kernel, caller, 1, Op::Open { rd, rights: R })
    else {
        panic!("open {path}")
    };
    let mut out = Vec::new();
    let mut seq = 1;
    loop {
        seq += 1;
        let at = out.len() as u64;
        let read = Op::Read { h, at, len: 1 << 16 };
        let Out::Bytes(bytes) = ok(kernel, caller, seq, read) else {
            panic!("read {path}")
        };
        if bytes.is_empty() {
            break;
        }
        out.extend_from_slice(&bytes);
    }
    ok(kernel, caller, seq + 1, Op::Close { h });
    String::from_utf8(out).unwrap()
}

const R: Rights = Rights::READ;
const W: Rights = Rights::WRITE;

/// `/sys/<path>` by its segments.
fn resolve(kernel: &Kernel, path: &str) -> Rd {
    let mut rd = SYS;
    for name in path.split('/') {
        let Out::Rd(next) = alone(kernel, Op::Lookup { rd, name: name.into() })
        else {
            panic!("lookup {name}")
        };
        rd = next;
    }
    rd
}

/// `/sys/<path>` written `value` by a caller of its own; what the
/// Kernel answered, the refusal of the open included, since a cell
/// with no WRITE in its flags refuses the handle and not the bytes.
fn write(kernel: &Kernel, path: &str, value: &str) -> Reply<Out> {
    let rd = resolve(kernel, path);
    let caller = kernel.caller_new();
    let opened = kernel.submit(caller, 1, Op::Open { rd, rights: W });
    let Reply::Ready(Ok(Out::Handle(h))) = opened else { return opened };
    let bytes = value.as_bytes().to_vec();
    kernel.submit(caller, 2, Op::Write { h, at: 0, bytes })
}

/// `count` entries in `/sys/log`, so a ring shrunk below it trims.
fn fill(kernel: &Kernel, count: usize) {
    let caller = kernel.caller_new();
    let open = Op::Open { rd: LOG, rights: W };
    let Out::Handle(h) = ok(kernel, caller, 1, open) else { panic!("open") };
    for seq in 0..count {
        let bytes = format!("entry {seq}").into_bytes();
        ok(kernel, caller, seq as u64 + 2, Op::Write { h, at: 0, bytes });
    }
}

/// The `(level, text)` of every `/sys/log` entry now.
fn said(kernel: &Kernel) -> Vec<(u8, String)> {
    written(kernel).into_iter().map(|(_, level, t)| (level, t)).collect()
}

#[test]
fn a_limit_is_read_in_decimal_written_and_said_at_notice() {
    let kernel = Kernel::new(tick, heap, Limits::default());
    assert_eq!(cat(&kernel, "limit/log_ring"), "4096");
    assert_eq!(cat(&kernel, "limit/reentry"), "8");

    fill(&kernel, 20);
    assert_eq!(said(&kernel).len(), 20);
    // A smaller ring is true at once: the notice this very write
    // leaves is the append that trims the ring down to it.
    let written = write(&kernel, "limit/log_ring", "8");
    assert!(matches!(written, Reply::Ready(Ok(_))), "{written:?}");
    assert_eq!(cat(&kernel, "limit/log_ring"), "8");
    let lines = said(&kernel);
    assert_eq!(lines.len(), 8, "the ring keeps eight: {lines:?}");
    let notice = lines.iter().find(|(_, text)| text.starts_with("limit "));
    let (level, text) = notice.unwrap_or_else(|| panic!("{lines:?}"));
    assert_eq!(*level, 5, "at notice");
    assert!(
        text.starts_with("limit name=log_ring was=4096 now=8 caller="),
        "{text}"
    );
}

#[test]
fn a_parameter_the_kernel_cannot_hold_is_refused() {
    let kernel = Kernel::new(tick, heap, Limits::default());
    let refused = |path: &str, value: &str| {
        assert_eq!(
            write(&kernel, path, value),
            Reply::err(Error::Invalid),
            "{path}={value}"
        );
    };
    // Not a number, and a number the Kernel needs to be more than 0:
    // at 0 a ring keeps nothing and `reentry` refuses every op.
    refused("limit/reentry", "x");
    refused("limit/reentry", "0");
    refused("limit/reentry", "300");
    refused("limit/log_ring", "0");
    refused("limit/watch_ring", "-1");
    assert_eq!(cat(&kernel, "limit/reentry"), "8");
    assert_eq!(cat(&kernel, "limit/log_ring"), "4096");
    // A parameter is one word: a write past its front is not one.
    let rd = resolve(&kernel, "limit/log_ring");
    let caller = kernel.caller_new();
    let Out::Handle(h) = ok(&kernel, caller, 1, Op::Open { rd, rights: W })
    else {
        panic!("open")
    };
    let write = Op::Write { h, at: 1, bytes: b"8".to_vec() };
    assert_eq!(kernel.submit(caller, 2, write), Reply::err(Error::Invalid));
}

#[test]
fn the_level_is_a_parameter_and_says_so_at_notice() {
    let kernel = Kernel::new(tick, heap, Limits::default());
    assert!(matches!(write(&kernel, "loglevel", "debug"), Reply::Ready(Ok(_))));
    let lines = said(&kernel);
    let notice = lines.iter().find(|(_, text)| text.starts_with("loglevel "));
    let (level, text) = notice.unwrap_or_else(|| panic!("{lines:?}"));
    assert_eq!(*level, 5, "at notice");
    assert!(text.starts_with("loglevel was=info now=debug caller="), "{text}");
    // The line is a notice, not a trace: ops on the level stay quiet.
    let traced =
        |(_, text): &(u8, String)| text.contains(&format!("rd={}", LOGLEVEL.0));
    assert!(!lines.iter().any(traced), "{lines:?}");
}

#[test]
fn mem_is_what_the_host_counts_at_the_read() {
    let kernel = Kernel::new(tick, heap, Limits::default());
    LIVE.store(64, Ordering::Relaxed);
    assert_eq!(cat(&kernel, "info/mem"), "live=64 peak=128 allocs=7");
    LIVE.store(4096, Ordering::Relaxed);
    assert_eq!(cat(&kernel, "info/mem"), "live=4096 peak=8192 allocs=7");
    // The fact has one path: what stood loose under `/sys` is gone.
    let caller = kernel.caller_new();
    let look = Op::Lookup { rd: SYS, name: "mem".into() };
    assert_eq!(kernel.submit(caller, 1, look), Reply::err(Error::Invalid));
}

#[test]
fn the_one_clock_is_time_and_the_moment_it_began_is_uptime() {
    let kernel = Kernel::new(tick, heap, Limits::default());
    let Out::Info(root) = alone(&kernel, Op::Info { rd: ROOT }) else {
        panic!("info")
    };
    let number = |path: &str| cat(&kernel, path).parse::<u64>().unwrap();
    // The root is stamped with the clock the Kernel was made with, so
    // `uptime` is `time` less that, read a few ticks later.
    let time = number("info/time");
    let uptime = number("info/uptime");
    assert!(time > root.ctime, "the clock stands where the root was made");
    assert!(uptime >= time - root.ctime, "uptime is time less the ctime");
    // A step of the test clock apart, each is that step further on.
    assert!(number("info/time") > time, "time stood still");
    assert!(number("info/uptime") > uptime, "uptime stood still");
    // One word, the release this Kernel was built as.
    assert_eq!(cat(&kernel, "info/version"), env!("CARGO_PKG_VERSION"));
}

#[test]
fn a_fact_is_one_value_however_many_reads_it_takes() {
    AT.store(0, Ordering::Relaxed);
    let kernel = Kernel::new(at, heap, Limits::default());
    let rd = resolve(&kernel, "info/uptime");
    let caller = kernel.caller_new();
    let Out::Handle(h) = ok(&kernel, caller, 1, Op::Open { rd, rights: R })
    else {
        panic!("open")
    };
    let byte = |seq, at| {
        let read = Op::Read { h, at, len: 1 };
        match ok(&kernel, caller, seq, read) {
            Out::Bytes(bytes) => bytes,
            other => panic!("{other:?}"),
        }
    };
    // A byte at a time, as every reader that walks a cell reads it, the
    // Kernel a tick older between the two: the second chunk is the end
    // of the text the first came from, so the number the reader holds
    // is the one it began on and not `9` with the tail of `10` on it.
    AT.store(9, Ordering::Relaxed);
    let mut read = byte(2, 0);
    AT.store(10, Ordering::Relaxed);
    read.extend(byte(3, 1));
    assert_eq!(String::from_utf8(read).unwrap(), "9");
    ok(&kernel, caller, 4, Op::Close { h });
    // A reader that wants the value as it stands now opens again.
    assert_eq!(cat(&kernel, "info/uptime"), "10");
}

#[test]
fn a_write_takes_back_what_the_handle_read_before_it() {
    let kernel = Kernel::new(tick, heap, Limits::default());
    let rd = resolve(&kernel, "loglevel");
    let caller = kernel.caller_new();
    let open = Op::Open { rd, rights: R | W };
    let Out::Handle(h) = ok(&kernel, caller, 1, open) else { panic!("open") };
    let whole = |seq| {
        let read = Op::Read { h, at: 0, len: 1 << 16 };
        match ok(&kernel, caller, seq, read) {
            Out::Bytes(bytes) => String::from_utf8(bytes).unwrap(),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(whole(2), "info");
    ok(&kernel, caller, 3, Op::Write { h, at: 0, bytes: b"debug".to_vec() });
    // The handle holds the value it read; the write it made is what
    // takes that back, so the next read of it is the level now.
    assert_eq!(whole(4), "debug");
}

#[test]
fn watchers_counts_the_open_handles_that_are_watching() {
    let kernel = Kernel::new(tick, heap, Limits::default());
    assert_eq!(cat(&kernel, "info/watchers"), "0");
    let caller = kernel.caller_new();
    let open = Op::Open { rd: LOG, rights: R };
    let Out::Handle(h) = ok(&kernel, caller, 1, open) else { panic!("open") };
    assert_eq!(cat(&kernel, "info/watchers"), "0", "an open is not a watch");
    ok(&kernel, caller, 2, Op::Watch { h, on: true });
    assert_eq!(cat(&kernel, "info/watchers"), "1");
    ok(&kernel, caller, 3, Op::Close { h });
    assert_eq!(cat(&kernel, "info/watchers"), "0", "the close took it");
}

#[test]
fn a_fact_is_read_only_and_its_read_leaves_nothing_behind() {
    let kernel = Kernel::new(tick, heap, Limits::default());
    let atime = |path: &str| {
        let rd = resolve(&kernel, path);
        match alone(&kernel, Op::Info { rd }) {
            Out::Info(info) => info.atime,
            other => panic!("{other:?}"),
        }
    };
    let five = [
        "info/mem",
        "info/time",
        "info/uptime",
        "info/version",
        "info/watchers",
    ];
    // Reading the log is itself a read, so the entries are counted
    // before the atimes and again after the reads between them.
    let entries = said(&kernel).len();
    let before: Vec<u64> = five.iter().copied().map(atime).collect();
    for path in five {
        assert!(!cat(&kernel, path).is_empty(), "{path} says nothing");
    }
    let after: Vec<u64> = five.iter().copied().map(atime).collect();
    assert_eq!(before, after, "a fact read moved an atime");
    assert_eq!(said(&kernel).len(), entries, "a fact read wrote a line");
    // A cell with no WRITE in its flags refuses the handle and not the
    // bytes, which is what `/sys/mem` answered a write with before the
    // fact stood here.
    for path in five {
        let refused = write(&kernel, path, "0");
        assert_eq!(refused, Reply::err(Error::Denied), "{path} took a write");
    }
}

/// A Module with one cell under an api, so a core has a `decl` line
/// and an rd a Driver serves.
#[derive(Default)]
struct Api(core::cell::Cell<Option<Rd>>);

fn decl() -> Decl {
    Decl {
        api: "say".to_string(),
        ver: 1,
        methods: vec![Method {
            number: 2,
            name: "hello".to_string(),
            input: Ty::Unit,
            output: Ty::Unit,
            right: Rights::READ,
        }],
        records: Vec::new(),
    }
}

impl Module for Api {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let cell = ctx.register(DriverId(0), home, Some("cell"))?;
        ctx.report(cell, Flags::READ, &[decl()])?;
        self.0.set(Some(cell));
        Ok(())
    }
}

impl Driver for Api {}

#[test]
fn a_core_holds_every_section_and_the_read_leaves_nothing() {
    let kernel = Kernel::new(tick, heap, Limits::default());
    let module = Rc::new(Api::default());
    kernel.load("api", module.clone(), vec![module]).unwrap();
    let session = kernel.session_new();
    let (caller, client) = kernel.connect(session, b"named").unwrap();
    let Out::Rd(cap) =
        alone(&kernel, Op::Lookup { rd: client, name: "capability".into() })
    else {
        panic!("lookup")
    };
    let open = Op::Open { rd: cap, rights: R | W };
    let Out::Handle(h) = ok(&kernel, caller, 1, open) else { panic!("open") };
    ok(&kernel, caller, 2, Op::Write { h, at: 0, bytes: b"hi".to_vec() });

    let atime = |rd: Rd| match alone(&kernel, Op::Info { rd }) {
        Out::Info(info) => info.atime,
        other => panic!("{other:?}"),
    };
    // Reading the log is itself a read, so it is asked before the two
    // atimes; between them and the two after, only the core is read.
    let entries = said(&kernel).len();
    let before = (atime(CORE), atime(LOG));
    let core = cat(&kernel, "core");
    // Read again at once: a core is the Kernel of its moment, and
    // between these two the Kernel did only the reading itself.
    let again = cat(&kernel, "core");
    let after = (atime(CORE), atime(LOG));

    // Every section, in the one order a core writes them.
    let heads: Vec<&str> =
        core.lines().filter_map(|line| line.split(' ').next()).collect();
    let mut at = 0;
    for word in [
        "limit", "module", "rd", "cell", "decl", "caller", "handle", "event",
        "log", "live=",
    ] {
        let rest = &heads[at..];
        let found = rest.iter().position(|head| head.starts_with(word));
        at += found.unwrap_or_else(|| panic!("no {word}: {heads:?}"));
    }
    // The `rd` lines are the table's rds: every fixed one, the Module's
    // home and the cell it serves.
    let rds: Vec<&str> =
        core.lines().filter(|line| line.starts_with("rd rd=")).collect();
    for rd in [LOG, LOGLEVEL, LIMITS, MEM, CORE, cap] {
        let head = format!("rd rd={} ", rd.0);
        assert!(rds.iter().any(|line| line.starts_with(&head)), "{head}");
    }
    assert!(rds.iter().any(|line| line.contains(" node=driver ")), "{rds:?}");
    assert!(rds.iter().any(|line| line.contains(" node=home ")), "{rds:?}");
    // The cell the link holds, its bytes as hex; the log lines are the
    // log's own entries, one line each however they were written.
    let head = format!("cell rd={} len=2 bytes=6869", cap.0);
    assert!(core.lines().any(|line| line == head), "{core}");
    let logs = core.lines().filter(|line| line.starts_with("log seq="));
    assert_eq!(logs.count(), entries, "one line per entry");
    assert!(core.contains("decl hash="), "{core}");
    assert!(core.contains(" api=say ver=1\n"), "{core}");
    assert!(core.contains("caller id="), "{core}");
    assert!(core.contains(" named=yes "), "the link presented one");

    // A core changes nothing: no atime, and no entry of its own.
    assert_eq!(before, after, "the core read moved an atime");
    assert_eq!(said(&kernel).len(), entries, "the core read wrote a line");
    // The same Kernel twice is the same text down to the reader
    // itself: what differs is the caller and the handle the second
    // read is, and the heap the reading moved.
    let head = |text: &str| {
        let at = text.find("\ncaller ").unwrap();
        text[..at].to_string()
    };
    assert_eq!(head(&core), head(&again));
    // A core is read, never written.
    assert_eq!(write(&kernel, "core", "x"), Reply::err(Error::Denied));
}
