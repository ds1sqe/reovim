//! Walkthrough 2 of 03-walkthroughs.md: an `lsp` coordinator renames
//! across five Buffers with a per-rd precondition; one is Stale because
//! someone typed; the coordinator reports what applied.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use reovim_host::clock;
use reovim_kernel_interface::{
    Booted, Caller, Calls, Driver, DriverId, Error, Flags, Handle, Hash,
    Module, Op, Out, Rd, Reply, Rights, Token,
};
use reovim_kernel_runtime::{Kernel, Limits, ROOT};
use reovim_module_text::{history, open, text};
use reovim_sdk::{declare, decode, encode};

struct Dir(PathBuf);

impl Dir {
    fn new() -> Self {
        let name = format!("reovim-lsp-{}", std::process::id());
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

declare! {
    api lsp ver 1 {
        0 rename(job: Rename) -> Report: WRITE;
    }
    record Rename { buffers: [Rd; 16], at: u64, name: utf8(64) }
    record Report { applied: u32, stale: [Rd; 16] }
}

/// One rename in flight: per Buffer the head revision seen at the
/// request, as the walkthrough's `lsp` remembers what it sent.
struct Job {
    buffers: Vec<(Rd, u64)>,
    at: u64,
    name: String,
}

#[derive(Default)]
struct Lsp {
    rename: Cell<Option<Rd>>,
    job: RefCell<Option<Job>>,
}

fn ready(reply: Reply<Out>) -> Result<Out, Error> {
    match reply {
        Reply::Ready(result) => result,
        Reply::Pending(_) => Err(Error::Busy),
    }
}

impl Lsp {
    /// The head revision of `buffer` now, through `history.tree`.
    fn head(ctx: &dyn Calls, buffer: Rd) -> Result<u64, Error> {
        let h = ctx.open(buffer, Rights::READ)?;
        let api = history::hash();
        let tree = Op::Call { h, api, method: 0, input: Vec::new() };
        let out = ready(ctx.submit(tree));
        let _ = ctx.submit(Op::Close { h });
        let Out::Bytes(bytes) = out? else { return Err(Error::Invalid) };
        Ok(decode::<history::Tree>(&bytes)?.head)
    }

    /// `replace_many` with the revision seen; `Stale` if it moved.
    fn apply(
        ctx: &dyn Calls,
        job: &Job,
        buffer: Rd,
        rev: u64,
    ) -> Result<(), Error> {
        let h = ctx.open(buffer, Rights::WRITE)?;
        let edit = text::Edit { at: job.at, len: 3, text: job.name.clone() };
        let input = encode(&text::Many { expected: rev, edits: vec![edit] });
        let api = text::hash();
        let out = ready(ctx.submit(Op::Call { h, api, method: 1, input }));
        let _ = ctx.submit(Op::Close { h });
        out.map(|_| ())
    }

    /// The server answered: apply everywhere, report the outcome.
    fn go(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let job = self.job.borrow_mut().take().ok_or(Error::Invalid)?;
        let mut applied = 0u32;
        let mut stale = Vec::new();
        for (buffer, rev) in &job.buffers {
            match Self::apply(ctx, &job, *buffer, *rev) {
                Ok(()) => applied += 1,
                Err(Error::Stale) => stale.push(*buffer),
                Err(other) => return Err(other),
            }
        }
        let report = lsp::Report { applied, stale };
        ctx.done(Token(1), Ok(Out::Bytes(encode(&report))))
    }
}

impl Module for Lsp {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let actions = ctx.register(DriverId(0), home, Some("actions"))?;
        ctx.report(actions, Flags::READ | Flags::CONTAINER, &[])?;
        let rename = ctx.register(DriverId(0), actions, Some("rename"))?;
        ctx.report(rename, Flags::READ | Flags::WRITE, &[lsp::decl()])?;
        self.rename.set(Some(rename));
        Ok(())
    }
}

impl Driver for Lsp {
    fn lookup(&self, ctx: &dyn Calls, _: Rd, name: &str) -> Reply<Rd> {
        // The language server "answers" when the test looks up `go`.
        let _ = name;
        self.go(ctx).and(Err(Error::Invalid)).into()
    }

    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        _: u64,
        _: Hash,
        _: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        let rename = match decode::<lsp::Rename>(input) {
            Ok(rename) => rename,
            Err(error) => return Reply::err(error),
        };
        let mut buffers = Vec::new();
        for rd in rename.buffers {
            match Self::head(ctx, rd) {
                Ok(rev) => buffers.push((rd, rev)),
                Err(error) => return Reply::err(error),
            }
        }
        let job = Job { buffers, at: rename.at, name: rename.name };
        *self.job.borrow_mut() = Some(job);
        Reply::Pending(Token(1))
    }
}

