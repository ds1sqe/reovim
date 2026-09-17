//! `load` and `unload` through a real server: hello.rvm built by the
//! system cc, driven by `reovim cli`.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use reovim_host::link;
use reovim_kernel_interface::wire::{Endpoint, Message};
use reovim_kernel_interface::{Op, Out, Rd, Reply, Rights};

struct Server {
    dir: PathBuf,
    socket: String,
    child: Child,
}

impl Server {
    fn start() -> Self {
        let name = format!("reovim-native-{}", std::process::id());
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

    /// hello.c built into the server's directory.
    fn hello(&self) -> String {
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

    /// `reovim cli` once; stdout, or the failure text.
    fn cli(&self, args: &[&str]) -> Result<String, String> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_reovim"))
            .args(["cli", "--socket", &self.socket])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"").unwrap();
        let out = child.wait_with_output().unwrap();
        let text = if out.status.success() { out.stdout } else { out.stderr };
        let text = String::from_utf8(text).unwrap().trim_end().to_string();
        if out.status.success() { Ok(text) } else { Err(text) }
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
fn hello_loads_and_unloads_through_the_cli() {
    let server = Server::start();
    let hello = server.hello();
    assert_eq!(
        server.cli(&["load", "/nowhere.rvm"]),
        Err("reovim: load /nowhere.rvm: invalid".into())
    );
    assert_eq!(server.cli(&["load", &hello]), Ok(String::new()));
    assert_eq!(
        server.cli(&["load", &hello]),
        Err(format!("reovim: load {hello}: invalid")),
        "taken"
    );
    assert_eq!(server.cli(&["read", "/module/hello/count"]), Ok("0".into()));
    assert_eq!(
        server.cli(&["write", "/module/hello/count", "7"]),
        Ok("1".into())
    );
    assert_eq!(server.cli(&["read", "/module/hello/count"]), Ok("7".into()));
    assert_eq!(
        server.cli(&["unload", "fs"]),
        Err("reovim: unload fs: denied".into())
    );

    // A raw link holds a handle open: unload waits for it.
    let end = link::open(&server.socket).unwrap();
    let mut endpoint = Endpoint::default();
    let mut ask = |message: &Message| {
        let (_, bytes) = endpoint.send(message);
        link::send(&end, &bytes).unwrap();
        let (_, reply) = endpoint.receive(&link::recv(&end).unwrap()).unwrap();
        reply
    };
    ask(&Message::Connect { credential: Vec::new(), session: None });
    let count: u64 = server
        .cli(&["lookup", "/module/hello/count"])
        .unwrap()
        .parse()
        .unwrap();
    let open = Op::Open { rd: Rd(count), rights: Rights::READ };
    let Message::Reply(Reply::Ready(Ok(Out::Handle(h)))) =
        ask(&Message::Request(open))
    else {
        panic!("open")
    };
    assert_eq!(
        server.cli(&["unload", "hello"]),
        Err("reovim: unload hello: busy; try again".into())
    );
    ask(&Message::Request(Op::Close { h }));
    assert_eq!(server.cli(&["unload", "hello"]), Ok(String::new()));
    assert_eq!(
        server.cli(&["lookup", "/module/hello"]),
        Err("reovim: lookup /module/hello: no such rd (hello)".into())
    );
    let log = server.cli(&["log"]).unwrap();
    assert!(
        log.contains("load module=hello")
            && log.contains("unload module=hello"),
        "{log}"
    );
}
