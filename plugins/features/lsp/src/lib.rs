//! LSP integration plugin for reovim.
//!
//! This plugin provides Language Server Protocol support:
//! - Document synchronization (`didOpen`, `didChange`, `didClose`)
//! - Diagnostics display (errors, warnings)
//!
//! # Architecture
//!
//! The plugin uses the saturator pattern for non-blocking LSP I/O:
//! - `LspSaturator` runs in a background tokio task
//! - `DiagnosticCache` uses `ArcSwap` for lock-free reads
//! - Event handlers schedule syncs with debouncing
//! - `LspRenderStage` sends content to the saturator when debounce elapsed

mod command;
mod document;
mod manager;
mod stage;

use std::{path::PathBuf, sync::Arc};

use {
    reovim_core::{
        bind::{CommandRef, KeymapScope},
        event_bus::{
            BufferClosed, BufferModified, EventBus, EventResult, FileOpened,
            RequestOpenFileAtPosition, ShutdownEvent,
        },
        keys,
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    },
    reovim_lsp::{
        ClientConfig, GotoDefinitionResponse, HoverContents, Location, LspSaturator, MarkedString,
    },
    tokio::sync::mpsc,
    tracing::{debug, error, info, warn},
};

/// Command IDs for LSP commands
pub mod command_id {
    use reovim_core::command::id::CommandId;

    pub const GOTO_DEFINITION: CommandId = CommandId::new("lsp_goto_definition");
    pub const GOTO_REFERENCES: CommandId = CommandId::new("lsp_goto_references");
    pub const SHOW_HOVER: CommandId = CommandId::new("lsp_show_hover");
}

pub use {
    document::{DocumentManager, DocumentState},
    manager::{LspManager, SharedLspManager},
    stage::LspRenderStage,
};

// Re-export events and commands for external use
pub use command::{
    LspGotoDefinition, LspGotoDefinitionCommand, LspGotoReferences, LspGotoReferencesCommand,
    LspShowHover, LspShowHoverCommand,
};

/// LSP integration plugin.
///
/// Manages document synchronization with language servers.
/// Currently supports rust-analyzer only.
pub struct LspPlugin {
    manager: Arc<SharedLspManager>,
}

impl Default for LspPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl LspPlugin {
    /// Create a new LSP plugin.
    #[must_use]
    pub fn new() -> Self {
        Self {
            manager: Arc::new(SharedLspManager::new()),
        }
    }

    /// Get the shared manager (for external access).
    #[must_use]
    pub fn manager(&self) -> Arc<SharedLspManager> {
        Arc::clone(&self.manager)
    }
}