struct Rig {
    dir: Dir,
    kernel: Kernel,
    caller: Caller,
    seq: Cell<u64>,
}

impl Rig {
    fn ok(&self, op: Op) -> Out {
        match self.submit(op) {
            Reply::Ready(Ok(out)) => out,
            other => panic!("{other:?}"),
        }
    }

    fn submit(&self, op: Op) -> Reply<Out> {
        self.seq.set(self.seq.get() + 1);
        self.kernel.submit(self.caller, self.seq.get(), op)
    }

    fn resolve(&self, path: &str) -> Rd {
        path.split('/').filter(|n| !n.is_empty()).fold(ROOT, |rd, n| match self
            .ok(Op::Lookup { rd, name: n.to_string() })
        {
            Out::Rd(rd) => rd,
            other => panic!("{other:?}"),
        })
    }

    fn open(&self, rd: Rd, rights: Rights) -> Handle {
        match self.ok(Op::Open { rd, rights }) {
            Out::Handle(h) => h,
            other => panic!("{other:?}"),
        }
    }

    fn read(&self, rd: Rd) -> Vec<u8> {
        let h = self.open(rd, Rights::READ);
        match self.ok(Op::Read { h, at: 0, len: 64 }) {
            Out::Bytes(bytes) => bytes,
            other => panic!("{other:?}"),
        }
    }

    /// A Buffer over the file `name` written with `content`.
    fn buffer(&self, name: &str, content: &[u8]) -> Rd {
        std::fs::write(self.dir.0.join(name), content).unwrap();
        let file = self.resolve(&format!("/module/fs/root/{name}"));
        let open = self.resolve("/module/text/open");
        let h = self.open(open, Rights::WRITE);
        let api = open::hash();
        match self.ok(Op::Call { h, api, method: 0, input: encode(&file) }) {
            Out::Bytes(bytes) => decode(&bytes).unwrap(),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn rename_across_five_buffers_reports_the_stale_one() {
    let dir = Dir::new();
    let kernel = Kernel::new(clock::now, Limits::default());
    let load = |name: &str, booted: Booted| {
        kernel.load(name, booted.module, booted.drivers).unwrap();
    };
    load("fs", reovim_module_fs::boot(dir.0.to_str().unwrap()));
    load("text", reovim_module_text::boot(""));
    let lsp = Rc::new(Lsp::default());
    load("lsp", Booted { module: lsp.clone(), drivers: vec![lsp] });
    let caller = kernel.caller_new();
    let rig = Rig { dir, kernel, caller, seq: Cell::new(0) };

    let buffers: Vec<Rd> =
        (1..=5).map(|i| rig.buffer(&format!("f{i}"), b"old text")).collect();
    let rename = rig.resolve("/module/lsp/actions/rename");
    let h = rig.open(rename, Rights::WRITE);
    let job =
        lsp::Rename { buffers: buffers.clone(), at: 0, name: "new".into() };
    let input = encode(&job);
    let api = lsp::hash();
    let Reply::Pending(token) =
        rig.submit(Op::Call { h, api, method: 0, input })
    else {
        panic!("pending")
    };

    // The user types into the third Buffer while the server thinks.
    let typed = rig.open(buffers[2], Rights::WRITE);
    rig.ok(Op::Write { h: typed, at: 0, bytes: b"typed".to_vec() });
    assert_eq!(rig.submit(Op::Ask { token }), Reply::err(Error::Busy));
    let go = Op::Lookup { rd: rename, name: "go".to_string() };
    assert_eq!(rig.submit(go), Reply::err(Error::Invalid));

    let Out::Bytes(report) = rig.ok(Op::Ask { token }) else { panic!("ask") };
    let report: lsp::Report = decode(&report).unwrap();
    assert_eq!((report.applied, report.stale.len()), (4, 1));
    assert_eq!(report.stale[0], buffers[2]);
    for (i, rd) in buffers.iter().enumerate() {
        let expected: &[u8] = if i == 2 { b"typed" } else { b"new text" };
        assert_eq!(rig.read(*rd), expected, "buffer {i}");
    }
}
