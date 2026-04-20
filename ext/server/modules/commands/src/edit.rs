//! Edit command - open/reload files.
//!
//! Implements the `:e` (edit) command for opening files in buffers.

use std::{path::Path, sync::Arc};

use {
    reovim_driver_codec::{
        CodecSessionState, ContentClassifierStore, ContentCodecFactoryStore, ContentType,
    },
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_text_session::{BufferApi, ExtensionApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, ModuleId, events::kernel::FileOpened},
    reovim_provider_text::VirtualBuffer,
    reovim_subsys_vfs::{FileMapping, VfsDriver},
};

const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

/// File size threshold for large file detection (64 MB).
/// Files larger than this are opened via mmap + `VirtualBuffer` instead of Rope.
const LARGE_FILE_THRESHOLD: u64 = 64 * 1024 * 1024;

/// Edit command - open a file in the current buffer.
#[derive(Debug, Clone, Copy)]
pub struct EditCommand;

impl Command for EditCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "edit")
    }

    fn description(&self) -> &'static str {
        "Edit (open) a file in the current buffer"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("file", ArgKind::Rest, "File to edit")]
    }

    fn names(&self) -> &[&'static str] {
        &["e", "edit"]
    }
}

// Needs VFS + codec pipeline — tested by integration tests.
#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for EditCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        // Get the filename argument
        let Some(filename) = ctx.string("file") else {
            return CommandResult::Error(
                "invalid arguments: No filename specified. Reload not yet implemented.".to_string(),
            );
        };

        // Get the buffer to operate on
        let Some(buffer_id) = ctx.buffer_id() else {
            return CommandResult::Error("no buffer".to_string());
        };

        // Get VFS for file operations
        let Some(vfs) = ctx.vfs() else {
            return CommandResult::Error("execution failed: VFS not available".to_string());
        };

        let path = Path::new(filename);

        // Check file size to decide between VirtualBuffer (large) and Rope (small).
        let file_size = vfs.metadata(path).map_or(0, |m| m.size);

        if file_size > LARGE_FILE_THRESHOLD {
            return open_large_file(runtime, vfs.as_ref(), path, filename);
        }

        // Small file path — existing Rope-based loading
        let bytes = match vfs.read(path) {
            Ok(b) => b,
            Err(e) => {
                return CommandResult::Error(format!(
                    "execution failed: Cannot read file '{filename}': {e}"
                ));
            }
        };

        // Decode via codec pipeline
        let content = match decode_file_content(&bytes, filename, runtime) {
            Ok(text) => text,
            Err(e) => {
                return CommandResult::Error(format!("execution failed: {e}"));
            }
        };

        // Get buffer and set content, then emit events.
        // Scope the immutable borrow so we can call
        // `record_buffer_modified` (which needs `&mut self`) afterwards.
        {
            let Some(buffer_arc) = runtime.text_buffer(buffer_id) else {
                return CommandResult::Error(format!(
                    "execution failed: Buffer {} not found",
                    buffer_id.as_usize()
                ));
            };

            // Canonicalize the path once so find_project_root can walk
            // parent directories regardless of the server's working directory.
            let canonical_path = std::fs::canonicalize(filename)
                .map_or_else(|_| filename.to_string(), |p| p.to_string_lossy().into_owned());

            // Update buffer content
            {
                let mut buffer = buffer_arc.write();
                buffer.set_content(&content);
                buffer.set_file_path(Some(canonical_path.clone()));
                buffer.set_modified(false);
            }

            // Emit FileOpened event for subscribers (LSP, syntax, etc.)
            #[allow(clippy::cast_possible_truncation)]
            let buffer_id_raw = buffer_id.as_usize() as u64;
            runtime.kernel().event_bus.emit(FileOpened {
                buffer_id: buffer_id_raw,
                path: canonical_path,
            });
        }

        // Record buffer modification so the notification pipeline emits
        // `TextBufferModified` to TUI clients, triggering a buffer cache
        // refresh and viewport redraw.
        runtime.record_buffer_modified(buffer_id);

        CommandResult::Success
    }
}

