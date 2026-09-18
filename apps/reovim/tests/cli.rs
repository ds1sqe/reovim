//! The milestone slice through a real `reovim server` process: a
//! buffer over a file, written through `reovim cli`, saved to disk.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use reovim_host::link;
use reovim_kernel_interface::wire::{Endpoint, Message};
use reovim_kernel_interface::{Op, Out, Rd, Reply};

/// A server over a directory of its own, killed on drop.
struct Server {
    dir: PathBuf,
    socket: String,
    child: Child,
}

impl Server {
    fn start(test: &str) -> Self {
        let name = format!("reovim-cli-{}-{test}", std::process::id());
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("s").to_str().unwrap().to_string();
        let child = Command::new(env!("CARGO_BIN_EXE_reovim"))
            .args(["server", "--socket", &socket, "--root"])
            .arg(&dir)
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let server = Self { dir, socket, child };
        let start = Instant::now();
        while link::open(&server.socket).is_err() {
            assert!(start.elapsed() < Duration::from_secs(10), "no server");
            std::thread::sleep(Duration::from_millis(10));
        }
        server
    }

    fn file(&self, name: &str, content: &[u8]) -> Vec<u8> {
        std::fs::write(self.dir.join(name), content).unwrap();
        content.to_vec()
    }

    fn disk(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.dir.join(name)).unwrap()
    }

    /// `reovim cli` once; stdout, or the failure text.
    fn cli(&self, args: &[&str], stdin: &[u8]) -> Result<Vec<u8>, String> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_reovim"))
            .args(["cli", "--socket", &self.socket])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(stdin).unwrap();
        let out = child.wait_with_output().unwrap();
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(String::from_utf8(out.stderr).unwrap())
        }
    }

    fn line(&self, args: &[&str]) -> String {
        let out = String::from_utf8(self.cli(args, b"").unwrap()).unwrap();
        out.trim_end().to_string()
    }

    /// The rd of a target, read from the `rd` line of `stat`.
    fn rd(&self, target: &str) -> String {
        let said = self.line(&["stat", target]);
        let line = said.lines().find(|line| line.starts_with("rd ")).unwrap();
        line["rd ".len()..].to_string()
    }

    /// The names under `/session`, once there are `wanted` of them or
    /// two seconds have gone. A link's end reaches the server a moment
    /// after the cli process that held it is done, so the count is
    /// asked for again until it settles.
    fn sessions(&self, wanted: usize) -> Vec<String> {
        let start = Instant::now();
        let over = || start.elapsed() > Duration::from_secs(2);
        let mut names = self.listed();
        while names.len() != wanted && !over() {
            std::thread::sleep(Duration::from_millis(10));
            names = self.listed();
        }
        names
    }

    /// The names under `/session`, one per line into a pipe.
    fn listed(&self) -> Vec<String> {
        self.line(&["ls", "/session"]).lines().map(str::to_string).collect()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn the_server_answers_a_raw_endpoint() {
    let server = Server::start("raw");
    let end = link::open(&server.socket).unwrap();
    let mut endpoint = Endpoint::default();
    let mut ask = |message: &Message| {
        let (_, bytes) = endpoint.send(message);
        link::send(&end, &bytes).unwrap();
        let (_, reply) = endpoint.receive(&link::recv(&end).unwrap()).unwrap();
        reply
    };
    let connect = Message::Connect { credential: Vec::new(), session: None };
    let Message::Reply(Reply::Ready(Ok(Out::Rds(ids)))) = ask(&connect) else {
        panic!("connect");
    };
    assert_eq!(ids.len(), 2);
    let mut rd = Rd(0);
    for name in ["module", "fs", "root"] {
        let op = Op::Lookup { rd, name: name.to_string() };
        let Message::Reply(Reply::Ready(Ok(Out::Rd(child)))) =
            ask(&Message::Request(op))
        else {
            panic!("lookup {name}");
        };
        rd = child;
    }
    let info = Message::Request(Op::Info { rd });
    let Message::Reply(Reply::Ready(Ok(Out::Info(info)))) = ask(&info) else {
        panic!("info");
    };
    assert_eq!(info.name.as_deref(), Some("root"));
}

#[test]
fn a_draft_reaches_disk_on_save_through_the_cli() {
    let server = Server::start("save");
    let original = server.file("note", b"first\n");
    server.file("second", b"second");
    let file = server.rd("/module/fs/root/note");
    let buffer = server.line(&["call", "/module/text/open", "open", &file]);
    let target = format!("rd:{buffer}");

    // `>` is the shell's own and copies a run's output, so what the
    // draft takes is the bytes of a second file on disk.
    let poured = ["cat", "/module/fs/root/second", ">", &target];
    assert_eq!(server.line(&poured), "");
    assert_eq!(server.line(&["cat", &target]), "second");
    assert_eq!(server.disk("note"), original, "unchanged before save");
    assert_eq!(server.line(&["call", &target, "save"]), "");
    assert_eq!(server.disk("note"), b"second");
    assert_eq!(
        server.line(&["call", &target, "tree"]),
        "{nodes=[{id=1 parent=0 label=open} {id=2 parent=1 label=write}] \
         head=2 published=2}"
    );

    // The children and the record through the cli, and the pointer
    // child.
    let listed = server.line(&["ls", "-a", "/module/text"]);
    assert!(listed.contains(&format!("{buffer} {file}")), "{listed}");
    let said = server.line(&["stat", &target]);
    assert!(said.starts_with("c rw  "), "{said}");
    assert!(said.contains("\ntext v1\n"), "{said}");
    // A Buffer is named by its source rd; `source` reads that rd raw.
    let pointer = format!("/module/text/{file}/source");
    let source = server.cli(&["cat", &pointer], b"").unwrap();
    let id = u64::from_le_bytes(source[..].try_into().unwrap());
    assert_eq!(id.to_string(), file);
}

#[test]
fn one_interactive_session_does_the_same() {
    let server = Server::start("interactive");
    server.file("note", b"first\n");
    server.file("third", b"third");
    let file = server.rd("/module/fs/root/note");
    // A session of lines keeps where it stands, so every target after
    // the `cd` is a name under it; the end of the input ends it, as it
    // ends sh.
    let script = format!(
        "cd /module/text\n\
         pwd\n\
         call open open {file}\n\
         cat /module/fs/root/third > {file}\n\
         cat {file}\n\
         call {file} nothing\n\
         call {file} save\n"
    );
    let out = String::from_utf8(server.cli(&[], script.as_bytes()).unwrap());
    let out = out.unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3, "{out}");
    assert_eq!(lines[0], "/module/text");
    assert_eq!(lines[2], "third");
    assert_eq!(server.disk("note"), b"third");
}

