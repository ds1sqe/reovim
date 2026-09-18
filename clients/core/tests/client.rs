//! The Client on a Kernel with fs, buffer and one slow Driver, through a
//! Carrier over an in-process `Connection`.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use reovim_client::{Carrier, Client, Fault, ROOT, log};
use reovim_host::clock;
use reovim_kernel_interface::wire::{Message, decode};
use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, Error, Flags, Hash, Kind, Method, Module, Op,
    Out, Rd, Reply, Rights, Token, Ty,
};
use reovim_kernel_runtime::{Connection, Heap, Kernel, Limits, NoLoader};
use reovim_sdk::dynamic;

/// The first method named `name` among what `rd` offers, which is what
/// a caller reads a declaration for.
fn method(client: &mut Client, rd: Rd, name: &str) -> Option<(Hash, Method)> {
    let named = |(hash, decl): (Hash, Decl)| {
        decl.methods.into_iter().find(|m| m.name == name).map(|m| (hash, m))
    };
    client.decls(rd).unwrap().into_iter().find_map(named)
}

/// A directory of its own for one test, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(test: &str) -> Self {
        let name = format!("reovim-client-{}-{test}", std::process::id());
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `/module/slow/late`: the first `read` is Pending until someone looks
/// up `finish` on it; then it reads `late`.
#[derive(Default)]
struct Slow {
    late: Cell<Option<Rd>>,
    done: Cell<bool>,
}

impl Module for Slow {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let rd = ctx.register(
            reovim_kernel_interface::DriverId(0),
            home,
            Some("late"),
        )?;
        ctx.report(rd, Flags::READ, &[])?;
        self.late.set(Some(rd));
        Ok(())
    }
}

impl Driver for Slow {
    fn lookup(&self, ctx: &dyn Calls, _: Rd, name: &str) -> Reply<Rd> {
        if name == "finish" {
            self.done.set(true);
            let out = Out::Bytes(b"late".to_vec());
            ctx.done(Token(1), Ok(out)).unwrap();
        }
        Reply::err(Error::Invalid)
    }

    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(&self, _: &dyn Calls, _: u64, at: u64, _: usize) -> Reply<Vec<u8>> {
        if !self.done.get() {
            return Reply::Pending(Token(1));
        }
        let bytes = b"late".get(usize::try_from(at).unwrap()..);
        Reply::ok(bytes.unwrap_or_default().to_vec())
    }
}

/// The Carrier: one `Connection` on a shared Kernel. On a Busy `Ask` it
/// finishes the slow read from a second caller, as a Driver would in
/// its own time.
struct Local {
    kernel: Rc<Kernel>,
    connection: Connection,
    slow: Rc<Slow>,
    asks: Rc<Cell<u32>>,
    /// Exchanges left before the link is cut once; `None` never cuts.
    cut: Option<u32>,
}

impl Carrier for Local {
    fn exchange(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Fault> {
        if self.cut == Some(0) {
            self.cut = None;
            self.connection.end(&self.kernel);
            return Err(Fault::Carrier);
        }
        self.cut = self.cut.map(|n| n - 1);
        let answer = self
            .connection
            .handle(&self.kernel, bytes, &mut NoLoader)
            .ok_or(Fault::Wire)?;
        let asking = matches!(
            decode(bytes),
            Some((_, Message::Request(Op::Ask { .. })))
        );
        let busy = matches!(
            decode(&answer),
            Some((_, Message::Reply(Reply::Ready(Err(Error::Busy)))))
        );
        if asking {
            self.asks.set(self.asks.get() + 1);
        }
        if asking && busy {
            let late = self.slow.late.get().unwrap();
            let poke = Op::Lookup { rd: late, name: "finish".to_string() };
            self.kernel.submit(self.kernel.caller_new(), 1, poke);
        }
        Ok(answer)
    }

    fn reconnect(&mut self) -> Result<(), Fault> {
        self.connection = Connection::new();
        Ok(())
    }
}

struct Rig {
    dir: Dir,
    kernel: Rc<Kernel>,
    slow: Rc<Slow>,
    asks: Rc<Cell<u32>>,
}

fn load(kernel: &Kernel, name: &str, booted: &Booted) {
    kernel.load(name, booted.module.clone(), booted.drivers.clone()).unwrap();
}

impl Rig {
    fn new(test: &str) -> Self {
        let dir = Dir::new(test);
        let kernel = Kernel::new(clock::now, Heap::default, Limits::default());
        load(&kernel, "fs", &reovim_module_fs::boot(dir.0.to_str().unwrap()));
        load(&kernel, "text", &reovim_module_text::boot(""));
        let slow = Rc::new(Slow::default());
        let booted =
            Booted { module: slow.clone(), drivers: vec![slow.clone()] };
        load(&kernel, "slow", &booted);
        let asks = Rc::new(Cell::new(0));
        Self { dir, kernel: Rc::new(kernel), slow, asks }
    }

    fn client(&self) -> Client {
        self.named(Vec::new(), None)
    }