/// Open a large file via mmap + `VirtualBuffer` (zero-copy path).
///
/// Called when file size exceeds the large file threshold.
/// Memory-maps the file and creates a `VirtualBuffer` (which validates
/// UTF-8 and builds its line index internally), then registers it in the
/// unified buffer manager.
#[cfg_attr(coverage_nightly, coverage(off))]
fn open_large_file(
    runtime: &mut SessionRuntime<'_>,
    vfs: &dyn VfsDriver,
    path: &Path,
    filename: &str,
) -> CommandResult {
    // Memory-map the file for zero-copy access
    let Ok(mapped_file) = vfs.mmap_read(path) else {
        return CommandResult::Error(format!("execution failed: Cannot mmap file '{filename}'"));
    };

    // Create VirtualBuffer backed by the mmap — validates UTF-8 internally
    let original: Arc<dyn FileMapping> = Arc::new(mapped_file);
    let Ok(mut vbuf) = VirtualBuffer::from_mapping(original) else {
        // Non-UTF-8 large file — try streaming codec decode
        return open_large_binary(runtime, vfs, path, filename);
    };

    // Canonicalize the path
    let canonical_path = std::fs::canonicalize(filename)
        .map_or_else(|_| filename.to_string(), |p| p.to_string_lossy().into_owned());
    vbuf.set_file_path(Some(canonical_path.clone()));

    // Register in both TextBufferRegistry (text access) and kernel BufferManager (byte identity).
    let arc = Arc::new(reovim_arch::sync::RwLock::new(vbuf));
    if let Some(reg) = runtime
        .kernel()
        .services
        .get::<reovim_provider_text::TextBufferRegistry>()
    {
        reg.register(arc.clone());
    }
    let vbuf_id = runtime.kernel().buffers.register(arc);

    // Emit FileOpened event for subscribers (LSP, syntax, etc.)
    #[allow(clippy::cast_possible_truncation)]
    let buffer_id_raw = vbuf_id.as_usize() as u64;
    runtime.kernel().event_bus.emit(FileOpened {
        buffer_id: buffer_id_raw,
        path: canonical_path,
    });

    // Switch active buffer to the new virtual buffer
    runtime.set_active_buffer(Some(vbuf_id));
    runtime.record_buffer_modified(vbuf_id);

    CommandResult::Success
}

/// Open a large binary file via streaming codec decode.
///
/// Called when a large file fails UTF-8 validation.  Tries the codec
/// pipeline's `decode_streaming()` method which reads only headers.
/// Falls back to full decode if streaming is not supported and the
/// file is below 256 MB; returns an error for larger files.
#[cfg_attr(coverage_nightly, coverage(off))]
fn open_large_binary(
    runtime: &mut SessionRuntime<'_>,
    vfs: &dyn VfsDriver,
    path: &Path,
    filename: &str,
) -> CommandResult {
    // 256 MB hard limit for non-streaming binary decode
    const BINARY_SIZE_LIMIT: u64 = 256 * 1024 * 1024;

    let file_size = vfs.metadata(path).map_or(0, |m| m.size);

    // Try streaming decode via codec pipeline
    let services = &runtime.kernel().services;
    let classifier_store = services.get::<ContentClassifierStore>();
    let factory_store = services.get::<ContentCodecFactoryStore>();

    if let (Some(classifiers), Some(factories)) = (&classifier_store, &factory_store)
        && let Ok(mut handle) = vfs.open(path, reovim_subsys_vfs::OpenOptions::read())
    {
        let mut header = [0u8; 64];
        let _ = handle.read(&mut header);
        let _ = handle.seek(reovim_subsys_vfs::SeekFrom::Start(0));

        if let Some(content_type) = classifiers.classify(&header, filename)
            && let Some(codec) = factories.find(&content_type)
        {
            let Some(buffer_id) = runtime.active_buffer() else {
                return CommandResult::Error("no active buffer".to_string());
            };
            if let Some(result) = codec.decode_streaming(handle.as_mut(), file_size) {
                match result {
                    Ok(decode_result) => {
                        if handle.seek(reovim_subsys_vfs::SeekFrom::Start(0)).is_ok() {
                            let mut canonical = Vec::new();
                            if handle.read_to_end(&mut canonical).is_ok() {
                                if let Some(codec_state) =
                                    runtime.shared_ext_mut::<CodecSessionState>()
                                {
                                    codec_state.mount_decoded(
                                        buffer_id,
                                        decode_result.metadata.clone(),
                                        "default".to_string(),
                                        canonical,
                                        Arc::clone(&codec),
                                    );
                                }

                                return finish_decoded_open(
                                    runtime,
                                    filename,
                                    &decode_result.content,
                                );
                            }
                        }

                        return CommandResult::Error(format!(
                            "execution failed: cannot read canonical bytes for '{filename}'"
                        ));
                    }
                    Err(e) => {
                        tracing::warn!("Streaming decode failed for {filename}: {e}");
                    }
                }
            }
        }
    }

    // Fallback: full read if under size limit
    if file_size > BINARY_SIZE_LIMIT {
        return CommandResult::Error(format!(
            "execution failed: File '{filename}' is too large ({} MB) \
             and no streaming codec is available",
            file_size / (1024 * 1024)
        ));
    }

    // Under limit — fall back to normal read + codec pipeline
    let bytes = match vfs.read(path) {
        Ok(b) => b,
        Err(e) => {
            return CommandResult::Error(format!(
                "execution failed: Cannot read file '{filename}': {e}"
            ));
        }
    };

    match decode_file_content(&bytes, filename, runtime) {
        Ok(content) => finish_decoded_open(runtime, filename, &content),
        Err(e) => CommandResult::Error(format!("execution failed: {e}")),
    }
}

