//! Buffer list service.
//!
//! `BufferListService` maintains a snapshot of buffer metadata populated
//! via `EventBus` subscriptions. The bridge reads from it during `tick()`.

use std::sync::Mutex;

use reovim_kernel::api::v1::Service;

/// Lightweight buffer metadata (no diagnostics — those come from `DiagnosticSnapshot`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferEntrySnapshot {
    /// Buffer identifier.
    pub id: u64,
    /// Display name.
    pub name: String,
    /// Full file path, if file-backed.
    pub path: Option<String>,
    /// Whether the buffer has unsaved changes.
    pub modified: bool,
    /// Filetype string (e.g., "rust").
    pub filetype: Option<String>,
}

/// Service holding the current buffer list.
///
/// Registered in `ServiceRegistry` during module `init()`. Updated by
/// `EventBus` subscriptions (`BufferCreated`, `BufferClosed`, `BufferSaved`).
/// Read by `BufferlineBridge::tick()`.
pub struct BufferListService {
    entries: Mutex<Vec<BufferEntrySnapshot>>,
}

impl Service for BufferListService {}

impl Default for BufferListService {
    fn default() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }
}

impl BufferListService {
    /// Replace the entire buffer list.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn update(&self, entries: Vec<BufferEntrySnapshot>) {
        *self
            .entries
            .lock()
            .expect("BufferListService lock poisoned") = entries;
    }

    /// Get a snapshot of the current buffer list.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn snapshot(&self) -> Vec<BufferEntrySnapshot> {
        self.entries
            .lock()
            .expect("BufferListService lock poisoned")
            .clone()
    }

    /// Add a single buffer entry.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn add(&self, entry: BufferEntrySnapshot) {
        self.entries
            .lock()
            .expect("BufferListService lock poisoned")
            .push(entry);
    }

    /// Remove a buffer by ID.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn remove(&self, buffer_id: u64) {
        self.entries
            .lock()
            .expect("BufferListService lock poisoned")
            .retain(|e| e.id != buffer_id);
    }

    /// Update the modified flag for a buffer.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn set_modified(&self, buffer_id: u64, modified: bool) {
        let mut entries = self
            .entries
            .lock()
            .expect("BufferListService lock poisoned");
        if let Some(entry) = entries.iter_mut().find(|e| e.id == buffer_id) {
            entry.modified = modified;
        }
    }

    /// Update path and filetype for a buffer (on save).
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn set_path(&self, buffer_id: u64, path: String) {
        let mut entries = self
            .entries
            .lock()
            .expect("BufferListService lock poisoned");
        if let Some(entry) = entries.iter_mut().find(|e| e.id == buffer_id) {
            let filetype = guess_filetype(&path);
            entry.name = path_to_name(&path);
            entry.path = Some(path);
            entry.filetype = filetype;
        }
    }
}

/// Extract display name from a file path.
fn path_to_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Guess filetype from file extension.
fn guess_filetype(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?;
    let ft = match ext {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "jsx" => "javascriptreact",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "cxx" => "cpp",
        "java" => "java",
        "rb" => "ruby",
        "sh" | "bash" | "zsh" => "sh",
        "html" | "htm" => "html",
        "css" => "css",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "xml" => "xml",
        "md" | "markdown" => "markdown",
        "lua" => "lua",
        "vim" => "vim",
        _ => return None,
    };
    Some(ft.to_string())
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
