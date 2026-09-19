//! What the text cases share. Each test binary compiles this on its
//! own and uses what it needs.

use std::path::PathBuf;

/// A directory of its own for one test, removed on drop.
pub struct Dir(pub PathBuf);

impl Dir {
    pub fn new(test: &str) -> Self {
        let name = format!("reovim-text-{}-{test}", std::process::id());
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