/// `reovim cli` without a server: what a run could not do, and
/// `--help`, which needs no link at all.
#[test]
fn a_line_says_what_it_could_not_do_and_help_needs_no_server() {
    let server = Server::start("lines");
    let held = server.cli(&["cat", "/module/fs/root"], b"").unwrap_err();
    assert_eq!(held, "reovim: cat /module/fs/root: unsupported\n");
    let nonsense = server.cli(&["nonsense"], b"").unwrap_err();
    assert_eq!(nonsense, "reovim: nonsense: not under /bin; help lists them\n");
    let entry = server.cli(&["help", "cat"], b"").unwrap();
    assert_eq!(
        String::from_utf8(entry).unwrap(),
        "cat [-f] [-n N] [targets..]\n"
    );

    // No link is opened for `--help`, which is the app's own usage and
    // no second spelling of the shell's `help`; a socket nobody listens
    // at is said with its path.
    let nowhere = server.dir.join("nowhere.sock");
    let nowhere = nowhere.to_str().unwrap();
    let cli = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_reovim"))
            .args(["cli", "--socket", nowhere])
            .args(args)
            .output()
            .unwrap()
    };
    let helped = cli(&["--help"]);
    assert!(helped.status.success());
    let text = String::from_utf8(helped.stdout).unwrap();
    assert!(text.starts_with("usage: reovim server "), "{text}");
    assert!(
        text.contains(
            "reovim cli [--socket PATH] [--color[=WHEN]] [--help] [LINE...]"
        ),
        "{text}"
    );
    assert!(
        text.contains("help typed at it lists what is under /bin"),
        "{text}"
    );
    let missing = cli(&["cat", "/"]);
    assert_eq!(missing.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(missing.stderr).unwrap(),
        format!("reovim: no server at {nowhere}\n")
    );
}

