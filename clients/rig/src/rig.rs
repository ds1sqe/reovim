use std::path::{Path, PathBuf};
use std::rc::Rc;

use reovim_client::{Carrier, Client, Fault};
use reovim_host::clock;
use reovim_kernel_runtime::{Connection, Kernel, Limits, NoLoader};

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

/// One Kernel with fs mirroring a fresh directory and text loaded;
/// the directory goes with it.
pub struct Rig {
    dir: PathBuf,
    kernel: Rc<Kernel>,
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
        let kernel = Kernel::new(clock::now, limits);
        let booted = [
            ("fs", reovim_module_fs::boot(dir.to_str().unwrap())),
            ("text", reovim_module_text::boot("")),
        ];
        for (name, module) in booted {
            kernel.load(name, module.module, module.drivers).unwrap();
        }
        Self { dir, kernel: Rc::new(kernel) }
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