    /// A Client as `credential` whose link is cut after `cut` exchanges.
    fn named(&self, credential: Vec<u8>, cut: Option<u32>) -> Client {
        let carrier = Local {
            kernel: self.kernel.clone(),
            connection: Connection::new(),
            slow: self.slow.clone(),
            asks: self.asks.clone(),
            cut,
        };
        Client::connect(Box::new(carrier), credential).unwrap()
    }

    fn file(&self, name: &str, content: &[u8]) {
        std::fs::write(self.dir.0.join(name), content).unwrap();
    }

    fn disk(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.dir.0.join(name)).unwrap()
    }
}

#[test]
fn connect_resolve_and_read_whole() {
    let rig = Rig::new("resolve");
    rig.file("note", b"hello\n");
    let mut client = rig.client();
    assert_ne!(client.session(), ROOT);
    let (session, cid) = (client.session(), client.cid());
    assert_eq!(client.parent(cid).unwrap(), {
        client.lookup(session, "client").unwrap()
    });

    let note = client.resolve("/module/fs/root/note").unwrap();
    assert_eq!(client.resolve(&format!("rd:{}", note.0)).unwrap(), note);
    assert_eq!(client.resolve("/").unwrap(), ROOT);
    assert_eq!(client.read_all(note).unwrap(), b"hello\n");
    assert_eq!(client.name(note).unwrap(), "note");
    let root = client.resolve("/module/fs/root").unwrap();
    assert_eq!(client.children(root).unwrap(), vec![note]);

    // One walk answers for every shape: a name with no `/` is read
    // under the root, an `rd:` that is no number is an ordinary name
    // there, and a `..` inside a path is a move and never a name.
    assert_eq!(client.resolve("note"), Err(Fault::Kernel(Error::Invalid)));
    assert_eq!(client.resolve("rd:x"), Err(Fault::Kernel(Error::Invalid)));
    assert_eq!(
        client.resolve("/module/fs/root/.."),
        client.resolve("/module/fs")
    );
}

#[test]
fn a_typed_call_and_a_watched_write() {
    let rig = Rig::new("call");
    rig.file("note", b"first\n");
    let mut client = rig.client();
    let note = client.resolve("/module/fs/root/note").unwrap();
    let open = client.resolve("/module/text/open").unwrap();

    let (_, opening) = method(&mut client, open, "open").unwrap();
    assert_eq!(opening.input, Ty::Rd);
    let input = dynamic::encode_arg(&opening.input, Some(&note.0.to_string()));
    let out = client.call_by_name(open, "open", input.unwrap()).unwrap();
    let shown = dynamic::show(&opening.output, &out).unwrap();
    let buffer: Rd = reovim_sdk::decode(&out).unwrap();
    assert_eq!(shown, format!("{}\n", buffer.0).into_bytes());
    let home = client.resolve("/module/text").unwrap();
    assert_eq!(client.parent(buffer).unwrap(), home);
    assert_eq!(client.name(buffer).unwrap(), note.0.to_string());

    // Watch, write, since: one Modify on the Buffer. The cursor starts
    // at 0, so the first `since` hands over the Buffer's history first.
    let h = client.open(buffer, Rights::READ).unwrap();
    client.watch(h, true).unwrap();
    let history = client.since().unwrap();
    assert!(history.iter().all(|e| e.rd == buffer), "{history:?}");
    assert_eq!(client.write_all(buffer, b"second".to_vec()).unwrap(), 6);
    let events = client.since().unwrap();
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!((events[0].rd, events[0].kind), (buffer, Kind::Modify));
    assert!(client.since().unwrap().is_empty(), "the cursor moved");
    client.close(h).unwrap();

    assert_eq!(client.read_all(buffer).unwrap(), b"second");
    assert_eq!(rig.disk("note"), b"first\n");
    assert_eq!(client.call_by_name(buffer, "save", Vec::new()).unwrap(), b"");
    assert_eq!(rig.disk("note"), b"second");
}

#[test]
fn a_bad_argument_is_refused_and_nothing_is_sent() {
    let rig = Rig::new("usage");
    let mut client = rig.client();
    let root = client.resolve("/module/fs/root").unwrap();
    let (_, create) = method(&mut client, root, "create").unwrap();
    let missing = dynamic::encode_arg(&create.input, None);
    assert_eq!(missing, Err(Error::Invalid), "{missing:?}");
    let unfit = dynamic::encode_arg(&Ty::U32, Some("x"));
    assert_eq!(unfit, Err(Error::Invalid), "{unfit:?}");
    let none = client.call_by_name(root, "nothing", Vec::new());
    assert_eq!(none, Err(Fault::Kernel(Error::Unsupported)), "{none:?}");

    let entries = log::tail(&mut client, 1).unwrap();
    let lines: Vec<String> = entries.iter().map(log::Entry::line).collect();
    assert!(lines.iter().any(|l| l.contains("connect session=")), "{lines:?}");
    assert!(!lines.iter().any(|l| l.contains("call rd=")), "{lines:?}");

    // A fit argument reaches the Driver and the Kernel stamps the
    // pair, which the log keeps once the level asks for it.
    let level = client.resolve("/sys/loglevel").unwrap();
    client.write_all(level, b"debug".to_vec()).unwrap();
    let input = dynamic::encode_arg(&create.input, Some("made")).unwrap();
    client.call_by_name(root, "create", input).unwrap();
    client.write_all(level, b"info".to_vec()).unwrap();
    let entries = log::tail(&mut client, 1).unwrap();
    let pair = entries.iter().filter(|e| e.line().contains("call rd=")).count();
    assert_eq!(pair, 2);
    assert!(rig.dir.0.join("made").exists());
}

