//! The cli library in-process: the 01.5 script through `input`, and
//! completion from the tree, without a terminal.

use std::path::PathBuf;
use std::rc::Rc;

use reovim_client::{Carrier, Client, Fault};
use reovim_client_cli::{complete, input};
use reovim_host::clock;
use reovim_kernel_interface::{Booted, Error};
use reovim_kernel_runtime::{Connection, Kernel, Limits, NoLoader};

struct Dir(PathBuf);

impl Dir {
    fn new(test: &str) -> Self {
        let name = format!("reovim-cli-lib-{}-{test}", std::process::id());
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

struct Local {
    kernel: Rc<Kernel>,
    connection: Connection,
}

impl Carrier for Local {
    fn exchange(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Fault> {
        self.connection
            .handle(&self.kernel, bytes, &mut NoLoader)
            .ok_or(Fault::Wire)
    }
}

struct Rig {
    dir: Dir,
    kernel: Rc<Kernel>,
}

fn load(kernel: &Kernel, name: &str, booted: &Booted) {
    kernel.load(name, booted.module.clone(), booted.drivers.clone()).unwrap();
}

impl Rig {
    fn new(test: &str) -> Self {
        let dir = Dir::new(test);
        let kernel = Kernel::new(clock::now, Limits::default());
        load(&kernel, "fs", &reovim_module_fs::boot(dir.0.to_str().unwrap()));
        load(&kernel, "text", &reovim_module_text::boot(""));
        Self { dir, kernel: Rc::new(kernel) }
    }

    fn client(&self) -> Client {
        let connection = Connection::new();
        let carrier = Local { kernel: self.kernel.clone(), connection };
        Client::connect(Box::new(carrier), Vec::new()).unwrap()
    }

    fn file(&self, name: &str, content: &[u8]) {
        std::fs::write(self.dir.0.join(name), content).unwrap();
    }

    fn disk(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.dir.0.join(name)).unwrap()
    }
}

/// One command line; its output as text.
fn run(client: &mut Client, line: &str) -> Result<String, Fault> {
    let words: Vec<String> =
        line.split_whitespace().map(str::to_string).collect();
    input(client, &words).map(|out| String::from_utf8(out).unwrap())
}

fn line(client: &mut Client, command: &str) -> String {
    run(client, command).unwrap().trim_end().to_string()
}

#[test]
fn the_script_of_the_process_test_runs_in_process() {
    let rig = Rig::new("script");
    rig.file("note", b"first\n");
    let mut client = rig.client();
    let file = line(&mut client, "lookup /module/fs/root/note");
    let open = format!("call /module/text/open open {file}");
    let buffer = line(&mut client, &open);
    let target = format!("rd:{buffer}");

    assert_eq!(line(&mut client, &format!("write {target} second")), "6");
    assert_eq!(line(&mut client, &format!("read {target}")), "second");
    assert_eq!(rig.disk("note"), b"first\n", "unchanged before save");
    assert_eq!(line(&mut client, &format!("call {target} save")), "");
    assert_eq!(rig.disk("note"), b"second");

    let listed = line(&mut client, "children /module/text");
    assert!(listed.contains(&format!("{buffer} {file}")), "{listed}");
    let info = line(&mut client, &format!("info {target}"));
    assert!(info.contains("flags=READ|WRITE|CONTAINER"), "{info}");
    let pointer = format!("read /module/text/{file}/source");
    let source =
        input(&mut client, &["read".to_string(), pointer[5..].to_string()])
            .unwrap();
    let id = u64::from_le_bytes(source[..].try_into().unwrap());
    assert_eq!(id.to_string(), file);

    // The log: every entry from 1, then from a later seq only.
    let all = line(&mut client, "log");
    assert!(all.lines().count() > 3, "{all}");
    assert!(all.contains("connect session="), "{all}");
    assert!(all.contains("call rd="), "{all}");
    let last = all.lines().last().unwrap();
    let seq: u64 = last.split(' ').next().unwrap().parse().unwrap();
    assert_eq!(line(&mut client, &format!("log {seq}")), last);
}

#[test]
fn faults_before_and_after_sending() {
    let rig = Rig::new("faults");
    let mut client = rig.client();
    let err = |r: Result<String, Fault>| r.unwrap_err();
    assert!(matches!(err(run(&mut client, "")), Fault::Usage(_)));
    assert!(matches!(err(run(&mut client, "nothing /")), Fault::Usage(_)));
    assert!(matches!(err(run(&mut client, "lookup")), Fault::Usage(_)));
    assert!(matches!(err(run(&mut client, "lookup / x")), Fault::Usage(_)));
    let bad = err(run(&mut client, "call /module/fs/root create"));
    assert!(bad.to_string().contains("does not fit"), "{bad}");
    let none = err(run(&mut client, "call /module/fs/root nothing"));
    assert!(none.to_string().contains("no method"), "{none}");
    let refused = err(run(&mut client, "call /module/fs/root create a/b"));
    assert_eq!(refused, Fault::Kernel(Error::Invalid));
    let made = line(&mut client, "call /module/fs/root create made");
    assert!(made.parse::<u64>().is_ok(), "an rd: {made}");
    assert!(rig.dir.0.join("made").exists());
}

#[test]
fn completion_offers_commands_paths_and_methods() {
    let rig = Rig::new("complete");
    rig.file("note", b"");
    rig.file("other", b"");
    let mut client = rig.client();
    let offer = |client: &mut Client, line: &str| {
        let c = complete(client, line);
        (c.from, c.candidates)
    };

    assert_eq!(offer(&mut client, "loo"), (0, vec!["lookup".to_string()]));
    assert_eq!(offer(&mut client, "lo").1.len(), 3, "lookup, log, load");
    let (_, all) = offer(&mut client, "");
    assert!(all.contains(&"quit".to_string()) && all.len() > 5, "{all:?}");
    assert_eq!(
        offer(&mut client, "lookup /mod"),
        (7, vec!["/module/".to_string()])
    );
    assert_eq!(
        offer(&mut client, "read /module/fs/root/no"),
        (5, vec!["/module/fs/root/note".to_string()])
    );
    let (_, both) = offer(&mut client, "read /module/fs/root/");
    assert_eq!(both.len(), 2, "{both:?}");
    assert_eq!(
        offer(&mut client, "call /module/text/open op"),
        (23, vec!["open".to_string()])
    );
    assert_eq!(
        offer(&mut client, "call /module/fs/root "),
        (21, vec!["create".to_string()])
    );
    // Nowhere, no slash, an rd, a third word, `log`: nothing.
    let none: Vec<String> = Vec::new();
    for line in
        ["lookup /none/x", "lookup mod", "lookup rd:3", "lookup / /", "log /"]
    {
        assert_eq!(offer(&mut client, line).1, none, "{line}");
    }
}
