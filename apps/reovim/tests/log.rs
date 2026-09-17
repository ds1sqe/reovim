//! `reovim server --log PATH`: every log entry in the file; without it
//! no file. Through real processes.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use reovim_host::link;

struct Server {
    dir: PathBuf,
    socket: String,
    child: Child,
}

impl Server {
    fn start(test: &str, log: bool) -> Self {
        let name = format!("reovim-log-{}-{test}", std::process::id());
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("s").to_str().unwrap().to_string();
        let mut command = Command::new(env!("CARGO_BIN_EXE_reovim"));
        command.args(["server", "--socket", &socket, "--root"]).arg(&dir);
        if log {
            command.arg("--log").arg(dir.join("log"));
        }
        let child = command.stderr(Stdio::null()).spawn().unwrap();
        let server = Self { dir, socket, child };
        let start = Instant::now();
        while link::open(&server.socket).is_err() {
            assert!(start.elapsed() < Duration::from_secs(10), "no server");
            std::thread::sleep(Duration::from_millis(10));
        }
        server
    }

    fn cli(&self, args: &[&str]) -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_reovim"))
            .args(["cli", "--socket", &self.socket])
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    /// The log file once it holds `wanted` lines, within two seconds.
    fn file(&self, wanted: usize) -> String {
        let start = Instant::now();
        let read = || {
            std::fs::read_to_string(self.dir.join("log")).unwrap_or_default()
        };
        let mut text = read();
        while text.lines().count() < wanted {
            assert!(start.elapsed() < Duration::from_secs(2), "{text}");
            std::thread::sleep(Duration::from_millis(10));
            text = read();
        }
        text
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
fn the_log_file_holds_every_entry() {
    let server = Server::start("file", true);
    // The three loads and the sink's own connect line, before any link.
    let first = server.file(4);
    assert!(first.starts_with("1 "), "{first}");
    // Each at its level: the Kernel's own are `info`, which is 6.
    assert!(first.contains(" kernel 6 load module=fs\n"), "{first}");
    assert!(first.contains(" kernel 6 connect session="), "{first}");

    let made = server.cli(&["call", "/module/fs/root", "create", "made"]);
    assert!(made.trim().parse::<u64>().is_ok(), "{made}");
    // The cli's connect line and the call pair.
    let text = server.file(7);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 7, "{text}");
    assert!(lines[4].contains("connect session="), "{text}");
    assert!(lines[5].contains("call rd=") && lines[5].ends_with(" dispatch"));
    assert!(lines[6].contains("call rd=") && lines[6].ends_with(" reply"));

    // `cli log` shows the same entries, and its own connect after them.
    let shown = server.cli(&["log"]);
    assert!(shown.starts_with(&text), "{shown}");
    assert_eq!(shown.lines().count(), 8, "{shown}");
    let from = server.cli(&["log", "6"]);
    assert!(from.starts_with(lines[5]), "{from}");
}

#[test]
fn without_the_option_no_file_is_touched() {
    let server = Server::start("none", false);
    server.cli(&["lookup", "/"]);
    assert!(!server.dir.join("log").exists());
}