/// Finish opening a decoded file by loading content into the active buffer.
#[cfg_attr(coverage_nightly, coverage(off))]
fn finish_decoded_open(
    runtime: &mut SessionRuntime<'_>,
    filename: &str,
    content: &str,
) -> CommandResult {
    let Some(buffer_id) = runtime.active_buffer() else {
        return CommandResult::Error("no buffer".to_string());
    };

    let canonical_path = std::fs::canonicalize(filename)
        .map_or_else(|_| filename.to_string(), |p| p.to_string_lossy().into_owned());

    {
        let Some(buffer_arc) = runtime.text_buffer(buffer_id) else {
            return CommandResult::Error(format!(
                "execution failed: Buffer {} not found",
                buffer_id.as_usize()
            ));
        };

        {
            let mut buffer = buffer_arc.write();
            buffer.set_content(content);
            buffer.set_file_path(Some(canonical_path.clone()));
            buffer.set_modified(false);
        }

        #[allow(clippy::cast_possible_truncation)]
        let buffer_id_raw = buffer_id.as_usize() as u64;
        runtime.kernel().event_bus.emit(FileOpened {
            buffer_id: buffer_id_raw,
            path: canonical_path,
        });
    }

    runtime.record_buffer_modified(buffer_id);

    CommandResult::Success
}

/// Decode file content through the codec pipeline.
///
/// Uses the classifier store to detect content type, then the factory store
/// to find and invoke the appropriate codec. Falls back to `String::from_utf8`
/// if no codec modules are loaded.
///
/// Stores codec metadata in `CodecSessionState` for round-trip save.
///
/// # Errors
///
/// - **No active buffer** (`#740` Phase 0, B5): returns `Err("no active buffer")`
///   when `runtime.active_buffer()` is `None`. Previously this case silently
///   dropped codec metadata, leaving a later `:w` to fall back to a UTF-8
///   encode that corrupted binary files. Phase 3 migrates this guard to
///   `EditError::NoActiveBuffer`.
/// - **Invalid UTF-8** when no codec applies and the bytes are not UTF-8.
fn decode_file_content(
    bytes: &[u8],
    filename: &str,
    runtime: &mut SessionRuntime<'_>,
) -> Result<String, String> {
    // #740 Phase 0 (B5): refuse to decode without an active buffer instead
    // of silently dropping codec metadata. The downstream `:w` would have
    // re-encoded as UTF-8 and corrupted binary files.
    let buffer_id = runtime
        .active_buffer()
        .ok_or_else(|| "no active buffer".to_string())?;

    // Try to use the codec pipeline
    let services = &runtime.kernel().services;
    let classifier_store = services.get::<ContentClassifierStore>();
    let factory_store = services.get::<ContentCodecFactoryStore>();

    if let (Some(classifiers), Some(factories)) = (classifier_store, factory_store) {
        // Classify the content type (None = treat as UTF-8 fallback)
        let content_type = classifiers
            .classify(bytes, filename)
            .unwrap_or_else(|| ContentType::new(ContentType::UTF8));

        // Find and create the codec
        if let Some(codec) = factories.find(&content_type) {
            match codec.decode(bytes) {
                Ok(result) => {
                    if result.truncated {
                        tracing::warn!(
                            filename,
                            "File content was truncated by codec (see buffer for details)"
                        );
                    }

                    let content = result.content;

                    // Store metadata + canonical inode bytes in shared extensions
                    // (per-buffer, not per-client) for round-trip save and view switching.
                    // Use `.map()` instead of `if let Some` to avoid an untestable
                    // MC/DC branch on the pattern match.
                    let metadata = result.metadata;
                    let source_bytes = bytes.to_vec();
                    let _ = runtime
                        .shared_ext_mut::<CodecSessionState>()
                        .map(|codec_state| {
                            codec_state.mount_decoded(
                                buffer_id,
                                metadata,
                                "default".to_string(),
                                source_bytes,
                                codec,
                            );
                        });

                    return Ok(content);
                }
                Err(e) => {
                    tracing::warn!(
                        "Codec decode failed for {filename}: {e}, falling back to UTF-8"
                    );
                }
            }
        }
    }

    // Graceful fallback: no codec module loaded or codec not found
    String::from_utf8(bytes.to_vec()).map_err(|e| {
        let offset = e.utf8_error().valid_up_to();
        format!("File is not valid UTF-8 (invalid byte at offset {offset})")
    })
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
