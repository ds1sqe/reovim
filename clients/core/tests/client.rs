//! The Client on a Kernel with fs, buffer and one slow Driver, through a
//! Carrier over an in-process `Connection`.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use reovim_client::{Carrier, Client, Fault, ROOT, log, value};
use reovim_host::clock;
use reovim_kernel_interface::wire::{Message, decode};
use reovim_kernel_interface::{
    Booted, Calls, Driver, Error, Flags, Kind, Module, Op, Out, Rd, Reply,
    Rights, Token, Ty,
};
use reovim_kernel_runtime::{Connection, Kernel, Limits};

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
}

impl Carrier for Local {
    fn exchange(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Fault> {
        let answer =
            self.connection.handle(&self.kernel, bytes).ok_or(Fault::Wire)?;
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
}

struct Rig {
    dir: Dir,
    kernel: Rc<Kernel>,
    slow: Rc<Slow>,
    asks: Rc<Cell<u32>>,
}

fn load(kernel: &Kernel, name: &str, booted: &Booted) {
    kernel.load(name, booted.module.as_ref(), booted.drivers.clone()).unwrap();
}

impl Rig {
    fn new(test: &str) -> Self {
        let dir = Dir::new(test);
        let kernel = Kernel::new(clock::now, Limits::default());
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
        let carrier = Local {
            kernel: self.kernel.clone(),
            connection: Connection::new(&self.kernel),
            slow: self.slow.clone(),
            asks: self.asks.clone(),
        };
        Client::connect(Box::new(carrier), Vec::new()).unwrap()
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
    assert_eq!(client.parent(client.cid()).unwrap(), {
        let session = client.session();
        client.lookup(session, "client").unwrap()
    });

    let note = client.resolve("/module/fs/root/note").unwrap();
    assert_eq!(client.resolve(&format!("rd:{}", note.0)).unwrap(), note);
    assert_eq!(client.resolve("/").unwrap(), ROOT);
    assert_eq!(client.read_all(note).unwrap(), b"hello\n");
    assert_eq!(client.name(note).unwrap(), "note");
    let root = client.resolve("/module/fs/root").unwrap();
    assert_eq!(client.children(root).unwrap(), vec![note]);

    assert!(matches!(client.resolve("note"), Err(Fault::Usage(_))));
    assert!(matches!(client.resolve("rd:x"), Err(Fault::Usage(_))));
    assert_eq!(
        client.resolve("/module/fs/root/none"),
        Err(Fault::Kernel(Error::Invalid))
    );
}

#[test]
fn a_typed_call_and_a_watched_write() {
    let rig = Rig::new("call");
    rig.file("note", b"first\n");
    let mut client = rig.client();
    let note = client.resolve("/module/fs/root/note").unwrap();
    let open = client.resolve("/module/text/open").unwrap();

    let (_, method) = client.method(open, "open").unwrap();
    assert_eq!(method.input, Ty::Rd);
    let input = value::encode_arg(&method.input, Some(&note.0.to_string()));
    let out = client.call_by_name(open, "open", input.unwrap()).unwrap();
    let shown = value::show(&method.output, &out).unwrap();
    let buffer = Rd(u64::from_le_bytes(out[..].try_into().unwrap()));
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
fn a_bad_argument_is_usage_and_nothing_is_sent() {
    let rig = Rig::new("usage");
    let mut client = rig.client();
    let root = client.resolve("/module/fs/root").unwrap();
    let (_, create) = client.method(root, "create").unwrap();
    let missing = value::encode_arg(&create.input, None);
    assert!(matches!(missing, Err(Fault::Usage(_))), "{missing:?}");
    let unfit = value::encode_arg(&Ty::U32, Some("x"));
    assert!(matches!(unfit, Err(Fault::Usage(_))), "{unfit:?}");
    let none = client.method(root, "nothing");
    assert!(matches!(none, Err(Fault::Usage(_))), "{none:?}");

    let entries = log::tail(&mut client, 1).unwrap();
    let lines: Vec<String> = entries.iter().map(log::Entry::line).collect();
    assert!(lines.iter().any(|l| l.contains("connect session=")), "{lines:?}");
    assert!(!lines.iter().any(|l| l.contains("call rd=")), "{lines:?}");

    // A fit argument reaches the Driver and the Kernel stamps the pair.
    let input = value::encode_arg(&create.input, Some("made")).unwrap();
    client.call_by_name(root, "create", input).unwrap();
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
fn the_log_parses_whole_entries_only() {
    let rig = Rig::new("log");
    let mut client = rig.client();
    let rd = client.resolve("/sys/log").unwrap();
    let bytes = client.read_all(rd).unwrap();
    let whole = log::parse(&bytes);
    assert_eq!(whole.len(), 1, "the connect line: {whole:?}");
    assert_eq!(whole[0].seq, 1);
    assert_eq!(whole[0].mark, "kernel");
    let cut = log::parse(&bytes[..bytes.len() - 1]);
    assert_eq!(cut.len(), whole.len() - 1);
    let line = whole[0].line();
    assert!(line.starts_with("1 ") && line.ends_with('\n'), "{line}");
}
