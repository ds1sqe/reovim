use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use reovim_client::{Carrier, Client, Clip, Fault, Host, Rvm, bin};
use reovim_host::{clock, mem};
use reovim_kernel_runtime::{Connection, Heap, Kernel, Limits, NoLoader};
use reovim_sdk::ops;

/// The process heap as the host counts it, in the Kernel's own shape.
/// The rig passes what the server passes, so a test reading `/sys/mem`
/// reads the same numbers the editor does.
fn heap() -> Heap {
    let counted = mem::heap();
    Heap { live: counted.live, peak: counted.peak, allocs: counted.allocs }
}

/// A Carrier over a `Connection` on this thread, no link; every
/// message it moves is counted, which is how a case says an op reached
/// the server and how one says none did.
struct Local {
    kernel: Rc<Kernel>,
    connection: Connection,
    sent: Rc<Cell<u32>>,
    /// The Hosts of this rig that were there before this Client, and
    /// never its own, whose wire is in use while it waits.
    others: Vec<Weak<Host>>,
}

impl Carrier for Local {
    fn exchange(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Fault> {
        self.sent.set(self.sent.get() + 1);
        self.connection
            .handle(&self.kernel, bytes, &mut NoLoader)
            .ok_or(Fault::Wire)
    }

    /// One turn of every Client that was here first, which is what
    /// makes two Clients on one thread able to wait on each other: the
    /// `Since` brings over what the server asks of them and `serve`
    /// answers it.
    fn pause(&mut self) {
        for host in self.others.iter().filter_map(Weak::upgrade) {
            let _ = ops::since(&host.own(), 0);
            host.serve();
        }
    }
}

/// One Kernel with fs mirroring a fresh directory, text and the
/// Drivers under `/bin` the server links, beside a clipboard store of
/// its own a Client takes as its machine's; the directory goes with it.
pub struct Rig {
    dir: PathBuf,
    kernel: Rc<Kernel>,
    clipboard: Rc<RefCell<Vec<u8>>>,
    /// Messages every Client of this rig has put on the link.
    sent: Rc<Cell<u32>>,
    /// Every Host this rig has made, in the order it made them: what a
    /// later Client's waits answer.
    hosts: RefCell<Vec<Weak<Host>>>,
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
        let booted = [
            ("fs", reovim_module_fs::boot(dir.to_str().unwrap())),
            ("text", reovim_module_text::boot("")),
            ("coreutils", reovim_module_coreutils::boot("")),
            ("sysutils", reovim_module_sysutils::boot("")),
        ];
        for (name, module) in booted {
            kernel.load(name, module.module, module.drivers).unwrap();
        }
        let clipboard = Rc::new(RefCell::new(Vec::new()));
        let sent = Rc::new(Cell::new(0));
        let hosts = RefCell::new(Vec::new());
        Self { dir, kernel: Rc::new(kernel), clipboard, sent, hosts }
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

    /// This store as the machine a Client is given, for `dev::boot`:
    /// what [`Self::clip`] sets and [`Self::clipboard`] reads back.
    #[must_use]
    pub fn clipping(&self) -> Clip {
        let (reads, writes) = (self.clipboard.clone(), self.clipboard.clone());
        Clip::new(
            Box::new(move || Ok(reads.borrow().clone())),
            Box::new(move |bytes: &[u8]| {
                *writes.borrow_mut() = bytes.to_vec();
                Ok(())
            }),
        )
    }

    /// A connected Client of its own.
    ///
    /// # Panics
    /// When the Kernel refuses the connection.
    #[must_use]
    pub fn client(&self) -> Client {
        let connection = Connection::new();
        let kernel = self.kernel.clone();
        let sent = self.sent.clone();
        let others = self.hosts.borrow().clone();
        let carrier = Local { kernel, connection, sent, others };
        Client::connect(Box::new(carrier), Vec::new(), clock::now).unwrap()
    }

    /// A Client of its own behind the one door, which is what
    /// everything above the Client takes its ops through: `/self/bin`
    /// served, with no `.rvm` files behind `-l` and no Module loaded
    /// of its own, so a typed line resolves at the server's `/bin` as
    /// it did before there was a Host. It is an `Rc`, a Driver of the
    /// Client's own holding the Host it speaks for.
    ///
    /// # Panics
    /// As [`Self::client`], and when `/self/bin` refuses the two names.
    #[must_use]
    pub fn host(&self) -> Rc<Host> {
        let host = Rc::new(Host::over(self.client(), clock::now));
        bin::boot(&host, Rvm::none()).unwrap();
        self.hosts.borrow_mut().push(Rc::downgrade(&host));
        host
    }

    /// How many messages the Clients of this rig have sent, which a
    /// case counts around what it claims costs none.
    #[must_use]
    pub fn sent(&self) -> u32 {
        self.sent.get()
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