impl Plugin for LspPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:lsp")
    }

    fn name(&self) -> &'static str {
        "LSP"
    }

    fn description(&self) -> &'static str {
        "Language Server Protocol integration for diagnostics and more"
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register render stage for buffer content access and sync
        let stage = Arc::new(LspRenderStage::new(Arc::clone(&self.manager)));
        ctx.register_render_stage(stage);

        // Register LSP commands
        if let Err(e) = ctx.register_command(LspGotoDefinitionCommand) {
            error!("Failed to register lsp_goto_definition command: {:?}", e);
        }
        if let Err(e) = ctx.register_command(LspGotoReferencesCommand) {
            error!("Failed to register lsp_goto_references command: {:?}", e);
        }
        if let Err(e) = ctx.register_command(LspShowHoverCommand) {
            error!("Failed to register lsp_show_hover command: {:?}", e);
        }
        debug!(
            "LspPlugin: registered commands - gd={}, gr={}, K={}",
            command_id::GOTO_DEFINITION.as_str(),
            command_id::GOTO_REFERENCES.as_str(),
            command_id::SHOW_HOVER.as_str()
        );

        // Register keybindings in editor normal mode
        let editor_normal = KeymapScope::editor_normal();

        // gd - Go to definition
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['g' 'd'],
            CommandRef::Registered(command_id::GOTO_DEFINITION),
        );

        // gr - Go to references
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['g' 'r'],
            CommandRef::Registered(command_id::GOTO_REFERENCES),
        );

        // K (Shift+k) - Show hover
        ctx.bind_key_scoped(
            editor_normal,
            keys!['K'],
            CommandRef::Registered(command_id::SHOW_HOVER),
        );

        info!("LspPlugin: registered render stage and keybindings (gd, gr, K)");
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Store in plugin state registry for other plugins to access
        registry.register(Arc::clone(&self.manager));

        debug!("LspPlugin: initialized state");
    }

    #[allow(clippy::too_many_lines)]
    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Handle file open - register document and send didOpen immediately
        {
            let state = Arc::clone(&state);
            bus.subscribe::<FileOpened, _>(100, move |event, _ctx| {
                info!(
                    buffer_id = event.buffer_id,
                    path = %event.path,
                    "LSP: FileOpened event received"
                );
                state.with_mut::<Arc<SharedLspManager>, _, _>(|manager| {
                    manager.with_mut(|m| {
                        let path = PathBuf::from(&event.path);
                        if let Some(doc) = m.documents.open_document(event.buffer_id, path.clone())
                        {
                            info!(
                                buffer_id = event.buffer_id,
                                language_id = %doc.language_id,
                                path = %event.path,
                                "LSP: opened document"
                            );

                            // Try to send didOpen immediately, or queue for later
                            if let Some(handle) = &m.handle {
                                let uri = doc.uri.clone();
                                let language_id = doc.language_id.clone();
                                let version = 1; // Initial version

                                // Read file content from disk
                                match std::fs::read_to_string(&path) {
                                    Ok(content) => {
                                        info!(
                                            buffer_id = event.buffer_id,
                                            uri = %uri.as_str(),
                                            "LSP: sending didOpen immediately"
                                        );
                                        handle.did_open(uri, language_id, version, content);
                                        m.documents.mark_opened(event.buffer_id);
                                    }
                                    Err(e) => {
                                        warn!(
                                            buffer_id = event.buffer_id,
                                            error = %e,
                                            "LSP: failed to read file for didOpen"
                                        );
                                    }
                                }
                            } else {
                                // Handle not ready yet - schedule immediate sync
                                // so render stage will send didOpen when handle is available
                                info!(
                                    buffer_id = event.buffer_id,
                                    "LSP: handle not ready, scheduling sync for render stage"
                                );
                                m.documents.schedule_immediate_sync(event.buffer_id);
                            }
                        }
                    });
                });
                EventResult::Handled
            });
        }

        // Handle buffer modifications - schedule sync with debounce
        {
            let state = Arc::clone(&state);
            bus.subscribe::<BufferModified, _>(100, move |event, _ctx| {
                state.with_mut::<Arc<SharedLspManager>, _, _>(|manager| {
                    manager.with_mut(|m| {
                        if m.documents.has_document(event.buffer_id)
                            && let Some(version) = m.documents.schedule_sync(event.buffer_id)
                        {
                            debug!(
                                buffer_id = event.buffer_id,
                                version = version,
                                "LSP: scheduled sync"
                            );
                        }
                    });
                });
                EventResult::Handled
            });
        }

        // Handle buffer close - send didClose and cleanup
        {
            let state = Arc::clone(&state);
            bus.subscribe::<BufferClosed, _>(100, move |event, _ctx| {
                state.with_mut::<Arc<SharedLspManager>, _, _>(|manager| {
                    manager.with_mut(|m| {
                        if let Some(doc) = m.documents.close_document(event.buffer_id) {
                            debug!(
                                buffer_id = event.buffer_id,
                                uri = ?doc.uri,
                                "LSP: closed document"
                            );

                            // Send didClose to the server
                            if let Some(handle) = &m.handle {
                                handle.did_close(doc.uri);
                            }
                        }
                    });
                });
                EventResult::Handled
            });
        }

        // Handle goto definition command
        {
            let state = Arc::clone(&state);
            let event_sender = bus.sender();
            bus.subscribe::<LspGotoDefinition, _>(100, move |event, _ctx| {
                info!(
                    buffer_id = event.buffer_id,
                    line = event.line,
                    column = event.column,
                    "LSP: LspGotoDefinition event received"
                );
                let buffer_id = event.buffer_id;
                let line = event.line;
                let column = event.column;

                // Get document URI and handle from manager
                let request_info = state.with::<Arc<SharedLspManager>, _, _>(|manager| {
                    manager.with(|m| {
                        let doc = m.documents.get(buffer_id)?;
                        let handle = m.handle.as_ref()?;
                        let uri = doc.uri.clone();
                        #[allow(clippy::cast_possible_truncation)]
                        let position = reovim_lsp::Position {
                            line: line as u32,       // Line numbers won't exceed u32
                            character: column as u32, // Column numbers won't exceed u32
                        };
                        let rx = handle.goto_definition(uri.clone(), position);
                        Some((uri, position, rx))
                    })
                });

                if let Some(Some((uri, _position, Some(rx)))) = request_info {
                    info!(
                        buffer_id,
                        line,
                        column,
                        uri = %uri.as_str(),
                        "LSP: goto definition request sent"
                    );

                    // Spawn async task to handle response
                    let sender = event_sender.clone();
                    tokio::spawn(async move {
                        match rx.await {
                            Ok(Ok(Some(response))) => {
                                // Extract location from response
                                let location = extract_first_location(&response);
                                if let Some(loc) = location {
                                    info!(
                                        uri = %loc.uri.as_str(),
                                        line = loc.range.start.line,
                                        col = loc.range.start.character,
                                        "LSP: navigating to definition"
                                    );
                                    // Convert file:// URI to path and emit navigation event
                                    if let Some(path) = uri_to_path(&loc.uri) {
                                        sender.try_send(RequestOpenFileAtPosition {
                                            path,
                                            line: loc.range.start.line as usize,
                                            column: loc.range.start.character as usize,
                                        });
                                    } else {
                                        warn!(uri = %loc.uri.as_str(), "LSP: cannot convert URI to file path");
                                    }
                                } else {
                                    info!("LSP: no definition location in response");
                                }
                            }
                            Ok(Ok(None)) => {
                                info!("LSP: no definition found");
                            }
                            Ok(Err(e)) => {
                                warn!("LSP: goto definition error: {}", e);
                            }
                            Err(_) => {
                                warn!("LSP: goto definition channel closed");
                            }
                        }
                    });
                } else {
                    debug!(buffer_id, "LSP: goto definition - no document or handle");
                }

                EventResult::Handled
            });
        }

        // Handle goto references command
        {
            let state = Arc::clone(&state);
            bus.subscribe::<LspGotoReferences, _>(100, move |event, _ctx| {
                let buffer_id = event.buffer_id;
                let line = event.line;
                let column = event.column;

                // Get document URI and handle from manager
                let request_info = state.with::<Arc<SharedLspManager>, _, _>(|manager| {
                    manager.with(|m| {
                        let doc = m.documents.get(buffer_id)?;
                        let handle = m.handle.as_ref()?;
                        let uri = doc.uri.clone();
                        #[allow(clippy::cast_possible_truncation)]
                        let position = reovim_lsp::Position {
                            line: line as u32,        // Line numbers won't exceed u32
                            character: column as u32, // Column numbers won't exceed u32
                        };
                        let rx = handle.references(uri.clone(), position, true);
                        Some((uri, position, rx))
                    })
                });

                if let Some(Some((uri, _position, Some(rx)))) = request_info {
                    info!(
                        buffer_id,
                        line,
                        column,
                        uri = %uri.as_str(),
                        "LSP: goto references request sent"
                    );

                    // Spawn async task to handle response
                    tokio::spawn(async move {
                        match rx.await {
                            Ok(Ok(Some(locations))) => {
                                info!(count = locations.len(), "LSP: references found");
                                // TODO: Show references in picker (telescope-style)
                            }
                            Ok(Ok(None)) => {
                                info!("LSP: no references found");
                            }
                            Ok(Err(e)) => {
                                warn!("LSP: references error: {}", e);
                            }
                            Err(_) => {
                                warn!("LSP: references channel closed");
                            }
                        }
                    });
                } else {
                    debug!(buffer_id, "LSP: goto references - no document or handle");
                }

                EventResult::Handled
            });
        }

        // Handle show hover command
        {
            let state = Arc::clone(&state);
            bus.subscribe::<LspShowHover, _>(100, move |event, _ctx| {
                let buffer_id = event.buffer_id;
                let line = event.line;
                let column = event.column;

                // Get document URI and handle from manager
                let request_info = state.with::<Arc<SharedLspManager>, _, _>(|manager| {
                    manager.with(|m| {
                        let doc = m.documents.get(buffer_id)?;
                        let handle = m.handle.as_ref()?;
                        let uri = doc.uri.clone();
                        #[allow(clippy::cast_possible_truncation)]
                        let position = reovim_lsp::Position {
                            line: line as u32,        // Line numbers won't exceed u32
                            character: column as u32, // Column numbers won't exceed u32
                        };
                        let rx = handle.hover(uri.clone(), position);
                        Some((uri, position, rx))
                    })
                });

                if let Some(Some((uri, _position, Some(rx)))) = request_info {
                    info!(
                        buffer_id,
                        line,
                        column,
                        uri = %uri.as_str(),
                        "LSP: hover request sent"
                    );

                    // Spawn async task to handle response
                    tokio::spawn(async move {
                        match rx.await {
                            Ok(Ok(Some(hover))) => {
                                // Extract hover text content
                                let content = extract_hover_text(&hover.contents);
                                if content.is_empty() {
                                    info!("LSP: hover response is empty");
                                } else {
                                    // Log hover content (TODO: display in popup)
                                    info!(
                                        content_lines = content.lines().count(),
                                        "LSP: hover content received"
                                    );
                                    // For now, log first few lines
                                    for (i, line) in content.lines().take(5).enumerate() {
                                        debug!(line_num = i, line, "LSP: hover");
                                    }
                                    if content.lines().count() > 5 {
                                        debug!(
                                            "LSP: hover ... ({} more lines)",
                                            content.lines().count() - 5
                                        );
                                    }
                                }
                            }
                            Ok(Ok(None)) => {
                                info!("LSP: no hover info");
                            }
                            Ok(Err(e)) => {
                                warn!("LSP: hover error: {}", e);
                            }
                            Err(_) => {
                                warn!("LSP: hover channel closed");
                            }
                        }
                    });
                } else {
                    debug!(buffer_id, "LSP: hover - no document or handle");
                }

                EventResult::Handled
            });
        }

        // Handle shutdown - gracefully shutdown LSP server
        {
            let state = Arc::clone(&state);
            bus.subscribe::<ShutdownEvent, _>(100, move |_event, _ctx| {
                state.with_mut::<Arc<SharedLspManager>, _, _>(|manager| {
                    manager.with_mut(|m| {
                        if m.is_running() {
                            info!("LSP: shutting down language server");
                            m.shutdown();
                        }
                    });
                });
                EventResult::Handled
            });
        }
    }

    fn boot(&self, _bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Start the LSP server in a background task
        let state_clone = Arc::clone(&state);

        tokio::spawn(async move {
            // Find the project root (look for Cargo.toml)
            let root_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

            // Check if rust-analyzer is available
            if !rust_analyzer_available() {
                warn!("LSP: rust-analyzer not found in PATH, LSP disabled");
                return;
            }

            info!(root = %root_path.display(), "LSP: starting rust-analyzer");

            let config = ClientConfig::rust_analyzer(&root_path);

            // Create a render signal channel (not used yet, but allows triggering re-renders)
            let (render_tx, _render_rx) = mpsc::channel::<()>(1);

            match LspSaturator::start(config, Some(render_tx)).await {
                Ok((handle, cache)) => {
                    info!("LSP: rust-analyzer started successfully");

                    state_clone.with_mut::<Arc<SharedLspManager>, _, _>(|manager| {
                        manager.with_mut(|m| {
                            m.set_connection(handle, cache);
                        });
                    });
                }
                Err(e) => {
                    error!("LSP: failed to start rust-analyzer: {}", e);
                }
            }
        });
    }
}

