//! The tui on a Kernel with fs and text, beside a second Client.

use std::path::PathBuf;
use std::rc::Rc;

use reovim_client::{Carrier, Client, Error, Fault, Key, Rd};
use reovim_client_tui::{Step, Tui};
use reovim_host::clock;
use reovim_kernel_interface::Booted;
use reovim_kernel_runtime::{Connection, Kernel, Limits, NoLoader};

struct Dir(PathBuf);

impl Dir {
    fn new(test: &str) -> Self {
        let name = format!("reovim-tui-{}-{test}", std::process::id());
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
        let connection = Connection::new(&self.kernel);
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

fn buffer_of(client: &mut Client, tui: &Tui) -> Rd {
    let buffer = tui.buffer();
    let name = client.name(buffer).unwrap();
    assert_eq!(client.resolve(&format!("/module/text/{name}")), Ok(buffer));
    buffer
}

#[test]
fn keys_change_the_draft_and_save_reaches_disk() {
    let rig = Rig::new("keys");
    rig.file("note", b"first\n");
    let mut tui = Tui::open(rig.client(), "note").unwrap();
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);

    assert_eq!(tui.key(Key::Tab), Ok(Step::Same));
    assert_eq!(tui.key(Key::Char('x')), Ok(Step::Changed));
    assert_eq!(tui.key(Key::End), Ok(Step::Changed));
    assert_eq!(tui.key(Key::Char('!')), Ok(Step::Changed));
    assert_eq!(other.read_all(buffer).unwrap(), b"xfirst!\n");
    assert_eq!(rig.disk("note"), b"first\n", "disk waits for save");
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["xfirst!", ""]);
    assert_eq!(frame.cursor, (0, 7));
    assert!(frame.status.starts_with("note  1:8  r3[+]"), "{}", frame.status);

    assert_eq!(tui.tick(), Ok(Step::Same), "its own edits change nothing");
    assert_eq!(tui.key(Key::Ctrl('s')), Ok(Step::Changed));
    assert_eq!(rig.disk("note"), b"xfirst!\n");
    assert!(tui.frame(80, 24).status.starts_with("note  1:8  r3   Ctrl"));
    assert_eq!(tui.key(Key::Ctrl('q')), Ok(Step::Quit));
}

#[test]
fn undo_and_redo_walk_the_revisions() {
    let rig = Rig::new("undo");
    rig.file("note", b"first\n");
    let mut tui = Tui::open(rig.client(), "note").unwrap();
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);

    assert_eq!(tui.key(Key::Char('x')), Ok(Step::Changed));
    assert_eq!(tui.key(Key::Ctrl('s')), Ok(Step::Changed));
    assert_eq!(tui.key(Key::Char('y')), Ok(Step::Changed));
    assert!(tui.frame(80, 24).status.contains("r3[+]"));
    assert_eq!(tui.key(Key::Ctrl('z')), Ok(Step::Changed));
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["xfirst", ""]);
    assert!(frame.status.contains("1:3  r2   "), "{}", frame.status);
    assert_eq!(tui.tick(), Ok(Step::Same));
    assert_eq!(tui.key(Key::Ctrl('y')), Ok(Step::Changed));
    assert_eq!(tui.frame(80, 24).lines, vec!["xyfirst", ""]);
    assert_eq!(tui.key(Key::Ctrl('z')), Ok(Step::Changed));
    assert_eq!(tui.key(Key::Ctrl('z')), Ok(Step::Changed));
    assert_eq!(tui.key(Key::Ctrl('z')), Err(Fault::Kernel(Error::Invalid)));

    other.call_by_name(buffer, "redo", Vec::new()).unwrap();
    assert_eq!(tui.tick(), Ok(Step::Changed));
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["xfirst", ""]);
    assert!(frame.status.contains("r2   "), "{}", frame.status);
    other.call_by_name(buffer, "redo", Vec::new()).unwrap();
    other.call_by_name(buffer, "save", Vec::new()).unwrap();
    assert_eq!(tui.tick(), Ok(Step::Changed), "a save by another shows");
    assert!(tui.frame(80, 24).status.contains("r3   "));
}

#[test]
fn a_write_from_another_client_shows_after_tick() {
    let rig = Rig::new("modify");
    rig.file("note", b"first\n");
    let mut tui = Tui::open(rig.client(), "note").unwrap();
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);

    assert_eq!(tui.tick(), Ok(Step::Same));
    other.write_all(buffer, b"other\nlines".to_vec()).unwrap();
    assert_eq!(tui.tick(), Ok(Step::Changed));
    assert_eq!(tui.tick(), Ok(Step::Same));
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["other", "lines"]);
    assert!(frame.status.contains("[+]"), "{}", frame.status);
    assert_eq!(tui.key(Key::Down), Ok(Step::Changed));
    assert_eq!(tui.frame(80, 24).cursor, (1, 0));
}

#[test]
fn a_missing_file_does_not_open() {
    let rig = Rig::new("missing");
    let Err(fault) = Tui::open(rig.client(), "none") else { panic!("opened") };
    assert_eq!(fault, Fault::Kernel(Error::Invalid));
}
