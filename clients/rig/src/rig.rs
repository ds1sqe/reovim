use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use reovim_client::{Carrier, Client, Fault};
use reovim_host::{clock, mem};
use reovim_kernel_runtime::{Connection, Heap, Kernel, Limits, NoLoader};
use reovim_module_clipboard::Clipboard;

/// The process heap as the host counts it, in the Kernel's own shape.
/// The rig passes what the server passes, so a test reading `/sys/mem`
/// reads the same numbers the editor does.
fn heap() -> Heap {
    let counted = mem::heap();
    Heap { live: counted.live, peak: counted.peak, allocs: counted.allocs }
}

/// A Carrier over a `Connection` on this thread, no link.
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

/// One Kernel with fs mirroring a fresh directory, text, a clipboard
/// over a store of its own, and the Drivers under `/bin` the server
/// links; the directory goes with it.
pub struct Rig {
    dir: PathBuf,
    kernel: Rc<Kernel>,
    clipboard: Rc<RefCell<Vec<u8>>>,
}

impl Rig {
    /// For the test named `test`, with the default limits.
    ///
    /// # Panics
    /// When the directory cannot be made or a Module refuses to load.
    #[must_use]
    pub fn new(test: &str) -> Self {
        Self::with(test, Limits::default())
    }

    /// For the test named `test`, with `limits`.
    ///
    /// # Panics
    /// As [`Self::new`].
    #[must_use]
    pub fn with(test: &str, limits: Limits) -> Self {
        let name = format!("reovim-rig-{}-{test}", std::process::id());
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let kernel = Kernel::new(clock::now, heap, limits);
        let clipboard = Rc::new(RefCell::new(Vec::new()));
        let (reads, writes) = (clipboard.clone(), clipboard.clone());
        let read = move || Ok(reads.borrow().clone());
        let write = move |bytes: &[u8]| {
            *writes.borrow_mut() = bytes.to_vec();
            Ok(())
        };
        let clip = Rc::new(Clipboard::new(Box::new(read), Box::new(write)));
        let booted = [
            ("fs", reovim_module_fs::boot(dir.to_str().unwrap())),
            ("text", reovim_module_text::boot("")),
            ("coreutils", reovim_module_coreutils::boot("")),
        ];
        for (name, module) in booted {
            kernel.load(name, module.module, module.drivers).unwrap();
        }
        kernel.load("clipboard", clip.clone(), vec![clip]).unwrap();
        Self { dir, kernel: Rc::new(kernel), clipboard }
    }

    /// What the clipboard holds.
    #[must_use]
    pub fn clipboard(&self) -> Vec<u8> {
        self.clipboard.borrow().clone()
    }

    /// The clipboard holds `bytes`, as if another program put them.
    pub fn clip(&self, bytes: &[u8]) {
        *self.clipboard.borrow_mut() = bytes.to_vec();
    }

    /// A connected Client of its own.
    ///
    /// # Panics
    /// When the Kernel refuses the connection.
    #[must_use]
    pub fn client(&self) -> Client {
        let connection = Connection::new();
        let carrier = Local { kernel: self.kernel.clone(), connection };
        Client::connect(Box::new(carrier), Vec::new()).unwrap()
    }

    /// The directory fs mirrors.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// `name` in the directory holds `content`.
    ///
    /// # Panics
    /// When the file cannot be written.
    pub fn file(&self, name: &str, content: &[u8]) {
        std::fs::write(self.dir.join(name), content).unwrap();
    }

    /// What `name` in the directory holds.
    ///
    /// # Panics
    /// When there is no such file.
    #[must_use]
    pub fn disk(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.dir.join(name)).unwrap()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
