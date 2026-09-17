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
    let file = server.line(&["lookup", "/module/fs/root/note"]);
    let buffer = server.line(&["call", "/module/text/open", "open", &file]);
    let target = format!("rd:{buffer}");

    assert_eq!(server.line(&["write", &target, "second"]), "6");
    assert_eq!(server.line(&["read", &target]), "second");
    assert_eq!(server.disk("note"), original, "unchanged before save");
    assert_eq!(server.line(&["call", &target, "save"]), "");
    assert_eq!(server.disk("note"), b"second");
    assert_eq!(
        server.line(&["call", &target, "tree"]),
        "{nodes=[{id=1 parent=0 label=open} {id=2 parent=1 label=write}] \
         head=2 published=2}"
    );

    // Children and info through the cli, and the pointer child.
    let listed = server.line(&["children", "/module/text"]);
    assert!(listed.contains(&format!("{buffer} {file}")), "{listed}");
    let info = server.line(&["info", &target]);
    assert!(info.contains("flags=READ|WRITE|CONTAINER"), "{info}");
    // A Buffer is named by its source rd; `source` reads that rd raw.
    let pointer = format!("/module/text/{file}/source");
    let source = server.cli(&["read", &pointer], b"").unwrap();
    let id = u64::from_le_bytes(source[..].try_into().unwrap());
    assert_eq!(id.to_string(), file);
}

#[test]
fn one_interactive_session_does_the_same() {
    let server = Server::start("interactive");
    server.file("note", b"first\n");
    let file = server.line(&["lookup", "/module/fs/root/note"]);
    // A session of lines keeps where it stands, so every target after
    // the `cd` is a name under it.
    let script = format!(
        "cd /module/text\n\
         pwd\n\
         call open open {file}\n\
         write {file} third\n\
         read {file}\n\
         call {file} nothing\n\
         call {file} save\n\
         quit\n\
         lookup /\n"
    );
    let out = String::from_utf8(server.cli(&[], script.as_bytes()).unwrap());
    let out = out.unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 4, "{out}");
    assert_eq!(lines[0], "/module/text");
    assert_eq!(lines[2], "5");
    assert_eq!(lines[3], "third");
    assert_eq!(server.disk("note"), b"third");
}

/// `reovim cli` without a server: what the word could not do, and
/// `--help`, which needs no link at all.
#[test]
fn a_command_says_its_line_and_help_needs_no_server() {
    let server = Server::start("lines");
    let read = server.cli(&["read", "/module/fs/root"], b"").unwrap_err();
    assert_eq!(
        read,
        "reovim: read /module/fs/root: a Container has no bytes; ls it\n"
    );
    let nonsense = server.cli(&["nonsense"], b"").unwrap_err();
    assert_eq!(nonsense, "reovim: unknown word nonsense; help lists them\n");
    let entry = server.cli(&["help", "read"], b"").unwrap();
    assert_eq!(
        String::from_utf8(entry).unwrap(),
        "read <target>                   bytes to stdout\n"
    );

    // No link is opened for `--help`, and a socket nobody listens at
    // is said with its path.
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
    let table = reovim_client_cli::USAGE.lines().count();
    assert_eq!(
        String::from_utf8(helped.stdout).unwrap().lines().count(),
        table
    );
    let missing = cli(&["read", "/"]);
    assert_eq!(missing.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(missing.stderr).unwrap(),
        format!("reovim: no server at {nowhere}\n")
    );
}

#[test]
fn a_bad_call_fails_before_anything_is_sent() {
    let server = Server::start("bad");
    server.file("note", b"");
    let err =
        server.cli(&["call", "/module/fs/root", "create"], b"").unwrap_err();
    assert!(err.contains("does not fit"), "{err}");
    let err =
        server.cli(&["call", "/module/fs/root", "nothing"], b"").unwrap_err();
    assert!(err.contains("no method"), "{err}");
    let err = server.cli(&["call", "/module/fs/root", "create", "a/b"], b"");
    assert_eq!(
        err.unwrap_err(),
        "reovim: call /module/fs/root: invalid\n",
        "the Driver refuses a slash"
    );
    let made = server.line(&["call", "/module/fs/root", "create", "made"]);
    assert!(made.parse::<u64>().is_ok(), "an rd: {made}");
    assert!(server.dir.join("made").exists());
}
