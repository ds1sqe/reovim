//! What every test of a real `reovim server` starts from: the process
//! over a directory of its own, `reovim cli` against it, and the kill
//! on drop. Each test binary compiles this on its own and uses what it
//! needs.
#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use reovim_host::link;

/// A server over a directory of its own, killed on drop.
pub struct Server {
    pub dir: PathBuf,
    pub socket: String,
    child: Child,
}

impl Server {
    /// A server over a directory of this test's own, once it listens.
    pub fn start(test: &str) -> Self {
        Self::with(test, &[], false)
    }

    /// The same, with `sys` on its command line and, when `log`, the
    /// `--log` file in its directory.
    pub fn with(test: &str, sys: &[&str], log: bool) -> Self {
        let (dir, socket) = place(test);
        let child = serving(&dir, &socket, sys, log)
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

    /// `reovim cli` once with `stdin`; stdout, or the failure text.
    pub fn cli(&self, args: &[&str], stdin: &[u8]) -> Result<Vec<u8>, String> {
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

    /// The same on no input, trimmed of its last newline either way.
    pub fn ran(&self, args: &[&str]) -> Result<String, String> {
        let trim = |said: String| said.trim_end().to_string();
        self.cli(args, b"")
            .map(|out| trim(String::from_utf8(out).unwrap()))
            .map_err(trim)
    }

    /// The trimmed stdout of a run that had to work.
    pub fn line(&self, args: &[&str]) -> String {
        self.ran(args).unwrap()
    }

    /// The whole stdout of a run that had to work.
    pub fn out(&self, args: &[&str]) -> String {
        match self.cli(args, b"") {
            Ok(out) => String::from_utf8(out).unwrap(),
            Err(said) => panic!("{said}"),
        }
    }

    /// `name` in the server's directory, holding `content`.
    pub fn file(&self, name: &str, content: &[u8]) -> Vec<u8> {
        std::fs::write(self.dir.join(name), content).unwrap();
        content.to_vec()
    }

    pub fn disk(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.dir.join(name)).unwrap()
    }

    /// The `--log` file once it holds `wanted` lines, within two
    /// seconds.
    pub fn logged(&self, wanted: usize) -> String {
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

    /// The rd of a target, read from the `rd` line of `stat`.
    pub fn rd(&self, target: &str) -> String {
        let said = self.line(&["stat", target]);
        let line = said.lines().find(|line| line.starts_with("rd ")).unwrap();
        line["rd ".len()..].to_string()
    }

    /// The names under `/session`, one per line into a pipe, once
    /// there are `wanted` of them or two seconds have gone. A link's
    /// end reaches the server a moment after the cli process that held
    /// it is done, so the count is asked for again until it settles.
    pub fn sessions(&self, wanted: usize) -> Vec<String> {
        let start = Instant::now();
        let listed = || {
            let said = self.line(&["ls", "/session"]);
            said.lines().map(str::to_string).collect::<Vec<_>>()
        };
        let over = || start.elapsed() > Duration::from_secs(2);
        let mut names = listed();
        while names.len() != wanted && !over() {
            std::thread::sleep(Duration::from_millis(10));
            names = listed();
        }
        names
    }

    /// hello.c built into the server's directory.
    pub fn hello(&self) -> String {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let out = self.dir.join("hello.rvm");
        let built = Command::new("cc")
            .args(["-shared", "-fPIC", "-I"])
            .arg(format!("{root}/kernel/abi/include"))
            .arg(format!("{root}/modules/hello/hello.c"))
            .arg("-o")
            .arg(&out)
            .status()
            .expect("cc");
        assert!(built.success());
        out.to_str().unwrap().to_string()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// `reovim server` over `dir`, unspawned: `sys` on its command line,
/// and the `--log` file in `dir` when `log`.
pub fn serving(dir: &Path, socket: &str, sys: &[&str], log: bool) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_reovim"));
    command.args(["server", "--socket", socket, "--root"]).arg(dir);
    for one in sys {
        command.args(["--sys", one]);
    }
    if log {
        command.arg("--log").arg(dir.join("log"));
    }
    command
}

/// A directory of this test's own, and the socket in it.
pub fn place(test: &str) -> (PathBuf, String) {
    let name = format!("reovim-{}-{test}", std::process::id());
    let dir = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let socket = dir.join("s").to_str().unwrap().to_string();
    (dir, socket)
}