#[test]
fn a_pending_reply_is_asked_until_ready() {
    let rig = Rig::new("pending");
    let mut client = rig.client();
    let late = client.resolve("/module/slow/late").unwrap();
    assert_eq!(client.read_all(late).unwrap(), b"late");
    assert!(rig.asks.get() >= 2, "asked {} times", rig.asks.get());
}

#[test]
fn the_handles_a_client_holds_are_the_ones_the_kernel_holds_for_it() {
    let rig = Rig::new("handles");
    rig.file("note", b"first\n");
    let mut client = rig.client();
    let note = client.resolve("/module/fs/root/note").unwrap();
    assert_eq!(client.handles(), [], "nothing opened yet");

    let read = client.open(note, Rights::READ).unwrap();
    let write = client.open(note, Rights::WRITE).unwrap();
    assert_eq!(client.handles(), [read, write], "in the order opened");
    // The helpers open and close for themselves, so they leave none.
    client.read_all(note).unwrap();
    assert_eq!(client.handles(), [read, write]);

    // What the Kernel holds for this caller, from its own core, read
    // through a handle that is itself one of them.
    let core = client.resolve("/sys/core").unwrap();
    let reading = client.open(core, Rights::READ).unwrap();
    // Read through that handle and not by path: a helper would open
    // one of its own, and the core is the one cell that names them.
    let bytes = client.read(reading, 0, 64 * 1024).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    let held: Vec<u64> = text
        .lines()
        .filter_map(|line| line.strip_prefix("handle "))
        .filter_map(|line| {
            line.split(' ').nth(1)?.strip_prefix("h=")?.parse().ok()
        })
        .collect();
    let ours: Vec<u64> = client.handles().iter().map(|h| h.0).collect();
    assert_eq!(held, ours, "exactly the handles open on the caller");
    client.close(reading).unwrap();

    client.close(read).unwrap();
    assert_eq!(client.handles(), [write], "close forgets");
    // A handle closed twice is the Kernel's to refuse; the mirror has
    // let it go either way.
    assert_eq!(client.close(read), Err(Fault::Kernel(Error::Invalid)));
    assert_eq!(client.handles(), [write]);
}

#[test]
fn the_log_parses_whole_entries_only() {
    let rig = Rig::new("log");
    let mut client = rig.client();
    let rd = client.resolve("/sys/log").unwrap();
    let bytes = client.read_all(rd).unwrap();
    let whole = log::parse(&bytes);
    assert_eq!(whole.len(), 4, "three loads and the connect: {whole:?}");
    assert_eq!(whole[0].seq, 1);
    assert_eq!(whole[0].mark, "kernel");
    assert!(whole[3].bytes.starts_with(b"connect "), "{whole:?}");
    let cut = log::parse(&bytes[..bytes.len() - 1]);
    assert_eq!(cut.len(), whole.len() - 1);
    let line = whole[0].line();
    assert!(line.starts_with("1 ") && line.ends_with('\n'), "{line}");
}

#[test]
fn a_named_client_goes_on_after_its_link_is_cut() {
    let rig = Rig::new("cut");
    rig.file("note", b"kept");
    let mut client = rig.named(b"me".to_vec(), Some(7));
    let note = client.resolve("/module/fs/root/note").unwrap();
    let h = client.open(note, Rights::READ).unwrap();
    client.watch(h, true).unwrap();
    let (session, cid) = (client.session(), client.cid());
    // The cut lands on this read; the answer still comes.
    assert_eq!(client.read(h, 0, 16).unwrap(), b"kept");
    assert_eq!((client.session(), client.cid()), (session, cid));
    assert_eq!(client.read(h, 0, 16).unwrap(), b"kept", "the handle lives");
    assert!(client.since().is_ok(), "the cursor goes on");

    // An empty credential names nothing the Kernel could give back:
    // the Session ended with the last caller in it, so there is no
    // Session left for the next Connect to take up.
    let mut unnamed = rig.named(Vec::new(), Some(2));
    let h = unnamed.open(note, Rights::READ).unwrap();
    let gone = unnamed.session();
    assert_eq!(unnamed.read(h, 0, 16), Err(Fault::Carrier));
    let mut other = rig.client();
    assert_eq!(other.info(gone), Err(Fault::Kernel(Error::Invalid)));
}
