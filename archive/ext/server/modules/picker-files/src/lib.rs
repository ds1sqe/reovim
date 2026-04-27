#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! File picker module for reovim.
//!
//! Provides a fuzzy file finder with `.gitignore` support via the `ignore` crate.
//! Registers `FilesPicker` in the `PickerRegistry` during module init.

use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    sync::Arc,
};

use {
    reovim_driver_picker::{
        Picker, PickerAction, PickerContext, PickerData, PickerItem, PickerRegistry,
        PreviewContent, SessionRuntime,
    },
    reovim_driver_text_session::{BufferApi, WindowApi},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    reovim_subsys_content_codec::{
        ContentClassifierStore, ContentCodecFactoryStore, FileOpenError, decode_file_bytes,
    },
    reovim_subsys_vfs::VfsInstance,
};

/// Maximum number of preview lines to read from a file.
const PREVIEW_MAX_LINES: usize = 50;

// ============================================================================
// FilesPicker
// ============================================================================

/// Picker that lists files in the working directory.
///
/// Walks files respecting `.gitignore` rules via the `ignore` crate.
/// Returns relative paths from `ctx.cwd`.
pub struct FilesPicker;

impl FilesPicker {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for FilesPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for FilesPicker {
    fn name(&self) -> &'static str {
        "files"
    }

    fn title(&self) -> &'static str {
        "Files"
    }

    fn items(
        &self,
        ctx: &PickerContext,
        _services: &reovim_kernel::api::v1::ServiceRegistry,
    ) -> Vec<PickerItem> {
        let walker = ignore::WalkBuilder::new(&ctx.cwd)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        walker
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
            .filter_map(|entry| {
                let path = entry.path();
                let relative = path.strip_prefix(&ctx.cwd).ok()?;
                let display = relative.to_string_lossy().into_owned();
                let icon = reovim_driver_picker::icon_for_path(&display);
                Some(PickerItem {
                    display,
                    detail: None,
                    data: PickerData::FilePath(path.to_path_buf()),
                    icon,
                })
            })
            .collect()
    }

    fn on_select(&self, item: &PickerItem) -> PickerAction {
        match &item.data {
            PickerData::FilePath(path) => PickerAction::OpenFile(path.clone()),
            _ => PickerAction::Close,
        }
    }

    fn preview(
        &self,
        item: &PickerItem,
        _services: &reovim_kernel::api::v1::ServiceRegistry,
    ) -> Option<PreviewContent> {
        let PickerData::FilePath(path) = &item.data else {
            return None;
        };

        // Skip binary files by checking the first few bytes.
        if is_likely_binary(path) {
            return None;
        }

        let file = fs::File::open(path).ok()?;
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader
            .lines()
            .take(PREVIEW_MAX_LINES)
            .filter_map(Result::ok)
            .collect();

        if lines.is_empty() {
            return None;
        }

        Some(PreviewContent {
            lines,
            highlight_line: None,
            file_path: Some(path.clone()),
            ..Default::default()
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, action: PickerAction, runtime: &mut SessionRuntime<'_>) {
        if let PickerAction::OpenFile(path) = action {
            open_file(runtime, &path);
        }
    }
}

/// Check if a file is likely binary by reading the first 512 bytes.
fn is_likely_binary(path: &PathBuf) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return true;
    };
    let check_len = bytes.len().min(512);
    bytes[..check_len].contains(&0)
}

// ============================================================================
// open_file utility
// ============================================================================

/// Open a file by path, reusing existing buffers when possible.
///
/// 1. Canonicalize the path
/// 2. Check if any buffer already has this file path
/// 3. If found: switch the active window to that buffer
/// 4. If not found: read via VFS, create buffer, switch to it
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn open_file(runtime: &mut SessionRuntime<'_>, path: &Path) {
    use reovim_kernel::api::v1::events::kernel::FileOpened;

    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical.to_string_lossy();

    // Buffer-reuse short-circuit: an existing buffer for this path is
    // re-activated without re-running the codec pipeline. Codec-driven
    // reopening is the `:mount` command's concern, not the picker's.
    let existing = runtime.kernel().buffers.list().into_iter().find(|&id| {
        runtime
            .buffer_file_path(id)
            .is_some_and(|p| Path::new(&p) == canonical)
    });

    let is_new = existing.is_none();
    let buf_id = if let Some(id) = existing {
        id
    } else {
        let Some(vfs) = runtime.kernel().services.get::<VfsInstance>() else {
            tracing::warn!(
                "picker-files: VFS not available; skipping open of {}",
                canonical.display()
            );
            return;
        };
        let bytes = match vfs.driver().read(&canonical) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("picker-files: VFS read failed for {}: {e}", canonical.display());
                return;
            }
        };
        let decode_result = {
            let services = &runtime.kernel().services;
            let class = services.get::<ContentClassifierStore>();
            let fact = services.get::<ContentCodecFactoryStore>();
            if let (Some(class), Some(fact)) = (&class, &fact) {
                decode_file_bytes(&bytes, &path_str, class, fact)
            } else {
                String::from_utf8(bytes)
                    .map(|s| (s, None))
                    .map_err(|err| FileOpenError::NotUtf8 {
                        offset: err.utf8_error().valid_up_to(),
                    })
            }
        };
        let text = match decode_result {
            Ok((text, _content_type)) => text,
            Err(FileOpenError::TooLarge { len, .. }) => {
                tracing::warn!(
                    "picker-files: file too large ({len} bytes); use :e to open via the streaming path: {}",
                    canonical.display()
                );
                return;
            }
            Err(FileOpenError::NotUtf8 { offset }) => {
                tracing::warn!(
                    "picker-files: cannot decode {} (not valid UTF-8 at offset {offset})",
                    canonical.display()
                );
                return;
            }
        };
        let id = runtime.create_buffer(Some(&path_str), &text);
        runtime.set_buffer_modified(id, false);
        id
    };

    if let Some(win) = runtime.active_window() {
        let _ = runtime.set_window_buffer(win, buf_id);
    }
    runtime.set_active_buffer(Some(buf_id));

    if is_new {
        runtime.record_buffer_modified(buf_id);
        #[allow(clippy::cast_possible_truncation)]
        let buf_id_raw = buf_id.as_usize() as u64;
        runtime.kernel().event_bus.emit(FileOpened {
            buffer_id: buf_id_raw,
            path: path_str.to_string(),
        });
    }
}

// ============================================================================
// Module implementation
// ============================================================================

/// File picker module.
///
/// Registers `FilesPicker` in `PickerRegistry` during init.
pub struct PickerFilesModule;

impl PickerFilesModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PickerFilesModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PickerFilesModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("picker-files")
    }

    fn name(&self) -> &'static str {
        "File Picker"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<PickerRegistry>();
        registry.register(Arc::new(FilesPicker));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(PickerFilesModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