/// The Drivers under `/bin` come of Modules like any other: each loads
/// at `/module/<name>`, registers its own under the root's `bin`, and
/// `unload` refuses it as it refuses fs.
#[test]
fn the_drivers_under_bin_come_of_linked_modules() {
    let server = Server::start("bin");
    assert_eq!(
        server.line(&["ls", "/bin"]),
        "call\ncat\ndate\ndmesg\nlog\nls\nstat\ntree\nuptime\nwatch"
    );
    for name in ["coreutils", "sysutils"] {
        let rd = server.rd(&format!("/module/{name}"));
        assert!(rd.parse::<u64>().is_ok(), "{rd}");
        assert_eq!(
            server.cli(&["unload", name], b"").unwrap_err(),
            format!("reovim: unload {name}: denied\n")
        );
    }

    // What one of each offers, as the shell will read it from the tree.
    let said = server.line(&["stat", "/bin/ls"]);
    assert!(said.contains("\nls v1\n"), "{said}");
    assert!(
        said.contains(
            "  0 run({long: bool, all: bool, w: bool, cwd: Rd, width: u64, \
             color: bool, targets: [utf8(256); 64]}) -> Rd READ"
        ),
        "{said}"
    );
    let said = server.line(&["stat", "/bin/dmesg"]);
    assert!(said.contains("\ndmesg v1\n"), "{said}");
    assert!(
        said.contains(
            "  0 run({t: bool, T: bool, H: bool, d: bool, r: bool, \
             x: bool, k: bool, w: bool, W: bool, f: utf8(256), \
             l: utf8(256), color: bool}) -> Rd READ"
        ),
        "{said}"
    );
}

#[test]
fn a_bad_call_fails_before_anything_is_sent() {
    let server = Server::start("bad");
    server.file("note", b"");
    let err =
        server.cli(&["call", "/module/fs/root", "create"], b"").unwrap_err();
    assert_eq!(err, "reovim: call /module/fs/root create: invalid\n");
    let err =
        server.cli(&["call", "/module/fs/root", "nothing"], b"").unwrap_err();
    assert_eq!(err, "reovim: call /module/fs/root nothing: unsupported\n");
    let err = server.cli(&["call", "/module/fs/root", "create", "a/b"], b"");
    assert_eq!(
        err.unwrap_err(),
        "reovim: call /module/fs/root create a/b: invalid\n",
        "the Driver refuses a slash"
    );
    let made = server.line(&["call", "/module/fs/root", "create", "made"]);
    assert!(made.parse::<u64>().is_ok(), "an rd: {made}");
    assert!(server.dir.join("made").exists());
}

/// Every cli run makes a Session of its own and takes it away again:
/// after ten of them `/session` holds only the one the cli that asks
/// is in, and a watcher on `/session` sees a run arrive and leave.
#[test]
fn a_cli_run_takes_its_session_with_it() {
    let server = Server::start("sessions");
    for _ in 0..10 {
        assert_eq!(server.rd("/"), "0");
    }
    let listed = server.sessions(1);
    assert_eq!(listed.len(), 1, "ten runs left {listed:?}");
    assert!(listed[0].parse::<u64>().is_ok(), "a Session id: {listed:?}");

    // A watcher in a run of its own, which the listing shows once it
    // has connected; then one more run for it to watch.
    let watcher = Command::new(env!("CARGO_BIN_EXE_reovim"))
        .args(["cli", "--socket", &server.socket, "watch", "/session"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    assert_eq!(server.sessions(2).len(), 2, "the watcher never connected");
    server.rd("/");
    std::thread::sleep(Duration::from_millis(200));
    let pid = watcher.id().to_string();
    // SIGINT is what Ctrl-C makes; the loop ends and exits 0.
    Command::new("kill").args(["-INT", &pid]).status().unwrap();
    let out = watcher.wait_with_output().unwrap();
    let seen = String::from_utf8(out.stdout).unwrap();
    assert!(out.status.success(), "{seen}");
    assert!(seen.contains(" Create "), "no Create: {seen}");
    assert!(seen.contains(" Delete "), "no Delete: {seen}");
}
