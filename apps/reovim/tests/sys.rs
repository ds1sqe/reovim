//! `reovim server --sys NAME=VALUE`: the kernel command line, writing
//! `/sys/NAME` before the first Module loads. Through real processes.

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
    /// A server with `sys` on its command line, once it listens.
    fn start(test: &str, sys: &[&str]) -> Self {
        let (dir, socket) = place(test);
        let mut command = Command::new(env!("CARGO_BIN_EXE_reovim"));
        command.args(["server", "--socket", &socket, "--root"]).arg(&dir);
        for one in sys {
            command.args(["--sys", one]);
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
        assert!(out.status.success(), "{}", text(&out.stderr));
        text(&out.stdout)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

/// A directory of this test's own, and the socket in it.
fn place(test: &str) -> (PathBuf, String) {
    let name = format!("reovim-sys-{}-{test}", std::process::id());
    let dir = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let socket = dir.join("s").to_str().unwrap().to_string();
    (dir, socket)
}

#[test]
fn a_boot_parameter_is_set_before_the_first_module() {
    let sys = ["loglevel=debug", "limit/log_ring=256"];
    let server = Server::start("boot", &sys);
    // The level was already `debug` when the Modules loaded, so their
    // loading was traced; the ring was already 256, so it kept 256. Read
    // before anything else: at debug a run under `/bin` traces a couple
    // of dozen ops, and the ring keeps the boot for only a few of them.
    let said = server.cli(&["dmesg", "-t"]);
    assert!(said.contains("load module=fs"), "{said}");
    assert!(said.lines().count() <= 256, "{said}");
    assert!(said.lines().any(|line| line.starts_with("op=")), "{said}");
    assert_eq!(server.cli(&["cat", "/sys/loglevel"]), "debug");
    assert_eq!(server.cli(&["cat", "/sys/limit/log_ring"]), "256");
}

#[test]
fn a_parameter_the_kernel_refuses_refuses_the_start() {
    let (dir, socket) = place("refused");
    let out = Command::new(env!("CARGO_BIN_EXE_reovim"))
        .args(["server", "--socket", &socket, "--root"])
        .arg(&dir)
        .args(["--sys", "limit/reentry=x"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success(), "the server started anyway");
    assert_eq!(
        text(&out.stderr).trim_end(),
        "reovim: --sys limit/reentry=x: invalid"
    );
    // Nothing listens: the Kernel never reached its first Module.
    assert!(link::open(&socket).is_err());
    // A name that is no parameter is refused the same way.
    let out = Command::new(env!("CARGO_BIN_EXE_reovim"))
        .args(["server", "--socket", &socket, "--root"])
        .arg(&dir)
        .args(["--sys", "nothing=1"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        text(&out.stderr).trim_end(),
        "reovim: --sys nothing=1: invalid"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