/// Check if rust-analyzer is available in PATH.
fn rust_analyzer_available() -> bool {
    std::process::Command::new("rust-analyzer")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Extract the first location from a `GotoDefinitionResponse`.
///
/// Handles all three variants: Scalar, Array, and Link.
fn extract_first_location(response: &GotoDefinitionResponse) -> Option<Location> {
    match response {
        GotoDefinitionResponse::Scalar(loc) => Some(loc.clone()),
        GotoDefinitionResponse::Array(locs) => locs.first().cloned(),
        GotoDefinitionResponse::Link(links) => links.first().map(|link| Location {
            uri: link.target_uri.clone(),
            range: link.target_selection_range,
        }),
    }
}

/// Convert a file:// URI to a filesystem path.
fn uri_to_path(uri: &reovim_lsp::Uri) -> Option<PathBuf> {
    let uri_str = uri.as_str();
    uri_str.strip_prefix("file://").map(|path_str| {
        // Handle percent-encoded characters (basic: %20 -> space)
        let decoded = path_str.replace("%20", " ");
        PathBuf::from(decoded)
    })
}

/// Extract text content from `HoverContents`.
fn extract_hover_text(contents: &HoverContents) -> String {
    match contents {
        HoverContents::Scalar(marked) => marked_string_to_text(marked),
        HoverContents::Array(items) => items
            .iter()
            .map(marked_string_to_text)
            .collect::<Vec<_>>()
            .join("\n\n"),
        HoverContents::Markup(markup) => markup.value.clone(),
    }
}

/// Convert a `MarkedString` to plain text.
fn marked_string_to_text(marked: &MarkedString) -> String {
    match marked {
        MarkedString::String(s) => s.clone(),
        MarkedString::LanguageString(ls) => {
            // Format as code block header + value
            format!("```{}\n{}\n```", ls.language, ls.value)
        }
    }
}
