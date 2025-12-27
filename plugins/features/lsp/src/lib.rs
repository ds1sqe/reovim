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
mod hover;
mod manager;
mod picker;
mod stage;
mod window;

use std::{path::PathBuf, sync::Arc};

use {
    reovim_core::{
        bind::{CommandRef, KeymapScope},
        event_bus::{
            BufferClosed, BufferModified, CursorMoved, EventBus, EventResult, FileOpened,
            ModeChanged, RequestOpenFileAtPosition, ShutdownEvent,
        },
        keys,
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    },
    reovim_lsp::{
        ClientConfig, GotoDefinitionResponse, HoverContents, Location, LspSaturator, MarkedString,
    },
    reovim_plugin_microscope::{MicroscopeOpen, PickerRegistry},
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
    LspHoverDismiss, LspShowHover, LspShowHoverCommand,
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

        // Register hover plugin window
        registry.register_plugin_window(Arc::new(window::HoverPluginWindow::new(Arc::clone(
            &self.manager,
        ))));

        // Register LSP pickers with microscope
        let references_picker = Arc::new(picker::LspReferencesPicker::new());
        registry.register(Arc::clone(&references_picker));

        let definitions_picker = Arc::new(picker::LspDefinitionsPicker::new());
        registry.register(Arc::clone(&definitions_picker));

        registry.with_mut::<PickerRegistry, _, _>(|picker_registry| {
            picker_registry.register(references_picker);
            picker_registry.register(definitions_picker);
        });

        debug!("LspPlugin: initialized state");
    }

    #[allow(clippy::too_many_lines)]
    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Handle file open - register document and schedule sync for render stage
        // Note: We don't send didOpen here directly. The render stage handles it
        // via schedule_immediate_sync() to avoid version synchronization bugs.
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
                        if let Some(doc) = m.documents.open_document(event.buffer_id, path) {
                            info!(
                                buffer_id = event.buffer_id,
                                language_id = %doc.language_id,
                                "LSP: opened document, scheduling sync for render stage"
                            );
                            // Let render stage handle didOpen to ensure version consistency
                            m.documents.schedule_immediate_sync(event.buffer_id);
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
                    let state_clone = Arc::clone(&state);
                    tokio::spawn(async move {
                        match rx.await {
                            Ok(Ok(Some(response))) => {
                                // Extract all locations from response
                                let locations = extract_all_locations(&response);

                                if locations.is_empty() {
                                    info!("LSP: no definition location in response");
                                    return;
                                }

                                if locations.len() == 1 {
                                    // Single definition - navigate directly
                                    let loc = &locations[0];
                                    info!(
                                        uri = %loc.uri.as_str(),
                                        line = loc.range.start.line,
                                        col = loc.range.start.character,
                                        "LSP: navigating to definition"
                                    );
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
                                    // Multiple definitions - show picker
                                    info!(count = locations.len(), "LSP: multiple definitions found, opening picker");

                                    // Store definitions in picker
                                    state_clone.with::<Arc<picker::LspDefinitionsPicker>, _, _>(
                                        |definitions_picker| {
                                            definitions_picker.set_definitions(locations);
                                        },
                                    );

                                    // Open microscope with definitions picker
                                    sender.try_send(MicroscopeOpen::new("lsp_definitions"));
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
            let event_sender = bus.sender();
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
                    let state_clone = Arc::clone(&state);
                    let sender = event_sender.clone();
                    tokio::spawn(async move {
                        match rx.await {
                            Ok(Ok(Some(locations))) => {
                                info!(count = locations.len(), "LSP: references found");

                                if locations.is_empty() {
                                    info!("LSP: no references to display");
                                    return;
                                }

                                // Store references in picker
                                state_clone.with::<Arc<picker::LspReferencesPicker>, _, _>(
                                    |picker| {
                                        picker.set_references(locations);
                                    },
                                );

                                // Open microscope with references picker
                                sender.try_send(MicroscopeOpen::new("lsp_references"));
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
                    let state_clone = Arc::clone(&state);
                    tokio::spawn(async move {
                        match rx.await {
                            Ok(Ok(Some(hover))) => {
                                // Extract hover text content
                                let content = extract_hover_text(&hover.contents);
                                if content.is_empty() {
                                    info!("LSP: hover response is empty");
                                } else {
                                    info!(
                                        content_lines = content.lines().count(),
                                        "LSP: hover content received"
                                    );

                                    // Store hover content in cache for popup display
                                    state_clone.with_mut::<Arc<SharedLspManager>, _, _>(
                                        |manager| {
                                            manager.with_mut(|m| {
                                                let snapshot = hover::HoverSnapshot::new(
                                                    content, line, column, buffer_id,
                                                );
                                                m.hover_cache.store(snapshot);
                                                // Trigger re-render to show popup
                                                m.send_render_signal();
                                            });
                                        },
                                    );
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
                    info!(buffer_id, ?request_info, "LSP: hover - no document or handle");
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

        // Handle explicit hover dismiss
        {
            let state = Arc::clone(&state);
            bus.subscribe::<command::LspHoverDismiss, _>(100, move |_event, _ctx| {
                state.with_mut::<Arc<SharedLspManager>, _, _>(|manager| {
                    manager.with_mut(|m| {
                        m.hover_cache.clear();
                    });
                });
                EventResult::Handled
            });
        }

        // Dismiss hover on cursor movement
        {
            let state = Arc::clone(&state);
            bus.subscribe::<CursorMoved, _>(200, move |_event, _ctx| {
                state.with_mut::<Arc<SharedLspManager>, _, _>(|manager| {
                    manager.with_mut(|m| {
                        if m.hover_cache.is_active() {
                            m.hover_cache.clear();
                        }
                    });
                });
                EventResult::Handled
            });
        }

        // Dismiss hover on mode change
        {
            let state = Arc::clone(&state);
            bus.subscribe::<ModeChanged, _>(200, move |_event, _ctx| {
                state.with_mut::<Arc<SharedLspManager>, _, _>(|manager| {
                    manager.with_mut(|m| {
                        if m.hover_cache.is_active() {
                            m.hover_cache.clear();
                        }
                    });
                });
                EventResult::Handled
            });
        }
    }

    fn boot(
        &self,
        _bus: &EventBus,
        state: Arc<PluginStateRegistry>,
        event_tx: Option<tokio::sync::mpsc::Sender<reovim_core::event::InnerEvent>>,
    ) {
        // Store event_tx in manager for triggering re-renders
        if let Some(tx) = event_tx {
            state.with_mut::<Arc<SharedLspManager>, _, _>(|manager| {
                manager.with_mut(|m| {
                    m.set_event_tx(tx);
                });
            });
        }

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

                    // Set connection and send didOpen for pending documents
                    state_clone.with_mut::<Arc<SharedLspManager>, _, _>(|manager| {
                        manager.with_mut(|m| {
                            m.set_connection(handle, cache);

                            // Send didOpen for all documents that aren't opened yet
                            // This handles documents opened before the server was ready
                            let buffer_ids: Vec<usize> = m.documents.buffer_ids();
                            info!(
                                count = buffer_ids.len(),
                                ?buffer_ids,
                                "LSP: checking pending documents on server ready"
                            );
                            for buffer_id in buffer_ids {
                                info!(buffer_id, "LSP: processing buffer_id");
                                let Some(doc) = m.documents.get(buffer_id) else {
                                    warn!(buffer_id, "LSP: document not found for buffer_id");
                                    continue;
                                };
                                info!(
                                    buffer_id,
                                    opened = doc.opened,
                                    has_handle = m.handle.is_some(),
                                    "LSP: checking doc state"
                                );
                                if let (false, Some(handle)) = (doc.opened, &m.handle) {
                                    // Read content from disk
                                    let path = doc.path.clone();
                                    let uri = doc.uri.clone();
                                    let version = doc.version;
                                    let language_id = doc.language_id.clone();

                                    if let Ok(content) = std::fs::read_to_string(&path) {
                                        info!(
                                            buffer_id,
                                            version,
                                            uri = %uri.as_str(),
                                            "LSP: sending didOpen on server ready"
                                        );
                                        handle.did_open(uri, language_id, version, content);
                                        m.documents.mark_opened(buffer_id);
                                    }
                                }
                            }
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

/// Extract all locations from a `GotoDefinitionResponse`.
///
/// Handles all three variants: Scalar, Array, and Link.
fn extract_all_locations(response: &GotoDefinitionResponse) -> Vec<Location> {
    match response {
        GotoDefinitionResponse::Scalar(loc) => vec![loc.clone()],
        GotoDefinitionResponse::Array(locs) => locs.clone(),
        GotoDefinitionResponse::Link(links) => links
            .iter()
            .map(|link| Location {
                uri: link.target_uri.clone(),
                range: link.target_selection_range,
            })
            .collect(),
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_core::{
            bind::CommandRef,
            keys,
            modd::ModeState,
            plugin::{Plugin, PluginContext},
        },
    };

    #[test]
    fn test_lsp_keybindings_registered() {
        let plugin = LspPlugin::new();
        let mut ctx = PluginContext::new();

        // Build the plugin (registers commands and keybindings)
        plugin.build(&mut ctx);

        // Get the keymap and bindings for editor normal scope
        let keymap = ctx.keymap();
        let scope = KeymapScope::editor_normal();
        let bindings = keymap.get_scope(&scope);

        // gd, gr, K should all be bound
        assert!(
            bindings.get(&keys!['g' 'd']).is_some(),
            "gd keybinding should be registered. Bindings in scope: {:?}",
            bindings.keys().collect::<Vec<_>>()
        );
        assert!(bindings.get(&keys!['g' 'r']).is_some(), "gr keybinding should be registered");
        assert!(bindings.get(&keys!['K']).is_some(), "K keybinding should be registered");
    }

    #[test]
    fn test_lsp_keybindings_lookup_with_mode() {
        let plugin = LspPlugin::new();
        let mut ctx = PluginContext::new();

        // Build the plugin
        plugin.build(&mut ctx);

        // Get the keymap
        let keymap = ctx.keymap();

        // Create a default mode state (editor + normal)
        let mode = ModeState::new();

        // Test lookup for gd
        let goto_def_binding = keymap.lookup_binding(&mode, &keys!['g' 'd']);
        assert!(
            goto_def_binding.is_some(),
            "lookup_binding should find gd. mode.interactor_id={:?}, mode.edit_mode={:?}",
            mode.interactor_id,
            mode.edit_mode
        );
        assert!(goto_def_binding.unwrap().command.is_some(), "gd binding should have a command");

        // Test lookup for K
        let hover_binding = keymap.lookup_binding(&mode, &keys!['K']);
        assert!(hover_binding.is_some(), "lookup_binding should find K");
        assert!(hover_binding.unwrap().command.is_some(), "K binding should have a command");
    }

    #[test]
    fn test_lsp_commands_registered_and_resolvable() {
        let plugin = LspPlugin::new();
        let mut ctx = PluginContext::new();

        // Build the plugin
        plugin.build(&mut ctx);

        // Get the command registry
        let cmd_registry = ctx.command_registry();

        // Test that gd command is registered
        let goto_def_cmd = cmd_registry.get(&command_id::GOTO_DEFINITION);
        assert!(
            goto_def_cmd.is_some(),
            "lsp_goto_definition command should be registered. ID = {:?}",
            command_id::GOTO_DEFINITION
        );
        assert_eq!(goto_def_cmd.unwrap().name(), "lsp_goto_definition");

        // Test that K command is registered
        let show_hover_cmd = cmd_registry.get(&command_id::SHOW_HOVER);
        assert!(show_hover_cmd.is_some(), "lsp_show_hover command should be registered");
        assert_eq!(show_hover_cmd.unwrap().name(), "lsp_show_hover");

        // Test that the keybinding points to the right command
        let keymap = ctx.keymap();
        let mode = ModeState::new();
        let binding = keymap.lookup_binding(&mode, &keys!['g' 'd']).unwrap();

        match binding.command.as_ref().unwrap() {
            CommandRef::Registered(id) => {
                assert_eq!(
                    id.as_str(),
                    "lsp_goto_definition",
                    "gd keybinding should point to lsp_goto_definition"
                );
                // Verify the command can be resolved
                assert!(
                    cmd_registry.get(id).is_some(),
                    "Command {id} should be resolvable from registry"
                );
            }
            CommandRef::Inline(_) => panic!("gd keybinding should be a Registered command ref"),
        }
    }
}
